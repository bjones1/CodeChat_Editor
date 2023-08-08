/// Copyright (C) 2023 Bryan A. Jones.
///
/// This file is part of the CodeChat Editor. The CodeChat Editor is free
/// software: you can redistribute it and/or modify it under the terms of the
/// GNU General Public License as published by the Free Software Foundation,
/// either version 3 of the License, or (at your option) any later version.
///
/// The CodeChat Editor is distributed in the hope that it will be useful, but
/// WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY
/// or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for
/// more details.
///
/// You should have received a copy of the GNU General Public License along with
/// the CodeChat Editor. If not, see
/// [http://www.gnu.org/licenses](http://www.gnu.org/licenses).
///
/// # `lexer.rs` -- Lex source code into code and doc blocks
///
/// ## Submodule definitions
pub mod supported_languages;

/// ## Imports
///
/// ### Standard library
use std::collections::HashMap;
use std::sync::Arc;

// ### Third-party
use lazy_static::lazy_static;
use regex;
use regex::Regex;

/// ## Data structures
///
/// ### Language definition
///
/// These data structures define everything the lexer needs in order to analyze
/// a programming language:
///
/// - It defines block and inline comment delimiters; these (when correctly
///   formatted) become doc blocks.
/// - It defines strings: what is the escape character? Are newlines allowed? If
///   so, must newlines be escaped?
/// - It defines heredocs in a flexible form (see `HeredocDelim` for more
///   details).
/// - It associates an Ace mode and filename extensions with the lexer.
///
/// This lexer ignores line continuation characters; in C/C++/Python, it's a `\`
/// character followed immediately by a newline
/// ([C reference](https://www.open-std.org/jtc1/sc22/WG14/www/docs/n1256.pdf#page22),
/// [Python reference](https://docs.python.org/3/reference/lexical_analysis.html#explicit-line-joining)).
/// From a lexer perspective, supporting these adds little value:
///
/// 1.  It would allow the lexer to recognize the following C/C++ snippet as a
///     doc block:\
///     `// This is an odd\`\
///     `two-line inline comment.`\
///     However, this such such unusual syntax (most authors would instead use either
///     a block comment or another inline comment) that recognizing it adds little
///     value.
/// 2.  I'm unaware of any valid syntax in which ignoring a line continuation
///     would cause the lexer to mis-recognize code as a comment. (Escaped
///     newlines in strings, a separate case, are handled correctly).
///
/// This struct defines the delimiters for a block comment.
pub struct BlockCommentDelim<'a> {
    /// A string specifying the opening comment delimiter for a block comment.
    pub opening: &'a str,
    /// A string specifying the closing comment delimiter for a block comment.
    pub closing: &'a str,
    /// True if block comment may be nested.
    is_nestable: bool,
}

/// Define the types of newlines supported in a string.
enum NewlineSupport {
    /// This string delimiter allows unescaped newlines. This is a multiline
    /// string.
    Unescaped,
    /// This string delimiter only allows newlines when preceded by the string
    /// escape character. This is (mostly) a single-line string.
    Escaped,
    /// This string delimiter does not allow newlines. This is strictly a
    /// single-line string.
    None,
}

/// Define a string from the lexer's perspective.
struct StringDelimiterSpec<'a> {
    /// Delimiter to indicate the start and end of a string.
    delimiter: &'a str,
    /// Escape character, to allow inserting the string delimiter into the
    /// string. Empty if this string delimiter doesn't provide an escape
    /// character.
    escape_char: &'a str,
    /// Newline handling. This value cannot be `Escaped` if the `escape_char` is
    /// empty.
    newline_support: NewlineSupport,
}

/// This defines the delimiters for a
/// [heredoc](https://en.wikipedia.org/wiki/Here_document) (or heredoc-like
/// literal).
struct HeredocDelim<'a> {
    /// The prefix before the heredoc's delimiting identifier.
    start_prefix: &'a str,
    /// A regex which matches the delimiting identifier.
    delim_ident_regex: &'a str,
    /// The suffix after the delimiting identifier.
    start_suffix: &'a str,
    /// The prefix before the second (closing) delimiting identifier.
    stop_prefix: &'a str,
    /// The suffix after the heredoc's closing delimiting identifier.
    stop_suffix: &'a str,
}

/// Provide a method to handle special cases that don't fit within the current
/// lexing strategy.
enum SpecialCase {
    /// There are no special cases for this language.
    None,
    /// [Template literal](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Template_literals)
    /// support (for languages such as JavaScript, TypeScript, etc.).
    TemplateLiteral,
    /// C#'s verbatim string literal -- see
    /// [6.4.5.6 String literals](https://learn.microsoft.com/en-us/dotnet/csharp/language-reference/language-specification/lexical-structure#6456-string-literals).
    CSharpVerbatimStringLiteral,
    /// MATLAB
    /// [block comments](https://www.mathworks.com/help/matlab/matlab_prog/comments.html)
    /// must start and end on a blank line.
    Matlab,
}

/// Define a language by providing everything this lexer needs in order to split
/// it into code and doc blocks.
pub struct LanguageLexer<'a> {
    /// The [Ace](https://ace.c9.io/)
    /// [mode](https://github.com/ajaxorg/ace/tree/master/src/mode) to use for
    /// this language. The CodeChat Editor Client uses this to tell Ace the mode
    /// to use. It's can also be used in a specially-formatted comment in a
    /// source file to override the lexer chosen by looking at the file's
    /// extension.
    pub ace_mode: &'a str,
    /// An array of file extensions for this language. They \_do not_begin with
    /// a period, such as `rs`. This is the typical way that the CodeChat Editor
    /// uses to determine which lexer to use for a given source file.
    ext_arr: &'a [&'a str],
    /// An array of strings which specify inline comment delimiters. Empty if
    /// this language doesn't provide inline comments.
    pub inline_comment_delim_arr: &'a [&'a str],
    /// An array which specifies opening and closing block comment delimiters.
    /// Empty if this language doesn't provide block comments.
    pub block_comment_delim_arr: &'a [BlockCommentDelim<'a>],
    /// Specify the strings supported by this language. While this could be
    /// empty, such a language would be very odd.
    string_delim_spec_arr: &'a [StringDelimiterSpec<'a>],
    /// A [heredoc](https://en.wikipedia.org/wiki/Here_document) delimiter;
    /// `None` if heredocs aren't supported.
    heredoc_delim: Option<&'a HeredocDelim<'a>>,
    /// Any special case treatment for this language.
    special_case: SpecialCase,
}

/// ### Compiled language definition
// Store the results of compiling a language lexer.
pub struct LanguageLexerCompiled<'a> {
    /// Provide a reference back to the language definition this came from.
    pub language_lexer: &'a LanguageLexer<'a>,
    /// A regex used to identify the next token when in a code block.
    next_token: Regex,
    /// A mapping from groups in this regex to the corresponding delimiter type
    /// matched.
    map: Vec<RegexDelimType>,
}

// Store all lexers and their associated maps after they're compiled.
pub struct LanguageLexersCompiled<'a> {
    // The resulting compiled lexers.
    pub language_lexer_compiled_vec: Vec<Arc<LanguageLexerCompiled<'a>>>,
    // Maps a file extension to indices into the lexers vector.
    pub map_ext_to_lexer_vec: HashMap<&'a str, Vec<Arc<LanguageLexerCompiled<'a>>>>,
    // Maps an Ace mode to an index into the lexers vector.
    pub map_mode_to_lexer: HashMap<&'a str, Arc<LanguageLexerCompiled<'a>>>,
}

/// Define which delimiter corresponds to a given regex group.
///
/// This struct stores the results of "compiling" a `LanguageLexer` into a set
/// of regexes and a map. For example, the JavaScript lexer becomes:
//// Regex          (//)     |    (/*)      |        (")           |         (')          |       (`)
//// Group            1             2                 3                       4                    5
////  Map       InlineComment   BlockComment   String(double-quote)   String(single-quote)   TemplateLiteral
/// The Regex in the table is stored in `next_token`, which is used to search
/// for the next token. The group is both the group number of the regex (in
/// other words, a match of `//` is group 1 of the regex) and the index into
/// `map` (after subtracting 1, so that group 1 is stored in `map[0]`). Map is
/// `map`, which labels each group with a `RegexDelimType`. The lexer uses this
/// to decide how to handle the token it just found -- as a inline comment,
/// block comment, etc. Note: this is a slightly simplified regex; group 1,
/// `(/*)`, would actually be `(/\*)`, since the `*` must be escaped.
enum RegexDelimType {
    InlineComment,
    BlockComment(
        /// The regex used to find the closing delimiter. If the regex contains
        /// groups, then this is a language that supports nested block comments.
        /// Group 1 must match an opening comment, while group 2 must match the
        /// closing comment.
        Regex,
    ),
    String(
        /// The regex used to find the closing delimiter for this string type.
        Regex,
    ),
    Heredoc(
        /// The regex-escaped `HeredocDelim.stop_prefix`.
        String,
        /// The regex-escaped `HeredocDelim.stop_suffix`.
        String,
    ),
    TemplateLiteral,
}

/// ### Code/doc blocks
// To allow comparison for unit tests.
#[derive(PartialEq)]
// To allow printing with `println!`.
#[derive(Debug)]
pub struct DocBlock {
    /// The whitespace characters which created the indent for this doc block.
    pub indent: String,
    /// The opening comment delimiter.
    pub delimiter: String,
    /// The contents of this block: documentation (with the comment delimiters
    /// removed).
    pub contents: String,
    /// The number of source code lines in this doc block. Only valid when
    /// converting from source code to its web-editable equivalent; in the
    /// opposite conversion (web-editable to source file), this is not valid
    /// (it's always set to 0).
    pub lines: usize,
}

// To allow comparison for unit tests.
#[derive(PartialEq)]
// To allow printing with `println!`.
#[derive(Debug)]
pub enum CodeDocBlock {
    CodeBlock(
        // This contains the code defining this code block.
        String,
    ),
    DocBlock(DocBlock),
}

// ## Globals
//
// Create constant regexes needed by the lexer, following the
// [Regex docs recommendation](https://docs.rs/regex/1.6.0/regex/index.html#example-avoid-compiling-the-same-regex-in-a-loop).
lazy_static! {
    static ref WHITESPACE_ONLY_REGEX: Regex = Regex::new("^[[:space:]]*$").unwrap();
    /// TODO: This regex should also allow termination on an unescaped `${`
    /// sequence, which then must count matching braces to find the end of the
    /// expression.
    static ref TEMPLATE_LITERAL_CLOSING_REGEX: Regex = Regex::new(
        // Allow `.` to match _any_ character, including a newline. See the
        // [regex docs](https://docs.rs/regex/1.6.0/regex/index.html#grouping-and-flags).
        &("(?s)".to_string() +
        // Start at the beginning of the string, and require a match of every
        // character. Allowing the regex to start matching in the middle means
        // it can skip over escape characters.
        "^(" +
            // Allow any non-special character,
            "[^\\\\`]|" +
            // or anything following an escape character (since whatever it is,
            // it can't be the end of the string).
            "\\\\." +
        // Look for an arbitrary number of these non-string-ending characters.
        ")*" +
        // Now, find the end of the string: the string delimiter.
        "`"),
    ).unwrap();
}

// Support C# verbatim string literals, which end with a `"`; a `""` inserts a
// single " in the string.
const C_SHARP_VERBATIM_STRING_CLOSING: &str =
    // Allow anything except for a lone double quote as the contents of the
    // string, followed by a double quote to end the string.
    r#"([^"]|"")*""#;

/// ### Language "compiler"
///
/// "Compile" a language description into regexes used to lex the language.
fn build_lexer_regex<'a>(
    // The language description to build regexes for.
    language_lexer: &'a LanguageLexer,
    // The "compiled" form of this language lexer.
) -> LanguageLexerCompiled<'a> {
    // Produce the overall regex from regexes which find a specific special
    // case. See the lexer walkthrough for an example.
    let mut regex_strings_arr: Vec<String> = Vec::new();
    // Also create a mapping between the groups in this regex being built and
    // the delimiter matched by that group. See docs on `RegexDelimType`.
    let mut regex_group_map: Vec<RegexDelimType> = Vec::new();

    // Given an array of strings containing unescaped characters which
    // identifies the start of one of the special cases, combine them into a
    // single string separated by an or operator. Return the index of the
    // resulting string in `regex_strings`, or `None` if the array is empty
    // (indicating that this language doesn't support the provided special
    // case).
    let mut regex_builder = |//
                             // An array of alternative delimiters, which will
                             // be combined with a regex or (`|`) operator.
                             string_arr: &Vec<&str>,
                             // The type of delimiter in `string_arr`.
                             regex_delim_type: RegexDelimType| {
        // If there are no delimiters, then there's nothing to do.
        if string_arr.is_empty() {
            return;
        }
        // Join the array of strings with an or operator.
        let tmp: Vec<String> = string_arr.iter().map(|x| regex::escape(x)).collect();
        regex_strings_arr.push(tmp.join("|"));
        // Store the type of this group.
        regex_group_map.push(regex_delim_type);
    };

    // Add the opening block comment delimiter to the overall regex; add the
    // closing block comment delimiter to the map for the corresponding group.
    let mut block_comment_opening_delim: Vec<&str> = vec![""];
    for block_comment_delim in language_lexer.block_comment_delim_arr {
        block_comment_opening_delim[0] = block_comment_delim.opening;
        regex_builder(
            &block_comment_opening_delim,
            // Determine the block closing regex:
            RegexDelimType::BlockComment(
                Regex::new(&if block_comment_delim.is_nestable {
                    // If nested, look for another opening delimiter or the
                    // closing delimiter.
                    format!(
                        "({})|({})",
                        regex::escape(block_comment_delim.opening),
                        regex::escape(block_comment_delim.closing)
                    )
                } else {
                    // Otherwise, just look for the closing delimiter.
                    regex::escape(block_comment_delim.closing)
                })
                .unwrap(),
            ),
        );
    }
    regex_builder(
        &language_lexer.inline_comment_delim_arr.to_vec(),
        RegexDelimType::InlineComment,
    );
    // Build regexes for each string delimiter.
    for string_delim_spec in language_lexer.string_delim_spec_arr {
        // Generate a regex based on the characteristics of this string.
        let has_escape_char = !string_delim_spec.escape_char.is_empty();
        // For multi-character string delimiters, build a regex: `'''` becomes
        // `(|'|'')`, which allows matches of a partial string delimiter, but
        // not the entire delimiter. For a single-character delimiter, the
        // "regex" is an empty string.
        let string_partial_builder = |delimiter: &str| -> String {
            // If this is a single-character string delimiter, then we're done.
            if delimiter.len() < 2 {
                return String::new();
            };

            // Otherwise, build a vector of substrings of the delimiter: for a
            // delimiter of `'''`, we want `["", "'"", "''"]`.
            let mut v: Vec<String> = vec![];
            let mut partial_delimiter = String::new();
            for c in delimiter.chars() {
                // Add the previous partial delimiter. This allows us to produce
                // a vector containing all the but full delimiter and including
                // the empty string case.
                v.push(regex::escape(&partial_delimiter));
                // Add the current character to the partial delimiter.
                partial_delimiter.push(c);
            }

            // Convert this vector into a regex.
            format!("({})", v.join("|"))
        };
        let string_partial_delimiter = string_partial_builder(string_delim_spec.delimiter);
        // Look for
        let escaped_delimiter = regex::escape(string_delim_spec.delimiter);
        let escaped_escape_char = regex::escape(string_delim_spec.escape_char);
        let end_of_string_regex = match (has_escape_char, &string_delim_spec.newline_support) {
            // This is the most complex case. This type of string can be
            // terminated by an unescaped newline or an unescaped delimiter.
            // Escaped newlines or terminators should be included in the string.
            (true, NewlineSupport::Escaped) => Regex::new(
                // Allow `.` to match _any_ character, including a newline. See
                // the
                // [regex docs](https://docs.rs/regex/1.6.0/regex/index.html#grouping-and-flags).
                &("(?s)".to_string() +
                // Start at the beginning of the string, and require a match of
                // every character. Allowing the regex to start matching in the
                // middle means it can skip over escape characters.
                "^(" +
                    // Allow a partial string delimiter inside the string (but
                    // not the full delimiter).
                    &string_partial_delimiter +
                    // Allow any non-special character,
                    &format!("([^\n{}{}]|", escaped_delimiter, escaped_escape_char) +
                    // or anything following an escape character (since whatever
                    // it is, it can't be the end of the string).
                    &escaped_escape_char + ".)" +
                // Look for an arbitrary number of these non-string-ending
                // characters.
                ")*" +
                // Now, find the end of the string: a newline or the string
                // delimiter.
                &format!("(\n|{})", escaped_delimiter)),
            ),

            // A bit simpler: this type of string can be terminated by a newline
            // or an unescaped delimiter. Escaped terminators should be included
            // in the string.
            (true, NewlineSupport::None) => Regex::new(
                // Start at the beginning of the string, and require a match of
                // every character. Allowing the regex to start matching in the
                // middle means it can skip over escape characters.
                &("^(".to_string() +
                    // Allow a partial string delimiter inside the string (but
                    // not the full delimiter).
                    &string_partial_delimiter +
                    // Allow any non-special character
                    &format!("([^\n{}{}]|", escaped_delimiter, escaped_escape_char) +
                    // or anything following an escape character except a
                    // newline.
                    &escaped_escape_char + "[^\n])" +
                // Look for an arbitrary number of these non-string-ending
                // characters.
                ")*" +
                // Now, find the end of the string: a newline optionally
                // preceded by the escape char or the string delimiter.
                &format!("({}?\n|{})", escaped_escape_char, escaped_delimiter)),
            ),

            // Even simpler: look for an unescaped string delimiter.
            (true, NewlineSupport::Unescaped) => Regex::new(
                // Allow `.` to match _any_ character, including a newline. See
                // the
                // [regex docs](https://docs.rs/regex/1.6.0/regex/index.html#grouping-and-flags).
                &("(?s)".to_string() +
                // Start at the beginning of the string, and require a match of
                // every character. Allowing the regex to start matching in the
                // middle means it can skip over escape characters.
                "^(" +
                    // Allow a partial string delimiter inside the string (but
                    // not the full delimiter).
                    &string_partial_delimiter +
                    // Allow any non-special character,
                    &format!("([^{}{}]|", escaped_delimiter, escaped_escape_char) +
                    // or anything following an escape character (since whatever
                    // it is, it can't be the end of the string).
                    &escaped_escape_char + ".)" +
                // Look for an arbitrary number of these non-string-ending
                // characters.
                ")*" +
                // Now, find the end of the string: the string delimiter.
                &escaped_delimiter),
            ),

            // This case makes no sense: there's no escape character, yet the
            // string allows escaped newlines?
            (false, NewlineSupport::Escaped) => panic!(
                "Invalid parameters for the language lexer where ace_mode = {} and ext_arr = {:?}.",
                language_lexer.ace_mode, language_lexer.ext_arr
            ),

            // The simplest case: just look for the delimiter!
            (false, NewlineSupport::Unescaped) => Regex::new(&escaped_delimiter),

            // Look for either the delimiter or a newline to terminate the
            // string.
            (false, NewlineSupport::None) => Regex::new(&format!("{}|\n", &escaped_delimiter)),
        }
        .unwrap();
        regex_builder(
            &[regex::escape(string_delim_spec.delimiter).as_str()].to_vec(),
            RegexDelimType::String(end_of_string_regex),
        );
    }

    match language_lexer.special_case {
        SpecialCase::None => (),
        // A C# verbatim string has asymmetric opening and closing delimiters,
        // making it a special case.
        SpecialCase::CSharpVerbatimStringLiteral => regex_builder(
            &["@\""].to_vec(),
            RegexDelimType::String(Regex::new(C_SHARP_VERBATIM_STRING_CLOSING).unwrap()),
        ),
        SpecialCase::TemplateLiteral => {
            // Template literals only exist in JavaScript. No other language
            // (that I know of) allows comments inside these, or nesting of
            // template literals.
            //
            // Build a regex for template strings.
            //
            // TODO: this is broken! Lexing nested template literals means
            // matching braces, yikes. For now, don't support this.
            //
            // TODO: match either an unescaped `${` -- which causes a nested
            // parse -- or the closing backtick (which must be unescaped).
            regex_builder(&["`"].to_vec(), RegexDelimType::TemplateLiteral);
        }
        SpecialCase::Matlab => {
            // MATLAB supports block comments, when the comment delimiters
            // appear alone on the line (also preceding and following whitespace
            // is allowed). Therefore, we need a regex that matches this
            // required whitespace.
            //
            // Also, this match needs to go before the inline comment of `%`, to
            // prevent that from matching before this does. Hence, use an
            // `insert` instead of a `push`.
            regex_strings_arr.insert(
                0,
                // Tricky: even though we match on optional leading and trailing
                // whitespace, we don't want the whitespace captured by the
                // regex. So, begin by defining the outer group (added when
                // `regex_strings_arr` are combined into a single string) as a
                // non-capturing group.
                "?:".to_string() +
                // To match on a line which consists only of leading and
                // trailing whitespace plus the opening comment delimiter, put
                // these inside a `(?m:exp)` block, so that `^` and `$` will
                // match on any newline in the string; see the
                // [regex docs](https://docs.rs/regex/latest/regex/#grouping-and-flags).
                // This also functions as a non-capturing group, to avoid
                // whitespace capture as discussed earlier.
                "(?m:" +
                    // Look for whitespace before the opening comment delimiter.
                    r#"^\s*"# +
                    // Capture just the opening comment delimiter,
                    r#"(%\{)"# +
                    // followed by whitespace until the end of the line.
                    r#"\s*$"# +
                // End the multi-line mode and this non-capturing group.
                ")",
            );
            regex_group_map.insert(
                0,
                RegexDelimType::BlockComment(
                    // Use a similar strategy for finding the closing delimiter.
                    Regex::new(r#"(?m:^\s*%\}\s*$)"#).unwrap(),
                ),
            );
        }
    };

    // This must be last, since it includes one group (so the index of all
    // future items will be off by 1). Build a regex for a heredoc start.
    let &regex_str;
    if let Some(heredoc_delim) = language_lexer.heredoc_delim {
        // First, create the string which defines the regex.
        regex_str = format!(
            "{}({}){}",
            regex::escape(heredoc_delim.start_prefix),
            heredoc_delim.delim_ident_regex,
            regex::escape(heredoc_delim.start_suffix)
        );
        // Then add it. Do this manually, since we don't want the regex escaped.
        regex_strings_arr.push(regex_str);
        regex_group_map.push(RegexDelimType::Heredoc(
            regex::escape(heredoc_delim.stop_prefix),
            regex::escape(heredoc_delim.stop_suffix),
        ));
    }

    // Combine all this into a single regex, which is this or of each
    // delimiter's regex. Create a capturing group for each delimiter.
    let classify_regex = Regex::new(&format!("({})", regex_strings_arr.join(")|("))).unwrap();

    LanguageLexerCompiled {
        language_lexer,
        next_token: classify_regex,
        map: regex_group_map,
    }
}

// ## Compile lexers
pub fn compile_lexers<'a>(
    language_lexer_arr: &'a [LanguageLexer<'a>],
) -> LanguageLexersCompiled<'a> {
    let mut language_lexers_compiled = LanguageLexersCompiled {
        language_lexer_compiled_vec: Vec::new(),
        map_ext_to_lexer_vec: HashMap::new(),
        map_mode_to_lexer: HashMap::new(),
    };
    // Walk through each lexer.
    for language_lexer in language_lexer_arr {
        // Compile and add it.
        let llc = Arc::new(build_lexer_regex(language_lexer));
        language_lexers_compiled
            .language_lexer_compiled_vec
            .push(Arc::clone(&llc));

        // Add all its extensions to the extension map.
        for ext in language_lexer.ext_arr {
            match language_lexers_compiled.map_ext_to_lexer_vec.get_mut(ext) {
                None => {
                    let new_lexer_vec = vec![Arc::clone(&llc)];
                    language_lexers_compiled
                        .map_ext_to_lexer_vec
                        .insert(ext, new_lexer_vec);
                }
                Some(v) => v.push(Arc::clone(&llc)),
            }
        }

        // Add its mode to the mode map.
        language_lexers_compiled
            .map_mode_to_lexer
            .insert(language_lexer.ace_mode, llc);
    }

    language_lexers_compiled
}

/// ## Source lexer
///
/// This lexer categorizes source code into code blocks or doc blocks.
///
/// These linter warnings would IMHO make the code less readable.
#[allow(clippy::bool_to_int_with_if)]
pub fn source_lexer(
    // The source code to lex.
    source_code: &str,
    // A description of the language, used to lex the `source_code`.
    language_lexer_compiled: &LanguageLexerCompiled,
    // The return value is an array of code and doc blocks.
) -> Vec<CodeDocBlock> {
    // Rather than attempt to lex the entire language, this lexer's only goal is
    // to categorize all the source code into code blocks or doc blocks. To do
    // it, it only needs to:
    //
    // - Recognize where comments can't be—inside strings or string-like syntax,
    //   such as [here text](https://en.wikipedia.org/wiki/Here_document) or
    //   [template literals](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Template_literals).
    //   These are always part of a code block and can never contain a comment
    //   or (by implication) a doc block.
    // - Outside of these special cases, look for inline or block comments,
    //   categorizing everything else as plain code.
    // - After finding either an inline or block comment, determine if this is a
    //   doc block.
    //
    // ### Lexer operation
    //
    // To accomplish this goal, use a
    // [regex](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Guide/Regular_Expressions)
    // named `language_lexer_compiled.next_token` and associated indices in
    // `language_lexer_compiled.map`. These divides source code into two
    // categories: plain code and special cases. The special cases consist of:
    //
    // - String-like code (strings, here text, template literals). In this case,
    //   the lexer must find the end of the string-like element before it can
    //   return to plain code.
    // - Comments (inline or block). In this case, the lexer must find the end
    //   of the comment before it can return to plain code.
    //
    // This regex assumes the string it analyzes was preceded by plain code; its
    // purpose is to identify the start of the next special case. **This code
    // makes heavy use of regexes -- read the previous link thoroughly.**
    //
    // To better explain the operation of the lexer, the following provides a
    // high-level walkthrough.
    //
    // ### Lexer walkthrough
    //
    // This walkthrough shows how the lexer parses the following Python code
    // fragment:
    //
    // <code>print(<span style="color: rgb(224, 62, 45);">"""¶</span></code>\
    // <code><span style="color: rgb(224, 62, 45);"># This is not a comment! It's
    // a multi-line string.¶</span></code>\
    // <code><span style="color: rgb(224, 62, 45);">"""</span>)¶</code>\
    //   <code><span style="color: rgb(45, 194, 107);"># This is a comment.</span></code>
    //
    // Paragraph marks (the ¶ character) are included to show how the lexer
    // handles newlines. To explain the operation of the lexer, the code will be
    // highlighted in yellow to represent the
    // <span style="background-color: rgb(251, 238, 184);">unlexed source
    // code</span>, represented by the contents of the
    // variable `source_code[source_code_unlexed_index..]` and in green for the
    // <span style="background-color: rgb(191, 237, 210);">current code
    // block</span>, defined by
    // `source_code[current_code_block_index..source_code_unlexed_index]`. Code
    // that is classified by the lexer will be placed in the `classified_code`
    // array.
    //
    // #### Start of parse
    //
    // The <span style="background-color: rgb(251, 238, 184);">unlexed source
    // code</span> holds all the code (everything is highlighted in yellow); the
    // <span style="background-color: rgb(191, 237, 210);">current code
    // block</span> is empty (there is no green highlight).
    //
    // <span style="background-color: rgb(251, 238, 184);"><code>print(<span style="color: rgb(224, 62, 45);">"""¶</span></code></span>\
    // <span style="background-color: rgb(251, 238, 184);"><code><span style="color: rgb(224, 62, 45);">#
    // This is not a comment! It's a multi-line string.¶</span></code></span>\
    // <span style="background-color: rgb(251, 238, 184);"><code><span style="color: rgb(224, 62, 45);">"""</span>)¶</code></span>\
    //   <code><span style="background-color: rgb(251, 238, 184);">&nbsp; <span style="color: rgb(45, 194, 107);">#
    // This is a comment.</span></span></code>
    //
    // `classified_code = [`\
    // `]`<span style="background-color: rgb(191, 237, 210);"><br></span>
    //
    // #### Search for a token
    //
    // The lexer begins by searching for the regex in
    // `language_lexer_compiled.next_token`, which is
    // `(\#)|(""")|(''')|(")|(')`. The first token found is
    // <span style="color: rgb(224, 62, 45);"><code>"""</code></span>.
    // Everything up to the match is moved from the unlexed source code to the
    // current code block, giving:
    //
    // <code><span style="background-color: rgb(191, 237, 210);">print(</span><span style="color: rgb(224, 62, 45); background-color: rgb(251, 238, 184);">"""¶</span></code>\
    // <span style="background-color: rgb(251, 238, 184);"><code><span style="color: rgb(224, 62, 45);">#
    // This is not a comment! It's a multi-line string.¶</span></code></span>\
    // <span style="background-color: rgb(251, 238, 184);"><code><span style="color: rgb(224, 62, 45);">"""</span>)¶</code></span>\
    //   <code><span style="background-color: rgb(251, 238, 184);">&nbsp; <span style="color: rgb(45, 194, 107);">#
    // This is a comment.</span></span></code>
    //
    // `classified_code = [`\
    // `]`<span style="background-color: rgb(191, 237, 210);"><br></span>
    //
    // #### String processing
    //
    // The regex is accompanied by a map named `language_lexer_compiled.map`,
    // which connects the mapped group to which token it matched (see
    // `struct RegexDelimType`):
    //
    // ```
    // Regex:           (#)       |  (""") | (''')  |  (")   |  (')
    // Mapping:    Inline comment   String   String   String   String
    // Group:            1            2        3        4        5
    // ```
    //
    // Since group 2 matched, looking up this group in the map tells the lexer
    // it’s a string, and also gives a regex which identifies the end of the
    // string . This regex identifies the end of the string, moving it from the
    // <span style="background-color: rgb(251, 238, 184);">(unclassified) source
    // code</span> to the (classified)
    // <span style="background-color: rgb(191, 237, 210);">current code
    // block</span>. It correctly skips what looks like a comment but is not a
    // comment. After this step, the lexer’s state is:
    //
    // <span style="background-color: rgb(191, 237, 210);"><code>print(<span style="color: rgb(224, 62, 45);">"""¶</span></code></span>\
    // <span style="background-color: rgb(191, 237, 210);"><code><span style="color: rgb(224, 62, 45);">#
    // This is not a comment! It's a multi-line string.¶</span></code></span>\
    // <code><span style="color: rgb(224, 62, 45); background-color: rgb(191, 237, 210);">"""</span><span style="background-color: rgb(251, 238, 184);">)¶</span></code>\
    //   <code><span style="background-color: rgb(251, 238, 184);">&nbsp; <span style="color: rgb(45, 194, 107);">#
    // This is a comment.</span></span></code>
    //
    // `classified_code = [`\
    // `]`
    //
    // #### Search for a token (second time)
    //
    // Now, the lexer is back to its state of looking through code (as opposed
    // to looking inside a string, comment, etc.). It uses the `next_token`
    // regex as before to identify the next token
    // <span style="color: rgb(45, 194, 107);"><code>#</code></span> and moves
    // all the preceding characters from source code to the current code block.
    // The lexer state is now:
    //
    // <code><span style="background-color: rgb(191, 237, 210);">print(<span style="color: rgb(224, 62, 45);">"""¶</span></span></code>\
    // <span style="background-color: rgb(191, 237, 210);"><code><span style="color: rgb(224, 62, 45);">#
    // This is not a comment! It's a multi-line string.¶</span></code></span>\
    // <span style="background-color: rgb(191, 237, 210);"><code><span style="color: rgb(224, 62, 45);">"""</span>)¶</code></span>\
    //   <code><span style="background-color: rgb(191, 237, 210);">&nbsp; </span><span style="color: rgb(45, 194, 107);"><span style="background-color: rgb(251, 238, 184);"><code>#
    // This is a comment.</code></span></span></code>
    //
    // `classified_code = [`\
    // `]`
    //
    // #### Inline comment lex
    //
    // Based on the map, the lexer identifies this as an inline comment. The
    // inline comment lexer first identifies the end of the comment (the next
    // newline or, as in this case, the end of the file), putting the entire
    // inline comment except for the comment opening delimiter
    // <span style="color: rgb(45, 194, 107);"><code>#</code></span> into
    // <span style="background-color: rgb(236, 240, 241);"><code>full_comment</code></span>.
    // It then splits the current code block into two
    // groups: <span style="background-color: rgb(236, 202, 250);"><code>code_lines_before_comment</code></span>
    // (lines in the current code block which come before the current line) and
    // the
    // <span style="background-color: rgb(194, 224, 244);"><code>comment_line_prefix</code></span>
    // (the current line up to the start of the comment). The classification is:
    //
    // <code><span style="background-color: rgb(236, 202, 250);">print(<span style="color: rgb(224, 62, 45);">"""¶</span></span></code>\
    // <span style="background-color: rgb(236, 202, 250);"><code><span style="color: rgb(224, 62, 45);">#
    // This is not a comment! It's a multi-line string.¶</span></code></span>\
    // <span style="background-color: rgb(236, 202, 250);"><code><span style="color: rgb(224, 62, 45);">"""</span>)¶</code></span>\
    //   <code><span style="background-color: rgb(194, 224, 244);">&nbsp; </span><span style="color: rgb(45, 194, 107);">#<span style="background-color: rgb(236, 240, 241);">
    // This is a comment.</span></span></code>
    //
    // `classified_code = [`\
    // `]`
    //
    // #### Code/doc block classification
    //
    // Because
    // <code><span style="background-color: rgb(194, 224, 244);">comment_line_prefix</span></code>
    // contains only whitespace and
    // <span style="background-color: rgb(236, 240, 241);">full_comment</span>
    // has a space after the comment delimiter, the lexer classifies this as a
    // doc block. It
    // adds <span style="background-color: rgb(236, 202, 250);">code_lines_before_comment</span>
    // as a code block, then the text of the comment as a doc block:
    //
    // `classified_code = [`\
    //   <code>&nbsp; Item 0 = CodeDocBlock {<br>&nbsp; &nbsp; </code>    `indent: "", delimiter: "", contents = "print("""¶`\
    // `# This is not a comment! It's a multi-line string.¶`\
    // `""")¶`\
    //         `" },`
    //
    // `Item 1 = CodeDocBlock { indent: " ", delimiter: "#", contents = "This is a comment" }`\
    // `]`
    //
    // #### Done
    //
    // After this, the unlexed source code is empty since the inline comment
    // classified moved the remainder of its contents into `classified_code`.
    // The function exits.
    //
    // ### Helper function
    //
    // Provide a method to intelligently append to the code/doc block vec. Empty
    // appends are ignored; appends of the same type append to `contents`
    // instead of creating a new entry.
    let mut classified_source: Vec<CodeDocBlock> = Vec::new();
    let mut append_code_doc_block = |indent: &str, delimiter: &str, contents: &str| {
        // Don't append empty entries.
        if delimiter.is_empty() && contents.is_empty() {
            assert!(indent.is_empty());
            return;
        }
        let lines = contents.matches('\n').count();
        let is_code_block = indent.is_empty() && delimiter.is_empty();
        // See if there's a previous entry to potentially append to.
        if !classified_source.is_empty() {
            // See if this is the same type of block.
            let end = classified_source.len() - 1;
            match classified_source[end] {
                CodeDocBlock::DocBlock(ref mut last_doc_block) => {
                    if last_doc_block.indent == indent && last_doc_block.delimiter == delimiter {
                        // Yes, so append the provided contents to it. We must
                        // access the array directly since `last_doc_block`
                        // provides only a reference.
                        last_doc_block.contents += contents;
                        last_doc_block.lines += lines;
                        return;
                    }
                }
                CodeDocBlock::CodeBlock(ref mut _last_code_block) => {
                    if indent.is_empty() && delimiter.is_empty() {
                        // Code blocks should never need to be appended to a
                        // previous entry.
                        panic!("Attempted to append code block contents to a previous entry.")
                        //_last_code_block.push_str(contents);
                    }
                }
            }
        }
        // We must append a new entry.
        classified_source.push(if is_code_block {
            CodeDocBlock::CodeBlock(contents.to_string())
        } else {
            CodeDocBlock::DocBlock(DocBlock {
                indent: indent.to_string(),
                delimiter: delimiter.to_string(),
                contents: contents.to_string(),
                lines,
            })
        });
    };

    // ### Main loop
    //
    // Normalize all line endings.
    let source_code = source_code.replace("\r\n", "\n").replace('\r', "\n");
    // This index marks the start of code that hasn't been lexed.
    let mut source_code_unlexed_index: usize = 0;
    // Ths index marks the start of code that belongs to the current code block.
    // The current code block is always defined as
    // `source_code[current_code_block_index..source_code_unlexed_index]`.
    let mut current_code_block_index: usize = 0;

    // Main loop: lex the provided source code.
    while source_code_unlexed_index < source_code.len() {
        #[cfg(feature = "lexer_explain")]
        println!(
            "Searching the following source_code using the pattern {:?}:\n'{}'\n\nThe current code block is '{}'\n",
            language_lexer_compiled.next_token, &source_code[source_code_unlexed_index..], &source_code[current_code_block_index..source_code_unlexed_index]
        );
        // #### Find the next token
        //
        // Look for the next special case. Per the earlier discussion, this
        // assumes that the text immediately preceding `source_code` was plain
        // code.
        if let Some(classify_match) = language_lexer_compiled
            .next_token
            .captures(&source_code[source_code_unlexed_index..])
        {
            // Find the first group in the regex that matched.
            let matching_group_index = classify_match
                .iter()
                // Group 0 is the entire match, which is always true. Skip this
                // group.
                .skip(1)
                .position(|x| x.is_some())
                .unwrap()
                // Correct the resulting group index, since we skipped group 0.
                + 1;
            let matching_group_str = &classify_match[matching_group_index];

            // Move everything preceding this match from `source_code` to the
            // current code block, since per the assumptions this is code.
            source_code_unlexed_index += classify_match.get(matching_group_index).unwrap().start();

            #[cfg(feature = "lexer_explain")]
            println!(
                "Matched the string {} in group {}. The current_code_block is now\n'{}'\n",
                matching_group_str,
                matching_group_index,
                &source_code[current_code_block_index..source_code_unlexed_index]
            );

            // This helper function moves code from unlexed source code to the
            // current code block based on the provided regex.
            let mut append_code =
                                   // The regex; code up to the end of this
                                   // match will be appended to the current code
                                   // block.
                                   |closing_regex: &Regex| {
                #[cfg(feature = "lexer_explain")]
                println!("Searching for the end of this token using the pattern '{:?}'.", closing_regex);

                // Add the opening delimiter to the code.
                source_code_unlexed_index += matching_group_str.len();
                // Find the closing delimiter.
                if let Some(closing_match) = closing_regex.find(&source_code[source_code_unlexed_index..]) {
                    #[cfg(feature = "lexer_explain")]
                    println!("Found; adding source_code up to and including this token to current_code_block.");

                    // Include this in code.
                    source_code_unlexed_index += closing_match.end();
                } else {
                    #[cfg(feature = "lexer_explain")]
                    println!("Not found; adding all the source_code to current_code_block.");

                    // Then the rest of the code is a string.
                    source_code_unlexed_index = source_code.len();
                }
                #[cfg(feature = "lexer_explain")]
                println!("The current_code_block is now\n\
                    '{}'\n", &source_code[current_code_block_index..source_code_unlexed_index]);

            };

            // In the map, index 0 refers to group 1 (since group 0 matches are
            // skipped). Adjust the index for this.
            match &language_lexer_compiled.map[matching_group_index - 1] {
                // #### Inline comment
                RegexDelimType::InlineComment => {
                    // **First**, find the end of this comment: a newline.
                    let end_of_comment_rel_index =
                        source_code[source_code_unlexed_index..].find('\n');

                    // Assign `full_comment` to contain the entire comment
                    // (excluding the inline comment delimiter) until the
                    // newline which ends the comment.
                    let full_comment_start_index =
                        source_code_unlexed_index + matching_group_str.len();

                    // The current code block contains preceding code (which
                    // might be multiple lines) until the inline comment
                    // delimiter. Split this on newlines, grouping all the lines
                    // before the last line into `code_lines_before_comment`
                    // (which is all code), and everything else (from the
                    // beginning of the last line to where the inline comment
                    // delimiter appears) into `comment_line_prefix`. For
                    // example, consider the fragment `a = 1\nb = 2 // Doc`.
                    // After processing,
                    // `code_lines_before_comment == "a = 1\n"` and
                    // `comment_line_prefix == "b = 2 "`.
                    let current_code_block =
                        &source_code[current_code_block_index..source_code_unlexed_index];
                    let comment_line_prefix = current_code_block.rsplit('\n').next().unwrap();
                    let code_lines_before_comment =
                        &current_code_block[..current_code_block.len() - comment_line_prefix.len()];

                    // Move to the next block of source code to be lexed. No
                    // matching newline means we're at the end of the file, so
                    // the comment is all the remaining `source_code`.
                    source_code_unlexed_index = if let Some(index) = end_of_comment_rel_index {
                        // Note that `index` is the index of the newline; add 1
                        // to include that newline in the comment.
                        source_code_unlexed_index + index + 1
                    } else {
                        source_code.len()
                    };
                    let full_comment =
                        &source_code[full_comment_start_index..source_code_unlexed_index];

                    #[cfg(feature = "lexer_explain")]
                    println!(
                        "This is an inline comment. Source code before the line containing this comment is:\n'{}'\n\
                        The text preceding this comment is: '{}'.\n\
                        The comment is: '{}'\n",
                        code_lines_before_comment, comment_line_prefix, full_comment
                    );

                    // **Next**, determine if this comment is a doc block.
                    // Criteria for doc blocks for an inline comment:
                    //
                    // 1.  All characters preceding the comment on the line
                    //     containing the comment must be whitespace.
                    // 2.  Either:
                    //     1.  The inline comment delimiter is immediately
                    //         followed by a space, or
                    //     2.  the inline comment delimiter is followed by a
                    //         newline or the end of the file.
                    //
                    // With this last line located, apply the doc block
                    // criteria.
                    let ws_only = WHITESPACE_ONLY_REGEX.is_match(comment_line_prefix);
                    let has_space_after_comment = full_comment.starts_with(' ');
                    // Criteria 1 -- the whitespace matched.
                    if ws_only &&
                        // TODO: generalize this to specific lines that are
                        // never doc blocks.
                        full_comment != " prettier-ignore\n"
                        && (
                            // Criteria 2.1
                            has_space_after_comment ||
                            // Criteria 2.2a
                            (full_comment == "\n" ||
                            // Criteria 2.2b -- end of file means the comment is
                            // empty.
                            full_comment.is_empty())
                        )
                    {
                        // This is a doc block. Transition from the preceding
                        // code block to this doc block.
                        append_code_doc_block("", "", code_lines_before_comment);

                        // Add this doc block by pushing the array \[whitespace
                        // before the inline comment, inline comment contents,
                        // inline comment delimiter\]. Since it's a doc block,
                        // then `comment_line_prefix` contains the whitespace
                        // before this comment and `matching_group_string`
                        // contains the inline comment delimiter. For the
                        // contents, omit the leading space if it's there (this
                        // might be just a newline or an EOF).
                        let contents = &full_comment[if has_space_after_comment { 1 } else { 0 }..];
                        append_code_doc_block(comment_line_prefix, matching_group_str, contents);

                        #[cfg(feature = "lexer_explain")]
                        println!(
                            "This is a doc block. Possibly added the preceding code block\n\
                            '{}'.\n\
                            Added a doc block with indent = '{}', delimiter = '{}', and contents =\n\
                            '{}'.\n",
                            current_code_block, comment_line_prefix, matching_group_str, contents
                        );

                        // We've now stored the current code block (which was
                        // classified as a doc block) in `classified_lines`.
                        // Make the current code block empty by moving its index
                        // up to the unlexed code.
                        current_code_block_index = source_code_unlexed_index;
                    } else {
                        // This comment is not a doc block; instead, treat it as
                        // code. This code is already in the current code block,
                        // so we're done.
                    }
                }

                // #### Block comment
                RegexDelimType::BlockComment(closing_regex) => 'block_comment: {
                    #[cfg(feature = "lexer_explain")]
                    println!("Block Comment Found.");

                    // Determine the location of the beginning of this block
                    // comment's content.
                    let comment_start_index = source_code_unlexed_index + matching_group_str.len();

                    #[cfg(feature = "lexer_explain")]
                    println!(
                        "The opening delimiter is '{}', and the closing delimiter regex is '{}'.",
                        matching_group_str, closing_regex
                    );

                    // get the index of the closing delimiter. TODO: for nested
                    // block comments, look for a group match then count nesting
                    // depth.
                    let closing_delimiter_match = if let Some(_match) =
                        closing_regex.find(&source_code[comment_start_index..])
                    {
                        _match
                    } else {
                        #[cfg(feature = "lexer_explain")]
                        println!("The closing comment delimiter wasn't found.");
                        // If there's no closing delimiter, this is not a doc
                        // block; it's a syntax error. The safe route is to
                        // assume the contents are code, which this program
                        // won't edit; it does edit comments by cleaning up HTML
                        // tags, word-wrapping, etc. which would be a disaster
                        // if this was applied to code.
                        source_code_unlexed_index = source_code.len();
                        // Exit the block comment processing code here.
                        break 'block_comment;
                    };
                    let closing_delimiter_start_index =
                        closing_delimiter_match.start() + comment_start_index;
                    let closing_delimiter_end_index =
                        closing_delimiter_match.end() + comment_start_index;

                    // Capture the body of the comment -- everything but the
                    // opening and closing delimiters.
                    let comment_body =
                        &source_code[comment_start_index..closing_delimiter_start_index];

                    #[cfg(feature = "lexer_explain")]
                    println!(
                        "The comment body is\n\
                        '{}'.\n\
                        The closing delimiter is '{}'.",
                        comment_body,
                        closing_delimiter_match.as_str()
                    );
                    // Find the first \\n after the closing delimiter. If there
                    // is a newline after the closing delimiter, set
                    // `newline_or_eof_after_closing_delimiter_index` to the
                    // index of the first newline after the closing delimiter
                    // else set it to the end of the file.
                    let newline_or_eof_after_closing_delimiter_index =
                        match source_code[closing_delimiter_end_index..].find('\n') {
                            // The + 1 includes the newline in the resulting
                            // index.
                            Some(index) => index + closing_delimiter_end_index + 1,
                            None => source_code.len(),
                        };

                    // Capture the line which begins after the closing delimiter
                    // and ends at the next newline/EOF.
                    let post_closing_delimiter_line = &source_code
                        [closing_delimiter_end_index..newline_or_eof_after_closing_delimiter_index];

                    #[cfg(feature = "lexer_explain")]
                    println!(
                        "The post-comment line is '{}'.",
                        post_closing_delimiter_line
                    );

                    // Set the current code block to contain preceding code
                    // (which might be multiple lines) until the block comment
                    // delimiter. Split this on newlines, grouping all the lines
                    // before the last line into `code_lines_before_comment`
                    // (which is all code), and everything else (from the
                    // beginning of the last line to where the block comment
                    // delimiter appears) into `comment_line_prefix`. For
                    // example, consider the fragment:
                    // `a = 1\nb = 2 /* comment */`. After processing,
                    // `code_lines_before_comment` will be "`a = 1\n`" and
                    // `comment_line_prefix` will be "`b = 2` ".
                    let current_code_block =
                        &source_code[current_code_block_index..source_code_unlexed_index];
                    let comment_line_prefix = current_code_block.rsplit('\n').next().unwrap();
                    let code_lines_before_comment =
                        &current_code_block[..current_code_block.len() - comment_line_prefix.len()];

                    // Move to the next block of source code to be lexed.
                    source_code_unlexed_index = newline_or_eof_after_closing_delimiter_index;

                    // divide full comment into 3 components
                    #[cfg(feature = "lexer_explain")]
                    println!(
                        "current_code_block is '{}'\n\
                        comment_line_prefix is '{}'\n\
                        code_lines_before_comment is '{}'",
                        current_code_block, comment_line_prefix, code_lines_before_comment
                    );

                    // next we have to determine if this is a doc block criteria
                    // for doc blocks for a block comment:
                    //
                    // 1.  must have a space or newline after the opening
                    //     delimiter
                    // 2.  must not have anything besides whitespace before the
                    //     opening comment delimiter on the same line
                    // 3.  must not have anything besides whitespace after the
                    //     closing comment delimiter on the same line
                    if (comment_body.starts_with(' ') || comment_body.starts_with('\n'))
                        && WHITESPACE_ONLY_REGEX.is_match(comment_line_prefix)
                        && WHITESPACE_ONLY_REGEX.is_match(post_closing_delimiter_line)
                    {
                        // put the code_lines_before_comment into the code block
                        append_code_doc_block("", "", code_lines_before_comment);

                        // If there's a space at the end of the comment body,
                        // remove it; also remove the initial space/newline at
                        // the beginning of the comment body.
                        let ends_with_space = match comment_body.chars().last() {
                            Some(last_char) => {
                                last_char == ' ' &&
                                // Don't remove a space at the end of the
                                // comment body when it's also the space at the
                                // beginning of the comment body (meaning it's a
                                // single-character comment body).
                                comment_body.len() > 1
                            }
                            None => false,
                        };
                        let trimmed_comment_body = &comment_body
                            [1..comment_body.len() - if ends_with_space { 1 } else { 0 }];

                        // Add this doc block:
                        let contents =
                            &(trimmed_comment_body.to_string() + post_closing_delimiter_line);
                        append_code_doc_block(
                            // The indent is the whitespace before the opening
                            // comment delimiter.
                            comment_line_prefix,
                            // The opening comment delimiter was captured in the
                            // initial match.
                            matching_group_str,
                            // The contents of the doc block are the trimmed
                            // comment body plus any whitespace after the
                            // closing comment delimiter.
                            contents,
                        );

                        // print the doc block
                        #[cfg(feature = "lexer_explain")]
                        println!("Appending a doc block with indent '{}', delimiter '{}', and contents '{}'.", &comment_line_prefix, matching_group_str, contents);

                        // advance `current_code_block_index` to
                        // `source_code_unlexed_index`, since we've moved
                        // everything in the current code block into the
                        // `classified_source`.
                        current_code_block_index = source_code_unlexed_index;
                    } else {
                        // Nothing to do -- the comment was simply added to the
                        // current code block already.
                    }
                }

                // #### String-like syntax
                RegexDelimType::String(closing_regex) => {
                    #[cfg(feature = "lexer_explain")]
                    print!("This is a string. ");
                    append_code(closing_regex)
                }

                RegexDelimType::TemplateLiteral => {
                    #[cfg(feature = "lexer_explain")]
                    print!("This is a template literal. ");
                    append_code(&TEMPLATE_LITERAL_CLOSING_REGEX);
                }

                RegexDelimType::Heredoc(stop_prefix, stop_suffix) => {
                    #[cfg(feature = "lexer_explain")]
                    print!("This is a heredoc. ");

                    // Get the string from the source code which (along with the
                    // stop prefix/suffix) defines the end of the heredoc.
                    let heredoc_string = &classify_match[language_lexer_compiled.map.len() + 1];
                    // Make a regex from it.
                    let closing_regex = Regex::new(
                        &(stop_prefix.to_owned() + &regex::escape(heredoc_string) + stop_suffix),
                    )
                    .unwrap();
                    // Use this to find the end of the heredoc and add that to
                    // `current_source_code`.
                    append_code(&closing_regex);
                }
            }
        } else {
            // There's no match, so the rest of the source code belongs in the
            // current code block.
            source_code_unlexed_index = source_code.len();
        }
    }

    // Any leftover code is source code.
    append_code_doc_block("", "", &source_code[current_code_block_index..]);

    classified_source
}

// ## Tests
//
// Rust
// [almost mandates](https://doc.rust-lang.org/book/ch11-03-test-organization.html)
// putting tests in the same file as the source, which I dislike. Here's a
// [good discussion](http://xion.io/post/code/rust-unit-test-placement.html) of
// how to place them in another file, for the time when I'm ready to adopt this
// more sane layout.
#[cfg(test)]
mod tests {
    use super::supported_languages::LANGUAGE_LEXER_ARR;
    use super::{compile_lexers, source_lexer, CodeDocBlock, DocBlock};

    // Provide a compact way to create a `CodeDocBlock`.
    fn build_doc_block(indent: &str, delimiter: &str, contents: &str) -> CodeDocBlock {
        return CodeDocBlock::DocBlock(DocBlock {
            indent: indent.to_string(),
            delimiter: delimiter.to_string(),
            contents: contents.to_string(),
            lines: contents.matches("\n").count(),
        });
    }

    fn build_code_block(contents: &str) -> CodeDocBlock {
        return CodeDocBlock::CodeBlock(contents.to_string());
    }

    // ### Source lexer tests
    #[test]
    fn test_py() {
        let llc = compile_lexers(&LANGUAGE_LEXER_ARR);
        let py = llc.map_mode_to_lexer.get("python").unwrap();

        // Try basic cases: make sure than newlines are processed correctly.
        assert_eq!(source_lexer("", py), []);
        assert_eq!(source_lexer("\n", py), [build_code_block("\n")]);
        assert_eq!(source_lexer("\r", py), [build_code_block("\n")]);
        assert_eq!(source_lexer("\r\n", py), [build_code_block("\n")]);

        // Look at a code to doc transition, checking various newline combos.
        assert_eq!(
            source_lexer("\n# Test", py),
            [build_code_block("\n"), build_doc_block("", "#", "Test")]
        );
        assert_eq!(
            source_lexer("\n# Test\n", py),
            [build_code_block("\n"), build_doc_block("", "#", "Test\n")]
        );
        assert_eq!(
            source_lexer("\n# Test\n\n", py),
            [
                build_code_block("\n"),
                build_doc_block("", "#", "Test\n"),
                build_code_block("\n"),
            ]
        );

        // Source followed by a comment.
        assert_eq!(
            source_lexer("a = 1\n# Test", py),
            [
                build_code_block("a = 1\n"),
                build_doc_block("", "#", "Test")
            ]
        );

        // Comments that aren't in doc blocks.
        assert_eq!(
            source_lexer("a = 1 # Test", py),
            [build_code_block("a = 1 # Test"),]
        );
        assert_eq!(
            source_lexer("\na = 1 # Test", py),
            [build_code_block("\na = 1 # Test"),]
        );
        assert_eq!(
            source_lexer("a = 1 # Test\n", py),
            [build_code_block("a = 1 # Test\n"),]
        );
        assert_eq!(source_lexer("#Test\n", py), [build_code_block("#Test\n"),]);

        // Doc blocks
        assert_eq!(source_lexer("#", py), [build_doc_block("", "#", ""),]);
        assert_eq!(source_lexer("#\n", py), [build_doc_block("", "#", "\n"),]);
        assert_eq!(
            source_lexer("  # Test", py),
            [build_doc_block("  ", "#", "Test")]
        );
        assert_eq!(
            source_lexer("  # Test\n", py),
            [build_doc_block("  ", "#", "Test\n")]
        );
        assert_eq!(
            source_lexer("\n  # Test", py),
            [build_code_block("\n"), build_doc_block("  ", "#", "Test")]
        );
        assert_eq!(
            source_lexer("# Test1\n # Test2", py),
            [
                build_doc_block("", "#", "Test1\n"),
                build_doc_block(" ", "#", "Test2")
            ]
        );

        // Doc blocks with empty comments
        assert_eq!(
            source_lexer("# Test 1\n#\n# Test 2", py),
            [build_doc_block("", "#", "Test 1\n\nTest 2"),]
        );
        assert_eq!(
            source_lexer("  # Test 1\n  #\n  # Test 2", py),
            [build_doc_block("  ", "#", "Test 1\n\nTest 2"),]
        );

        // Single-line strings
        assert_eq!(source_lexer("''", py), [build_code_block("''"),]);
        // An unterminated string before EOF.
        assert_eq!(source_lexer("'", py), [build_code_block("'"),]);
        assert_eq!(source_lexer("\"\"", py), [build_code_block("\"\""),]);
        assert_eq!(
            source_lexer("a = 'test'\n", py),
            [build_code_block("a = 'test'\n"),]
        );
        // Terminate a string with a newline
        assert_eq!(
            source_lexer("a = 'test\n", py),
            [build_code_block("a = 'test\n"),]
        );
        assert_eq!(source_lexer(r"'\''", py), [build_code_block(r"'\''"),]);
        assert_eq!(source_lexer("'\\\n'", py), [build_code_block("'\\\n'"),]);
        // This is `\\` followed by a newline, which terminates the string early
        // (syntax error -- unescaped newline in a single-line string).
        assert_eq!(
            source_lexer("'\\\\\n# Test'", py),
            [
                build_code_block("'\\\\\n"),
                build_doc_block("", "#", "Test'")
            ]
        );
        // This is `\\\` followed by a newline, which puts a `\` followed by a
        // newline in the string, so there's no comment.
        assert_eq!(
            source_lexer("'\\\\\\\n# Test'", py),
            [build_code_block("'\\\\\\\n# Test'"),]
        );
        assert_eq!(
            source_lexer("'\\\n# Test'", py),
            [build_code_block("'\\\n# Test'"),]
        );
        assert_eq!(
            source_lexer("'\n# Test'", py),
            [build_code_block("'\n"), build_doc_block("", "#", "Test'")]
        );

        // Multi-line strings
        assert_eq!(
            source_lexer("'''\n# Test'''", py),
            [build_code_block("'''\n# Test'''"),]
        );
        assert_eq!(
            source_lexer("\"\"\"\n#Test\"\"\"", py),
            [build_code_block("\"\"\"\n#Test\"\"\""),]
        );
        assert_eq!(
            source_lexer("\"\"\"Test 1\n\"\"\"\n# Test 2", py),
            [
                build_code_block("\"\"\"Test 1\n\"\"\"\n"),
                build_doc_block("", "#", "Test 2")
            ]
        );
        // Quotes nested inside a multi-line string.
        assert_eq!(
            source_lexer("'''\n# 'Test' 1'''\n# Test 2", py),
            [
                build_code_block("'''\n# 'Test' 1'''\n"),
                build_doc_block("", "#", "Test 2")
            ]
        );
        // An empty string, follow by a comment which ignores the fake
        // multi-line string.
        assert_eq!(
            source_lexer("''\n# Test 1'''\n# Test 2", py),
            [
                build_code_block("''\n"),
                build_doc_block("", "#", "Test 1'''\nTest 2")
            ]
        );
        assert_eq!(
            source_lexer("'''\n# Test 1\\'''\n# Test 2", py),
            [build_code_block("'''\n# Test 1\\'''\n# Test 2"),]
        );
        assert_eq!(
            source_lexer("'''\n# Test 1\\\\'''\n# Test 2", py),
            [
                build_code_block("'''\n# Test 1\\\\'''\n"),
                build_doc_block("", "#", "Test 2")
            ]
        );
        assert_eq!(
            source_lexer("'''\n# Test 1\\\\\\'''\n# Test 2", py),
            [build_code_block("'''\n# Test 1\\\\\\'''\n# Test 2"),]
        );
    }

    #[test]
    fn test_js() {
        let llc = compile_lexers(&LANGUAGE_LEXER_ARR);
        let js = llc.map_mode_to_lexer.get("javascript").unwrap();

        // JavaScript tests.
        //
        // simple inline comment
        assert_eq!(
            source_lexer("// Test", js),
            [build_doc_block("", "//", "Test"),]
        );

        // An empty block comment.
        assert_eq!(source_lexer("/* */", js), [build_doc_block("", "/*", ""),]);
        assert_eq!(source_lexer("/*\n*/", js), [build_doc_block("", "/*", ""),]);

        // basic test
        assert_eq!(
            source_lexer("/* Basic Test */", js),
            [build_doc_block("", "/*", "Basic Test"),]
        );

        // no space after opening delimiter (criteria 1)
        assert_eq!(
            source_lexer("/*Test */", js),
            [build_code_block("/*Test */"),]
        );

        // no space after closing delimiter
        assert_eq!(
            source_lexer("/* Test*/", js),
            [build_doc_block("", "/*", "Test"),]
        );

        // extra spaces after opening delimiter (ok, drop 1)
        assert_eq!(
            source_lexer("/*   Extra Space */", js),
            [build_doc_block("", "/*", "  Extra Space"),]
        );

        // code before opening delimiter (criteria 2)
        assert_eq!(
            source_lexer("a = 1 /* Code Before */", js),
            [build_code_block("a = 1 /* Code Before */"),]
        );

        // 4 spaces before opening delimiter (criteria 2 ok)
        assert_eq!(
            source_lexer("    /* Space Before */", js),
            [build_doc_block("    ", "/*", "Space Before"),]
        );

        // newline in comment
        assert_eq!(
            source_lexer("/* Newline\nIn Comment */", js),
            [build_doc_block("", "/*", "Newline\nIn Comment"),]
        );

        // 3 trailing whitespaces (criteria 3 ok)
        assert_eq!(
            source_lexer("/* Trailing Whitespace  */  ", js),
            [build_doc_block("", "/*", "Trailing Whitespace   "),]
        );

        // code after closing delimiter (criteria 3)
        assert_eq!(
            source_lexer("/* Code After */ a = 1", js),
            [build_code_block("/* Code After */ a = 1"),]
        );

        // Another important case:
        assert_eq!(
            source_lexer("/* Another Important Case */\n", js),
            [build_doc_block("", "/*", "Another Important Case\n"),]
        );

        // No closing delimiter
        assert_eq!(
            source_lexer("/* No Closing Delimiter", js),
            [build_code_block("/* No Closing Delimiter"),]
        );

        // Two closing delimiters
        assert_eq!(
            source_lexer("/* Two Closing Delimiters */ \n */", js),
            [
                build_doc_block("", "/*", "Two Closing Delimiters \n"),
                build_code_block(" */"),
            ]
        );
        // Code before a block comment.
        assert_eq!(
            source_lexer("bears();\n/* Bears */\n", js),
            [
                build_code_block("bears();\n"),
                build_doc_block("", "/*", "Bears\n"),
            ]
        );

        // A newline after the opening comment delimiter.
        assert_eq!(
            source_lexer("test_1();\n/*\nTest 2\n*/", js),
            [
                build_code_block("test_1();\n"),
                build_doc_block("", "/*", "Test 2\n"),
            ]
        );

        // Some basic template literal tests. Comments inside template literal
        // expressions aren't parsed correctly; neither are nested template
        // literals.
        assert_eq!(source_lexer("``", js), [build_code_block("``"),]);
        assert_eq!(source_lexer("`", js), [build_code_block("`"),]);
        assert_eq!(
            source_lexer("`\n// Test`", js),
            [build_code_block("`\n// Test`"),]
        );
        assert_eq!(
            source_lexer("`\\`\n// Test`", js),
            [build_code_block("`\\`\n// Test`"),]
        );
        assert_eq!(
            source_lexer("`\n// Test 1`\n// Test 2", js),
            [
                build_code_block("`\n// Test 1`\n"),
                build_doc_block("", "//", "Test 2")
            ]
        );
        assert_eq!(
            source_lexer("`\n// Test 1\\`\n// Test 2`\n// Test 3", js),
            [
                build_code_block("`\n// Test 1\\`\n// Test 2`\n"),
                build_doc_block("", "//", "Test 3")
            ]
        );
    }

    #[test]
    fn test_cpp() {
        let llc = compile_lexers(&LANGUAGE_LEXER_ARR);
        let cpp = llc.map_mode_to_lexer.get("c_cpp").unwrap();

        // Try out a C++ heredoc.
        assert_eq!(
            source_lexer("R\"heredoc(\n// Test 1)heredoc\"\n// Test 2", cpp),
            [
                build_code_block("R\"heredoc(\n// Test 1)heredoc\"\n"),
                build_doc_block("", "//", "Test 2")
            ]
        );
    }

    #[test]
    fn test_csharp() {
        let llc = compile_lexers(&LANGUAGE_LEXER_ARR);
        let csharp = llc.map_mode_to_lexer.get("csharp").unwrap();

        // Try out a verbatim string literal with embedded double quotes.
        assert_eq!(
            source_lexer("// Test 1\n@\"\n// Test 2\"\"\n// Test 3\"", csharp),
            [
                build_doc_block("", "//", "Test 1\n"),
                build_code_block("@\"\n// Test 2\"\"\n// Test 3\"")
            ]
        );
    }

    #[test]
    fn test_matlab() {
        let llc = compile_lexers(&LANGUAGE_LEXER_ARR);
        let matlab = llc.map_mode_to_lexer.get("matlab").unwrap();

        // Test both inline comment styles. Verify that escaped quotes are
        // ignored, and that doubled quotes are handled correctly.
        assert_eq!(
            source_lexer(
                r#"% Test 1
v = ["Test 2\", ...
 ... "Test 3", ...
     "Test""4"];
"#,
                matlab
            ),
            [
                build_doc_block("", "%", "Test 1\n"),
                build_code_block("v = [\"Test 2\\\", ...\n"),
                build_doc_block(" ", "...", "\"Test 3\", ...\n"),
                build_code_block("     \"Test\"\"4\"];\n"),
            ]
        );

        // Test block comments.
        assert_eq!(
            source_lexer(
                "%{ Test 1
a = 1
  %{
a = 2
  %}
",
                matlab
            ),
            [
                build_code_block("%{ Test 1\na = 1\n"),
                // TODO: currently, whitespace on the line containing the
                // closing block delimiter isn't captured. Fix this.
                build_doc_block("  ", "%{", "a = 2\n"),
            ]
        );
    }

    #[test]
    fn test_rust() {
        let llc = compile_lexers(&LANGUAGE_LEXER_ARR);
        let rust = llc.map_mode_to_lexer.get("rust").unwrap();

        // Test Rust raw strings.
        assert_eq!(
            source_lexer("r###\"\n// Test 1\"###\n// Test 2", rust),
            [
                build_code_block("r###\"\n// Test 1\"###\n"),
                build_doc_block("", "//", "Test 2")
            ]
        );

        // Test Rust comments, which can be nested but aren't here. TODO: test
        // nested comments.
        assert_eq!(
            source_lexer("test_1();\n/* Test 2 */\n", rust),
            [
                build_code_block("test_1();\n"),
                build_doc_block("", "/*", "Test 2\n")
            ]
        );
    }

    #[test]
    fn test_sql() {
        let llc = compile_lexers(&LANGUAGE_LEXER_ARR);
        let sql = llc.map_mode_to_lexer.get("sql").unwrap();

        // Test strings with embedded single quotes.
        assert_eq!(
            source_lexer("-- Test 1\n'\n-- Test 2''\n-- Test 3'", sql),
            [
                build_doc_block("", "--", "Test 1\n"),
                build_code_block("'\n-- Test 2''\n-- Test 3'")
            ]
        );
    }

    #[test]
    fn test_toml() {
        let llc = compile_lexers(&LANGUAGE_LEXER_ARR);
        let toml = llc.map_mode_to_lexer.get("toml").unwrap();
        assert_eq!(toml.language_lexer.ace_mode, "toml");

        // Multi-line literal strings don't have escapes.
        assert_eq!(
            source_lexer("'''\n# Test 1\\'''\n# Test 2", toml),
            [
                build_code_block("'''\n# Test 1\\'''\n"),
                build_doc_block("", "#", "Test 2")
            ]
        );
        // Basic strings have an escape, but don't allow newlines.
        assert_eq!(
            source_lexer("\"\\\n# Test 1\"", toml),
            [
                build_code_block("\"\\\n"),
                build_doc_block("", "#", "Test 1\"")
            ]
        );
    }

    // ### Compiler tests
    #[test]
    fn test_compiler() {
        let llc = compile_lexers(&LANGUAGE_LEXER_ARR);

        let c_ext_lexer_arr = llc.map_ext_to_lexer_vec.get("c").unwrap();
        assert_eq!(c_ext_lexer_arr.len(), 1);
        assert_eq!(c_ext_lexer_arr[0].language_lexer.ace_mode, "c_cpp");
        assert_eq!(
            llc.map_mode_to_lexer
                .get("verilog")
                .unwrap()
                .language_lexer
                .ace_mode,
            "verilog"
        );
    }
}
