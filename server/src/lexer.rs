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
// ## Submodule definitions
pub mod supported_languages;

// ## Imports
//
// ### Standard library
#[cfg(feature = "lexer_explain")]
use std::cmp::min;
use std::{collections::HashMap, sync::Arc};

// ### Third-party
use lazy_static::lazy_static;
use regex::Regex;

// ### Local
use supported_languages::get_language_lexer_vec;

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
///     However, this such such unusual syntax (most authors would instead use
///     either a block comment or another inline comment) that recognizing it
///     adds little value.
/// 2.  I'm unaware of any valid syntax in which ignoring a line continuation
///     would cause the lexer to mis-recognize code as a comment. (Escaped
///     newlines in strings, a separate case, are handled correctly).
///
/// This struct defines the delimiters for a block comment.
#[derive(Clone)]
pub struct BlockCommentDelim {
    /// A string specifying the opening comment delimiter for a block comment.
    pub opening: String,
    /// A string specifying the closing comment delimiter for a block comment.
    pub closing: String,
    /// True if block comment may be nested.
    is_nestable: bool,
}

/// Define the types of newlines supported in a string.
#[derive(Clone)]
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
#[derive(Clone)]
struct StringDelimiterSpec {
    /// Delimiter to indicate the start and end of a string.
    delimiter: String,
    /// Escape character, to allow inserting the string delimiter into the
    /// string. Empty if this string delimiter doesn't provide an escape
    /// character.
    escape_char: String,
    /// Newline handling. This value cannot be `Escaped` if the `escape_char` is
    /// empty.
    newline_support: NewlineSupport,
}

/// This defines the delimiters for a
/// [heredoc](https://en.wikipedia.org/wiki/Here_document) (or heredoc-like
/// literal).
struct HeredocDelim {
    /// The prefix before the heredoc's delimiting identifier.
    start_prefix: String,
    /// A regex which matches the delimiting identifier.
    delim_ident_regex: String,
    /// The suffix after the delimiting identifier.
    start_suffix: String,
    /// The prefix before the second (closing) delimiting identifier.
    stop_prefix: String,
    /// The suffix after the heredoc's closing delimiting identifier.
    stop_suffix: String,
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
pub struct LanguageLexer {
    /// The lexer name which the CodeChat Editor Client uses this to tell
    /// CodeMirror the mode to use. It's can also be used in a
    /// specially-formatted comment in a source file to override the lexer
    /// chosen by looking at the file's extension.
    pub lexer_name: Arc<String>,
    /// An array of file extensions for this language. They \_do not_begin with
    /// a period, such as `rs`. This is the typical way that the CodeChat Editor
    /// uses to determine which lexer to use for a given source file.
    ext_arr: Vec<Arc<String>>,
    /// An array of strings which specify inline comment delimiters. Empty if
    /// this language doesn't provide inline comments.
    pub inline_comment_delim_arr: Vec<String>,
    /// An array which specifies opening and closing block comment delimiters.
    /// Empty if this language doesn't provide block comments.
    pub block_comment_delim_arr: Vec<BlockCommentDelim>,
    /// Specify the strings supported by this language. While this could be
    /// empty, such a language would be very odd.
    string_delim_spec_arr: Vec<StringDelimiterSpec>,
    /// A [heredoc](https://en.wikipedia.org/wiki/Here_document) delimiter;
    /// `None` if heredocs aren't supported.
    heredoc_delim: Option<HeredocDelim>,
    /// Any special case treatment for this language.
    special_case: SpecialCase,
}

/// ### Compiled language definition
// Store the results of compiling a language lexer.
pub struct LanguageLexerCompiled {
    /// Provide the language definition this came from.
    pub language_lexer: LanguageLexer,
    /// A regex used to identify the next token when in a code block.
    next_token: Regex,
    /// A mapping from groups in this regex to the corresponding delimiter type
    /// matched.
    map: Vec<RegexDelimType>,
}

// Store all lexers and their associated maps after they're compiled.
pub struct LanguageLexersCompiled {
    // The resulting compiled lexers.
    pub language_lexer_compiled_vec: Vec<Arc<LanguageLexerCompiled>>,
    // Maps a file extension to indices into the lexers vector.
    pub map_ext_to_lexer_vec: HashMap<Arc<String>, Vec<Arc<LanguageLexerCompiled>>>,
    // Maps an Ace mode to an index into the lexers vector.
    pub map_mode_to_lexer: HashMap<Arc<String>, Arc<LanguageLexerCompiled>>,
}

#[allow(clippy::four_forward_slashes)]
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

    /// A vector of all supported languages.
    pub static ref LEXERS: LanguageLexersCompiled = compile_lexers(get_language_lexer_vec());
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
fn build_lexer_regex(
    // The language description to build regexes for.
    language_lexer: LanguageLexer,
    // The "compiled" form of this language lexer.
) -> LanguageLexerCompiled {
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
                             string_arr: &Vec<String>,
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
    let mut block_comment_opening_delim: Vec<String> = vec!["".to_string()];
    for block_comment_delim in &language_lexer.block_comment_delim_arr {
        block_comment_opening_delim[0].clone_from(&block_comment_delim.opening);
        regex_builder(
            &block_comment_opening_delim,
            // Determine the block closing regex:
            RegexDelimType::BlockComment(
                Regex::new(&if block_comment_delim.is_nestable {
                    // If nested, look for another opening delimiter or the
                    // closing delimiter.
                    format!(
                        "({})|({})",
                        regex::escape(&block_comment_delim.opening),
                        regex::escape(&block_comment_delim.closing)
                    )
                } else {
                    // Otherwise, just look for the closing delimiter.
                    regex::escape(&block_comment_delim.closing)
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
    for string_delim_spec in &language_lexer.string_delim_spec_arr {
        // Generate a regex based on the characteristics of this string.
        let has_escape_char = !string_delim_spec.escape_char.is_empty();
        // For multi-character string delimiters, build a regex: `'''` becomes
        // `(|'|'')`, which allows matches of a partial string delimiter, but
        // not the entire delimiter. For a single-character delimiter, the
        // "regex" is an empty string.
        let string_partial_builder = |delimiter: &str| -> String {
            // If this is a single-character string delimiter, then we're done.
            if delimiter.chars().count() < 2 {
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
        let string_partial_delimiter = string_partial_builder(&string_delim_spec.delimiter);
        // Look for
        let escaped_delimiter = regex::escape(&string_delim_spec.delimiter);
        let escaped_escape_char = regex::escape(&string_delim_spec.escape_char);
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
                "Invalid parameters for the language lexer where lexer_name = {} and ext_arr = {:?}.",
                language_lexer.lexer_name, language_lexer.ext_arr
            ),

            // The simplest case: just look for the delimiter!
            (false, NewlineSupport::Unescaped) => Regex::new(&escaped_delimiter),

            // Look for either the delimiter or a newline to terminate the
            // string.
            (false, NewlineSupport::None) => Regex::new(&format!("{}|\n", &escaped_delimiter)),
        }
        .unwrap();
        regex_builder(
            &[regex::escape(&string_delim_spec.delimiter)].to_vec(),
            RegexDelimType::String(end_of_string_regex),
        );
    }

    match language_lexer.special_case {
        SpecialCase::None => (),
        // A C# verbatim string has asymmetric opening and closing delimiters,
        // making it a special case.
        SpecialCase::CSharpVerbatimStringLiteral => regex_builder(
            &vec!["@\"".to_string()],
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
            regex_builder(&vec!["`".to_string()], RegexDelimType::TemplateLiteral);
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
                    r"^\s*" +
                    // Capture just the opening comment delimiter,
                    r"(%\{)" +
                    // followed by whitespace until the end of the line.
                    r"\s*$" +
                // End the multi-line mode and this non-capturing group.
                ")",
            );
            regex_group_map.insert(
                0,
                RegexDelimType::BlockComment(
                    // Use a similar strategy for finding the closing delimiter.
                    Regex::new(r"(?m:^\s*%\}\s*$)").unwrap(),
                ),
            );
        }
    };

    // This must be last, since it includes one group (so the index of all
    // future items will be off by 1). Build a regex for a heredoc start.
    let regex_str;
    if let Some(heredoc_delim) = &language_lexer.heredoc_delim {
        // First, create the string which defines the regex.
        regex_str = format!(
            "{}({}){}",
            regex::escape(&heredoc_delim.start_prefix),
            heredoc_delim.delim_ident_regex,
            regex::escape(&heredoc_delim.start_suffix)
        );
        // Then add it. Do this manually, since we don't want the regex escaped.
        regex_strings_arr.push(regex_str);
        regex_group_map.push(RegexDelimType::Heredoc(
            regex::escape(&heredoc_delim.stop_prefix),
            regex::escape(&heredoc_delim.stop_suffix),
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
pub fn compile_lexers(language_lexer_arr: Vec<LanguageLexer>) -> LanguageLexersCompiled {
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
        for ext in &llc.language_lexer.ext_arr {
            match language_lexers_compiled.map_ext_to_lexer_vec.get_mut(ext) {
                None => {
                    let new_lexer_vec = vec![Arc::clone(&llc)];
                    language_lexers_compiled
                        .map_ext_to_lexer_vec
                        .insert(ext.clone(), new_lexer_vec);
                }
                Some(v) => v.push(Arc::clone(&llc)),
            }
        }

        // Add its mode to the mode map.
        language_lexers_compiled
            .map_mode_to_lexer
            .insert(llc.language_lexer.lexer_name.clone(), llc);
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
    // To better explain the operation of the lexer, see the
    // [lexer walkthrough](lexer/lexer-walkthrough.md).
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
        // Define a line as any characters up to an including a newline. If the
        // contents doesn't end in a newline, then add an extra line. The
        // reasoning: A string such as "foo" is one line (not zero lines), even
        // without a final newline. Only the empty string "" is zero lines.
        let lines = contents.matches('\n').count()
            + (if contents.chars().last().unwrap_or('\n') == '\n' {
                0
            } else {
                1
            });
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
                RegexDelimType::BlockComment(comment_delim_regex) => 'block_comment: {
                    #[cfg(feature = "lexer_explain")]
                    println!("Block Comment Found.");

                    // Determine the location of the beginning of this block
                    // comment's content.
                    let mut comment_start_index =
                        source_code_unlexed_index + matching_group_str.len();

                    #[cfg(feature = "lexer_explain")]
                    println!(
                        "The opening delimiter is '{}', and the closing delimiter regex is '{}'.",
                        matching_group_str, comment_delim_regex
                    );

                    // For nested comments, only treat the innermost comment as
                    // a potential doc block; everything else is treated as
                    // code. The rationale:
                    //
                    // 1.  Typically, nested comments are used to comment out a
                    //     block of code, which may already contain "real"
                    //     comments (as opposed to commented-out code).
                    //     Therefore, we assume that only these innermost
                    //     comments are true comments, while everything else is
                    //     code. I can't think of any reason to nest true
                    //     comments. Assuming a legitimate use for nested
                    //     comments, what criteria would distinguish a nested
                    //     comment from a commented-out code block?
                    // 2.  The CodeChat Editor data structures don't support
                    //     nested doc blocks. So, while we might be able to
                    //     correctly parse nested comments as doc blocks, the
                    //     code that transforms these back to code would remove
                    //     the nesting.
                    // 3.  We lack criteria that would distinguish a nested doc
                    //     block from commented-out code.
                    //
                    // With these assumptions, we need to know if the current
                    // comment is the innermost or not. If the last block
                    // comment delimiter encountered was an opening comment, and
                    // the current block comment delimiter is a closing block
                    // comment, then this is an innermost comment which could be
                    // a doc block. Otherwise, treat the text as a code block.
                    let mut last_delimiter_was_opening = true;
                    // To correctly handle nested block comments, we must avoid
                    // any other parsing (recognizing strings/heredocs, in
                    // particular) until we leave the nested comment block.
                    // Therefore, keep track of the nesting depth; when this
                    // returns to 0, we've found outermost closing block comment
                    // delimiter, and can return to normal parsing. At this
                    // point in the code, we've found one opening block comment
                    // delimiter, so the nesting depth starts at 1.
                    let mut nesting_depth = 1;
                    let mut loop_count = 0;
                    // Loop until we've outside all nested block comments.
                    while nesting_depth != 0 && loop_count < 10 {
                        loop_count += 1;
                        // Get the index of the next block comment delimiter.
                        #[cfg(feature = "lexer_explain")]
                        println!(
                            "Looking for a block comment delimiter in '{}'.",
                            &source_code[comment_start_index
                                ..min(comment_start_index + 30, source_code.len())]
                        );
                        let delimiter_captures_wrapped =
                            comment_delim_regex.captures(&source_code[comment_start_index..]);
                        if delimiter_captures_wrapped.is_none() {
                            #[cfg(feature = "lexer_explain")]
                            println!("The closing comment delimiter wasn't found.");
                            // If there's no closing delimiter, this is not a
                            // doc block; it's a syntax error. The safe route is
                            // to assume the rest of the contents are code,
                            // which this program won't edit; it does edit
                            // comments by cleaning up HTML tags, word-wrapping,
                            // etc. which would be a disaster if this was
                            // applied to code.
                            source_code_unlexed_index = source_code.len();
                            // Exit the block comment processing code here.
                            break 'block_comment;
                        }
                        let delimiter_captures = delimiter_captures_wrapped.unwrap();
                        // Sanity check:
                        assert!(
                            // either this language doesn't support nested
                            // comments, so only the overall match group (a
                            // closing block comment delimiter) was captured,
                            // or...
                            delimiter_captures.len() == 1
                                    // ...for languages that support nested
                                    // comments, there are two capture groups
                                    // (in addition to capture group 0, the
                                    // overall match): the opening comment
                                    // delimiter and the closing comment
                                    // delimiter.
                                    || (delimiter_captures.len() == 3
                                        // Exactly one of these two groups should
                                        // match.
                                        && ((delimiter_captures.get(1).is_some()
                                            && delimiter_captures.get(2).is_none())
                                            || (delimiter_captures.get(1).is_none()
                                                && delimiter_captures.get(2).is_some())))
                        );
                        // Is this an opening comment delimiter?
                        if let Some(opening_delimiter) = delimiter_captures.get(1) {
                            // Yes.
                            last_delimiter_was_opening = true;
                            nesting_depth += 1;
                            // Mark all previous text as code, then continue the
                            // loop.
                            #[cfg(feature = "lexer_explain")]
                            println!(
                                "opening_delimiter.start() = {}, opening_delimiter.len() = {}",
                                opening_delimiter.start(),
                                opening_delimiter.len()
                            );
                            source_code_unlexed_index +=
                                comment_start_index + opening_delimiter.start();
                            comment_start_index =
                                source_code_unlexed_index + opening_delimiter.len();
                            #[cfg(feature = "lexer_explain")]
                            println!(
                                "Found a nested opening block comment delimiter. Nesting depth: {}",
                                &nesting_depth
                            );
                            continue;
                        } else {
                            // This is a closing comment delimiter.
                            nesting_depth -= 1;
                            assert!(nesting_depth >= 0);
                            let closing_delimiter_match = if delimiter_captures.len() == 3 {
                                delimiter_captures.get(2).unwrap()
                            } else {
                                delimiter_captures.get(0).unwrap()
                            };

                            // If `last_delimiter_was_opening` was false, then
                            // mark this text as code and continue the loop.
                            if !last_delimiter_was_opening {
                                source_code_unlexed_index += comment_start_index
                                    + closing_delimiter_match.start()
                                    + closing_delimiter_match.len();
                                last_delimiter_was_opening = false;
                                #[cfg(feature = "lexer_explain")]
                                println!("Found a non-innermost closing block comment delimiter. Nesting depth: {}", &nesting_depth);
                                continue;
                            }

                            // Otherwise, this is a potential doc block: it's an
                            // innermost nested block comment. See if this
                            // qualifies as a doc block.
                            let closing_delimiter_start_index =
                                closing_delimiter_match.start() + comment_start_index;
                            let closing_delimiter_end_index =
                                closing_delimiter_match.end() + comment_start_index;

                            // Capture the body of the comment -- everything but
                            // the opening and closing delimiters.
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
                            // Find the first \\n after the closing delimiter.
                            // If there is a newline after the closing
                            // delimiter, set
                            // `newline_or_eof_after_closing_delimiter_index` to
                            // the index of the first newline after the closing
                            // delimiter else set it to the end of the file.
                            let newline_or_eof_after_closing_delimiter_index =
                                match source_code[closing_delimiter_end_index..].find('\n') {
                                    // The + 1 includes the newline in the
                                    // resulting index.
                                    Some(index) => index + closing_delimiter_end_index + 1,
                                    None => source_code.len(),
                                };

                            // Capture the line which begins after the closing
                            // delimiter and ends at the next newline/EOF.
                            let post_closing_delimiter_line = &source_code
                                [closing_delimiter_end_index
                                    ..newline_or_eof_after_closing_delimiter_index];

                            #[cfg(feature = "lexer_explain")]
                            println!(
                                "The post-comment line is '{}'.",
                                post_closing_delimiter_line
                            );

                            // Set the `current_code_block` to contain preceding
                            // code (which might be multiple lines) until the
                            // block comment delimiter. Split this on newlines,
                            // grouping all the lines before the last line into
                            // `code_lines_before_comment` (which is all code),
                            // and everything else (from the beginning of the
                            // last line to where the block comment delimiter
                            // appears) into `comment_line_prefix`. For example,
                            // consider the fragment:
                            // `a = 1\nb = 2 /* comment */`. After processing,
                            // `code_lines_before_comment` will be "`a = 1\n`"
                            // and `comment_line_prefix` will be "`b = 2` ".
                            let current_code_block =
                                &source_code[current_code_block_index..source_code_unlexed_index];
                            let comment_line_prefix =
                                current_code_block.rsplit('\n').next().unwrap();
                            let code_lines_before_comment = &current_code_block
                                [..current_code_block.len() - comment_line_prefix.len()];

                            // Move to the next block of source code to be
                            // lexed.
                            source_code_unlexed_index =
                                newline_or_eof_after_closing_delimiter_index;

                            #[cfg(feature = "lexer_explain")]
                            println!(
                                "current_code_block is '{}'\n\
                            comment_line_prefix is '{}'\n\
                            code_lines_before_comment is '{}'",
                                current_code_block, comment_line_prefix, code_lines_before_comment
                            );

                            // Next, determine if this is a doc block. Criteria
                            // for doc blocks for a block comment:
                            //
                            // 1.  Must have a space or newline after the
                            //     opening delimiter.
                            // 2.  Must not have anything besides whitespace
                            //     before the opening comment delimiter on the
                            //     same line. This whitespace becomes the
                            //     indent.
                            // 3.  Must not have anything besides whitespace
                            //     after the closing comment delimiter on the
                            //     same line. This whitespace is included, as if
                            //     it were inside the block comment. Rationale:
                            //     this avoids deleting text (or, in this case,
                            //     whitespace); moving that whitespace around
                            //     seems like a better alternative than deleting
                            //     it.
                            if (comment_body.starts_with(' ') || comment_body.starts_with('\n'))
                                && WHITESPACE_ONLY_REGEX.is_match(comment_line_prefix)
                                && WHITESPACE_ONLY_REGEX.is_match(post_closing_delimiter_line)
                            {
                                // Put the `code_lines_before_comment` into the
                                // code block.
                                append_code_doc_block("", "", code_lines_before_comment);

                                // If there's a space at the end of the comment
                                // body, remove it; also remove the initial
                                // space/newline at the beginning of the comment
                                // body.
                                //
                                // This `unwrap()` is always safe, since we know
                                // that `comment_body` starts with a space or
                                // newline.
                                let last_char = comment_body.chars().last().unwrap();
                                let ends_with_space = last_char == ' ' &&
                                    // Don't remove a space at the end of the
                                    // comment body when it's also the space at
                                    // the beginning of the comment body
                                    // (meaning it's a single-character comment
                                    // body).
                                    comment_body.len() > 1;
                                let trimmed_comment_body = &comment_body
                                    [1..comment_body.len() - if ends_with_space { 1 } else { 0 }];
                                // The contents of the doc block are the trimmed
                                // comment body plus any whitespace after the
                                // closing comment delimiter.
                                let contents = &(trimmed_comment_body.to_string()
                                    + post_closing_delimiter_line);
                                // The indent is the whitespace before the
                                // opening comment delimiter.
                                let indent = comment_line_prefix;
                                // The opening comment delimiter was captured in
                                // the initial match.
                                let delimiter = matching_group_str;

                                // #### Block comment indentation processing
                                //
                                // There are several cases:
                                //
                                // - A single line: `/* comment */`. No special
                                //   handling needed.
                                // - Multiple lines, in two styles.
                                //   - Each line of the comment is not
                                //     consistently whitespace-indented. No
                                //     special handling needed. For example:
                                //
                                //     ```C
                                //     /* This is
                                //       not
                                //        consistently indented. */
                                //     ```
                                //
                                //   - Each line of the comment is consistently
                                //     whitespace-indented; for example:
                                //
                                //     ```C
                                //     /* This is
                                //        consistently indented. */
                                //     ```
                                //
                                //     Consistently indented means the first
                                //     non-whitespace character on a line aligns
                                //     with, but never comes before, the
                                //     comment's start. Another example:
                                //
                                //     ```C
                                //     /* This is
                                //        correct
                                //
                                //        indentation.
                                //      */
                                //     ```
                                //
                                //     Note that the third (blank) line doesn't
                                //     have an indent; since that line consists
                                //     only of whitespace, this is OK. Likewise,
                                //     the last line (containing the closing
                                //     comment delimiter of `*/`) consists only
                                //     of whitespace after the comment
                                //     delimiters are removed.
                                //
                                // Determine if this comment is indented.
                                //
                                // Determine the starting column of the indent
                                // (assuming this block comment has a valid
                                // indent). The +1 represents the space after
                                // the opening delimiter.
                                let indent_column = indent.len() + delimiter.len() + 1;
                                let split_contents: Vec<&str> =
                                    contents.split_inclusive('\n').collect();
                                // We need at least two lines of comment
                                // contents to look for an indent. This is just
                                // a first guess at `is_indented`, not the final
                                // value.
                                let mut is_indented = split_contents.len() > 1;
                                if is_indented {
                                    // Ignore the first line, since the indent
                                    // and delimiter have already been split out
                                    // for that line.
                                    for line in &split_contents[1..] {
                                        let this_line_indent = if line.len() < indent_column {
                                            line
                                        } else {
                                            &line[..indent_column]
                                        };
                                        if !WHITESPACE_ONLY_REGEX.is_match(this_line_indent) {
                                            is_indented = false;
                                            break;
                                        }
                                    }
                                }

                                // If the comment was indented, dedent it;
                                // otherwise, leave it unchanged.
                                let mut buf = String::new();
                                let dedented_contents = if is_indented {
                                    // If this is indented, then the first line
                                    // must exist.
                                    buf += split_contents[0];
                                    for line in &split_contents[1..] {
                                        // Remove the indent, unless this line
                                        // didn't have an indent (just whitespace).
                                        buf += if line.len() < indent_column {
                                            // Tricky case: in the middle of a comment,
                                            // every line always ends with a newline;
                                            // if there's not enough whitespace to
                                            // remove the indent, then replace that
                                            // with just a newline. At the end of a
                                            // comment which is the last line of a
                                            // file, a lack of whitespace shouldn't be
                                            // replaced with a newline, since it's not
                                            // there in the original.
                                            if line.ends_with('\n') {
                                                "\n"
                                            } else {
                                                ""
                                            }
                                        } else {
                                            &line[indent_column..]
                                        };
                                    }
                                    &buf
                                } else {
                                    contents
                                };

                                // Add this doc block:
                                append_code_doc_block(indent, delimiter, dedented_contents);

                                // print the doc block
                                #[cfg(feature = "lexer_explain")]
                                println!("Appending a doc block with indent '{}', delimiter '{}', and contents '{}'.", &comment_line_prefix, matching_group_str, contents);

                                // advance `current_code_block_index` to
                                // `source_code_unlexed_index`, since we've
                                // moved everything in the current code block
                                // into the `classified_source`.
                                current_code_block_index = source_code_unlexed_index;
                                // Likewise, move the `comment_start_index` up,
                                // since everything before
                                // `source_code_unlexed_index` has been
                                // classified.
                                comment_start_index = source_code_unlexed_index;
                            } else {
                                // Nothing to do -- the comment was simply added
                                // to the current code block already.
                            }
                        }
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
// putting tests in the same file as the source, which I dislike. Here's a way
// to place them in a separate file.
#[cfg(test)]
mod tests;
