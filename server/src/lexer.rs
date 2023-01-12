/// <details>
///      <summary>Copyright (C) 2022 Bryan A. Jones.</summary>
///      <p>This file is part of the CodeChat Editor.</p>
///      <p>The CodeChat Editor is free software: you can redistribute it and/or
///          modify it under the terms of the GNU General Public License as
///          published by the Free Software Foundation, either version 3 of the
///          License, or (at your option) any later version.</p>
///      <p>The CodeChat Editor is distributed in the hope that it will be useful,
///          but WITHOUT ANY WARRANTY; without even the implied warranty of
///          MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
///          General Public License for more details.</p>
///      <p>You should have received a copy of the GNU General Public License
///          along with the CodeChat Editor. If not, see <a
///              href="http://www.gnu.org/licenses/">http://www.gnu.org/licenses/</a>.
///      </p>
///  </details>
/// <h1><code>lexer.rs</code> &mdash; Lex source code into code and doc blocks</h1>
pub mod supported_languages;

use lazy_static::lazy_static;
use regex;
use regex::Regex;

/// <h2>Data structures</h2>
/// <p>This struct defines the delimiters for a block comment.</p>
struct BlockCommentDelim<'a> {
    /// <p>A string specifying the opening comment delimiter for a block comment.
    /// </p>
    opening: &'a str,
    /// <p>A string specifying the closing comment delimiter for a block comment.
    /// </p>
    closing: &'a str,
    /// True if block comment may be nested.
    is_nestable: bool,
}

/// Define the types of newlines supported in a string.
enum NewlineSupport {
    /// This string delimiter allows unescaped newlines. This is a multiline string.
    Unescaped,
    /// This string delimiter only allows newlines when preceded by the string escape character. This is (mostly) a single-line string.
    Escaped,
    /// This string delimiter does not allow newlines. This is strictly a single-line string.
    None,
}

/// Define a string from the lexer's perspective.
struct StringDelimiterSpec<'a> {
    /// Delimiter to indicate the start and end of a string.
    delimiter: &'a str,
    /// Escape character, to allow inserting the string delimiter into the string. Empty if this string delimiter doesn't provide an escape character.
    escape_char: &'a str,
    /// <p>Newline handling. This value cannot be <code>Escaped</code> if the <code>escape_char</code> is empty.
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

/// Define a language by providing everything this lexer needs in order to split it into code and doc blocks.
pub struct LanguageLexer<'a> {
    /// <p>The Ace mode to use for this language</p>
    ace_mode: &'a str,
    /// <p>An array of file extensions for this language. They begin with a period,
    ///     such as <code>.rs</code>.</p>
    ext_arr: &'a [&'a str],
    /// <p>An array of strings which specify inline comment delimiters. Empty if this language doesn't provide inline comments.</p>
    inline_comment_delim_arr: &'a [&'a str],
    /// <p>An array which specifies opening and closing block comment delimiters. Empty if this language doesn't provide block comments.
    /// </p>
    block_comment_delim_arr: &'a [BlockCommentDelim<'a>],
    /// Specify the strings supported by this language. While this could be empty, such a language would be very odd.
    string_delim_spec_arr: &'a [StringDelimiterSpec<'a>],
    /// <p>A heredoc delimiter; <code>None</code> if heredocs aren't supported.</p>
    heredoc_delim: Option<&'a HeredocDelim<'a>>,
    /// <p>Template literal support (for languages such as JavaScript, TypeScript,
    ///     etc.).
    template_literal: bool,
}

/// Define which delimiter corresponds to a given regex group.
enum RegexDelimType {
    InlineComment,
    BlockComment(
        /// The regex used to find the closing delimiter. If the regex contains groups, then this is a language that supports nested block comments. Group 1 must match an opening comment, while group 2 must match the closing comment.
        Regex,
    ),
    String(
        // The regex used to find the closing delimiter for this string type.
        Regex,
    ),
    Heredoc(
        /// The regex-escaped <code>HeredocDelim.stop_prefix</code>.
        String,
        /// The regex-escaped <code>HeredocDelim.stop_suffix</code>.
        String,
    ),
    TemplateLiteral,
    // TODO: Will need more options for nested template literals. Match on opening brace, closing brace, closing template literal, etc.
}

/// This struct store the results of "compiling" a <code>LanguageLexer</code> into a set of regexes and a map. For example, the JavaScript lexer becomes:
//// Regex          (//)     |    (/*)      |        (")           |         (')          |       (`)
//// Group            0             1                 2                       3                    4
////  Map       InlineComment   BlockComment   String(double-quote)   String(single-quote)   TemplateLiteral
/// <p>The Regex in the table is stored in <code>next_token</code>, which is used to search for the next token. The group is both the group number of the regex - 1 (in other words, a match of <code>//<code> is group 1 of the regex) and the index into <code>map</code>. Map is <code>map</code>, which labeled each group with a <code>RegexDelimType</code>. The lexer uses this to decide how to handle the token it just found -- as a inline comment, block comment, etc. Note: this is a slightly simplified regex; group 1, <code>(/*)</code>, would actually be <code>(/\*)</code>, since the <code>*</code> must be escaped.</p>
struct LanguageLexerCompiled {
    /// A regex used to identify the next token when in a code block.
    next_token: Regex,
    /// A mapping from groups in this regex to the corresponding delimiter type matched.
    map: Vec<RegexDelimType>,
}

// Create constant regexes needed by the lexer, following the <a href="https://docs.rs/regex/1.6.0/regex/index.html#example-avoid-compiling-the-same-regex-in-a-loop">Regex docs recommendation</a>.
lazy_static! {
    static ref WHITESPACE_ONLY_REGEX: Regex = Regex::new("^[[:space:]]*$").unwrap();
    /// TODO: This regex should also allow termination on an unescaped <code>${</code> sequence, which then must count matching braces to find the end of the expression.
    static ref TEMPLATE_LITERAL_CLOSING_REGEX: Regex = Regex::new(
        // Allow <code>.</code> to match <em>any</em> character, including a newline. See the <a href="https://docs.rs/regex/1.6.0/regex/index.html#grouping-and-flags">regex docs</a>.
        &("(?s)".to_string() +
        // Start at the beginning of the string, and require a match of every character. Allowing the regex to start matching in the middle means it can skip over escape characters.
        "^(" +
            //  Allow any non-special character,
            "[^\\\\`]|" +
            // or anything following an escape character (since whatever it is, it can't be the end of the string).
            "\\\\." +
        // Look for an arbitrary number of these non-string-ending characters.
        ")*" +
        // Now, find the end of the string: the string delimiter.
        "`"),
    ).unwrap();
}

/// "Compile" a language description into regexes used to lex the language.
fn build_lexer_regex(
    // The language description to build regexes for.
    language_lexer: &LanguageLexer,
    // The "compiled" form of this language lexer.
) -> LanguageLexerCompiled {
    // <p>Produce the overall regex from regexes which find a specific special
    //     case. TODO: explain this and the next variable.</p>
    let mut regex_strings_arr: Vec<String> = Vec::new();
    // Also create a mapping between the groups in this regex being built and the delimiter matched by that group.
    let mut regex_group_map: Vec<RegexDelimType> = Vec::new();

    // <p>Given an array of strings containing unescaped characters which
    //     identifies the start of one of the special cases, combine them into a
    //     single string separated by an or operator. Return the index of the
    //     resulting string in <code>regex_strings</code>, or <code>None</code>
    //     if the array is empty (indicating that this language doesn't support
    //     the provided special case).</p>
    let mut regex_builder = |//
                             // An array of alternative delimiters, which will be combined with a regex or (<code>|</code>) operator.
                             string_arr: &Vec<&str>,
                             // The type of delimiter in <code>string_arr</code>.
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

    // <p>Add the opening block comment delimiter to the overall regex; add the closing block comment delimiter to the map for the corresponding group.</p>
    let mut block_comment_opening_delim: Vec<&str> = vec![""];
    for block_comment_delim in language_lexer.block_comment_delim_arr {
        block_comment_opening_delim[0] = block_comment_delim.opening;
        regex_builder(
            &block_comment_opening_delim,
            // Determine the block closing regex:
            RegexDelimType::BlockComment(
                Regex::new(&if block_comment_delim.is_nestable {
                    // If nested, look for another opening delimiter or the closing delimiter.
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
        // Look for
        let escaped_delimiter = regex::escape(string_delim_spec.delimiter);
        let escaped_escape_char = regex::escape(string_delim_spec.escape_char);
        let end_of_string_regex = match (has_escape_char, &string_delim_spec.newline_support) {
            // This is the most complex case. This type of string can be terminated by an unescaped newline or an unescaped delimiter. Escaped newlines or terminators should be included in the string.
            (true, NewlineSupport::Escaped) => Regex::new(
                // Allow <code>.</code> to match <em>any</em> character, including a newline. See the <a href="https://docs.rs/regex/1.6.0/regex/index.html#grouping-and-flags">regex docs</a>.
                &("(?s)".to_string() +
                // Start at the beginning of the string, and require a match of every character. Allowing the regex to start matching in the middle means it can skip over escape characters.
                "^(" +
                    //  Allow any non-special character,
                    &format!("[^\n{}{}]|", escaped_delimiter, escaped_escape_char) +
                    // or anything following an escape character (since whatever it is, it can't be the end of the string).
                    &escaped_escape_char + "." +
                // Look for an arbitrary number of these non-string-ending characters.
                ")*" +
                // Now, find the end of the string: a newline or the string delimiter.
                &format!("(\n|{})", escaped_delimiter)),
            ),

            // A bit simpler: this type of string can be terminated by a newline or an unescaped delimiter. Escaped terminators should be included in the string.
            (true, NewlineSupport::None) => Regex::new(
                // Start at the beginning of the string, and require a match of every character. Allowing the regex to start matching in the middle means it can skip over escape characters.
                &("^(".to_string() +
                    //  Allow any non-special character
                    &format!("[^\n{}{}]|", escaped_delimiter, escaped_escape_char) +
                    // or anything following an escape character except a newline.
                    &escaped_escape_char + "[^\n]" +
                // Look for an arbitrary number of these non-string-ending characters.
                ")*" +
                // Now, find the end of the string: a newline optinally preceded by the escape char or the string delimiter.
                &format!("({}?\n|{})", escaped_escape_char, escaped_delimiter)),
            ),

            // Even simpler: look for an unescaped string delimiter.
            (true, NewlineSupport::Unescaped) => Regex::new(
                // Allow <code>.</code> to match <em>any</em> character, including a newline. See the <a href="https://docs.rs/regex/1.6.0/regex/index.html#grouping-and-flags">regex docs</a>.
                &("(?s)".to_string() +
                // Start at the beginning of the string, and require a match of every character. Allowing the regex to start matching in the middle means it can skip over escape characters.
                "^(" +
                    //  Allow any non-special character,
                    &format!("[^{}{}]|", escaped_delimiter, escaped_escape_char) +
                    // or anything following an escape character (since whatever it is, it can't be the end of the string).
                    &escaped_escape_char + "." +
                // Look for an arbitrary number of these non-string-ending characters.
                ")*" +
                // Now, find the end of the string: the string delimiter.
                &escaped_delimiter),
            ),

            // This case makes no sense: there's no escape character, yet the string allows escaped newlines?
            (false, NewlineSupport::Escaped) => panic!(
                "Invalid parameters for the language lexer where ace_mode = {} and ext_arr = {:?}.",
                language_lexer.ace_mode, language_lexer.ext_arr
            ),

            // The simplest case: just look for the delimiter!
            (false, NewlineSupport::Unescaped) => Regex::new(&escaped_delimiter),

            // Look for either the delimiter or a newline to terminate the string.
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
    // TODO: this is broken! Lexing nested template literals means matching braces, yikes. For now, don't support this.
    if language_lexer.template_literal {
        // TODO: match either an unescaped <code>${</code> -- which causes a nested parse -- or the closing backtick (which must be unescaped).
        regex_builder(&["`"].to_vec(), RegexDelimType::TemplateLiteral);
    }
    // This must be last, since it includes one group (so the index of all future items will be off by 1). Build a regex for a heredoc start.
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

    // Combine all this into a single regex, which is this or of each delimiter's regex. Create a capturing group for each delimiter.
    let classify_regex = Regex::new(&format!("({})", regex_strings_arr.join(")|("))).unwrap();

    LanguageLexerCompiled {
        next_token: classify_regex,
        map: regex_group_map,
    }
}

// To allow comparison for unit tests.
#[derive(PartialEq)]
// To allow printing with <code>println!</code>.
#[derive(Debug)]
/// This defines either a code block or a doc block.
pub struct CodeDocBlock {
    /// For a doc block, the whitespace characters which created the indent for this doc block. For a code block, an empty string.
    indent: String,
    /// For a doc block, the opening comment delimiter. For a code block, an empty string.
    delimiter: String,
    /// The contents of this block -- documentation (with the comment delimiters removed) or code.
    contents: String,
}

/// <h2>Source lexer</h2>
/// <p>This lexer categorizes source code into code blocks or doc blocks.
pub fn source_lexer(
    // <p>The source code to lex.</p>
    source_code: &str,
    // A description of the language, used to lex the <code>source_code</code>.
    language_lexer: &LanguageLexer,
    // The return value is an array of code and doc blocks. The contents of these blocks contain slices from <code>source_code</code>, so these have the same lifetime.
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
    //     <code>extension_strings</code>, etc.). It divides source code
    //     into two categories: plain code and special cases. The special cases
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
    let language_lexer_compiled = build_lexer_regex(language_lexer);
    let mut classified_source: Vec<CodeDocBlock> = Vec::new();

    // Provide a method to intelligently append to the code/doc block vec. Empty appends are ignored; appends of the same type append to <code>contents</code> instead of creating a new entry.
    let mut append_code_doc_block = |indent: &str, delimiter: &str, contents: &str| {
        // Don't append empty entries.
        if delimiter.is_empty() && contents.is_empty() {
            assert!(indent.is_empty());
            return;
        }
        // See if there's a previous entry to potentially append to.
        if let Some(last_code_doc_block) = classified_source.last() {
            // See if this is the same type of block.
            if last_code_doc_block.indent == indent && last_code_doc_block.delimiter == delimiter {
                // Yes, so append the provided contents to it. We must access the array directly since <code>last</code> provides only a reference.
                let end = classified_source.len() - 1;
                classified_source[end].contents += contents;
                return;
            }
        }
        // We must append a new entry.
        classified_source.push(CodeDocBlock {
            indent: indent.to_string(),
            delimiter: delimiter.to_string(),
            contents: contents.to_string(),
        });
    };

    // <p>An accumulating string composing the current code block.</p>
    let mut current_code_block = String::new();
    // Normalize all line endings.
    let source_code_normalized = source_code.replace("\r\n", "\n").replace('\r', "\n");
    let mut source_code = source_code_normalized.as_str();

    // Main loop: lexer the provided source code.
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

            // Find the first group that matches.
            let matching_group_index = classify_match
                .iter()
                // Group 0 is the entire match, which is always true. Skip this group.
                .skip(1)
                .position(|x| x.is_some())
                .unwrap()
                // Correct the resulting group index, since we skipped group 0.
                + 1;
            let matching_group_str = &classify_match[matching_group_index];

            #[cfg(feature = "lexer_explain")]
            println!(
                "Matched the string {} in group {}. The current_code_block is now\n'{}'\n",
                matching_group_str, matching_group_index, current_code_block
            );

            // Append code to <code>current_code_block</code> based on the provided regex.
            let mut append_code =
                                   // The regex; code up to the end of this match will be appended to <code>current_code_block</code>.
                                   |closing_regex: &Regex| {
                #[cfg(feature = "lexer_explain")]
                println!("Searching for the end of this token using the pattern '{:?}'.", closing_regex);

                // Add the opening delimiter to the code.
                current_code_block += matching_group_str;
                source_code = &source_code[matching_group_str.len()..];
                // Find the closing delimiter.
                if let Some(closing_match) = closing_regex.find(source_code) {
                    #[cfg(feature = "lexer_explain")]
                    println!("Found; adding source_code up to and including this token to current_code_block.");

                    // Include this in code.
                    current_code_block += &source_code[..closing_match.end()];
                    source_code = &source_code[closing_match.end()..];
                } else {
                    #[cfg(feature = "lexer_explain")]
                    println!("Not found; adding all the source_code to current_code_block.");

                    // Then the rest of the code is a string.
                    current_code_block += source_code;
                    source_code = "";
                }
                #[cfg(feature = "lexer_explain")]
                println!("The current_code_block is now\n\
                    '{}'\n", current_code_block);

            };

            // In the map, index 0 refers to group 1 (since group 0 matches are skipped). Adjust the index for this.
            match &language_lexer_compiled.map[matching_group_index - 1] {
                // <h3>Inline comment</h3>
                // <p>Was this a comment, assuming the selected language supports inline comments?</p>
                RegexDelimType::InlineComment => {
                    // <p>An inline comment delimiter matched.</p>
                    // <p><strong>First</strong>, find the end of this comment: a
                    //     newline.</p>
                    let end_of_comment_index = source_code.find('\n');

                    // <p>Assign <code>full_comment</code> to contain the entire
                    //     comment, from the inline comment delimiter until the
                    //     newline which ends the comment. No matching newline means
                    //     we're at the end of the file, so the comment is all the
                    //     remaining <code>source_code</code>.</p>
                    let full_comment = if let Some(index) = end_of_comment_index {
                        // Note that <code>index</code> is the index of the newline; add 1 to include that newline in the comment.
                        &source_code[..index + 1]
                    } else {
                        source_code
                    };

                    // <p>Move to the next block of source code to be lexed.</p>
                    source_code = &source_code[full_comment.len()..];

                    // Currently, <code>current_code_block</code> contains preceding code
                    //     (which might be multiple lines) until the inline comment
                    //     delimiter. Split this on newlines, grouping all the lines before the last line into <code>code_lines_before_comment</code> (which is all code), and everything else (from the beginning of the last line to where the inline comment delimiter appears) into <code>comment_line_prefix</code>. For example, consider the fragment <code>a = 1\nb = 2 // Doc</code>. After processing, <code>code_lines_before_comment == "a = 1\n"</code> and <code>comment_line_prefix == "b = 2 "</code>.
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

                    // <p><strong>Next</strong>, determine if this comment is a doc
                    //     block. Criteria for doc blocks for an inline comment:</p>
                    // <ul>
                    //     <li>All characters preceding the comment on the line
                    //         containing the comment must be whitespace.</li>
                    //     <li>Either:
                    //         <ul>
                    //             <li>The inline comment delimiter is immediately
                    //                 followed by a space, or</li>
                    //             <li>the inline comment delimiter is followed by a
                    //                 newline or the end of the file.</li>
                    //         </ul>
                    //     </li>
                    // </ul>
                    // <p>With this last line located, apply the doc block criteria.
                    // </p>
                    let ws_only = WHITESPACE_ONLY_REGEX.is_match(comment_line_prefix);
                    // Criteria 1 -- the whitespace matched.
                    if ws_only
                        && (
                            // Criteria 2.1
                            full_comment.starts_with(&(matching_group_str.to_string() + " ")) ||
                            // Criteria 2.2
                            (full_comment == (matching_group_str.to_string() + if end_of_comment_index.is_some() {
                            // Compare with a newline if it was found; the group of the found newline is 8.
                            "\n" } else {
                            // Compare with an empty string if there's no newline.
                            ""
                        }))
                        )
                    {
                        // <p>This is a doc block. Transition from a code block to
                        //     this doc block.</p>
                        append_code_doc_block("", "", code_lines_before_comment);

                        // <p>Add this doc block by pushing the array [whitespace
                        //     before the inline comment, inline comment contents,
                        //     inline comment delimiter]. Since it's a doc block,
                        //     then <code>comment_line_prefix</code> contains
                        //     the whitespace before this comment.
                        //     <code>inline_comment_string</code> contains the
                        //     inline comment delimiter. For the contents, omit the
                        //     leading space it it's there (this might be just a
                        //     newline or an EOF).</p>
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

                        // We've now stored the current code block in <code>classified_lines</code>.
                        current_code_block.clear();
                    } else {
                        // <p>This comment is not a doc block. Add it to the current code block.</p>
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

                    // Get the string from the source code which (along with the stop prefix/suffix) defines the end of the heredoc.
                    let heredoc_string = &classify_match[language_lexer_compiled.map.len() + 1];
                    // Make a regex from it.
                    let closing_regex = Regex::new(
                        &(stop_prefix.to_owned() + &regex::escape(heredoc_string) + stop_suffix),
                    )
                    .unwrap();
                    // Use this to find the end of the heredoc and add that to <code>current_source_code</code>.
                    append_code(&closing_regex);
                }
            }
        } else {
            // <p>There's no match, so the rest of the source code belongs in the current code block.</p>
            current_code_block += source_code;
            source_code = "";
        }
    }

    // Any leftover code is source code.
    append_code_doc_block("", "", &current_code_block);

    classified_source
}

// Rust <a href="https://doc.rust-lang.org/book/ch11-03-test-organization.html">almost mandates</a> putting tests in the same file as the source, which I dislike. Here's a <a href="http://xion.io/post/code/rust-unit-test-placement.html">good discussion</a> of how to place them in another file, for the time when I'm ready to adopt this more sane layout.
#[cfg(test)]
mod tests {
    use super::supported_languages::LANGUAGE_LEXER_ARR;
    use super::{source_lexer, CodeDocBlock};

    // Provide a compact way to create a <code>CodeDocBlock</code>.
    fn build_code_doc_block(indent: &str, delimiter: &str, contents: &str) -> CodeDocBlock {
        return CodeDocBlock {
            indent: indent.to_string(),
            delimiter: delimiter.to_string(),
            contents: contents.to_string(),
        };
    }

    #[test]
    fn test_py() {
        let py = &LANGUAGE_LEXER_ARR[4];
        assert_eq!(py.ace_mode, "python");

        // Try basic cases: make sure than newlines are processed correctly.
        assert_eq!(source_lexer("", py), []);
        assert_eq!(source_lexer("\n", py), [build_code_doc_block("", "", "\n")]);
        assert_eq!(source_lexer("\r", py), [build_code_doc_block("", "", "\n")]);
        assert_eq!(
            source_lexer("\r\n", py),
            [build_code_doc_block("", "", "\n")]
        );

        // Look at a code to doc transition, checking various newline combos.
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

        // Source followed by a comment.
        assert_eq!(
            source_lexer("a = 1\n# Test", py),
            [
                build_code_doc_block("", "", "a = 1\n"),
                build_code_doc_block("", "#", "Test")
            ]
        );

        // Comments that aren't in doc blocks.
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

        // Doc blocks
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

        // Doc blocks with empty comments
        assert_eq!(
            source_lexer("# Test 1\n#\n# Test 2", py),
            [build_code_doc_block("", "#", "Test 1\n\nTest 2"),]
        );
        assert_eq!(
            source_lexer("  # Test 1\n  #\n  # Test 2", py),
            [build_code_doc_block("  ", "#", "Test 1\n\nTest 2"),]
        );

        // Single-line strings
        assert_eq!(
            source_lexer("''", py),
            [build_code_doc_block("", "", "''"),]
        );
        // An unterminated string before EOF.
        assert_eq!(source_lexer("'", py), [build_code_doc_block("", "", "'"),]);
        assert_eq!(
            source_lexer("\"\"", py),
            [build_code_doc_block("", "", "\"\""),]
        );
        assert_eq!(
            source_lexer("a = 'test'\n", py),
            [build_code_doc_block("", "", "a = 'test'\n"),]
        );
        // Terminate a string with a newline
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
        // This is <code>\\</code> followed by a newline, which terminates the string early (syntax error -- unescaped newline in a single-line string).
        assert_eq!(
            source_lexer("'\\\\\n# Test'", py),
            [
                build_code_doc_block("", "", "'\\\\\n"),
                build_code_doc_block("", "#", "Test'")
            ]
        );
        // This is <code>\\\</code> followed by a newline, which puts a <code>\</code> followed by a newline in the string, so there's no comment.
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

        // Multi-line strings
        assert_eq!(
            source_lexer("'''\n# Test'''", py),
            [build_code_doc_block("", "", "'''\n# Test'''"),]
        );
        assert_eq!(
            source_lexer("\"\"\"\n#Test\"\"\"", py),
            [build_code_doc_block("", "", "\"\"\"\n#Test\"\"\""),]
        );
        // An empty string, follow by a comment which ignores the fake multi-line string.
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
        let js = &LANGUAGE_LEXER_ARR[2];
        assert_eq!(js.ace_mode, "javascript");

        // JavaScript tests. TODO: block comments
        assert_eq!(
            source_lexer("// Test", js),
            [build_code_doc_block("", "//", "Test"),]
        );

        // Some basic template literal tests. Comments inside template literal expressions aren't parsed correctly; neither are nested template literals.
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
        let cpp = &LANGUAGE_LEXER_ARR[0];
        assert_eq!(cpp.ace_mode, "c_cpp");

        // Try out a C++ heredoc.
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
        let toml = &LANGUAGE_LEXER_ARR[6];
        assert_eq!(toml.ace_mode, "toml");

        // Multi-line literal strings don't have escapes.
        assert_eq!(
            source_lexer("'''\n# Test 1\\'''\n# Test 2", toml),
            [
                build_code_doc_block("", "", "'''\n# Test 1\\'''\n"),
                build_code_doc_block("", "#", "Test 2")
            ]
        );
        // Basic strings have an escape, but don't allow newlines.
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
        let rust = &LANGUAGE_LEXER_ARR[5];
        assert_eq!(rust.ace_mode, "rust");

        // Test Rust raw strings.
        assert_eq!(
            source_lexer("r###\"\n// Test 1\"###\n// Test 2", rust),
            [
                build_code_doc_block("", "", "r###\"\n// Test 1\"###\n"),
                build_code_doc_block("", "//", "Test 2")
            ]
        );
    }
}
