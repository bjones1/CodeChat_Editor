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
/// # `processing.rs` -- Transform source code to its web-editable equivalent and back
//
// ## Imports
// None.
//
// ### Standard library
// None.
//
// ### Third-party
use lazy_static::lazy_static;
use pulldown_cmark::{html, Options, Parser};
use regex::Regex;

// ### Local
use crate::lexer::{
    source_lexer, CodeDocBlock, DocBlock, LanguageLexerCompiled, LanguageLexersCompiled,
};
use crate::webserver::{CodeChatForWeb, CodeMirror, FileType, SourceFileMetadata};

// ## Data structures
//
// On save, the process is CodeChatForWeb ->
// Vec\<CodeDocBlocks> -> source code.
//
// ## Globals
lazy_static! {
    /// Match the lexer directive in a source file.
    static ref LEXER_DIRECTIVE: Regex = Regex::new(r#"CodeChat Editor lexer: (\w+)"#).unwrap();
}

// ## Transform `CodeChatForWeb` to source code
//
// This function takes in a source file in web-editable format
// (the `CodeChatForWeb` struct) and transforms it into source code.
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
    let code_doc_block_vec = code_mirror_to_code_doc_blocks(&codechat_for_web.source);
    code_doc_block_vec_to_source(code_doc_block_vec, lexer)
}

/// Translate from CodeMirror to CodeDocBlocks.
fn code_mirror_to_code_doc_blocks(code_mirror: &CodeMirror) -> Vec<CodeDocBlock> {
    let doc_blocks = &code_mirror.doc_blocks;
    // A CodeMirror "document" is really source code.
    let code = &code_mirror.doc;
    let mut code_doc_block_arr: Vec<CodeDocBlock> = Vec::new();
    // Keep track of the to index of the previous doc block. Since we haven't processed any doc blocks, start at 0.
    let mut code_index: usize = 0;

    // Walk through each doc block, inserting the previous code block followed by the doc block.
    for codemirror_doc_block in doc_blocks {
        // Append the code block, unless it's empty.
        let code_contents = &code[code_index..codemirror_doc_block.0];
        if !code_contents.is_empty() {
            code_doc_block_arr.push(CodeDocBlock::CodeBlock(code_contents.to_string()))
        }
        // Append the doc block.
        code_doc_block_arr.push(CodeDocBlock::DocBlock(DocBlock {
            indent: codemirror_doc_block.2.to_string(),
            delimiter: codemirror_doc_block.3.to_string(),
            contents: codemirror_doc_block.4.to_string(),
            lines: 0,
        }));
        code_index = codemirror_doc_block.1 + 1;
    }

    // See if there's a code block after the last doc block.
    let code_contents = &code[code_index..];
    if !code_contents.is_empty() {
        code_doc_block_arr.push(CodeDocBlock::CodeBlock(code_contents.to_string()));
    }

    code_doc_block_arr
}

fn code_doc_block_vec_to_source(
    code_doc_block_vec: Vec<CodeDocBlock>,
    lexer: &LanguageLexerCompiled,
) -> Result<String, String> {
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
                    // indent and inline comment delimiter to each line.
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
                                "Unknown comment opening delimiter '{}'.",
                                doc_block.delimiter
                            ))
                        }
                    };
                    // A block comment should always end with a newline.
                    assert!(&doc_block.contents.ends_with('\n'));

                    // Split the contents into a series of lines, adding the
                    // indent to each line.
                    for content_line in doc_block.contents.split_inclusive('\n') {
                        append_doc_block(&doc_block.indent, &doc_block.delimiter, content_line);
                    }

                    // Add the indent and opening delimiter to the first line, plus a space if the line has content (just like the inline comment case).
                    // For body lines, add the indent only if the line has content (that is, it's more than just a newline).
                    // Add the closing delimiter before the newline on the last line. Precede it with a space if the line has content.
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

// ## Transform from source code to `CodeChatForWeb`
//
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

    // Transform the provided file into the `CodeChatForWeb` structure.
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
            // This is a source file.
            //
            // Create an initially-empty struct; the source code will be
            // translated to this.
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

// Convert markdown to HTML. (This assumes the Markdown defined in the
// CommonMark spec.)
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
    use crate::lexer::supported_languages::LANGUAGE_LEXER_ARR;
    use crate::lexer::{compile_lexers, CodeDocBlock, DocBlock};
    use crate::processing::{code_doc_block_vec_to_source, code_mirror_to_code_doc_blocks};
    use crate::webserver::{CodeChatForWeb, CodeMirror, CodeMirrorDocBlocks, SourceFileMetadata};
    use std::borrow::Cow;

    // ### Utilities
    fn build_codechat_for_web<'a>(
        mode: &str,
        doc: &str,
        doc_blocks: CodeMirrorDocBlocks<'a>,
    ) -> CodeChatForWeb<'a> {
        // Wrap the provided parameters in the necessary data structures.
        CodeChatForWeb {
            metadata: SourceFileMetadata {
                mode: mode.to_string(),
            },
            source: CodeMirror {
                doc: doc.to_string(),
                doc_blocks,
            },
        }
    }

    // Provide a way to construct one element of the `CodeMirrorDocBlocks` vector.
    fn build_codemirror_doc_blocks<'a>(
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

    fn build_doc_block(indent: &str, delimiter: &str, contents: &str) -> CodeDocBlock {
        return CodeDocBlock::DocBlock(DocBlock {
            indent: indent.to_string(),
            delimiter: delimiter.to_string(),
            contents: contents.to_string(),
            lines: 0,
        });
    }

    fn build_code_block(contents: &str) -> CodeDocBlock {
        return CodeDocBlock::CodeBlock(contents.to_string());
    }

    fn run_test1<'a>(mode: &str, doc: &str, doc_blocks: CodeMirrorDocBlocks) -> Vec<CodeDocBlock> {
        let codechat_for_web = build_codechat_for_web(mode, doc, doc_blocks);
        code_mirror_to_code_doc_blocks(&codechat_for_web.source)
    }

    // ### Tests for `code_mirror_to_code_doc_blocks`
    #[test]
    fn test_codemirror_to_code_doc_blocks_py() {
        // Pass nothing to the function.
        assert_eq!(run_test1("python", "", vec![]), vec![]);

        // Pass one code block.
        assert_eq!(
            run_test1("python", "Test", vec![]),
            vec![build_code_block("Test")]
        );

        // Pass one doc block.
        assert_eq!(
            run_test1(
                "python",
                "\n",
                vec![build_codemirror_doc_blocks(0, 0, "", "#", "Test")],
            ),
            vec![build_doc_block("", "#", "Test")]
        );

        // A code block then a doc block
        assert_eq!(
            run_test1(
                "python",
                "code\n\n",
                vec![build_codemirror_doc_blocks(5, 5, "", "#", "doc")],
            ),
            vec![build_code_block("code\n"), build_doc_block("", "#", "doc")]
        );

        // A doc block then a code block
        assert_eq!(
            run_test1(
                "python",
                "\ncode\n",
                vec![build_codemirror_doc_blocks(0, 0, "", "#", "doc")],
            ),
            vec![build_doc_block("", "#", "doc"), build_code_block("code\n")]
        );

        // A code block, then a doc block, then another code block
        assert_eq!(
            run_test1(
                "python",
                "\ncode\n\n",
                vec![
                    build_codemirror_doc_blocks(0, 0, "", "#", "doc 1"),
                    build_codemirror_doc_blocks(6, 6, "", "#", "doc 2")
                ],
            ),
            vec![
                build_doc_block("", "#", "doc 1"),
                build_code_block("code\n"),
                build_doc_block("", "#", "doc 2")
            ]
        );
    }

    #[test]
    fn test_codemirror_to_code_doc_blocks_cpp() {
        // Pass an inline comment.
        assert_eq!(
            run_test1(
                "c_cpp",
                "\n",
                vec![build_codemirror_doc_blocks(0, 0, "", "//", "Test")]
            ),
            vec![build_doc_block("", "//", "Test")]
        );

        // Pass a block comment.
        assert_eq!(
            run_test1(
                "c_cpp",
                "\n",
                vec![build_codemirror_doc_blocks(0, 0, "", "/*", "Test")]
            ),
            vec![build_doc_block("", "/*", "Test")]
        );

        // Two back-to-back doc blocks.
        assert_eq!(
            run_test1(
                "c_cpp",
                "\n\n",
                vec![
                    build_codemirror_doc_blocks(0, 0, "", "//", "Test 1"),
                    build_codemirror_doc_blocks(1, 1, "", "/*", "Test 2")
                ]
            ),
            vec![
                build_doc_block("", "//", "Test 1"),
                build_doc_block("", "/*", "Test 2")
            ]
        );
    }

    // ### Tests for `code_doc_block_vec_to_source`
    //
    // A language with just one inline comment delimiter and no block comments.
    #[test]
    fn test_code_doc_blocks_to_source_py() {
        let llc = compile_lexers(LANGUAGE_LEXER_ARR);
        let py_lexer = llc.map_mode_to_lexer.get("python").unwrap();

        // An empty document.
        assert_eq!(code_doc_block_vec_to_source(vec![], py_lexer).unwrap(), "");
        // A one-line comment.
        assert_eq!(
            code_doc_block_vec_to_source(vec![build_doc_block("", "#", "Test")], py_lexer).unwrap(),
            "# Test"
        );
        assert_eq!(
            code_doc_block_vec_to_source(vec![build_doc_block("", "#", "Test\n")], py_lexer)
                .unwrap(),
            "# Test\n"
        );
        // Check empty doc block lines and multiple lines.
        assert_eq!(
            code_doc_block_vec_to_source(
                vec![build_doc_block("", "#", "Test 1\n\nTest 2")],
                py_lexer
            )
            .unwrap(),
            "# Test 1\n#\n# Test 2"
        );

        // Repeat the above tests with an indent.
        assert_eq!(
            code_doc_block_vec_to_source(vec![build_doc_block(" ", "#", "Test")], py_lexer)
                .unwrap(),
            " # Test"
        );
        assert_eq!(
            code_doc_block_vec_to_source(vec![build_doc_block("  ", "#", "Test\n")], py_lexer)
                .unwrap(),
            "  # Test\n"
        );
        assert_eq!(
            code_doc_block_vec_to_source(
                vec![build_doc_block("   ", "#", "Test 1\n\nTest 2")],
                py_lexer
            )
            .unwrap(),
            "   # Test 1\n   #\n   # Test 2"
        );

        // Basic code.
        assert_eq!(
            code_doc_block_vec_to_source(vec![build_code_block("Test")], py_lexer).unwrap(),
            "Test"
        );

        // An incorrect delimiter.
        assert_eq!(
            code_doc_block_vec_to_source(vec![build_doc_block("", "?", "Test")], py_lexer)
                .unwrap_err(),
            "Unknown comment opening delimiter '?'."
        );
    }

    // A language with just one block comment delimiter and no inline comment delimiters.
    #[test]
    fn test_code_doc_blocks_to_source_css() {
        let llc = compile_lexers(LANGUAGE_LEXER_ARR);
        let css_lexer = llc.map_mode_to_lexer.get("css").unwrap();

        // An empty document.
        assert_eq!(code_doc_block_vec_to_source(vec![], css_lexer).unwrap(), "");
        // A one-line comment.
        assert_eq!(
            code_doc_block_vec_to_source(vec![build_doc_block("", "/*", "Test\n")], css_lexer)
                .unwrap(),
            "/* Test */\n"
        );
        // Check empty doc block lines and multiple lines.
        assert_eq!(
            code_doc_block_vec_to_source(
                vec![build_doc_block("", "/*", "Test 1\n\nTest 2\n")],
                css_lexer
            )
            .unwrap(),
            "/* Test 1\n\nTest 2 */\n"
        );

        // Repeat the above tests with an indent.
        assert_eq!(
            code_doc_block_vec_to_source(vec![build_doc_block("  ", "/*", "Test\n")], css_lexer)
                .unwrap(),
            "  /* Test */\n"
        );
        assert_eq!(
            code_doc_block_vec_to_source(
                vec![build_doc_block("   ", "/*", "Test 1\n\nTest 2\n")],
                css_lexer
            )
            .unwrap(),
            "   /* Test 1\n\n   Test 2 */\n"
        );

        // Basic code.
        assert_eq!(
            code_doc_block_vec_to_source(vec![build_code_block("Test")], css_lexer).unwrap(),
            "Test"
        );

        // An incorrect delimiter.
        assert_eq!(
            code_doc_block_vec_to_source(vec![build_doc_block("", "?", "Test")], css_lexer)
                .unwrap_err(),
            "Unknown comment opening delimiter '?'."
        );
    }
}
