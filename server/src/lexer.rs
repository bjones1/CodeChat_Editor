/// <details>
///     <summary>Copyright (C) 2022 Bryan A. Jones.</summary>
///     <p>This file is part of the CodeChat Editor.</p>
///     <p>The CodeChat Editor is free software: you can redistribute it and/or
///         modify it under the terms of the GNU General Public License as
///         published by the Free Software Foundation, either version 3 of the
///         License, or (at your option) any later version.</p>
///     <p>The CodeChat Editor is distributed in the hope that it will be
///         useful, but WITHOUT ANY WARRANTY; without even the implied warranty
///         of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
///         General Public License for more details.</p>
///     <p>You should have received a copy of the GNU General Public License
///         along with the CodeChat Editor. If not, see <a
///             href="http://www.gnu.org/licenses/">http://www.gnu.org/licenses/</a>.
///     </p>
/// </details>
/// <h1><code>lexer.rs</code> &mdash; Lex source code into code and doc blocks
/// </h1>
/// <h2>Submodule definitions</h2>
pub mod supported_languages;

/// <h2>Imports</h2>
/// <h3>Standard library</h3>
use std::collections::HashMap;
use std::sync::Arc;

// <h3>Third-party</h3>
use lazy_static::lazy_static;
use regex;
use regex::Regex;
use serde::ser::{Serialize, SerializeSeq, Serializer};

/// <h2>Data structures</h2>
/// <p>These data structures define everything the lexer needs in order to
///     analyze a programming language:</p>
/// <ul>
///     <li>It defines block and inline comment delimiters; these (when
///         correctly formatted) become doc blocks.</li>
///     <li>It defines strings: what is the escape character? Are newlines
///         allowed? If so, must newlines be escaped?</li>
///     <li>It defines heredocs in a flexible form (see
///         <code>HeredocDelim</code> for more details).</li>
///     <li>It associates an Ace mode and filename extensions with the lexer.
///     </li>
/// </ul>
/// <p>This lexer ignores line continuation characters; in C/C++/Python, it's a
///     <code>\</code> character followed immediately by a newline (<a
///         href="https://www.open-std.org/jtc1/sc22/WG14/www/docs/n1256.pdf#page22">C
///         reference</a>, <a
///         href="https://docs.python.org/3/reference/lexical_analysis.html#explicit-line-joining">Python
///         reference</a>). From a lexer perspective, supporting these adds
///     little value:</p>
/// <ol>
///     <li>It would allow the lexer to recognize the following C/C++ snippet as
///         a doc block:<br><code>// This is an odd\</code><br><code>two-line
///             inline comment.</code><br>However, this such such unusual syntax
///         (most authors would instead use either a block comment or another
///         inline comment) that recognizing it adds little value.</li>
///     <li>I'm unaware of any valid syntax in which ignoring a line
///         continuation would cause the lexer to mis-recognize code as a
///         comment. (Escaped newlines in strings, a separate case, are handled
///         correctly).</li>
/// </ol>
/// <p>This struct defines the delimiters for a block comment.</p>
pub struct BlockCommentDelim<'a> {
    /// <p>A string specifying the opening comment delimiter for a block
    ///     comment.</p>
    pub opening: &'a str,
    /// <p>A string specifying the closing comment delimiter for a block
    ///     comment.</p>
    pub closing: &'a str,
    /// <p>True if block comment may be nested.</p>
    is_nestable: bool,
}

/// <p>Define the types of newlines supported in a string.</p>
enum NewlineSupport {
    /// <p>This string delimiter allows unescaped newlines. This is a multiline
    ///     string.</p>
    Unescaped,
    /// <p>This string delimiter only allows newlines when preceded by the
    ///     string escape character. This is (mostly) a single-line string.</p>
    Escaped,
    /// <p>This string delimiter does not allow newlines. This is strictly a
    ///     single-line string.</p>
    None,
}

/// <p>Define a string from the lexer's perspective.</p>
struct StringDelimiterSpec<'a> {
    /// <p>Delimiter to indicate the start and end of a string.</p>
    delimiter: &'a str,
    /// <p>Escape character, to allow inserting the string delimiter into the
    ///     string. Empty if this string delimiter doesn't provide an escape
    ///     character.</p>
    escape_char: &'a str,
    /// <p>Newline handling. This value cannot be <code>Escaped</code> if the
    ///     <code>escape_char</code> is empty.</p>
    newline_support: NewlineSupport,
}

/// <p>This defines the delimiters for a <a
///         href="https://en.wikipedia.org/wiki/Here_document">heredoc</a> (or
///     heredoc-like literal).</p>
struct HeredocDelim<'a> {
    /// <p>The prefix before the heredoc's delimiting identifier.</p>
    start_prefix: &'a str,
    /// <p>A regex which matches the delimiting identifier.</p>
    delim_ident_regex: &'a str,
    /// <p>The suffix after the delimiting identifier.</p>
    start_suffix: &'a str,
    /// <p>The prefix before the second (closing) delimiting identifier.</p>
    stop_prefix: &'a str,
    /// <p>The suffix after the heredoc's closing delimiting identifier.</p>
    stop_suffix: &'a str,
}

/// <p>Define a language by providing everything this lexer needs in order to
///     split it into code and doc blocks.</p>
pub struct LanguageLexer<'a> {
    /// <p>The <a href="https://ace.c9.io/">Ace</a> <a
    ///         href="https://github.com/ajaxorg/ace/tree/master/src/mode">mode</a>
    ///     to use for this language. The CodeChat Editor Client uses this to
    ///     tell Ace the mode to use. It's can also be used in a
    ///     specially-formatted comment in a source file to override the lexer
    ///     chosen by looking at the file's extension.</p>
    pub ace_mode: &'a str,
    /// <p>An array of file extensions for this language. They <em>do
    ///         not</em>begin with a period, such as <code>rs</code>. This is
    ///     the typical way that the CodeChat Editor uses to determine which
    ///     lexer to use for a given source file.</p>
    ext_arr: &'a [&'a str],
    /// <p>An array of strings which specify inline comment delimiters. Empty if
    ///     this language doesn't provide inline comments.</p>
    pub inline_comment_delim_arr: &'a [&'a str],
    /// <p>An array which specifies opening and closing block comment
    ///     delimiters. Empty if this language doesn't provide block comments.
    /// </p>
    pub block_comment_delim_arr: &'a [BlockCommentDelim<'a>],
    /// <p>Specify the strings supported by this language. While this could be
    ///     empty, such a language would be very odd.</p>
    string_delim_spec_arr: &'a [StringDelimiterSpec<'a>],
    /// <p>A <a href="https://en.wikipedia.org/wiki/Here_document">heredoc</a>
    ///     delimiter; <code>None</code> if heredocs aren't supported.</p>
    heredoc_delim: Option<&'a HeredocDelim<'a>>,
    /// <p><a
    ///         href="https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Template_literals">Template
    ///         literal</a> support (for languages such as JavaScript,
    ///     TypeScript, etc.).</p>
    template_literal: bool,
}

/// <p>Define which delimiter corresponds to a given regex group.</p>
/// <p>This struct stores the results of "compiling" a
///     <code>LanguageLexer</code> into a set of regexes and a map. For example,
///     the JavaScript lexer becomes:</p>
//// Regex          (//)     |    (/*)      |        (")           |         (')          |       (`)
//// Group            1             2                 3                       4                    5
////  Map       InlineComment   BlockComment   String(double-quote)   String(single-quote)   TemplateLiteral
/// <p>The Regex in the table is stored in <code>next_token</code>, which is
///     used to search for the next token. The group is both the group number of
///     the regex - 1 (in other words, a match of <code>//</code> is group 1 of
///     the regex) and the index into <code>map</code>. Map is <code>map</code>,
///     which labeled each group with a <code>RegexDelimType</code>. The lexer
///     uses this to decide how to handle the token it just found -- as a inline
///     comment, block comment, etc. Note: this is a slightly simplified regex;
///     group 1, <code>(/*)</code>, would actually be <code>(/\*)</code>, since
///     the <code>*</code> must be escaped.</p>
enum RegexDelimType {
    InlineComment,
    BlockComment(
        /// <p>The regex used to find the closing delimiter. If the regex
        ///     contains groups, then this is a language that supports nested
        ///     block comments. Group 1 must match an opening comment, while
        ///     group 2 must match the closing comment.</p>
        Regex,
    ),
    String(
        /// <p>The regex used to find the closing delimiter for this string
        ///     type.</p>
        Regex,
    ),
    Heredoc(
        /// <p>The regex-escaped <code>HeredocDelim.stop_prefix</code>.</p>
        String,
        /// <p>The regex-escaped <code>HeredocDelim.stop_suffix</code>.</p>
        String,
    ),
    TemplateLiteral,
    // <p>TODO: Will need more options for nested template literals. Match on
    //     opening brace, closing brace, closing template literal, etc.</p>
}

// <p>To allow comparison for unit tests.</p>
#[derive(PartialEq)]
// <p>To allow printing with <code>println!</code>.</p>
#[derive(Debug)]
/// <p>This defines either a code block or a doc block.</p>
pub struct CodeDocBlock {
    /// <p>For a doc block, the whitespace characters which created the indent
    ///     for this doc block. For a code block, an empty string.</p>
    pub indent: String,
    /// <p>For a doc block, the opening comment delimiter. For a code block, an
    ///     empty string.</p>
    pub delimiter: String,
    /// <p>The contents of this block -- documentation (with the comment
    ///     delimiters removed) or code.</p>
    pub contents: String,
}

// <p>Store the results of compiling a language lexer.</p>
pub struct LanguageLexerCompiled<'a> {
    /// <p>A copy of LanguageLexer::ace_mode.</p>
    pub language_lexer: &'a LanguageLexer<'a>,
    /// <p>A regex used to identify the next token when in a code block.</p>
    next_token: Regex,
    /// <p>A mapping from groups in this regex to the corresponding delimiter
    ///     type matched.</p>
    map: Vec<RegexDelimType>,
}

// <p>Store all lexers and their associated maps after they're compiled.</p>
pub struct LanguageLexersCompiled<'a> {
    // <p>The resulting compiled lexers.</p>
    pub language_lexer_compiled_vec: Vec<Arc<LanguageLexerCompiled<'a>>>,
    // <p>Maps a file extension to indices into the lexers vector.</p>
    pub map_ext_to_lexer_vec: HashMap<&'a str, Vec<Arc<LanguageLexerCompiled<'a>>>>,
    // <p>Maps an Ace mode to an index into the lexers vector.</p>
    pub map_mode_to_lexer: HashMap<&'a str, Arc<LanguageLexerCompiled<'a>>>,
}

// <h2>Globals</h2>
// <p>Create constant regexes needed by the lexer, following the&nbsp;<a
//         href="https://docs.rs/regex/1.6.0/regex/index.html#example-avoid-compiling-the-same-regex-in-a-loop">Regex
//         docs recommendation</a>.</p>
lazy_static! {
    static ref WHITESPACE_ONLY_REGEX: Regex = Regex::new("^[[:space:]]*$").unwrap();
    /// <p>TODO: This regex should also allow termination on an unescaped
    ///     <code>${</code> sequence, which then must count matching braces to
    ///     find the end of the expression.</p>
    static ref TEMPLATE_LITERAL_CLOSING_REGEX: Regex = Regex::new(
        // <p>Allow <code>.</code> to match <em>any</em> character, including a
        //     newline. See the <a
        //         href="https://docs.rs/regex/1.6.0/regex/index.html#grouping-and-flags">regex
        //         docs</a>.</p>
        &("(?s)".to_string() +
        // <p>Start at the beginning of the string, and require a match of every
        //     character. Allowing the regex to start matching in the middle
        //     means it can skip over escape characters.</p>
        "^(" +
            // <p>Allow any non-special character,</p>
            "[^\\\\`]|" +
            // <p>or anything following an escape character (since whatever it
            //     is, it can't be the end of the string).</p>
            "\\\\." +
        // <p>Look for an arbitrary number of these non-string-ending
        //     characters.</p>
        ")*" +
        // <p>Now, find the end of the string: the string delimiter.</p>
        "`"),
    ).unwrap();
}

/// <h2>Functions</h2>
/// <p>Provide a way to turn a <code>CodeDocBlock</code> into JSON.</p>
impl Serialize for CodeDocBlock {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // <p>3 is the number of fields in the seq.</p>
        let mut seq = serializer.serialize_seq(Some(3))?;
        seq.serialize_element(&self.indent)?;
        seq.serialize_element(&self.delimiter)?;
        seq.serialize_element(&self.contents)?;
        seq.end()
    }
}

/// <h3>Language "compiler"</h3>
/// <p>"Compile" a language description into regexes used to lex the language.
/// </p>
fn build_lexer_regex<'a>(
    // <p>The language description to build regexes for.</p>
    language_lexer: &'a LanguageLexer,
    // <p>The "compiled" form of this language lexer.</p>
) -> LanguageLexerCompiled<'a> {
    // <p>Produce the overall regex from regexes which find a specific special
    //     case. TODO: explain this and the next variable.</p>
    let mut regex_strings_arr: Vec<String> = Vec::new();
    // <p>Also create a mapping between the groups in this regex being built and
    //     the delimiter matched by that group.</p>
    let mut regex_group_map: Vec<RegexDelimType> = Vec::new();

    // <p>Given an array of strings containing unescaped characters which
    //     identifies the start of one of the special cases, combine them into a
    //     single string separated by an or operator. Return the index of the
    //     resulting string in <code>regex_strings</code>, or <code>None</code>
    //     if the array is empty (indicating that this language doesn't support
    //     the provided special case).</p>
    let mut regex_builder = |//
                             // <p>An array of alternative delimiters, which
                             //     will be combined with a regex or
                             //     (<code>|</code>) operator.</p>
                             string_arr: &Vec<&str>,
                             // <p>The type of delimiter in
                             //     <code>string_arr</code>.</p>
                             regex_delim_type: RegexDelimType| {
        // <p>If there are no delimiters, then there's nothing to do.</p>
        if string_arr.is_empty() {
            return;
        }
        // <p>Join the array of strings with an or operator.</p>
        let tmp: Vec<String> = string_arr.iter().map(|x| regex::escape(x)).collect();
        regex_strings_arr.push(tmp.join("|"));
        // <p>Store the type of this group.</p>
        regex_group_map.push(regex_delim_type);
    };

    // <p>Add the opening block comment delimiter to the overall regex; add the
    //     closing block comment delimiter to the map for the corresponding
    //     group.</p>
    let mut block_comment_opening_delim: Vec<&str> = vec![""];
    for block_comment_delim in language_lexer.block_comment_delim_arr {
        block_comment_opening_delim[0] = block_comment_delim.opening;
        regex_builder(
            &block_comment_opening_delim,
            // <p>Determine the block closing regex:</p>
            RegexDelimType::BlockComment(
                Regex::new(&if block_comment_delim.is_nestable {
                    // <p>If nested, look for another opening delimiter or the
                    //     closing delimiter.</p>
                    format!(
                        "({})|({})",
                        regex::escape(block_comment_delim.opening),
                        regex::escape(block_comment_delim.closing)
                    )
                } else {
                    // <p>Otherwise, just look for the closing delimiter.</p>
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
    // <p>Build regexes for each string delimiter.</p>
    for string_delim_spec in language_lexer.string_delim_spec_arr {
        // <p>Generate a regex based on the characteristics of this string.</p>
        let has_escape_char = !string_delim_spec.escape_char.is_empty();
        // <p>Look for</p>
        let escaped_delimiter = regex::escape(string_delim_spec.delimiter);
        let escaped_escape_char = regex::escape(string_delim_spec.escape_char);
        let end_of_string_regex = match (has_escape_char, &string_delim_spec.newline_support) {
            // <p>This is the most complex case. This type of string can be
            //     terminated by an unescaped newline or an unescaped delimiter.
            //     Escaped newlines or terminators should be included in the
            //     string.</p>
            (true, NewlineSupport::Escaped) => Regex::new(
                // <p>Allow <code>.</code> to match <em>any</em> character,
                //     including a newline. See the <a
                //         href="https://docs.rs/regex/1.6.0/regex/index.html#grouping-and-flags">regex
                //         docs</a>.</p>
                &("(?s)".to_string() +
                // <p>Start at the beginning of the string, and require a match
                //     of every character. Allowing the regex to start matching
                //     in the middle means it can skip over escape characters.
                // </p>
                "^(" +
                    // <p>Allow any non-special character,</p>
                    &format!("[^\n{}{}]|", escaped_delimiter, escaped_escape_char) +
                    // <p>or anything following an escape character (since
                    //     whatever it is, it can't be the end of the string).
                    // </p>
                    &escaped_escape_char + "." +
                // <p>Look for an arbitrary number of these non-string-ending
                //     characters.</p>
                ")*" +
                // <p>Now, find the end of the string: a newline or the string
                //     delimiter.</p>
                &format!("(\n|{})", escaped_delimiter)),
            ),

            // <p>A bit simpler: this type of string can be terminated by a
            //     newline or an unescaped delimiter. Escaped terminators should
            //     be included in the string.</p>
            (true, NewlineSupport::None) => Regex::new(
                // <p>Start at the beginning of the string, and require a match
                //     of every character. Allowing the regex to start matching
                //     in the middle means it can skip over escape characters.
                // </p>
                &("^(".to_string() +
                    // <p>Allow any non-special character</p>
                    &format!("[^\n{}{}]|", escaped_delimiter, escaped_escape_char) +
                    // <p>or anything following an escape character except a
                    //     newline.</p>
                    &escaped_escape_char + "[^\n]" +
                // <p>Look for an arbitrary number of these non-string-ending
                //     characters.</p>
                ")*" +
                // <p>Now, find the end of the string: a newline optinally
                //     preceded by the escape char or the string delimiter.</p>
                &format!("({}?\n|{})", escaped_escape_char, escaped_delimiter)),
            ),

            // <p>Even simpler: look for an unescaped string delimiter.</p>
            (true, NewlineSupport::Unescaped) => Regex::new(
                // <p>Allow <code>.</code> to match <em>any</em> character,
                //     including a newline. See the <a
                //         href="https://docs.rs/regex/1.6.0/regex/index.html#grouping-and-flags">regex
                //         docs</a>.</p>
                &("(?s)".to_string() +
                // <p>Start at the beginning of the string, and require a match
                //     of every character. Allowing the regex to start matching
                //     in the middle means it can skip over escape characters.
                // </p>
                "^(" +
                    // <p>Allow any non-special character,</p>
                    &format!("[^{}{}]|", escaped_delimiter, escaped_escape_char) +
                    // <p>or anything following an escape character (since
                    //     whatever it is, it can't be the end of the string).
                    // </p>
                    &escaped_escape_char + "." +
                // <p>Look for an arbitrary number of these non-string-ending
                //     characters.</p>
                ")*" +
                // <p>Now, find the end of the string: the string delimiter.</p>
                &escaped_delimiter),
            ),

            // <p>This case makes no sense: there's no escape character, yet the
            //     string allows escaped newlines?</p>
            (false, NewlineSupport::Escaped) => panic!(
                "Invalid parameters for the language lexer where ace_mode = {} and ext_arr = {:?}.",
                language_lexer.ace_mode, language_lexer.ext_arr
            ),

            // <p>The simplest case: just look for the delimiter!</p>
            (false, NewlineSupport::Unescaped) => Regex::new(&escaped_delimiter),

            // <p>Look for either the delimiter or a newline to terminate the
            //     string.</p>
            (false, NewlineSupport::None) => Regex::new(&format!("{}|\n", &escaped_delimiter)),
        }
        .unwrap();
        regex_builder(
            &[regex::escape(string_delim_spec.delimiter).as_str()].to_vec(),
            RegexDelimType::String(end_of_string_regex),
        );
    }
    // <p>Template literals only exist in JavaScript. No other language (that I
    //     know of) allows comments inside these, or nesting of template
    //     literals.</p>
    // <p>Build a regex for template strings.</p>
    // <p>TODO: this is broken! Lexing nested template literals means matching
    //     braces, yikes. For now, don't support this.</p>
    if language_lexer.template_literal {
        // <p>TODO: match either an unescaped <code>${</code> -- which causes a
        //     nested parse -- or the closing backtick (which must be
        //     unescaped).</p>
        regex_builder(&["`"].to_vec(), RegexDelimType::TemplateLiteral);
    }
    // <p>This must be last, since it includes one group (so the index of all
    //     future items will be off by 1). Build a regex for a heredoc start.
    // </p>
    let &regex_str;
    if let Some(heredoc_delim) = language_lexer.heredoc_delim {
        // <p>First, create the string which defines the regex.</p>
        regex_str = format!(
            "{}({}){}",
            regex::escape(heredoc_delim.start_prefix),
            heredoc_delim.delim_ident_regex,
            regex::escape(heredoc_delim.start_suffix)
        );
        // <p>Then add it. Do this manually, since we don't want the regex
        //     escaped.</p>
        regex_strings_arr.push(regex_str);
        regex_group_map.push(RegexDelimType::Heredoc(
            regex::escape(heredoc_delim.stop_prefix),
            regex::escape(heredoc_delim.stop_suffix),
        ));
    }

    // <p>Combine all this into a single regex, which is this or of each
    //     delimiter's regex. Create a capturing group for each delimiter.</p>
    let classify_regex = Regex::new(&format!("({})", regex_strings_arr.join(")|("))).unwrap();

    LanguageLexerCompiled {
        language_lexer,
        next_token: classify_regex,
        map: regex_group_map,
    }
}

// <h2>Compile lexers</h2>
pub fn compile_lexers<'a>(
    language_lexer_arr: &'a [LanguageLexer<'a>],
) -> LanguageLexersCompiled<'a> {
    let mut language_lexers_compiled = LanguageLexersCompiled {
        language_lexer_compiled_vec: Vec::new(),
        map_ext_to_lexer_vec: HashMap::new(),
        map_mode_to_lexer: HashMap::new(),
    };
    // <p>Walk through each lexer.</p>
    for language_lexer in language_lexer_arr {
        // <p>Compile and add it.</p>
        let llc = Arc::new(build_lexer_regex(language_lexer));
        language_lexers_compiled
            .language_lexer_compiled_vec
            .push(Arc::clone(&llc));

        // <p>Add all its extensions to the extension map.</p>
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

        // <p>Add its mode to the mode map.</p>
        language_lexers_compiled
            .map_mode_to_lexer
            .insert(language_lexer.ace_mode, llc);
    }

    language_lexers_compiled
}

/// <h2>Source lexer</h2>
/// <p>This lexer categorizes source code into code blocks or doc blocks.</p>
pub fn source_lexer(
    // <p>The source code to lex.</p>
    source_code: &str,
    // <p>A description of the language, used to lex the
    //     <code>source_code</code>.</p>
    language_lexer_compiled: &LanguageLexerCompiled,
    // <p>The return value is an array of code and doc blocks. The contents of
    //     these blocks contain slices from <code>source_code</code>, so these
    //     have the same lifetime.</p>
) -> Vec<CodeDocBlock> {
    // <p>Rather than attempt to lex the entire language, this lexer's only goal
    //     is to categorize all the source code into code blocks or doc blocks.
    //     To do it, it only needs to:</p>
    // <ul>
    //     <li>Recognize where comments can't be&mdash;inside strings, <a
    //             href="https://en.wikipedia.org/wiki/Here_document">here
    //             text</a>, or <a
    //             href="https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Template_literals">template
    //             literals</a>. These are always part of a code block and can
    //         never contain a comment or (by implication) a doc block.</li>
    //     <li>Outside of these special cases, look for inline or block
    //         comments, categorizing everything else as plain code.</li>
    //     <li>After finding either an inline or block comment, determine if
    //         this is a doc block.</li>
    // </ul>
    // <h3>Lexer construction</h3>
    // <p>To accomplish this goal, construct a <a
    //         href="https://developer.mozilla.org/en-US/docs/Web/JavaScript/Guide/Regular_Expressions">regex</a>
    //     named <code>classify_regex</code> and associated indices from the
    //     language information provided (<code>language_name</code>,
    //     <code>extension_strings</code>, etc.). It divides source code into
    //     two categories: plain code and special cases. The special cases
    //     consist of:</p>
    // <ul>
    //     <li>String-like code (strings, here text, template literals). In this
    //         case, the lexer must find the end of the string-like element
    //         before it can return to plain code.</li>
    //     <li>Comments (inline or block). In this case, the lexer must find the
    //         end of the comment before it can return to plain code.</li>
    // </ul>
    // <p>This regex assumes the string it analyzes was preceded by plain code;
    //     its purpose is to identify the start of the next special case.
    //     <strong>This code makes heavy use of regexes -- read the previous
    //         link thoroughly.</strong></p>
    let mut classified_source: Vec<CodeDocBlock> = Vec::new();

    // <p>Provide a method to intelligently append to the code/doc block vec.
    //     Empty appends are ignored; appends of the same type append to
    //     <code>contents</code> instead of creating a new entry.</p>
    let mut append_code_doc_block = |indent: &str, delimiter: &str, contents: &str| {
        // <p>Don't append empty entries.</p>
        if delimiter.is_empty() && contents.is_empty() {
            assert!(indent.is_empty());
            return;
        }
        // <p>See if there's a previous entry to potentially append to.</p>
        if let Some(last_code_doc_block) = classified_source.last() {
            // <p>See if this is the same type of block.</p>
            if last_code_doc_block.indent == indent && last_code_doc_block.delimiter == delimiter {
                // <p>Yes, so append the provided contents to it. We must access
                //     the array directly since <code>last</code> provides only
                //     a reference.</p>
                let end = classified_source.len() - 1;
                classified_source[end].contents += contents;
                return;
            }
        }
        // <p>We must append a new entry.</p>
        classified_source.push(CodeDocBlock {
            indent: indent.to_string(),
            delimiter: delimiter.to_string(),
            contents: contents.to_string(),
        });
    };

    // <p>An accumulating string composing the current code block.</p>
    let mut current_code_block = String::new();
    // <p>Normalize all line endings.</p>
    let source_code_normalized = source_code.replace("\r\n", "\n").replace('\r', "\n");
    let mut source_code = source_code_normalized.as_str();

    // <p>Main loop: lexer the provided source code.</p>
    while !source_code.is_empty() {
        #[cfg(feature = "lexer_explain")]
        println!(
            "Searching the following source_code using the pattern {:?}:\n'{}'\n\nThe current_code_block is '{}'\n",
            language_lexer_compiled.next_token, source_code, current_code_block
        );
        // <p>Look for the next special case. Per the earlier discussion, this
        //     assumes that the text immediately
        //     preceding&nbsp;<code>source_code</code> was plain code.</p>
        if let Some(classify_match) = language_lexer_compiled.next_token.captures(source_code) {
            // <p>Move everything preceding this match from
            //     <code>source_code</code> to the current code block, since per
            //     the assumptions this is code. Per the <a
            //         href="https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/RegExp/exec#return_value">docs</a>,
            //     <code>m.index</code> is the index of the beginning of the
            //     match.</p>
            let classify_match_start = classify_match.get(0).unwrap().start();
            current_code_block += &source_code[..classify_match_start];
            source_code = &source_code[classify_match_start..];

            // <p>Find the first group that matches.</p>
            let matching_group_index = classify_match
                .iter()
                // <p>Group 0 is the entire match, which is always true. Skip
                //     this group.</p>
                .skip(1)
                .position(|x| x.is_some())
                .unwrap()
                // <p>Correct the resulting group index, since we skipped group
                //     0.</p>
                + 1;
            let matching_group_str = &classify_match[matching_group_index];

            #[cfg(feature = "lexer_explain")]
            println!(
                "Matched the string {} in group {}. The current_code_block is now\n'{}'\n",
                matching_group_str, matching_group_index, current_code_block
            );

            // <p>Append code to <code>current_code_block</code> based on the
            //     provided regex.</p>
            let mut append_code =
                                   // <p>The regex; code up to the end of this
                                   //     match will be appended to
                                   //     <code>current_code_block</code>.</p>
                                   |closing_regex: &Regex| {
                #[cfg(feature = "lexer_explain")]
                println!("Searching for the end of this token using the pattern '{:?}'.", closing_regex);

                // <p>Add the opening delimiter to the code.</p>
                current_code_block += matching_group_str;
                source_code = &source_code[matching_group_str.len()..];
                // <p>Find the closing delimiter.</p>
                if let Some(closing_match) = closing_regex.find(source_code) {
                    #[cfg(feature = "lexer_explain")]
                    println!("Found; adding source_code up to and including this token to current_code_block.");

                    // <p>Include this in code.</p>
                    current_code_block += &source_code[..closing_match.end()];
                    source_code = &source_code[closing_match.end()..];
                } else {
                    #[cfg(feature = "lexer_explain")]
                    println!("Not found; adding all the source_code to current_code_block.");

                    // <p>Then the rest of the code is a string.</p>
                    current_code_block += source_code;
                    source_code = "";
                }
                #[cfg(feature = "lexer_explain")]
                println!("The current_code_block is now\n\
                    '{}'\n", current_code_block);

            };

            // <p>In the map, index 0 refers to group 1 (since group 0 matches
            //     are skipped). Adjust the index for this.</p>
            match &language_lexer_compiled.map[matching_group_index - 1] {
                // <h3>Inline comment</h3>
                // <p>Was this a comment, assuming the selected language
                //     supports inline comments?</p>
                RegexDelimType::InlineComment => {
                    // <p>An inline comment delimiter matched.</p>
                    // <p><strong>First</strong>, find the end of this comment:
                    //     a newline.</p>
                    let end_of_comment_index = source_code.find('\n');

                    // <p>Assign <code>full_comment</code> to contain the entire
                    //     comment, from the inline comment delimiter until the
                    //     newline which ends the comment. No matching newline
                    //     means we're at the end of the file, so the comment is
                    //     all the remaining <code>source_code</code>.</p>
                    let full_comment = if let Some(index) = end_of_comment_index {
                        // <p>Note that <code>index</code> is the index of the
                        //     newline; add 1 to include that newline in the
                        //     comment.</p>
                        &source_code[..index + 1]
                    } else {
                        source_code
                    };

                    // <p>Move to the next block of source code to be lexed.</p>
                    source_code = &source_code[full_comment.len()..];

                    // <p>Currently, <code>current_code_block</code> contains
                    //     preceding code (which might be multiple lines) until
                    //     the inline comment delimiter. Split this on newlines,
                    //     grouping all the lines before the last line into
                    //     <code>code_lines_before_comment</code> (which is all
                    //     code), and everything else (from the beginning of the
                    //     last line to where the inline comment delimiter
                    //     appears) into <code>comment_line_prefix</code>. For
                    //     example, consider the fragment <code>a = 1\nb = 2 //
                    //         Doc</code>. After processing,
                    //     <code>code_lines_before_comment == "a = 1\n"</code>
                    //     and <code>comment_line_prefix == "b = 2 "</code>.</p>
                    let comment_line_prefix = current_code_block.rsplit('\n').next().unwrap();
                    let code_lines_before_comment =
                        &current_code_block[..current_code_block.len() - comment_line_prefix.len()];

                    #[cfg(feature = "lexer_explain")]
                    println!(
                        "This is an inline comment. Source code before the line containing this comment is:\n'{}'\n\
                        The text preceding this comment is: '{}'.\n\
                        The comment is: '{}'\n",
                        code_lines_before_comment, comment_line_prefix, full_comment
                    );

                    // <p><strong>Next</strong>, determine if this comment is a
                    //     doc block. Criteria for doc blocks for an inline
                    //     comment:</p>
                    // <ul>
                    //     <li>All characters preceding the comment on the line
                    //         containing the comment must be whitespace.</li>
                    //     <li>Either:
                    //         <ul>
                    //             <li>The inline comment delimiter is
                    //                 immediately followed by a space, or</li>
                    //             <li>the inline comment delimiter is followed
                    //                 by a newline or the end of the file.</li>
                    //         </ul>
                    //     </li>
                    // </ul>
                    // <p>With this last line located, apply the doc block
                    //     criteria.</p>
                    let ws_only = WHITESPACE_ONLY_REGEX.is_match(comment_line_prefix);
                    // <p>Criteria 1 -- the whitespace matched.</p>
                    if ws_only &&
                        // <p>TODO: generalize this to specific lines that are
                        //     never doc blocks.</p>
                        full_comment != "// prettier-ignore\n"
                        && (
                            // <p>Criteria 2.1</p>
                            full_comment.starts_with(&(matching_group_str.to_string() + " ")) ||
                            // <p>Criteria 2.2</p>
                            (full_comment == (matching_group_str.to_string() + if end_of_comment_index.is_some() {
                            // <p>Compare with a newline if it was found; the
                            //     group of the found newline is 8.</p>
                            "\n" } else {
                            // <p>Compare with an empty string if there's no
                            //     newline.</p>
                            ""
                        }))
                        )
                    {
                        // <p>This is a doc block. Transition from a code block
                        //     to this doc block.</p>
                        append_code_doc_block("", "", code_lines_before_comment);

                        // <p>Add this doc block by pushing the array
                        //     [whitespace before the inline comment, inline
                        //     comment contents, inline comment delimiter].
                        //     Since it's a doc block, then
                        //     <code>comment_line_prefix</code> contains the
                        //     whitespace before this comment.
                        //     <code>inline_comment_string</code> contains the
                        //     inline comment delimiter. For the contents, omit
                        //     the leading space it it's there (this might be
                        //     just a newline or an EOF).</p>
                        let has_space_after_comment =
                            full_comment.starts_with(&(matching_group_str.to_string() + " "));
                        let contents = &full_comment[matching_group_str.len()
                            + if has_space_after_comment { 1 } else { 0 }..];
                        append_code_doc_block(comment_line_prefix, matching_group_str, contents);

                        #[cfg(feature = "lexer_explain")]
                        println!(
                            "This is a doc block. Possibly added the preceding code block\n\
                            '{}'.\n\
                            Added a doc block with indent = '{}', delimiter = '{}', and contents =\n\
                            '{}'.\n",
                            current_code_block, comment_line_prefix, matching_group_str, contents
                        );

                        // <p>We've now stored the current code block in
                        //     <code>classified_lines</code>.</p>
                        current_code_block.clear();
                    } else {
                        // <p>This comment is not a doc block. Add it to the
                        //     current code block.</p>
                        current_code_block += full_comment;
                    }
                }

                RegexDelimType::BlockComment(closing_regex) => {
                    panic!("Unimplemented.")
                }

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

                    // <p>Get the string from the source code which (along with
                    //     the stop prefix/suffix) defines the end of the
                    //     heredoc.</p>
                    let heredoc_string = &classify_match[language_lexer_compiled.map.len() + 1];
                    // <p>Make a regex from it.</p>
                    let closing_regex = Regex::new(
                        &(stop_prefix.to_owned() + &regex::escape(heredoc_string) + stop_suffix),
                    )
                    .unwrap();
                    // <p>Use this to find the end of the heredoc and add that
                    //     to <code>current_source_code</code>.</p>
                    append_code(&closing_regex);
                }
            }
        } else {
            // <p>There's no match, so the rest of the source code belongs in
            //     the current code block.</p>
            current_code_block += source_code;
            source_code = "";
        }
    }

    // <p>Any leftover code is source code.</p>
    append_code_doc_block("", "", &current_code_block);

    classified_source
}

// <h2>Tests</h2>
// <p>Rust&nbsp;<a
//         href="https://doc.rust-lang.org/book/ch11-03-test-organization.html">almost
//         mandates</a> putting tests in the same file as the source, which I
//     dislike. Here's a <a
//         href="http://xion.io/post/code/rust-unit-test-placement.html">good
//         discussion</a> of how to place them in another file, for the time
//     when I'm ready to adopt this more sane layout.</p>
#[cfg(test)]
mod tests {
    use super::supported_languages::LANGUAGE_LEXER_ARR;
    use super::{compile_lexers, source_lexer, CodeDocBlock};

    // <p>Provide a compact way to create a <code>CodeDocBlock</code>.</p>
    fn build_code_doc_block(indent: &str, delimiter: &str, contents: &str) -> CodeDocBlock {
        return CodeDocBlock {
            indent: indent.to_string(),
            delimiter: delimiter.to_string(),
            contents: contents.to_string(),
        };
    }

    // <h3>Source lexer tests</h3>
    #[test]
    fn test_py() {
        let llc = compile_lexers(&LANGUAGE_LEXER_ARR);
        let py = &llc.language_lexer_compiled_vec[4];
        assert_eq!(py.language_lexer.ace_mode, "python");

        // <p>Try basic cases: make sure than newlines are processed correctly.
        // </p>
        assert_eq!(source_lexer("", py), []);
        assert_eq!(source_lexer("\n", py), [build_code_doc_block("", "", "\n")]);
        assert_eq!(source_lexer("\r", py), [build_code_doc_block("", "", "\n")]);
        assert_eq!(
            source_lexer("\r\n", py),
            [build_code_doc_block("", "", "\n")]
        );

        // <p>Look at a code to doc transition, checking various newline combos.
        // </p>
        assert_eq!(
            source_lexer("\n# Test", py),
            [
                build_code_doc_block("", "", "\n"),
                build_code_doc_block("", "#", "Test")
            ]
        );
        assert_eq!(
            source_lexer("\n# Test\n", py),
            [
                build_code_doc_block("", "", "\n"),
                build_code_doc_block("", "#", "Test\n")
            ]
        );
        assert_eq!(
            source_lexer("\n# Test\n\n", py),
            [
                build_code_doc_block("", "", "\n"),
                build_code_doc_block("", "#", "Test\n"),
                build_code_doc_block("", "", "\n"),
            ]
        );

        // <p>Source followed by a comment.</p>
        assert_eq!(
            source_lexer("a = 1\n# Test", py),
            [
                build_code_doc_block("", "", "a = 1\n"),
                build_code_doc_block("", "#", "Test")
            ]
        );

        // <p>Comments that aren't in doc blocks.</p>
        assert_eq!(
            source_lexer("a = 1 # Test", py),
            [build_code_doc_block("", "", "a = 1 # Test"),]
        );
        assert_eq!(
            source_lexer("\na = 1 # Test", py),
            [build_code_doc_block("", "", "\na = 1 # Test"),]
        );
        assert_eq!(
            source_lexer("a = 1 # Test\n", py),
            [build_code_doc_block("", "", "a = 1 # Test\n"),]
        );
        assert_eq!(
            source_lexer("#Test\n", py),
            [build_code_doc_block("", "", "#Test\n"),]
        );

        // <p>Doc blocks</p>
        assert_eq!(source_lexer("#", py), [build_code_doc_block("", "#", ""),]);
        assert_eq!(
            source_lexer("#\n", py),
            [build_code_doc_block("", "#", "\n"),]
        );
        assert_eq!(
            source_lexer("  # Test", py),
            [build_code_doc_block("  ", "#", "Test")]
        );
        assert_eq!(
            source_lexer("  # Test\n", py),
            [build_code_doc_block("  ", "#", "Test\n")]
        );
        assert_eq!(
            source_lexer("\n  # Test", py),
            [
                build_code_doc_block("", "", "\n"),
                build_code_doc_block("  ", "#", "Test")
            ]
        );
        assert_eq!(
            source_lexer("# Test1\n # Test2", py),
            [
                build_code_doc_block("", "#", "Test1\n"),
                build_code_doc_block(" ", "#", "Test2")
            ]
        );

        // <p>Doc blocks with empty comments</p>
        assert_eq!(
            source_lexer("# Test 1\n#\n# Test 2", py),
            [build_code_doc_block("", "#", "Test 1\n\nTest 2"),]
        );
        assert_eq!(
            source_lexer("  # Test 1\n  #\n  # Test 2", py),
            [build_code_doc_block("  ", "#", "Test 1\n\nTest 2"),]
        );

        // <p>Single-line strings</p>
        assert_eq!(
            source_lexer("''", py),
            [build_code_doc_block("", "", "''"),]
        );
        // <p>An unterminated string before EOF.</p>
        assert_eq!(source_lexer("'", py), [build_code_doc_block("", "", "'"),]);
        assert_eq!(
            source_lexer("\"\"", py),
            [build_code_doc_block("", "", "\"\""),]
        );
        assert_eq!(
            source_lexer("a = 'test'\n", py),
            [build_code_doc_block("", "", "a = 'test'\n"),]
        );
        // <p>Terminate a string with a newline</p>
        assert_eq!(
            source_lexer("a = 'test\n", py),
            [build_code_doc_block("", "", "a = 'test\n"),]
        );
        assert_eq!(
            source_lexer(r"'\''", py),
            [build_code_doc_block("", "", r"'\''"),]
        );
        assert_eq!(
            source_lexer("'\\\n'", py),
            [build_code_doc_block("", "", "'\\\n'"),]
        );
        // <p>This is <code>\\</code> followed by a newline, which terminates
        //     the string early (syntax error -- unescaped newline in a
        //     single-line string).</p>
        assert_eq!(
            source_lexer("'\\\\\n# Test'", py),
            [
                build_code_doc_block("", "", "'\\\\\n"),
                build_code_doc_block("", "#", "Test'")
            ]
        );
        // <p>This is <code>\\\</code> followed by a newline, which puts a
        //     <code>\</code> followed by a newline in the string, so there's no
        //     comment.</p>
        assert_eq!(
            source_lexer("'\\\\\\\n# Test'", py),
            [build_code_doc_block("", "", "'\\\\\\\n# Test'"),]
        );
        assert_eq!(
            source_lexer("'\\\n# Test'", py),
            [build_code_doc_block("", "", "'\\\n# Test'"),]
        );
        assert_eq!(
            source_lexer("'\n# Test'", py),
            [
                build_code_doc_block("", "", "'\n"),
                build_code_doc_block("", "#", "Test'")
            ]
        );

        // <p>Multi-line strings</p>
        assert_eq!(
            source_lexer("'''\n# Test'''", py),
            [build_code_doc_block("", "", "'''\n# Test'''"),]
        );
        assert_eq!(
            source_lexer("\"\"\"\n#Test\"\"\"", py),
            [build_code_doc_block("", "", "\"\"\"\n#Test\"\"\""),]
        );
        // <p>An empty string, follow by a comment which ignores the fake
        //     multi-line string.</p>
        assert_eq!(
            source_lexer("''\n# Test 1'''\n# Test 2", py),
            [
                build_code_doc_block("", "", "''\n"),
                build_code_doc_block("", "#", "Test 1'''\nTest 2")
            ]
        );
        assert_eq!(
            source_lexer("'''\n# Test 1\\'''\n# Test 2", py),
            [build_code_doc_block("", "", "'''\n# Test 1\\'''\n# Test 2"),]
        );
        assert_eq!(
            source_lexer("'''\n# Test 1\\\\'''\n# Test 2", py),
            [
                build_code_doc_block("", "", "'''\n# Test 1\\\\'''\n"),
                build_code_doc_block("", "#", "Test 2")
            ]
        );
        assert_eq!(
            source_lexer("'''\n# Test 1\\\\\\'''\n# Test 2", py),
            [build_code_doc_block(
                "",
                "",
                "'''\n# Test 1\\\\\\'''\n# Test 2"
            ),]
        );
    }

    #[test]
    fn test_js() {
        let llc = compile_lexers(&LANGUAGE_LEXER_ARR);
        let js = &llc.language_lexer_compiled_vec[2];
        assert_eq!(js.language_lexer.ace_mode, "javascript");

        // <p>JavaScript tests. TODO: block comments</p>
        assert_eq!(
            source_lexer("// Test", js),
            [build_code_doc_block("", "//", "Test"),]
        );

        // <p>Some basic template literal tests. Comments inside template
        //     literal expressions aren't parsed correctly; neither are nested
        //     template literals.</p>
        assert_eq!(
            source_lexer("``", js),
            [build_code_doc_block("", "", "``"),]
        );
        assert_eq!(source_lexer("`", js), [build_code_doc_block("", "", "`"),]);
        assert_eq!(
            source_lexer("`\n// Test`", js),
            [build_code_doc_block("", "", "`\n// Test`"),]
        );
        assert_eq!(
            source_lexer("`\\`\n// Test`", js),
            [build_code_doc_block("", "", "`\\`\n// Test`"),]
        );
        assert_eq!(
            source_lexer("`\n// Test 1`\n// Test 2", js),
            [
                build_code_doc_block("", "", "`\n// Test 1`\n"),
                build_code_doc_block("", "//", "Test 2")
            ]
        );
        assert_eq!(
            source_lexer("`\n// Test 1\\`\n// Test 2`\n// Test 3", js),
            [
                build_code_doc_block("", "", "`\n// Test 1\\`\n// Test 2`\n"),
                build_code_doc_block("", "//", "Test 3")
            ]
        );
    }

    #[test]
    fn test_cpp() {
        let llc = compile_lexers(&LANGUAGE_LEXER_ARR);
        let cpp = &llc.language_lexer_compiled_vec[0];
        assert_eq!(cpp.language_lexer.ace_mode, "c_cpp");

        // <p>Try out a C++ heredoc.</p>
        assert_eq!(
            source_lexer("R\"heredoc(\n// Test 1)heredoc\"\n// Test 2", cpp),
            [
                build_code_doc_block("", "", "R\"heredoc(\n// Test 1)heredoc\"\n"),
                build_code_doc_block("", "//", "Test 2")
            ]
        );
    }

    #[test]
    fn test_toml() {
        let llc = compile_lexers(&LANGUAGE_LEXER_ARR);
        let toml = &llc.language_lexer_compiled_vec[6];
        assert_eq!(toml.language_lexer.ace_mode, "toml");

        // <p>Multi-line literal strings don't have escapes.</p>
        assert_eq!(
            source_lexer("'''\n# Test 1\\'''\n# Test 2", toml),
            [
                build_code_doc_block("", "", "'''\n# Test 1\\'''\n"),
                build_code_doc_block("", "#", "Test 2")
            ]
        );
        // <p>Basic strings have an escape, but don't allow newlines.</p>
        assert_eq!(
            source_lexer("\"\\\n# Test 1\"", toml),
            [
                build_code_doc_block("", "", "\"\\\n"),
                build_code_doc_block("", "#", "Test 1\"")
            ]
        );
    }

    #[test]
    fn test_rust() {
        let llc = compile_lexers(&LANGUAGE_LEXER_ARR);
        let rust = &llc.language_lexer_compiled_vec[5];
        assert_eq!(rust.language_lexer.ace_mode, "rust");

        // <p>Test Rust raw strings.</p>
        assert_eq!(
            source_lexer("r###\"\n// Test 1\"###\n// Test 2", rust),
            [
                build_code_doc_block("", "", "r###\"\n// Test 1\"###\n"),
                build_code_doc_block("", "//", "Test 2")
            ]
        );
    }

    // <h3>Compiler tests</h3>
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
