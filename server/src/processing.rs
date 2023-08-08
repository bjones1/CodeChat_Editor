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
/// # `processing.rs` -- Transforms source code to its web-editable equivalent and back
///
/// ## Imports
///
/// ### Standard library
/// None.
// ### Third-party
use lazy_static::lazy_static;
use pulldown_cmark::{html, Options, Parser};
use regex::Regex;

// ### Local
use crate::lexer::{source_lexer, CodeDocBlock, DocBlock, LanguageLexersCompiled};
use crate::webserver::{CodeChatForWeb, CodeMirror, FileType, SourceFileMetadata};

/// ## Data structures
/// On save, the process is CodeChatForWeb -> SortaCodeDocBlocks -> Vec\<CodeDocBlocks> -> source code.
///
/// This is like a `CodeDocBlock`, but allows doc blocks with an unspecified
/// delimiter. Code blocks have `delimiter == ""` and `indent == ""`.
type SortaCodeDocBlocks = Vec<(
    // The indent.
    String,
    // The delimiter. If None, the delimiter wasn't specified; this code
    // should select a valid delimiter for the language.
    Option<String>,
    // The contents.
    String,
)>;

// ## Globals
lazy_static! {
    /// Match the lexer directive in a source file.
    static ref LEXER_DIRECTIVE: Regex = Regex::new(r#"CodeChat Editor lexer: (\w+)"#).unwrap();
}

// This function takes in a source file in web-editable format (the `CodeChatForWeb` struct) and transforms it into source code.
pub fn codechat_for_web_to_source(
    // The file to save plus metadata, stored in the `LexedSourceFile`
    codechat_for_web: CodeChatForWeb<'_>,
    // Lexer info, needed to transform the `LexedSourceFile` into source code.
    language_lexers_compiled: &LanguageLexersCompiled<'_>,
) -> Result<String, String> {
    // Given the mode, find the lexer.
    let lexer: &std::sync::Arc<crate::lexer::LanguageLexerCompiled> = match language_lexers_compiled
        .map_mode_to_lexer
        .get(codechat_for_web.metadata.mode.as_str())
    {
        Some(v) => v,
        None => return Err("Invalid mode".to_string()),
    };

    // Convert from `CodeMirror` to a `SortaCodeDocBlocks`.
    let sorta_code_doc_blocks = code_mirror_to_client(&codechat_for_web.source);

    // Turn this back into code and doc blocks by filling in any missing comment
    // delimiters.
    //
    // This line assigns the variable 'inline_comment' with what a inline
    // comment would look like in this file.
    let inline_comment = lexer.language_lexer.inline_comment_delim_arr.first();
    // This line assigns the variable 'block_comment' with what a block comment
    // would look like in this file.
    let block_comment = lexer.language_lexer.block_comment_delim_arr.first();
    // The outcome of the translation: a vector of CodeDocBlock, in which all comment delimiters are now present.
    let mut code_doc_block_vec: Vec<CodeDocBlock> = Vec::new();
    // 'some_empty' is just a string "".
    let some_empty = Some("".to_string());
    // This for loop sorts the data from the site into code blocks and doc
    // blocks.
    for cdb in &sorta_code_doc_blocks {
        // A code block is a defines as an empty indent and an empty delimiter.
        let is_code_block = cdb.0.is_empty() && cdb.1 == some_empty;
        code_doc_block_vec.push(if is_code_block {
            CodeDocBlock::CodeBlock(cdb.2.to_string())
        } else {
            // It's a doc block; translate this from a sorta doc block to a real doc block by filling in the comment delimiter, if it's not provided (e.g. it's `None`).
            CodeDocBlock::DocBlock(DocBlock {
                indent: cdb.0.to_string(),
                // If no delimiter is provided, use an inline comment (if
                // available), then a block comment.
                delimiter: match &cdb.1 {
                    // The delimiter was provided. Simply use that.
                    Some(v) => v.to_string(),
                    // No delimiter was provided -- fill one in.
                    None => {
                        // Pick an inline comment, if this language has one.
                        if let Some(ic) = inline_comment {
                            ic.to_string()
                        // Otherwise, use a block comment.
                        } else if let Some(bc) = block_comment {
                            bc.opening.to_string()
                        // Neither are available. Help!
                        } else {
                            return Err(
                                "Neither inline nor block comments are defined for this language."
                                    .to_string(),
                            );
                        }
                    }
                },
                contents: cdb.2.to_string(),
                // This doesn't matter when converting from edited code back to
                // source code.
                lines: 0,
            })
        });
    }

    // Turn this vec of CodeDocBlocks into a string of source code.
    let mut file_contents = String::new();
    for code_doc_block in code_doc_block_vec {
        match code_doc_block {
            CodeDocBlock::DocBlock(doc_block) => {
                // Append a doc block, adding a space between the opening
                // delimiter and the contents when necessary.
                let mut append_doc_block = |indent: &str, delimiter: &str, contents: &str| {
                    file_contents += indent;
                    file_contents += delimiter;
                    // Add a space between the delimiter and comment body,
                    // unless the comment was a newline or we're at the end of
                    // the file.
                    if contents.is_empty() || contents == "\n" {
                        // Nothing to append in this case.
                    } else {
                        // Put a space between the delimiter and the contents.
                        file_contents += " ";
                    }
                    file_contents += contents;
                };

                let is_inline_delim = lexer
                    .language_lexer
                    .inline_comment_delim_arr
                    .contains(&doc_block.delimiter.as_str());

                // Build a comment based on the type of the delimiter.
                if is_inline_delim {
                    // Split the contents into a series of lines, adding the
                    // inline comment delimiter to each line.
                    for content_line in doc_block.contents.split_inclusive('\n') {
                        append_doc_block(&doc_block.indent, &doc_block.delimiter, content_line);
                    }
                } else {
                    // Determine the closing comment delimiter matching the
                    // provided opening delimiter.
                    let block_comment_closing_delimiter = match lexer
                        .language_lexer
                        .block_comment_delim_arr
                        .iter()
                        .position(|bc| bc.opening == doc_block.delimiter)
                    {
                        Some(index) => lexer.language_lexer.block_comment_delim_arr[index].closing,
                        None => {
                            return Err(format!(
                                "Unknown block comment opening delimiter '{}'.",
                                doc_block.delimiter
                            ))
                        }
                    };
                    // Produce the resulting block comment. They should always
                    // end with a newline.
                    assert!(&doc_block.contents.ends_with('\n'));
                    append_doc_block(
                        &doc_block.indent,
                        &doc_block.delimiter,
                        // Omit the newline, so we can instead put on the
                        // closing delimiter, then the newline.
                        &doc_block.contents[..&doc_block.contents.len() - 1],
                    );
                    file_contents = file_contents + " " + block_comment_closing_delimiter + "\n";
                }
            }
            CodeDocBlock::CodeBlock(contents) =>
            // This is code. Simply append it (by definition, indent and
            // delimiter are empty).
            {
                file_contents += &contents
            }
        }
    }
    Ok(file_contents)
}

/// Translate from CodeMirror to SortaCodeDocBlocks.
fn code_mirror_to_client(code_mirror: &CodeMirror) -> SortaCodeDocBlocks {
    //Declare 3 mutable variables. The CodeDocBlockArray to append all changes to, and a index for the last docblock and current
    let mut code_doc_block_arr: Vec<(String, Option<String>, String)> = Vec::new();
    let mut current_idx: usize = 0;
    let mut last_doc_block_idx: Option<usize> = None;

    // Iterate through Code Mirror Structure
    for (idx, _) in code_mirror.doc.match_indices('\n') {
        if let Some((from, to, indent, delimiter, contents)) = code_mirror
            .doc_blocks
            .iter()
            .find(|(from, to, _, _, _)| *from <= current_idx && *to >= idx)
        {
            // Check if the current line belongs to a doc block
            if let Some(doc_block_idx) = last_doc_block_idx {
                // Merge consecutive doc blocks by appending the contents
                let (_, _, prev_content) = &mut code_doc_block_arr[doc_block_idx];
                *prev_content = format!("{}{}{}{}", prev_content, indent, delimiter, contents);
            } else {
                // Append a new code/doc block to the array
                code_doc_block_arr.push((
                    code_mirror
                        .doc
                        .get(current_idx..*from)
                        .unwrap_or("")
                        .to_string(),
                    Some(indent.to_string()),
                    format!("{}{}", delimiter, contents),
                ));
                last_doc_block_idx = Some(code_doc_block_arr.len() - 1);
            }
            current_idx = *to + 1;
        } else {
            // Else the current line is part of a code block, not a doc block
            code_doc_block_arr.push((
                code_mirror
                    .doc
                    .get(current_idx..idx)
                    .unwrap_or("")
                    .to_string(),
                None,
                "".to_string(),
            ));
            last_doc_block_idx = None;
            current_idx = idx + 1;
        }
    }

    // Handle the remaining part of the document after the last newline
    code_doc_block_arr.push((
        code_mirror.doc.get(current_idx..).unwrap_or("").to_string(),
        None,
        "".to_string(),
    ));

    code_doc_block_arr
}

// Given the contents of a file, classify it and (often) convert it to HTML.
pub fn source_to_codechat_for_web<'a>(
    // The file's contents.
    file_contents: String,
    // The file's extension.
    file_ext: &str,
    // True if this file is a TOC.
    _is_toc: bool,
    // Lexers.
    language_lexers_compiled: &LanguageLexersCompiled<'_>,
) -> Result<FileType<'a>, String> {
    // Determine the lexer to use for this file.
    let ace_mode;
    // First, search for a lexer directive in the file contents.
    let lexer = if let Some(captures) = LEXER_DIRECTIVE.captures(&file_contents) {
        ace_mode = captures[1].to_string();
        match language_lexers_compiled
            .map_mode_to_lexer
            .get(&ace_mode.as_ref())
        {
            Some(v) => v,
            None => return Err(format!("<p>Unknown lexer type {}.</p>", &ace_mode)),
        }
    } else {
        // Otherwise, look up the lexer by the file's extension.
        if let Some(llc) = language_lexers_compiled.map_ext_to_lexer_vec.get(file_ext) {
            llc.first().unwrap()
        } else {
            // The file type is unknown; treat it as plain text.
            return Ok(FileType::Text(file_contents));
        }
    };

    // Lex the code and put it in the `CodeChatForWeb` structure.
    let code_doc_block_arr;
    let codechat_for_web = CodeChatForWeb {
        metadata: SourceFileMetadata {
            mode: lexer.language_lexer.ace_mode.to_string(),
        },
        source: if lexer.language_lexer.ace_mode == "markdown" {
            // Document-only files are easy: just encode the contents.
            CodeMirror {
                doc: markdown_to_html(&file_contents),
                doc_blocks: vec![],
            }
        } else {
            // Create an initially-empty struct; the source code will be translated to this.
            let mut code_mirror = CodeMirror {
                doc: "".to_string(),
                doc_blocks: Vec::new(),
            };

            // Lex the code.
            code_doc_block_arr = source_lexer(&file_contents, lexer);

            // Translate each `CodeDocBlock` to its `CodeMirror` equivalent.
            for code_or_doc_block in code_doc_block_arr {
                match code_or_doc_block {
                    CodeDocBlock::CodeBlock(code_string) => code_mirror.doc.push_str(&code_string),
                    CodeDocBlock::DocBlock(mut doc_block) => {
                        // Create the doc block.
                        let len = code_mirror.doc.len();
                        doc_block.contents = markdown_to_html(&doc_block.contents);
                        code_mirror.doc_blocks.push((
                            // From
                            len,
                            // To. Make this one line short, which allows
                            // CodeMirror to correctly handle inserts at the
                            // first character of the following code block.
                            len + doc_block.lines - 1,
                            std::borrow::Cow::Owned(doc_block.indent.to_string()),
                            std::borrow::Cow::Owned(doc_block.delimiter.to_string()),
                            std::borrow::Cow::Owned(doc_block.contents.to_string()),
                        ));
                        // Append newlines to the document; the doc block will
                        // replace these in the editor. This keeps the line
                        // numbering of non-doc blocks correct.
                        code_mirror.doc.push_str(&"\n".repeat(doc_block.lines));
                    }
                }
            }
            code_mirror
        },
    };

    Ok(FileType::CodeChat(codechat_for_web))
}

// Convert markdown to HTML. (This assumes the Markdown defined in the CommonMark spec.)
fn markdown_to_html(markdown: &str) -> String {
    let mut options = Options::all();
    // Turndown (which converts HTML back to Markdown) doesn't support smart
    // punctuation.
    options.remove(Options::ENABLE_SMART_PUNCTUATION);
    let parser = Parser::new_ext(markdown, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

// ## Tests
//
// As mentioned in the lexer.rs tests, Rust
// [almost mandates](https://doc.rust-lang.org/book/ch11-03-test-organization.html)
// putting tests in the same file as the source. Here's some
// [good information](http://xion.io/post/code/rust-unit-test-placement.html) on
// how to put tests in another file, for future refactoring reference.
#[cfg(test)]

// ### Save Endpoint Testing
mod tests {
    use crate::lexer::compile_lexers;
    use crate::lexer::supported_languages::LANGUAGE_LEXER_ARR;
    use crate::processing::codechat_for_web_to_source;
    use crate::webserver::{CodeChatForWeb, CodeMirror, SourceFileMetadata};
    use std::borrow::Cow;

    // Wrap the common test operations in a function.
    fn run_test<'a>(
        mode: &str,
        doc: &str,
        doc_blocks: Vec<(
            usize,
            usize,
            Cow<'a, String>,
            Cow<'a, String>,
            Cow<'a, String>,
        )>,
    ) -> String {
        let test_source_file = CodeChatForWeb {
            metadata: SourceFileMetadata {
                mode: mode.to_string(),
            },
            source: CodeMirror {
                doc: doc.to_string(),
                doc_blocks,
            },
        };
        let llc = compile_lexers(LANGUAGE_LEXER_ARR);
        let file_contents = codechat_for_web_to_source(test_source_file, &llc).unwrap();
        file_contents
    }

    fn build_doc_block<'a>(
        start: usize,
        end: usize,
        indent: &str,
        delimiter: &str,
        contents: &str,
    ) -> (
        usize,
        usize,
        Cow<'a, String>,
        Cow<'a, String>,
        Cow<'a, String>,
    ) {
        (
            start,
            end,
            Cow::Owned(indent.to_string()),
            Cow::Owned(delimiter.to_string()),
            Cow::Owned(contents.to_string()),
        )
    }

    // ### Python Tests
    #[test]
    fn test_save_endpoint_py() {
        // Pass nothing to the function.
        assert_eq!(run_test("python", "", vec![]), "");

        // Pass text only.
        assert_eq!(run_test("python", "Test", vec![]), "Test");

        // Pass one doc block.
        assert_eq!(
            run_test("python", "\n", vec![build_doc_block(0, 0, "", "#", "Test")],),
            "# Test"
        );

        // Test a doc block with no delimiter provided.
        assert_eq!(
            run_test("python", "\n", vec![build_doc_block(0, 0, "", "", "Test")]),
            "# Test"
        );
    }

    // ### C / C++ Tests
    #[test]
    fn test_save_endpoint_cpp() {
        // Pass text without comment delimiter
        assert_eq!(
            run_test("c_cpp", "\n", vec![build_doc_block(0, 0, "", "", "Test")]),
            "// Test"
        );

        // Pass an inline comment
        assert_eq!(
            run_test("c_cpp", "\n", vec![build_doc_block(0, 0, "", "//", "Test")]),
            "// Test"
        );

        // **Pass a block comment**
        assert_eq!(
            run_test("c_cpp", "\n", vec![build_doc_block(0, 0, "", "/*", "Test")]),
            "// Test"
        );
    }
}
