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
    // <p>A string specifying the opening comment delimiter for a block comment.
    // </p>
    opening: &'a str,
    // <p>A string specifying the closing comment delimiter for a block comment.
    // </p>
    closing: &'a str,
}

// Define the types of newlines supported in a string.
enum NewlineSupport {
    // This string delimiter allows unescaped newlines. This is a multiline string.
    Unescaped,
    // This string delimiter only allows newlines when preceded by the string escape character. This is (mostly) a single-line string.
    Escaped,
    // This string delimiter does not allow newlines. This is strictly a single-line string.
    None,
}

// Define a string from the lexer's perspective.
struct StringDelimiterSpec<'a> {
    // Delimiter to indicate the start and end of a string.
    delimiter: &'a str,
    // Escape character, to allow inserting the string delimiter into the string. Empty if this string delimiter doesn't provide an escape character.
    escape_char: &'a str,
    // <p>Newline handling. This value cannot be <code>Escaped</code> if the <code>escape_char</code> is empty.
    newline_support: NewlineSupport,
}

// <p>This defines the delimiters for a <a
//         href="https://en.wikipedia.org/wiki/Here_document">heredoc</a> (or
//     heredoc-like literal).</p>
struct HeredocDelim<'a> {
    // <p>The prefix before the heredoc's delimiting identifier.</p>
    start_prefix: &'a str,
    // <p>A regex which matches the delimiting identifier.</p>
    delim_ident_regex: &'a str,
    // <p>The suffix after the delimiting identifier.</p>
    start_suffix: &'a str,
    // <p>The prefix before the second (closing) delimiting identifier.</p>
    stop_prefix: &'a str,
    // <p>The suffix after the heredoc's closing delimiting identifier.</p>
    stop_suffix: &'a str,
}

// Indicate this language's support for <a href="https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Template_literals">template literals</a>.
enum TemplateLiteral {
    // This language does not contain template literals.
    No,
    // This language does contain template literals.
    Yes,
    // Indicates the lexer is inside a nested template literal; for internal use only.
    Nested,
}

// Define a language by providing everything this lexer needs in order to split it into code and doc blocks.
pub struct LanguageLexer<'a> {
    // <p>The Ace mode to use for this language</p>
    ace_mode: &'a str,
    // <p>An array of file extensions for this language. They begin with a period,
    //     such as <code>.rs</code>.</p>
    ext_arr: &'a [&'a str],
    // A string specifying the line continuation character; an empty string if this language doesn't contain it.
    line_continuation: &'a str,
    // <p>An array of strings which specify inline comment delimiters. Empty if this language doesn't provide inline comments.</p>
    inline_comment_delim_arr: &'a [&'a str],
    // <p>An array which specifies opening and closing block comment delimiters. Empty if this language doesn't provide block comments.
    // </p>
    block_comment_delim_arr: &'a [BlockCommentDelim<'a>],
    // Specify the strings supported by this language. While this could be empty, such a language would be very odd.
    string_delim_spec_arr: &'a [StringDelimiterSpec<'a>],
    // <p>A heredoc delimiter; <code>None</code> if heredocs aren't supported.</p>
    heredoc_delim: Option<&'a HeredocDelim<'a>>,
    // <p>Template literal support (for languages such as JavaScript, TypeScript,
    //     etc.). A value of <code>none</code> indicates the lexer is inside a template; this should only be used by the <code>source_lexer</code> itself.</p>
    template_literal: TemplateLiteral,
}

#[derive(Debug)]
pub struct CodeDocBlock {
    // For a doc block, the whitespace characters which created the indent for this doc block. For a code block, an empty string.
    indent: String,
    // For a doc block, the opening comment delimiter. For a code block, an empty string.
    delimiter: String,
    // The contents of this block -- documentation (with the comment delimiters removed) or code.
    contents: String,
}

// Define which delimiter corresponds to a given regex group.
enum RegexDelimType {
    InlineComment,
    BlockComment,
    String(
        // The index into <code>string_index_arr</code> for this delimiter.
        usize,
    ),
    Heredoc,
    TemplateLiteral,
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
        // If there are no delimiters, then indicate that this returned index isn't valid.
        if string_arr.len() == 0 {
            return;
        }
        // Join the array of strings with an or operator.
        let tmp: Vec<String> = string_arr.iter().map(|x| regex::escape(x)).collect();
        regex_strings_arr.push(tmp.join("|"));
        // Store the type of this group.
        regex_group_map.push(regex_delim_type);
    };

    // <p>Include only the opening block comment string (element 0) in the
    //     regex.</p>
    let block_comment_opening_delim: Vec<&str> = language_lexer
        .block_comment_delim_arr
        .iter()
        .map(|x| x.opening)
        .collect();
    regex_builder(&block_comment_opening_delim, RegexDelimType::BlockComment);
    regex_builder(
        &language_lexer.inline_comment_delim_arr.to_vec(),
        RegexDelimType::InlineComment,
    );
    // Build regexes for each string delimiter.
    for (index, string_delim) in language_lexer.string_delim_spec_arr.iter().enumerate() {
        regex_builder(
            &[regex::escape(string_delim.delimiter).as_str()].to_vec(),
            RegexDelimType::String(index),
        );
    }
    // <p>Template literals only exist in JavaScript. No other language (that I
    //     know of) allows comments inside these, or nesting of template
    //     literals.</p>
    // <p>Build a regex for template strings.</p>
    let mut tmp: Vec<&str> = Vec::new();
    match language_lexer.template_literal {
        TemplateLiteral::Yes => tmp.push("`"),
        TemplateLiteral::No => (),
        // <p>If inside a template literal, look for a nested template literal
        //     (<code>`</code>) or the end of the current expression
        //     (<code>}</code>).</p>
        TemplateLiteral::Nested => tmp.push("`|}"),
    };
    regex_builder(&tmp, RegexDelimType::TemplateLiteral);
    // This must be last, since it includes one group (so the index of all future items will be off by 1). Build a regex for a heredoc start.
    let &regex_str;
    tmp.clear();
    if let Some(heredoc_delim) = language_lexer.heredoc_delim {
        // First, create the string which defines the regex.
        regex_str = format!(
            "{}({}){}",
            regex::escape(heredoc_delim.start_prefix),
            heredoc_delim.delim_ident_regex,
            regex::escape(heredoc_delim.start_suffix)
        );
        // Then add it.
        regex_builder(&[regex_str.as_str()].to_vec(), RegexDelimType::Heredoc);
    }

    // Combine all this into a single regex, which is this or of each delimiter's regex. Create a capturing group for each delimiter.
    let tmp = format!("({})", regex_strings_arr.join(")|("));
    let classify_regex = Regex::new(tmp.as_str()).unwrap();
    println!("{:?}", classify_regex);

    // Create other regexes needed by the lexer. Where possible, follow the <a href="https://docs.rs/regex/1.6.0/regex/index.html#example-avoid-compiling-the-same-regex-in-a-loop">Regex docs recommendation</a>.
    const NEWLINE_REGEX_FRAGMENT: &str = "(\r\n|\n|\r)";
    lazy_static! {
        static ref NEWLINE_REGEX: Regex = Regex::new(&NEWLINE_REGEX_FRAGMENT).unwrap();
    }
    let tmp;
    let end_of_line_regex = Regex::new(match language_lexer.line_continuation {
        // If there is no line continuation character, the regex is straightforward.
        "" => NEWLINE_REGEX_FRAGMENT,
        // Otherwise, create a regex which ignores newlines preceded by the line continuation character.
        _ => {
            // Look for
            tmp = "(".to_string() +
                    // a line continuation character followed by a newline
                    &regex::escape(language_lexer.line_continuation) + NEWLINE_REGEX_FRAGMENT +
                    // or anything except a newline
                    "|[^\n\r]" +
                // zero or more times,
                r")*" +
                // followed by a newline. Note that using a negative lookbehind
                //     assertion would make this much simpler:
                //     <code>/(?&lt;!\\)(\n|\r\n|\r)/</code>. However, Regex doesn't
                //     support this.
                NEWLINE_REGEX_FRAGMENT;
            tmp.as_str()
        }
    })
    .unwrap();
    lazy_static! {
        static ref WHITESPACE_ONLY_REGEX: Regex = Regex::new("^\\s*$").unwrap();
    }

    // <h3>Lexer operation</h3>
    let mut classified_source: Vec<CodeDocBlock> = Vec::new();
    // <p>An accumulating string composing the current code block.</p>
    let mut current_code_block = String::new();
    // Make a mutable reference to <code>source_code</code>.
    let mut source_code = source_code;
    while source_code.len() != 0 {
        // <p>Look for the next special case. Per the earlier discussion, this
        //     assumes that the text immediately
        //     preceding&nbsp;<code>source_code</code> was plain code.</p>
        if let Some(classify_match) = classify_regex.captures(source_code) {
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
                .unwrap();
            let matching_group_str = &classify_match[matching_group_index];
            match regex_group_map[matching_group_index] {
                // <h3>Inline comment</h3>
                // <p>Was this a comment, assuming the select language supports inline comments?</p>
                RegexDelimType::InlineComment => {
                    // <p>An inline comment delimiter matched.</p>
                    // <p><strong>First</strong>, find the end of this comment: a
                    //     newline that's not escaped by a line continuation
                    //     character (which is <code>\</code> in C/C++/many
                    //     languages). </p>
                    let end_of_comment = &end_of_line_regex.captures(source_code);

                    // <p>Assign <code>full_comment</code> to contain the entire
                    //     comment, from the inline comment delimiter until the
                    //     newline which ends the comment. No matching newline means
                    //     we're at the end of the file, so the comment is all the
                    //     remaining <code>source_code</code>.</p>
                    let full_comment = if let Some(end_of_comment_match) = end_of_comment {
                        &source_code[..end_of_comment_match.get(0).unwrap().end()]
                    } else {
                        source_code
                    };

                    // <p>Move to the next block of source code to be lexed.</p>
                    source_code = &source_code[full_comment.len()..];

                    // Currently, <code>current_code_block</code> contains preceding code
                    //     (which might be multiple lines) until the inline comment
                    //     delimiter. Split this on newlines, grouping all the lines before the last line into <code>code_lines_before_comment</code> (which is all code), and everything else (from the beginning of the last line to where the inline comment delimiter appears) into <code>comment_line_prefix</code>. For example, consider the fragment <code>a = 1\nb = 2 // Doc</code>. After processing, <code>code_lines_before_comment == "a = 1\n"</code> and <code>comment_line_prefix == "b = 2 "</code>.
                    let comment_line_prefix =
                        NEWLINE_REGEX.split(&current_code_block).last().unwrap();
                    let code_lines_before_comment =
                        &current_code_block[..current_code_block.len() - comment_line_prefix.len()];

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
                            full_comment.starts_with((matching_group_str.to_string() + &" ").as_str()) ||
                        // Criteria 2.2
                        (full_comment == &(matching_group_str.to_string() + if let Some(ref end_of_comment_match) = end_of_comment {
                            // Compare with a newline if it was found; the group of the found newline is 8.
                            &end_of_comment_match[0] } else {
                            // Compare with an empty string if there's no newline.
                            &""
                        }))
                        )
                    {
                        // <p>This is a doc block. Transition from a code block to
                        //     this doc block.</p>
                        if code_lines_before_comment != "" {
                            // <p>Save only code blocks with some content.</p>
                            classified_source.push(CodeDocBlock {
                                indent: "".to_string(),
                                delimiter: "".to_string(),
                                contents: code_lines_before_comment.to_string(),
                            });
                        }

                        // <p>Add this doc block by pushing the array [whitespace
                        //     before the inline comment, inline comment contents,
                        //     inline comment delimiter]. Since it's a doc block,
                        //     then <code>comment_line_prefix</code> contains
                        //     the whitespace before this comment.
                        //     <code>inline_comment_string</code> contains the
                        //     inline comment delimiter. For the contents, omit the
                        //     leading space it it's there (this might be just a
                        //     newline or an EOF).</p>
                        let has_space_after_comment = full_comment
                            .starts_with((matching_group_str.to_string() + &" ").as_str());
                        classified_source.push(CodeDocBlock {
                            indent: comment_line_prefix.to_string(),
                            delimiter: matching_group_str.to_string(),
                            contents: full_comment[matching_group_str.len()
                                + if has_space_after_comment { 1 } else { 0 }..]
                                .to_string(),
                        });

                        // We've now stored the current code block in <code>classified_lines</code>.
                        current_code_block.clear();
                    } else {
                        // <p>This comment is not a doc block. Add it to the current code block.</p>
                        current_code_block += &full_comment;
                    }
                }
                // TODO: handle block comments, strings, heredocs, template literals.
                _ => panic!("Unimplemented."),
            }
        } else {
            // <p>There's no match, so the rest of the source code is in the code block.</p>
            classified_source.push(CodeDocBlock {
                indent: "".to_string(),
                delimiter: "".to_string(),
                contents: source_code.to_string(),
            });
            source_code = "";
        }
    }

    classified_source
}

// Rust <a href="https://doc.rust-lang.org/book/ch11-03-test-organization.html">almost mandates</a> putting tests in the same file as the source, which I dislike. Here's a <a href="http://xion.io/post/code/rust-unit-test-placement.html">good discussion</a> of how to place them in another file, for the time when I'm ready to adopt this more sane layout.
#[cfg(test)]
mod tests {
    use super::source_lexer;
    use super::supported_languages::LANGUAGE_LEXER_ARR;

    #[test]
    fn test_1() {
        let r = source_lexer("a = 1\n# Test", &LANGUAGE_LEXER_ARR[4]);
        println!("{:?}", r);
        assert_eq!(1, 2);
    }
}
