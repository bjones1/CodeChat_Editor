// The CodeChat Editor, a word processor for software developers.<br>Copyright
// (C) 2024-2026 Bryan A. Jones.
//
// This file is part of the CodeChat Editor. The CodeChat Editor is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, either
// version 3 of the License, or (at your option) any later version.
//
// The CodeChat Editor is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along with
// the CodeChat Editor. If not, see
// [http://www.gnu.org/licenses](http://www.gnu.org/licenses).
//
// `test.rs` - Tests for `processing.rs`
// =====================================
//
// Imports
// -------
//
// ### Standard library
use std::{
    io,
    path::{Path, PathBuf},
    rc::Rc,
    str::FromStr,
    sync::{Arc, Mutex},
};

// ### Third-party
use indoc::{formatdoc, indoc};
use markup5ever_rcdom::{Node, NodeData};
use predicates::prelude::predicate::str;
use pretty_assertions::assert_eq;
use regex::Regex;

// ### Local
use super::{
    AMMONIA_OPTIONS, CLEAN_FRAGMENT_HTML, CLEAN_LINK_CONTENT_HTML, CodeChatForWeb, CodeMirror,
    CodeMirrorDocBlock, SourceFileMetadata, StringDiff, find_path_to_toc,
};
use crate::{
    lexer::{
        CodeDocBlock, DocBlock, compile_lexers,
        supported_languages::{MARKDOWN_MODE, get_language_lexer_vec},
    },
    processing::{
        CodeDocBlockVecToSourceError, CodeMirrorDiffable, CodeMirrorDocBlockDelete,
        CodeMirrorDocBlockTransaction, CodeMirrorDocBlockUpdate, CodechatForWebToSourceError,
        HtmlToMarkdownWrapped, SourceToCodeChatForWebError, UNICODE_CURSOR_MARKER, byte_index_of,
        cache::{Cache, FileStatus},
        code_doc_block_vec_to_source, code_mirror_to_code_doc_blocks, codechat_for_web_to_source,
        dehydrating_walk_node, diff_code_mirror_doc_blocks, diff_str, doc_block_html_to_markdown,
        html_to_dom, hydrate_html, is_css_identifier, markdown_to_html, new_html_to_markdown,
        source_to_codechat_for_web,
    },
};
use test_utils::{cast, prep_test_dir, test_utils::stringit};

// Utilities
// ---------
fn build_codechat_for_web(
    mode: &str,
    doc: &str,
    doc_blocks: Vec<CodeMirrorDocBlock>,
) -> CodeChatForWeb {
    // Wrap the provided parameters in the necessary data structures.
    CodeChatForWeb {
        metadata: SourceFileMetadata {
            mode: mode.to_string(),
        },
        source: CodeMirrorDiffable::Plain(CodeMirror {
            doc: doc.to_string(),
            doc_blocks,
        }),
        version: 0.0,
    }
}

// Provide a way to construct one element of the `CodeMirrorDocBlocks` vector.
fn build_codemirror_doc_block(
    start: usize,
    end: usize,
    indent: &str,
    delimiter: &str,
    contents: &str,
) -> CodeMirrorDocBlock {
    CodeMirrorDocBlock {
        from: start,
        to: end,
        indent: indent.to_string(),
        delimiter: delimiter.to_string(),
        contents: contents.to_string(),
    }
}

fn build_doc_block(indent: &str, delimiter: &str, contents: &str) -> CodeDocBlock {
    CodeDocBlock::DocBlock(DocBlock {
        indent: indent.to_string(),
        delimiter: delimiter.to_string(),
        contents: contents.to_string(),
        lines: 0,
    })
}

fn build_code_block(contents: &str) -> CodeDocBlock {
    CodeDocBlock::CodeBlock(contents.to_string())
}

fn run_test(mode: &str, doc: &str, doc_blocks: Vec<CodeMirrorDocBlock>) -> Vec<CodeDocBlock> {
    let codechat_for_web = build_codechat_for_web(mode, doc, doc_blocks);
    let CodeMirrorDiffable::Plain(code_mirror) = codechat_for_web.source else {
        panic!("No diff!");
    };
    code_mirror_to_code_doc_blocks(&code_mirror)
}

// ### Tests for `codechat_for_web_to_source`
//
// `code_mirror_to_code_doc_blocks`, `doc_block_html_to_markdown`, and
// `code_doc_block_vec_to_source`, the stages it invokes, have their own sets of
// tests. These tests therefore focus on what it adds: reporting errors and
// supplying the newline which ends a doc block.
#[test]
fn test_codechat_for_web_to_source() {
    let codechat_for_web = build_codechat_for_web("python", "", vec![]);
    assert_eq!(
        cast!(codechat_for_web_to_source(&codechat_for_web), Ok),
        String::new()
    );

    let codechat_for_web = build_codechat_for_web("undefined", "", vec![]);
    matches!(
        cast!(codechat_for_web_to_source(&codechat_for_web), Err),
        CodechatForWebToSourceError::InvalidLexer(_)
    );
}

// Verify that saving a doc block keeps the newline which ends it; without this
// newline, the next line of the file joins the comment. Word wrapping supplies
// this newline for only some of these doc blocks. Only a comment with no text,
// ending a file which has no final newline, lacks one.
#[test]
fn test_codechat_for_web_to_source_empty_doc_block() {
    for (source, saved) in [
        // A bare delimiter.
        ("//\nlet a = 1;\n", "//\nlet a = 1;\n"),
        // A closing tag with no matching opening tag.
        ("// </div>\nlet a = 1;\n", "//\nlet a = 1;\n"),
        // A link reference definition; saving inlines its URL into the link.
        (
            "// [Link][1]\nlet a = 1;\n// [1]: http://foo.org\nlet b = 2;\n",
            "// [Link](http://foo.org)\nlet a = 1;\n//\nlet b = 2;\n",
        ),
        // A doc block with a different indent, instead of code.
        (
            "//\n  // Next.\nlet a = 1;\n",
            "//\n  // Next.\nlet a = 1;\n",
        ),
        // The end of a file which has no final newline. A comment with no text
        // occupies no lines of the document, so saving adds no newline. The
        // Unicode character verifies that the document's length is measured in
        // UTF-16 code units.
        ("let a = 1;\n//", "let a = 1;\n//"),
        ("//", "//"),
        ("let σ = 1;\n//", "let σ = 1;\n//"),
        ("let a = 1;\n// ", "let a = 1;\n//"),
        ("let a = 1;\n/* */", "let a = 1;\n/* */"),
        // The end of a file which has a final newline.
        ("let a = 1;\n//\n", "let a = 1;\n//\n"),
        ("//\n", "//\n"),
        // A block comment, which must survive with its delimiters.
        ("/* </div> */\nlet a = 1;\n", "/* */\nlet a = 1;\n"),
        ("let a = 1;\n/* </div> */\n", "let a = 1;\n/* */\n"),
        // A comment with text occupies one line of the document, even if the
        // file has no final newline. Saving therefore adds this newline,
        // whether or not its Markdown is empty.
        ("let a = 1;\n/* </div> */", "let a = 1;\n/* */\n"),
        ("let a = 1;\n// </div>", "let a = 1;\n//\n"),
        ("let a = 1;\n// Test", "let a = 1;\n// Test\n"),
        // Word wrapping returns these unchanged, with no final newline: this
        // directive makes it skip the text, and zero-width characters give its
        // output no printed width.
        (
            "// <!-- dprint-ignore-file -->\nlet a = 1;\n",
            "// <!-- dprint-ignore-file -->\nlet a = 1;\n",
        ),
        ("// \u{200b}\nlet a = 1;\n", "// \u{200b}\nlet a = 1;\n"),
        (
            "/* \u{200b} */\nlet a = 1;\n",
            "/* \u{200b} */\nlet a = 1;\n",
        ),
        ("// \u{200b}\n", "// \u{200b}\n"),
        ("let a = 1;\n// \u{200b}", "let a = 1;\n// \u{200b}\n"),
    ] {
        let translation =
            source_to_codechat_for_web(source, Path::new("foo.js"), 0.0, false, None).unwrap();
        println!("{translation:#?}");
        assert_eq!(codechat_for_web_to_source(&translation).unwrap(), saved);
    }
}

// Check each stage of saving a bare delimiter followed by code. The stages
// don't supply the newline which ends this doc block;
// `codechat_for_web_to_source` does this between stages 2 and 3.
#[test]
fn test_codechat_for_web_to_source_empty_doc_block_stages() {
    let llc = compile_lexers(get_language_lexer_vec());
    let js_lexer = llc.map_mode_to_lexer.get(&stringit("javascript")).unwrap();

    // Stage 1, `code_mirror_to_code_doc_blocks`: the doc block spans the newline
    // in the CodeMirror document, so only that document, not the doc block's
    // contents, holds this newline.
    let code_doc_block_vec_html = run_test(
        "javascript",
        "\nlet a = 1;\n",
        vec![build_codemirror_doc_block(0, 1, "", "//", "")],
    );
    assert_eq!(
        code_doc_block_vec_html,
        vec![
            build_doc_block("", "//", ""),
            build_code_block("let a = 1;\n")
        ]
    );

    // Stage 2, `doc_block_html_to_markdown`: empty HTML produces empty
    // Markdown.
    assert_eq!(
        doc_block_html_to_markdown(code_doc_block_vec_html, None).unwrap(),
        vec![
            build_doc_block("", "//", ""),
            build_code_block("let a = 1;\n")
        ]
    );

    // Stage 3, `code_doc_block_vec_to_source`: a doc block containing only a
    // newline saves correctly.
    assert_eq!(
        code_doc_block_vec_to_source(
            &[
                build_doc_block("", "//", "\n"),
                build_code_block("let a = 1;\n")
            ],
            js_lexer
        )
        .unwrap(),
        "//\nlet a = 1;\n"
    );

    // All stages together, with a newline in the document and without one.
    assert_eq!(
        codechat_for_web_to_source(&build_codechat_for_web(
            "javascript",
            "\nlet a = 1;\n",
            vec![build_codemirror_doc_block(0, 1, "", "//", "")],
        ))
        .unwrap(),
        "//\nlet a = 1;\n"
    );
    assert_eq!(
        codechat_for_web_to_source(&build_codechat_for_web(
            "javascript",
            "",
            vec![build_codemirror_doc_block(0, 1, "", "//", "")],
        ))
        .unwrap(),
        "//"
    );
}

// ### Tests for `code_mirror_to_code_doc_blocks`
#[test]
fn test_codemirror_to_code_doc_blocks_py() {
    // Pass nothing to the function.
    assert_eq!(run_test("python", "", vec![]), vec![]);

    // Pass one code block.
    assert_eq!(
        run_test("python", "Test", vec![]),
        vec![build_code_block("Test")]
    );

    // Pass one doc block.
    assert_eq!(
        run_test(
            "python",
            "\n",
            vec![build_codemirror_doc_block(0, 1, "", "#", "Test")],
        ),
        vec![build_doc_block("", "#", "Test")]
    );

    // Pass a code and doc block containing Unicode.
    assert_eq!(
        run_test(
            "python",
            "σ\n",
            vec![build_codemirror_doc_block(1, 2, "", "#", "⑤")],
        ),
        vec![build_code_block("σ"), build_doc_block("", "#", "⑤")]
    );

    // Pass one doc block containing Unicode composed of two UTF-16 code units.
    assert_eq!(
        run_test(
            "python",
            "😄\n",
            vec![build_codemirror_doc_block(2, 3, "", "#", "👨‍👦")],
        ),
        vec![build_code_block("😄"), build_doc_block("", "#", "👨‍👦")]
    );

    // A code block then a doc block
    assert_eq!(
        run_test(
            "python",
            "code\n\n",
            vec![build_codemirror_doc_block(5, 6, "", "#", "doc")],
        ),
        vec![build_code_block("code\n"), build_doc_block("", "#", "doc")]
    );

    // A doc block then a code block
    assert_eq!(
        run_test(
            "python",
            "\ncode\n",
            vec![build_codemirror_doc_block(0, 1, "", "#", "doc")],
        ),
        vec![build_doc_block("", "#", "doc"), build_code_block("code\n")]
    );

    // A code block, then a doc block, then another code block
    assert_eq!(
        run_test(
            "python",
            "\ncode\n\n",
            vec![
                build_codemirror_doc_block(0, 1, "", "#", "doc 1"),
                build_codemirror_doc_block(6, 7, "", "#", "doc 2")
            ],
        ),
        vec![
            build_doc_block("", "#", "doc 1"),
            build_code_block("code\n"),
            build_doc_block("", "#", "doc 2")
        ]
    );

    // Empty doc blocks separated by an empty code block
    assert_eq!(
        run_test(
            "python",
            "\n\n\n",
            vec![
                build_codemirror_doc_block(0, 1, "", "#", ""),
                build_codemirror_doc_block(2, 3, "", "#", "")
            ],
        ),
        vec![
            build_doc_block("", "#", ""),
            build_code_block("\n"),
            build_doc_block("", "#", "")
        ]
    );

    // Error -- instead of newlines, doc blocks replace something else.
}

#[test]
#[should_panic(expected = "assertion `left == right` failed")]
fn test_codemirror_to_code_doc_blocks_error() {
    run_test(
        "python",
        "a\n\n",
        vec![
            build_codemirror_doc_block(0, 1, "", "#", ""),
            build_codemirror_doc_block(2, 3, "", "#", ""),
        ],
    );
}

#[test]
fn test_codemirror_to_code_doc_blocks_cpp() {
    // Pass an inline comment.
    assert_eq!(
        run_test(
            "cpp",
            "\n",
            vec![build_codemirror_doc_block(0, 1, "", "//", "Test")]
        ),
        vec![build_doc_block("", "//", "Test")]
    );

    // Pass a block comment.
    assert_eq!(
        run_test(
            "cpp",
            "\n",
            vec![build_codemirror_doc_block(0, 1, "", "/*", "Test")]
        ),
        vec![build_doc_block("", "/*", "Test")]
    );

    // Two back-to-back doc blocks.
    assert_eq!(
        run_test(
            "cpp",
            "\n\n",
            vec![
                build_codemirror_doc_block(0, 1, "", "//", "Test 1"),
                build_codemirror_doc_block(1, 2, "", "/*", "Test 2")
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
    let llc = compile_lexers(get_language_lexer_vec());
    let py_lexer = llc.map_mode_to_lexer.get(&stringit("python")).unwrap();

    // An empty document.
    assert_eq!(code_doc_block_vec_to_source(&[], py_lexer).unwrap(), "");
    // A one-line comment.
    assert_eq!(
        code_doc_block_vec_to_source(&[build_doc_block("", "#", "Test")], py_lexer).unwrap(),
        "# Test"
    );
    assert_eq!(
        code_doc_block_vec_to_source(&[build_doc_block("", "#", "Test\n")], py_lexer).unwrap(),
        "# Test\n"
    );
    // Check empty doc block lines and multiple lines.
    assert_eq!(
        code_doc_block_vec_to_source(&[build_doc_block("", "#", "Test 1\n\nTest 2")], py_lexer)
            .unwrap(),
        "# Test 1\n#\n# Test 2"
    );

    // Repeat the above tests with an indent.
    assert_eq!(
        code_doc_block_vec_to_source(&[build_doc_block(" ", "#", "Test")], py_lexer).unwrap(),
        " # Test"
    );
    assert_eq!(
        code_doc_block_vec_to_source(&[build_doc_block("  ", "#", "Test\n")], py_lexer).unwrap(),
        "  # Test\n"
    );
    assert_eq!(
        code_doc_block_vec_to_source(&[build_doc_block("   ", "#", "Test 1\n\nTest 2")], py_lexer)
            .unwrap(),
        "   # Test 1\n   #\n   # Test 2"
    );

    // Basic code.
    assert_eq!(
        code_doc_block_vec_to_source(&[build_code_block("Test")], py_lexer).unwrap(),
        "Test"
    );

    // An incorrect delimiter.
    assert_eq!(
        code_doc_block_vec_to_source(&[build_doc_block("", "?", "Test")], py_lexer),
        Err(CodeDocBlockVecToSourceError::UnknownCommentOpeningDelimiter("?".to_string()))
    );

    // Empty doc blocks separated by an empty code block.
    assert_eq!(
        code_doc_block_vec_to_source(
            &[
                build_doc_block("", "#", "\n"),
                build_code_block("\n"),
                build_doc_block("", "#", "")
            ],
            py_lexer
        )
        .unwrap(),
        "#\n\n#"
    );

    assert_eq!(
        code_doc_block_vec_to_source(
            &[
                build_doc_block("", "#", "σ\n"),
                build_code_block("σ\n"),
                build_doc_block("", "#", "σ")
            ],
            py_lexer
        )
        .unwrap(),
        "# σ\nσ\n# σ"
    );
}

// A language with just one block comment delimiter and no inline comment
// delimiters.
#[test]
fn test_code_doc_blocks_to_source_css() {
    let llc = compile_lexers(get_language_lexer_vec());
    let css_lexer = llc.map_mode_to_lexer.get(&stringit("css")).unwrap();

    // An empty document.
    assert_eq!(code_doc_block_vec_to_source(&[], css_lexer).unwrap(), "");
    // A one-line comment.
    assert_eq!(
        code_doc_block_vec_to_source(&[build_doc_block("", "/*", "Test\n")], css_lexer).unwrap(),
        "/* Test */\n"
    );
    assert_eq!(
        code_doc_block_vec_to_source(&[build_doc_block("", "/*", "Test")], css_lexer).unwrap(),
        "/* Test */"
    );
    // Check empty doc block lines and multiple lines.
    assert_eq!(
        code_doc_block_vec_to_source(
            &[
                build_code_block("Test_0\n"),
                build_doc_block("", "/*", "Test 1\n\nTest 2\n")
            ],
            css_lexer
        )
        .unwrap(),
        r"Test_0
/* Test 1

   Test 2 */
"
    );

    // Repeat the above tests with an indent.
    assert_eq!(
        code_doc_block_vec_to_source(&[build_doc_block("  ", "/*", "Test\n")], css_lexer).unwrap(),
        "  /* Test */\n"
    );
    assert_eq!(
        code_doc_block_vec_to_source(
            &[
                build_code_block("Test_0\n"),
                build_doc_block("   ", "/*", "Test 1\n\nTest 2\n")
            ],
            css_lexer
        )
        .unwrap(),
        r"Test_0
   /* Test 1

      Test 2 */
"
    );

    // Basic code.
    assert_eq!(
        code_doc_block_vec_to_source(&[build_code_block("Test")], css_lexer).unwrap(),
        "Test"
    );

    // An incorrect delimiter.
    assert_eq!(
        code_doc_block_vec_to_source(&[build_doc_block("", "?", "Test")], css_lexer),
        Err(CodeDocBlockVecToSourceError::UnknownCommentOpeningDelimiter("?".to_string()))
    );
}

// A language with multiple inline and block comment styles.
#[test]
fn test_code_doc_blocks_to_source_csharp() {
    let llc = compile_lexers(get_language_lexer_vec());
    let csharp_lexer = llc.map_mode_to_lexer.get(&stringit("csharp")).unwrap();

    // An empty document.
    assert_eq!(code_doc_block_vec_to_source(&[], csharp_lexer).unwrap(), "");

    // An invalid comment.
    assert_eq!(
        code_doc_block_vec_to_source(&[build_doc_block("", "?", "Test\n")], csharp_lexer),
        Err(CodeDocBlockVecToSourceError::UnknownCommentOpeningDelimiter("?".to_string()))
    );

    // Inline comments.
    assert_eq!(
        code_doc_block_vec_to_source(&[build_doc_block("", "//", "Test\n")], csharp_lexer).unwrap(),
        "// Test\n"
    );
    assert_eq!(
        code_doc_block_vec_to_source(&[build_doc_block("", "///", "Test\n")], csharp_lexer)
            .unwrap(),
        "/// Test\n"
    );

    // Block comments.
    assert_eq!(
        code_doc_block_vec_to_source(&[build_doc_block("", "/*", "Test\n")], csharp_lexer).unwrap(),
        "/* Test */\n"
    );
    assert_eq!(
        code_doc_block_vec_to_source(&[build_doc_block("", "/**", "Test\n")], csharp_lexer)
            .unwrap(),
        "/** Test */\n"
    );
}

// ### Tests for `source_to_codechat_for_web`
#[test]
fn test_source_to_codechat_for_web_1() {
    // A file with an unknown extension and no lexer, which is classified as a
    // text file.
    assert_eq!(
        source_to_codechat_for_web("", Path::new("foo.xxx"), 0.0, false, None),
        Err(SourceToCodeChatForWebError::NoLexer)
    );

    // A file with an invalid lexer specification. Obscure this, so that this
    // file can be successfully lexed by the CodeChat editor.
    let lexer_spec = format!("{}{}", "CodeChat Editor ", "lexer: ");
    assert_eq!(
        source_to_codechat_for_web(
            &format!("{lexer_spec}unknown"),
            Path::new("foo.xxx"),
            0.0,
            false,
            None
        ),
        Err(SourceToCodeChatForWebError::UnknownLexer(
            "unknown".to_string()
        ))
    );

    // A CodeChat Editor document via filename.
    assert_eq!(
        source_to_codechat_for_web("", Path::new("foo.md"), 0.0, false, None),
        Ok(build_codechat_for_web(MARKDOWN_MODE, "", vec![]))
    );

    // A CodeChat Editor document via lexer specification.
    assert_eq!(
        source_to_codechat_for_web(
            &format!("{lexer_spec}markdown"),
            Path::new("foo.xxx"),
            0.0,
            false,
            None
        ),
        Ok(build_codechat_for_web(
            MARKDOWN_MODE,
            &format!("<p>{lexer_spec}markdown"),
            vec![]
        ))
    );

    // An empty source file.
    assert_eq!(
        source_to_codechat_for_web("", Path::new("foo.js"), 0.0, false, None),
        Ok(build_codechat_for_web("javascript", "", vec![]))
    );

    // A zero doc block source file.
    assert_eq!(
        source_to_codechat_for_web("let a = 1;", Path::new("foo.js"), 0.0, false, None),
        Ok(build_codechat_for_web("javascript", "let a = 1;", vec![]))
    );

    // One doc block source files.
    assert_eq!(
        source_to_codechat_for_web("// Test", Path::new("foo.js"), 0.0, false, None),
        Ok(build_codechat_for_web(
            "javascript",
            "\n",
            vec![build_codemirror_doc_block(0, 1, "", "//", "<p>Test")]
        ))
    );
    assert_eq!(
        source_to_codechat_for_web("let a = 1;\n// Test", Path::new("foo.js"), 0.0, false, None),
        Ok(build_codechat_for_web(
            "javascript",
            "let a = 1;\n\n",
            vec![build_codemirror_doc_block(11, 12, "", "//", "<p>Test")]
        ))
    );
    assert_eq!(
        source_to_codechat_for_web("// Test\nlet a = 1;", Path::new("foo.js"), 0.0, false, None),
        Ok(build_codechat_for_web(
            "javascript",
            "\nlet a = 1;",
            vec![build_codemirror_doc_block(0, 1, "", "//", "<p>Test")]
        ))
    );

    // A two doc block source file. This also tests references in one block to a
    // target in another block.
    assert_eq!(
        source_to_codechat_for_web(
            "// [Link][1]\nlet a = 1;\n/* [1]: http://b.org */",
            Path::new("foo.js"),
            0.0,
            false,
            None
        ),
        Ok(build_codechat_for_web(
            "javascript",
            "\nlet a = 1;\n\n",
            vec![
                build_codemirror_doc_block(0, 1, "", "//", "<p><a href=http://b.org>Link</a>"),
                build_codemirror_doc_block(12, 13, "", "/*", "")
            ]
        ))
    );

    // Trigger special cases:
    //
    // * An empty doc block at the beginning of the file.
    // * A doc block in the middle of the file
    // * A doc block with no trailing newline at the end of the file.
    assert_eq!(
        source_to_codechat_for_web("//\n\n//\n\n//", Path::new("foo.cpp"), 0.0, false, None),
        Ok(build_codechat_for_web(
            "cpp",
            "\n\n\n\n",
            vec![
                build_codemirror_doc_block(0, 1, "", "//", ""),
                build_codemirror_doc_block(2, 3, "", "//", ""),
                build_codemirror_doc_block(4, 5, "", "//", "")
            ]
        ))
    );
    assert_eq!(
        source_to_codechat_for_web("// ~~~\n\n//\n\n//", Path::new("foo.cpp"), 0.0, false, None),
        Ok(build_codechat_for_web(
            "cpp",
            "\n\n\n\n",
            vec![
                build_codemirror_doc_block(0, 1, "", "//", "<pre><code>\n</code></pre>"),
                build_codemirror_doc_block(2, 3, "", "//", ""),
                build_codemirror_doc_block(4, 5, "", "//", "")
            ]
        ))
    );

    // Test Unicode characters and multi-byte Unicode characters in code.
    //
    // ```
    //       \u03c3  \ud83d \ude04 \ud83d \udc49 \ud83c \udfff \ud83d \udc68 \u200d \ud83d \udc66 \ud83c \uddfa \ud83c \uddf3
    // index:   0       1      2      3      4      5      6      7      8      9      10     11     12     13     14     15
    // char:  --σ--     ---😄---      ----------👉🏿---------      --------------👨‍👦--------------     -----------🇺🇳----------
    // ```
    //
    // These are taken from the
    // [MDN UTF-16 docs](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/String#utf-16_characters_unicode_code_points_and_grapheme_clusters).
    assert_eq!(
        source_to_codechat_for_web("; // σ😄👉🏿👨‍👦🇺🇳\n//", Path::new("foo.cpp"), 0.0, false, None),
        Ok(build_codechat_for_web(
            "cpp",
            "; // σ😄👉🏿👨‍👦🇺🇳\n",
            vec![build_codemirror_doc_block(22, 23, "", "//", ""),]
        ))
    );

    // Test Unicode characters and multi-byte Unicode characters in strings.
    assert_eq!(
        source_to_codechat_for_web("\"σ😄👉🏿👨‍👦🇺🇳\";\n//", Path::new("foo.cpp"), 0.0, false, None),
        Ok(build_codechat_for_web(
            "cpp",
            "\"σ😄👉🏿👨‍👦🇺🇳\";\n",
            vec![build_codemirror_doc_block(20, 21, "", "//", ""),]
        ))
    );

    // Test Unicode characters and multi-byte Unicode characters in comments.
    assert_eq!(
        source_to_codechat_for_web("// σ😄👉🏿👨‍👦🇺🇳\n;", Path::new("foo.cpp"), 0.0, false, None),
        Ok(build_codechat_for_web(
            "cpp",
            "\n;",
            vec![build_codemirror_doc_block(0, 1, "", "//", "<p>σ😄👉🏿👨‍👦🇺🇳"),]
        ))
    );

    // Test a fenced code block that's unterminated. See
    // [fence mending](#fence-mending).
    assert_eq!(
        source_to_codechat_for_web(
            "/* ``` foo\n*/\n// Test",
            Path::new("foo.cpp"),
            0.0,
            false,
            None
        ),
        Ok(build_codechat_for_web(
            "cpp",
            "\n\n\n",
            vec![
                build_codemirror_doc_block(
                    0,
                    2,
                    "",
                    "/*",
                    "<pre><code class=language-foo>\n\n</code></pre>"
                ),
                build_codemirror_doc_block(2, 3, "", "//", "<p>Test"),
            ]
        ))
    );
    // Test the other code fence character (the tilde).
    assert_eq!(
        source_to_codechat_for_web(
            "/* ~~~~~~~ foo\n*/\n// Test",
            Path::new("foo.cpp"),
            0.0,
            false,
            None
        ),
        Ok(build_codechat_for_web(
            "cpp",
            "\n\n\n",
            vec![
                build_codemirror_doc_block(
                    0,
                    2,
                    "",
                    "/*",
                    "<pre><code class=language-foo>\n\n</code></pre>"
                ),
                build_codemirror_doc_block(2, 3, "", "//", "<p>Test"),
            ]
        ))
    );
    // Test multiple unterminated fenced code blocks.
    assert_eq!(
        source_to_codechat_for_web("// ```\n // ~~~", Path::new("foo.cpp"), 0.0, false, None),
        Ok(build_codechat_for_web(
            "cpp",
            "\n\n",
            vec![
                build_codemirror_doc_block(0, 1, "", "//", "<pre><code>\n</code></pre>"),
                build_codemirror_doc_block(1, 2, " ", "//", "<pre><code></code></pre>"),
            ]
        ))
    );

    // Test an unterminated HTML block.
    assert_eq!(
        source_to_codechat_for_web(
            "// <strong>\n // Test",
            Path::new("foo.cpp"),
            0.0,
            false,
            None
        ),
        Ok(build_codechat_for_web(
            "cpp",
            "\n\n",
            vec![
                build_codemirror_doc_block(0, 1, "", "//", "<strong> </strong>"),
                build_codemirror_doc_block(1, 2, " ", "//", "<p>Test"),
            ]
        ))
    );

    // Test an unterminated `<pre>` block. Ensure that markdown after this is
    // still parsed. Ammonia closes the unterminated `<pre>`, dropping the
    // trailing newline.
    assert_eq!(
        source_to_codechat_for_web(
            "// <pre>\n // *Test*",
            Path::new("foo.cpp"),
            0.0,
            false,
            None
        ),
        Ok(build_codechat_for_web(
            "cpp",
            "\n\n",
            vec![
                build_codemirror_doc_block(0, 1, "", "//", "<pre></pre>"),
                build_codemirror_doc_block(1, 2, " ", "//", "<p><em>Test</em>"),
            ]
        ))
    );

    // Test that minify functions correctly across multiple paragraphs separated
    // by a code block.
    assert_eq!(
        source_to_codechat_for_web(
            indoc!(
                "
                // One
                //
                // Two
                three();
                // Four
                "
            ),
            Path::new("foo.cpp"),
            0.0,
            false,
            None
        ),
        Ok(build_codechat_for_web(
            "cpp",
            "\n\n\nthree();\n\n",
            vec![
                build_codemirror_doc_block(0, 3, "", "//", "<p>One<p>Two"),
                build_codemirror_doc_block(12, 13, "", "//", "<p>Four"),
            ]
        ))
    );

    // Test that minify functions correctly across multiple paragraphs separated
    // by a code block.
    assert_eq!(
        source_to_codechat_for_web(
            indoc!(
                r#"
                // <a id="one"></a>1
                "#
            ),
            Path::new("foo.cpp"),
            0.0,
            false,
            None
        ),
        Ok(build_codechat_for_web(
            "cpp",
            "\n",
            vec![build_codemirror_doc_block(
                0,
                1,
                "",
                "//",
                r"<p><a id=one></a>1"
            ),]
        ))
    );
}

#[test]
fn test_find_path_to_toc_1() {
    let (temp_dir, test_dir) = prep_test_dir!();

    // Test 1: the TOC is in the same directory as the file.
    let fp = find_path_to_toc(&test_dir.join("1/foo.py"));
    assert_eq!(fp, Some(PathBuf::from_str("toc.md").unwrap()));

    // Test 2: no TOC. (We assume all temp directory parents lack a TOC as
    // well.)
    let fp = find_path_to_toc(&test_dir.join("2/foo.py"));
    assert_eq!(fp, None);

    // Test 3: the TOC is a few levels above the file.
    let fp = find_path_to_toc(&test_dir.join("3/bar/baz/foo.py"));
    assert_eq!(fp, Some(PathBuf::from_str("../../toc.md").unwrap()));

    // Report any errors produced when removing the temporary directory.
    temp_dir.close().unwrap();
}

// Given a diff, apply it to the provided `before` string to produce the
// resulting `after` string.
fn apply_str_diff(before: &str, diffs: &[StringDiff]) -> String {
    let mut before = before.to_string();
    // Walk from the last diff to the first.
    for diff in diffs.iter().rev() {
        // Convert from a character index to a byte index. If the index is past
        // the end of the string, report the length of the string.
        let from_index = byte_index_of(&before, diff.from);
        if let Some(to) = diff.to {
            let to_index = byte_index_of(&before, to);
            before.replace_range(from_index..to_index, &diff.insert);
        } else {
            before.insert_str(from_index, &diff.insert);
        }
    }
    before
}

// Option 1: implement separate JS and Rust. Pro: simple. Con: how to test?
// Duplicate them. But eventually I want to send diffs back, so I'll have to
// implement both sides. Let's do this later. Also, I'm a bit concerned about
// performance -- probably have to translate strings between the two platforms.
// Per https://rustwasm.github.io/wasm-bindgen/reference/types/string.html, this
// means a decode/encode and copy each direction, which is not exciting.
//
// Option 2: implement partly in Rust then use in JS. Pro: easier to test. Con:
// Complex.

#[test]
fn test_diff_1() {
    let test_diff = |before: &str, after: &str, expected_change_spec: &[StringDiff]| {
        let after = after.to_string();
        let diff = diff_str(before, &after);
        let before = apply_str_diff(before, &diff);
        assert_eq!(diff.len(), 1);
        assert_eq!(before, after);
        assert_eq!(diff, expected_change_spec);
    };

    // Insert at beginning.
    test_diff(
        "1\n234\n56",
        "aa\n1\n234\n56",
        &[StringDiff {
            from: 0,
            to: None,
            insert: "aa\n".to_string(),
        }],
    );

    // Replace at beginning.
    test_diff(
        "1\n234\n56",
        "aa\n234\n56",
        &[StringDiff {
            from: 0,
            to: Some(2),
            insert: "aa\n".to_string(),
        }],
    );

    // Delete at beginning.
    test_diff(
        "1\n234\n56",
        "234\n56",
        &[StringDiff {
            from: 0,
            to: Some(2),
            insert: String::new(),
        }],
    );

    // Repeat, but in middle.
    test_diff(
        "1\n234\n56",
        "1\naa\n234\n56",
        &[StringDiff {
            from: 2,
            to: None,
            insert: "aa\n".to_string(),
        }],
    );
    test_diff(
        "1\n234\n56",
        "1\naa\n56",
        &[StringDiff {
            from: 2,
            to: Some(6),
            insert: "aa\n".to_string(),
        }],
    );
    test_diff(
        "1\n234\n56",
        "1\n56",
        &[StringDiff {
            from: 2,
            to: Some(6),
            insert: String::new(),
        }],
    );

    // Repeat, but at end.
    test_diff(
        "1\n234\n56",
        "1\n234\n56\naa",
        &[StringDiff {
            from: 6,
            to: Some(8),
            insert: "56\naa".to_string(),
        }],
    );
    test_diff(
        "1\n234\n56",
        "1\n234\naa",
        &[StringDiff {
            from: 6,
            to: Some(8),
            insert: "aa".to_string(),
        }],
    );
    test_diff(
        "1\n234\n56",
        "1\n234\n",
        &[StringDiff {
            from: 6,
            to: Some(8),
            insert: String::new(),
        }],
    );

    // Test with unicode.
    test_diff(
        // This encodes to the following UTF-16 string:
        //
        // ```
        //       \ud83d \ude04 \u000a \ud83d \udc49 \ud83c \udfff \ud83d \udc68 \u200d \ud83d \udc66 \ud83c \uddfa \ud83c \uddf3 \u000a \u2464 \u2465
        // index:   0      1      2   [  3      4      5      6      7      8      9      10     11     12     13     14     15     16  ]  17     18
        // char:    ---😄---     \n      ----------👉🏿---------      --------------👨‍👦--------------     -----------🇺🇳----------      \n     ⑤      ⑥
        // ```
        //
        // These are taken from the
        // [MDN UTF-16 docs](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/String#utf-16_characters_unicode_code_points_and_grapheme_clusters).
        "😄\n👉🏿👨‍👦🇺🇳\n⑤⑥",
        "😄\n❷❸\n⑤⑥",
        &[StringDiff {
            from: 3,
            to: Some(17),
            insert: "❷❸\n".to_string(),
        }],
    );
}

#[test]
fn test_diff_2() {
    // Test with empty data.
    let before = vec![];
    let after = vec![];
    let ret = diff_code_mirror_doc_blocks(&before, &after);
    assert_eq!(ret, vec![]);

    // Test with identical data.
    let before = vec![build_codemirror_doc_block(0, 1, "", "#", "test")];
    let after = vec![build_codemirror_doc_block(0, 1, "", "#", "test")];
    let ret = diff_code_mirror_doc_blocks(&before, &after);
    assert_eq!(ret, vec![]);

    // Replacement, with various fields.
    let before = vec![build_codemirror_doc_block(10, 11, "", "#", "test")];
    let after = vec![build_codemirror_doc_block(10, 12, "", "#", "test")];
    let ret = diff_code_mirror_doc_blocks(&before, &after);
    assert_eq!(
        ret,
        vec![CodeMirrorDocBlockTransaction::Update(
            CodeMirrorDocBlockUpdate {
                from: 10,
                from_new: None,
                to: Some(12),
                indent: None,
                delimiter: None,
                contents: vec![]
            }
        )]
    );

    let before = vec![build_codemirror_doc_block(10, 11, "", "#", "test")];
    let after = vec![build_codemirror_doc_block(10, 11, " ", "#", "test")];
    let ret = diff_code_mirror_doc_blocks(&before, &after);
    assert_eq!(
        ret,
        vec![CodeMirrorDocBlockTransaction::Update(
            CodeMirrorDocBlockUpdate {
                from: 10,
                from_new: None,
                to: None,
                indent: Some(" ".to_string()),
                delimiter: None,
                contents: vec![]
            }
        )]
    );

    let before = vec![build_codemirror_doc_block(10, 11, "", "#", "test")];
    let after = vec![build_codemirror_doc_block(10, 11, "", "*", "test")];
    let ret = diff_code_mirror_doc_blocks(&before, &after);
    assert_eq!(
        ret,
        vec![CodeMirrorDocBlockTransaction::Update(
            CodeMirrorDocBlockUpdate {
                from: 10,
                from_new: None,
                to: None,
                indent: None,
                delimiter: Some("*".to_string()),
                contents: vec![]
            }
        )]
    );

    let before = vec![build_codemirror_doc_block(10, 11, "", "#", "test\n")];
    let after = vec![build_codemirror_doc_block(10, 11, "", "#", "test\n1")];
    let ret = diff_code_mirror_doc_blocks(&before, &after);
    assert_eq!(
        ret,
        vec![CodeMirrorDocBlockTransaction::Update(
            CodeMirrorDocBlockUpdate {
                from: 10,
                from_new: None,
                to: None,
                indent: None,
                delimiter: None,
                contents: vec![StringDiff {
                    from: 5,
                    to: None,
                    insert: "1".to_string()
                }]
            }
        )]
    );

    // Insert at beginning -- contents changed.
    let before = vec![build_codemirror_doc_block(11, 12, "", "#", "test2")];
    let after = vec![
        build_codemirror_doc_block(10, 11, "", "#", "test1"),
        build_codemirror_doc_block(11, 12, "", "#", "test2"),
    ];
    let ret = diff_code_mirror_doc_blocks(&before, &after);
    assert_eq!(
        ret,
        vec![CodeMirrorDocBlockTransaction::Add(CodeMirrorDocBlock {
            from: 10,
            to: 11,
            indent: String::new(),
            delimiter: "#".to_string(),
            contents: "test1".to_string()
        })]
    );

    // Insert at beginning -- contents unchanged.
    let before = vec![build_codemirror_doc_block(11, 12, "", "#", "test")];
    let after = vec![
        build_codemirror_doc_block(10, 11, "", "#", "test"),
        build_codemirror_doc_block(11, 12, "", "#", "test"),
    ];
    let ret = diff_code_mirror_doc_blocks(&before, &after);
    assert_eq!(
        ret,
        // The "dumb" (non-diff) algorithm see this as a replace followed by an
        // insert, not a single insert.
        vec![
            CodeMirrorDocBlockTransaction::Update(CodeMirrorDocBlockUpdate {
                from: 11,
                from_new: Some(10),
                to: Some(11),
                indent: None,
                delimiter: None,
                contents: vec![]
            }),
            CodeMirrorDocBlockTransaction::Add(CodeMirrorDocBlock {
                from: 11,
                to: 12,
                indent: String::new(),
                delimiter: "#".to_string(),
                contents: "test".to_string()
            }),
        ]
    );

    // Insert in middle.
    let before = vec![
        build_codemirror_doc_block(10, 11, "", "#", "test1"),
        build_codemirror_doc_block(12, 13, "", "#", "test3"),
    ];
    let after = vec![
        build_codemirror_doc_block(10, 11, "", "#", "test1"),
        build_codemirror_doc_block(11, 12, "", "#", "test2"),
        build_codemirror_doc_block(12, 13, "", "#", "test3"),
    ];
    let ret = diff_code_mirror_doc_blocks(&before, &after);
    assert_eq!(
        ret,
        vec![CodeMirrorDocBlockTransaction::Add(CodeMirrorDocBlock {
            from: 11,
            to: 12,
            indent: String::new(),
            delimiter: "#".to_string(),
            contents: "test2".to_string()
        })]
    );
    // Insert at end -- contents changed.
    let before = vec![build_codemirror_doc_block(10, 11, "", "#", "test1")];
    let after = vec![
        build_codemirror_doc_block(10, 11, "", "#", "test1"),
        build_codemirror_doc_block(11, 12, "", "#", "test2"),
    ];
    let ret = diff_code_mirror_doc_blocks(&before, &after);
    assert_eq!(
        ret,
        vec![CodeMirrorDocBlockTransaction::Add(CodeMirrorDocBlock {
            from: 11,
            to: 12,
            indent: String::new(),
            delimiter: "#".to_string(),
            contents: "test2".to_string()
        })]
    );

    // Delete at beginning.
    let before = vec![
        build_codemirror_doc_block(10, 11, "", "#", "test1"),
        build_codemirror_doc_block(11, 12, "", "#", "test2"),
    ];
    let after = vec![build_codemirror_doc_block(11, 12, "", "#", "test2")];
    let ret = diff_code_mirror_doc_blocks(&before, &after);
    assert_eq!(
        ret,
        vec![CodeMirrorDocBlockTransaction::Delete(
            CodeMirrorDocBlockDelete { from: 10 }
        )]
    );

    // Delete in middle.
    let before = vec![
        build_codemirror_doc_block(10, 11, "", "#", "test1"),
        build_codemirror_doc_block(11, 12, "", "#", "test2"),
        build_codemirror_doc_block(12, 13, "", "#", "test3"),
    ];
    let after = vec![
        build_codemirror_doc_block(10, 11, "", "#", "test1"),
        build_codemirror_doc_block(12, 13, "", "#", "test3"),
    ];
    let ret = diff_code_mirror_doc_blocks(&before, &after);
    assert_eq!(
        ret,
        vec![CodeMirrorDocBlockTransaction::Delete(
            CodeMirrorDocBlockDelete { from: 11 }
        )]
    );

    // Delete multiple.
    let before = vec![
        build_codemirror_doc_block(10, 11, "", "#", "test1"),
        build_codemirror_doc_block(11, 12, "", "#", "test2"),
        build_codemirror_doc_block(12, 13, "", "#", "test3"),
    ];
    let after = vec![];
    let ret = diff_code_mirror_doc_blocks(&before, &after);
    assert_eq!(
        ret,
        vec![
            CodeMirrorDocBlockTransaction::Delete(CodeMirrorDocBlockDelete { from: 10 }),
            CodeMirrorDocBlockTransaction::Delete(CodeMirrorDocBlockDelete { from: 11 }),
            CodeMirrorDocBlockTransaction::Delete(CodeMirrorDocBlockDelete { from: 12 }),
        ]
    );

    // Delete at end.
    let before = vec![
        build_codemirror_doc_block(10, 11, "", "#", "test1"),
        build_codemirror_doc_block(11, 12, "", "#", "test2"),
    ];
    let after = vec![build_codemirror_doc_block(10, 11, "", "#", "test1")];
    let ret = diff_code_mirror_doc_blocks(&before, &after);
    assert_eq!(
        ret,
        vec![CodeMirrorDocBlockTransaction::Delete(
            CodeMirrorDocBlockDelete { from: 11 }
        )]
    );

    // Test ordering of inserts, deletes, and updates.
    let before = vec![
        build_codemirror_doc_block(9, 10, "", "#", "test1"),
        build_codemirror_doc_block(10, 11, "", "#", "test2"),
        build_codemirror_doc_block(11, 12, "", "#", "test3"),
        build_codemirror_doc_block(12, 13, "", "#", "test4"),
        build_codemirror_doc_block(22, 23, "", "#", "test5"),
    ];
    let after = vec![
        build_codemirror_doc_block(8, 9, "", "#", "test1"),
        build_codemirror_doc_block(10, 11, "", "#", "test3"),
        build_codemirror_doc_block(13, 14, "", "#", "test4"),
        build_codemirror_doc_block(14, 15, "", "#", "test4a"),
        build_codemirror_doc_block(23, 24, "", "#", "test5"),
    ];
    let ret = diff_code_mirror_doc_blocks(&before, &after);
    assert_eq!(
        ret,
        vec![
            // Order is important! Deletions are ordered beginning to end.
            CodeMirrorDocBlockTransaction::Update(CodeMirrorDocBlockUpdate {
                from: 9,
                from_new: Some(8),
                to: Some(9),
                indent: None,
                delimiter: None,
                contents: vec![]
            }),
            CodeMirrorDocBlockTransaction::Delete(CodeMirrorDocBlockDelete { from: 10 }),
            CodeMirrorDocBlockTransaction::Update(CodeMirrorDocBlockUpdate {
                from: 11,
                from_new: Some(10),
                to: Some(11),
                indent: None,
                delimiter: None,
                contents: vec![]
            }),
            // Insertions are ordered end to beginning.
            CodeMirrorDocBlockTransaction::Update(CodeMirrorDocBlockUpdate {
                from: 22,
                from_new: Some(23),
                to: Some(24),
                indent: None,
                delimiter: None,
                contents: vec![]
            }),
            CodeMirrorDocBlockTransaction::Add(CodeMirrorDocBlock {
                from: 14,
                to: 15,
                indent: String::new(),
                delimiter: "#".to_string(),
                contents: "test4a".to_string()
            }),
            CodeMirrorDocBlockTransaction::Update(CodeMirrorDocBlockUpdate {
                from: 12,
                from_new: Some(13),
                to: Some(14),
                indent: None,
                delimiter: None,
                contents: vec![]
            }),
        ]
    );
}

#[test]
fn test_doc_block_html_to_markdown_1() {
    assert_eq!(
        doc_block_html_to_markdown(
            vec![build_doc_block(
                "",
                "",
                "<p>Index 0</p><p>Index 1.0<b>Index 1.1</b>012345</p>"
            )],
            Some(&(vec![1, 2], 3)),
        )
        .unwrap(),
        vec![build_doc_block(
            "",
            "",
            &formatdoc!(
                "
                Index 0

                Index 1.0**Index 1.1**012{UNICODE_CURSOR_MARKER}345
                "
            )
        )]
    );
}

// Empty block round trips
// -----------------------
//
// Companion to `test_nested_list_creation` in
// [overall_5.rs](../../tests/overall/overall_5.rs), which drives one of these
// cases through the Client with a WebDriver. The test here exercises just the
// two translations involved, without the browser: the HTML TinyMCE builds for a
// newly-created, still-empty block, converted to Markdown, then that Markdown
// converted back to HTML the way the Server's re-translation does.
//
// This test deliberately doesn't pin down the exact Markdown produced, since
// more than one encoding of an empty block is reasonable. It checks only that
// the round trip preserves the document's structure and its text.

// One empty-block case.
struct EmptyBlockCase {
    // The editing action which produces `html`, used in failure messages.
    name: &'static str,
    // The HTML TinyMCE builds for that action. The `data-mce-bogus="1"`
    // attribute TinyMCE puts on the placeholder `<br>` is omitted, since the
    // dehydration performed by `doc_block_html_to_markdown` removes it before
    // the conversion sees it; a plain `<br>` is therefore equivalent here.
    html: &'static str,
}

const EMPTY_BLOCK_CASES: &[EmptyBlockCase] = &[
    // ### Empty list items
    //
    // `End`, `Enter`, `Tab` at the end of a list item: the case
    // `test_nested_list_creation` drives through the browser. Without the
    // dehydration rewrite these lose the nested list, since -- per the
    // [CommonMark spec](https://spec.commonmark.org/0.31.2/#list-items) -- a
    // list may interrupt a paragraph only if its first item is non-empty. The
    // marker emitted for an empty item directly after paragraph text is
    // therefore read as a lazy continuation of that paragraph: the word wrap
    // pass, which re-parses the `*   Item one\n    *` produced by
    // HTML-to-Markdown conversion, writes `* Item one *` to the file.
    EmptyBlockCase {
        name: "empty item nested under an item's text",
        html: "<ul><li>Item one<ul><li><br></li></ul></li><li>Item two</li></ul>",
    },
    EmptyBlockCase {
        name: "empty item nested under an ordered item's text",
        html: "<ol><li>Item one<ol><li><br></li></ol></li></ol>",
    },
    EmptyBlockCase {
        name: "empty item nested two levels deep",
        html: "<ul><li>Item one<ul><li>Item 1a<ul><li><br></li></ul></li></ul></li></ul>",
    },
    // Only the nested list's *first* item must be non-empty for it to interrupt
    // the text above it, so an empty item with a non-empty sibling depends on
    // which of the two comes first.
    EmptyBlockCase {
        name: "empty first item of a nested list",
        html: "<ul><li>Item one<ul><li><br></li><li>Item 1b</li></ul></li></ul>",
    },
    EmptyBlockCase {
        name: "empty last item of a nested list",
        html: "<ul><li>Item one<ul><li>Item 1a</li><li><br></li></ul></li></ul>",
    },
    // The placeholder satisfies only half of the CommonMark rule above: the
    // list's first item must be non-empty, *and* an ordered list must be
    // numbered from 1. A list numbered from anything else is read as a lazy
    // continuation of the text above it no matter what its items contain, so
    // this case needs the other half of the fix -- the blank line
    // `separate_ordered_lists_from_preceding_text` in
    // [processing.rs](../processing.rs) inserts, which stops the list from
    // interrupting a paragraph at all. Reachable by emptying the only item of a
    // nested list which the file numbers from 3.
    //
    /*** TODO: re-enable this when htmd list creation is much smarter.
    EmptyBlockCase {
        name: "empty item nested under an item's text, in a list numbered from 3",
        html: "<ul><li>Item one<ol start=\"3\"><li><br></li></ol></li></ul>",
    },
    // The same rule with nothing empty in the document: the numbering alone
    // costs the list its ability to interrupt a paragraph, so this case depends
    // on that blank line and on nothing else in this test.
    EmptyBlockCase {
        name: "non-empty list numbered from 3 nested under an item's text",
        html: "<ul><li>Item one<ol start=\"3\"><li>Item three</li></ol></li></ul>",
    },
    */
    // No paragraph text precedes the nested list here, so the CommonMark rule
    // above doesn't apply even without the rewrite: the nested marker lands on
    // a line of its own (`*\n  *`), where it starts a list instead of
    // continuing a paragraph.
    EmptyBlockCase {
        name: "empty item nested under an empty item",
        html: "<ul><li><ul><li><br></li></ul></li></ul>",
    },
    // `End`, `Enter` at the end of a list item, without the `Tab`: a sibling
    // item rather than a nested one. The list is already open, so its marker
    // isn't interrupting a paragraph.
    EmptyBlockCase {
        name: "empty item at the end of a list",
        html: "<ul><li>Item one</li><li><br></li></ul>",
    },
    EmptyBlockCase {
        name: "empty item between two items",
        html: "<ul><li>Item one</li><li><br></li><li>Item two</li></ul>",
    },
    EmptyBlockCase {
        name: "empty item at the start of a list",
        html: "<ul><li><br></li><li>Item one</li></ul>",
    },
    EmptyBlockCase {
        name: "empty item at the end of an ordered list",
        html: "<ol><li>Item one</li><li><br></li></ol>",
    },
    EmptyBlockCase {
        name: "empty task list item",
        html: "<ul><li><input disabled type=\"checkbox\">Task</li>\
               <li><input disabled type=\"checkbox\"><br></li></ul>",
    },
    // A second paragraph inside a list item makes the list loose, so the empty
    // block here is a `<p>` -- the paragraph case below -- but inside a
    // container.
    EmptyBlockCase {
        name: "empty paragraph appended to a list item",
        html: "<ul><li><p>Item one</p><p><br></p></li></ul>",
    },
    // ### Empty headings
    //
    // Levels 1 and 2 are written as setext headings, whose `=` or `-` underline
    // needs text above it. Without the rewrite these vanish outright rather
    // than becoming stray text: HTML-to-Markdown conversion emits nothing at
    // all for a heading whose only content is the placeholder `<br>`.
    EmptyBlockCase {
        name: "empty heading",
        html: "<h1><br></h1>",
    },
    EmptyBlockCase {
        name: "empty heading after a paragraph",
        html: "<p>Text</p><h2><br></h2>",
    },
    // An empty setext heading followed by text puts its underline between the
    // placeholder and that text, where a `-` underline could just as well be
    // read as a bullet marker or a thematic break.
    EmptyBlockCase {
        name: "empty heading before a paragraph",
        html: "<h1><br></h1><p>Text</p>",
    },
    EmptyBlockCase {
        name: "empty level 2 heading before a paragraph",
        html: "<h2><br></h2><p>Text</p>",
    },
    // Levels 3 and up are written as ATX headings instead, whose `#` prefix
    // marks an empty heading's place; they pass with or without the rewrite,
    // which lists them anyway. This case is here to keep that true: it fails if
    // a future encoding of an empty heading works for setext headings but not
    // for ATX ones.
    EmptyBlockCase {
        name: "empty headings at levels 3 through 6",
        html: "<h3><br></h3><h4><br></h4><h5><br></h5><h6><br></h6>",
    },
    // ### Empty blocks carrying attributes
    //
    // The rewrite applies only to a block with no attributes. These cases pin
    // down what saves the blocks it therefore skips: with an attribute to
    // preserve, the converter emits the block as raw HTML -- placeholder `<br>`
    // and all -- instead of Markdown, and raw HTML survives the round trip
    // unchanged.
    EmptyBlockCase {
        name: "empty centered paragraph",
        html: "<p style=\"text-align: center;\"><br></p>",
    },
    EmptyBlockCase {
        name: "empty heading with a named anchor's id",
        html: "<h2 id=\"notes\"><br></h2>",
    },
    // ### Empty block quotes
    //
    // TinyMCE wraps block quote contents in a paragraph, so these reach
    // dehydration as a `<p><br></p>`, which the rewrite handles.
    EmptyBlockCase {
        name: "empty block quote",
        html: "<blockquote><p><br></p></blockquote>",
    },
    // A block quote with no paragraph inside doesn't come from the editor; it
    // comes from a file whose Markdown contains a block quote with no content (a
    // lone `>`). The rewrite supplies the paragraph TinyMCE would have, since a
    // placeholder alone can't save this shape: the `>` is emitted once per line
    // of the block quote's content, and a placeholder-only block quote has no
    // content lines.
    EmptyBlockCase {
        name: "empty block quote with no paragraph inside",
        html: "<blockquote><br></blockquote>",
    },
    // Unlike a list, a block quote may interrupt a paragraph even when empty.
    EmptyBlockCase {
        name: "empty block quote after a paragraph",
        html: "<p>Text</p><blockquote><p><br></p></blockquote>",
    },
    EmptyBlockCase {
        name: "empty paragraph appended to a block quote",
        html: "<blockquote><p>Quote</p><p><br></p></blockquote>",
    },
    // ### Empty table cells
    //
    // A GFM table cell may be empty -- its row's pipes hold its place -- so
    // these need no rewrite.
    EmptyBlockCase {
        name: "empty table body cell",
        html: "<table><thead><tr><th>H1</th><th>H2</th></tr></thead>\
               <tbody><tr><td>a</td><td><br></td></tr></tbody></table>",
    },
    EmptyBlockCase {
        name: "empty table header cell",
        html: "<table><thead><tr><th>H1</th><th><br></th></tr></thead>\
               <tbody><tr><td>a</td><td>b</td></tr></tbody></table>",
    },
    EmptyBlockCase {
        name: "empty table row",
        html: "<table><thead><tr><th>H1</th><th>H2</th></tr></thead>\
               <tbody><tr><td>a</td><td>b</td></tr>\
               <tr><td><br></td><td><br></td></tr></tbody></table>",
    },
    // A cell holding a paragraph can't be written as a GFM table at all, since
    // a cell's content is a single line; the converter emits the whole table as
    // raw HTML instead, which survives the round trip unchanged.
    EmptyBlockCase {
        name: "empty paragraph in a table cell",
        html: "<table><thead><tr><th>H1</th></tr></thead>\
               <tbody><tr><td><p><br></p></td></tr></tbody></table>",
    },
    // ### Empty paragraphs
    //
    // The original case the rewrite was written for: a paragraph containing
    // only a `<br>` produces a blank line, which no longer separates anything.
    EmptyBlockCase {
        name: "empty paragraph at the end of a document",
        html: "<p>Text</p><p><br></p>",
    },
    EmptyBlockCase {
        name: "empty paragraph between two paragraphs",
        html: "<p>One</p><p><br></p><p>Two</p>",
    },
    // `Enter` pressed twice. Each placeholder needs a blank line on both sides
    // to stay a paragraph of its own, so consecutive empty paragraphs cost more
    // separators than a single one does.
    EmptyBlockCase {
        name: "two consecutive empty paragraphs",
        html: "<p>One</p><p><br></p><p><br></p><p>Two</p>",
    },
];

// Run one case through the round trip a save performs -- HTML to Markdown, then
// (as the Server's re-translation does) that Markdown back to HTML -- and report
// what the round trip changed, if anything.
fn check_empty_block_round_trip(case: &EmptyBlockCase) -> Option<String> {
    let code_doc_block_vec =
        doc_block_html_to_markdown(vec![build_doc_block("", "", case.html)], None).unwrap();
    let CodeDocBlock::DocBlock(doc_block) = &code_doc_block_vec[0] else {
        panic!(
            "Expected a doc block, but saw {:#?}.",
            code_doc_block_vec[0]
        );
    };
    let markdown = &doc_block.contents;
    let html = markdown_to_html(markdown);
    // Print both stages before reporting, so that a run shows the whole round
    // trip for every case, not just the ones which failed.
    println!(
        "--- {}\nHTML in:\n{}\nMarkdown:\n{markdown}\nHTML out:\n{html}",
        case.name, case.html
    );

    // Compare against the dehydrated input, since dehydration is part of the
    // conversion under test: its `<p><br></p>` rewrite must count as preserving
    // the paragraph, not as changing it.
    let before = structure_and_text(&dehydrate_html(case.html).unwrap());
    let after = structure_and_text(&html_to_dom(&html, None).unwrap());
    if before == after {
        return None;
    }
    Some(format!(
        "The round trip changed the document's structure or text.\n\
         Before:   {}\nAfter:    {}\nHTML in:  {}\nMarkdown: {markdown:?}\nHTML out: {html}",
        before.join(" "),
        after.join(" "),
        case.html
    ))
}

// A description of the document rooted at `node` which the round trip must
// preserve: the name and nesting depth of each element, and each run of text, in
// document order.
//
// Two things are deliberately left out. Whitespace, so that differences in word
// wrapping and indentation -- and the non-breaking space an empty block may be
// encoded as -- don't count as changes; whitespace runs within a text node are
// collapsed rather than removed, so that two words merging into one still does.
// And `<br>` elements, since an empty block is legitimately encoded some other
// way, as the dehydration rewrite does by replacing the `<br>` with that
// non-breaking space.
fn structure_and_text(node: &Rc<Node>) -> Vec<String> {
    fn walk(node: &Rc<Node>, depth: usize, description: &mut Vec<String>) {
        match &node.data {
            NodeData::Element { name, .. } => {
                if &*name.local != "br" {
                    description.push(format!("{depth}:<{}>", name.local));
                }
            }
            NodeData::Text { contents } => {
                let text = contents.borrow();
                let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
                if !collapsed.is_empty() {
                    description.push(format!("{depth}:{collapsed:?}"));
                }
            }
            _ => {}
        }
        for child in node.children.borrow().iter() {
            walk(child, depth + 1, description);
        }
    }

    let mut description = Vec::new();
    walk(node, 0, &mut description);
    description
}

// Check that every empty block a user can create in the editor survives the
// round trip, reporting all failures rather than stopping at the first, so that
// the effect of a change on every case is visible in a single run.
#[test]
fn test_empty_block_round_trip() {
    let problems: Vec<_> = EMPTY_BLOCK_CASES
        .iter()
        .filter_map(|case| {
            check_empty_block_round_trip(case).map(|problem| format!("{}: {problem}", case.name))
        })
        .collect();
    assert!(
        problems.is_empty(),
        "{} of {} empty-block cases failed:\n\n{}",
        problems.len(),
        EMPTY_BLOCK_CASES.len(),
        problems.join("\n\n")
    );
}

#[test]
fn test_hydrate_html_1() {
    // These tests check the translation from Markdown to "wet" HTML (what the
    // user provides) instead of dry -> wet HTML.
    assert_eq!(
        hydrate_html(
            &markdown_to_html(indoc!(
                "```mermaid
            flowchart LR
                start --> stop
            ```
            "
            )),
            Path::new("foo.md"),
            &Arc::new(Mutex::new(Cache::default()))
        )
        .unwrap(),
        indoc!(
            "
            <wc-mermaid>flowchart LR
                start --&gt; stop
            </wc-mermaid>
            "
        )
    );

    assert_eq!(
        hydrate_html(
            &markdown_to_html(indoc!(
                "```graphviz
            digraph {
                start -> stop
            }
            ```
            "
            )),
            Path::new("foo.md"),
            &Arc::new(Mutex::new(Cache::default()))
        )
        .unwrap(),
        indoc!(
            "
            <graphviz-graph>digraph {
                start -&gt; stop
            }
            </graphviz-graph>
            "
        )
    );

    // Ensure math doesn't need escaping.
    assert_eq!(
        hydrate_html(
            &markdown_to_html(indoc!(
                "
            ${a}_1, b_{2}$
            $a*1, b*2$
            $[a](b)$
            $3 <a> b$
            $a \\; b$

            $${a}_1, b_{2}, a*1, b*2, [a](b), 3 <a> b, a \\; b$$
            "
            )),
            Path::new("foo.md"),
            &Arc::new(Mutex::new(Cache::default()))
        )
        .unwrap(),
        indoc!(
            r#"
            <p><span class="math math-inline mceNonEditable" contenteditable="false">\({a}_1, b_{2}\)</span>
            <span class="math math-inline mceNonEditable" contenteditable="false">\(a*1, b*2\)</span>
            <span class="math math-inline mceNonEditable" contenteditable="false">\([a](b)\)</span>
            <span class="math math-inline mceNonEditable" contenteditable="false">\(3 &lt;a&gt; b\)</span>
            <span class="math math-inline mceNonEditable" contenteditable="false">\(a \; b\)</span></p>
            <p><span class="math math-display mceNonEditable" contenteditable="false">$${a}_1, b_{2}, a*1, b*2, [a](b), 3 &lt;a&gt; b, a \; b$$</span></p>
            "#
        )
    );

    assert_eq!(
        hydrate_html(
            &markdown_to_html("1. foo\u{a0}\n2. bar \n3. baz&#32;"),
            Path::new("foo.md"),
            &Arc::new(Mutex::new(Cache::default()))
        )
        .unwrap(),
        indoc!(
            "
            <ol>
            <li>foo&nbsp;</li>
            <li>bar</li>
            <li>baz </li>
            </ol>
            "
        )
    );
}

// ### Cache hydration tests
//
// Verify that a cross-reference without optional content to a target in the
// same file hydrates to a link whose text is the target's inner HTML.
#[test]
fn test_hydrate_xref_same_file() {
    assert_eq!(
        source_to_codechat_for_web(
            "// <h1 id=\"a\">Title</h1>\nlet x = 1;\n// See <xref ref=\"a\"></xref>",
            Path::new("foo.js"),
            0.0,
            false,
            None
        ),
        Ok(build_codechat_for_web(
            "javascript",
            "\nlet x = 1;\n\n",
            vec![
                build_codemirror_doc_block(0, 1, "", "//", "<h1 id=a>Title</h1>"),
                build_codemirror_doc_block(
                    12,
                    13,
                    "",
                    "//",
                    "<p>See <xref contenteditable=false data-content ref=a><a href=#a>Title</a></xref>"
                )
            ]
        ))
    );
}

// Verify that a cross-reference to an unknown id hydrates to an error message.
#[test]
fn test_hydrate_xref_missing() {
    assert_eq!(
        source_to_codechat_for_web(
            "// See <xref ref=\"nope\"></xref>",
            Path::new("foo.js"),
            0.0,
            false,
            None
        ),
        Ok(build_codechat_for_web(
            "javascript",
            "\n",
            vec![build_codemirror_doc_block(
                0,
                1,
                "",
                "//",
                "<p>See <xref contenteditable=false data-content ref=nope><span class=cc-error>id \"nope\" not found</span></xref>"
            )]
        ))
    );
}

// Verify that a cross-reference lacking a `ref` attribute hydrates to an error
// message.
#[test]
fn test_hydrate_xref_without_ref() {
    assert_eq!(
        source_to_codechat_for_web(
            "// See <xref></xref>.",
            Path::new("foo.js"),
            0.0,
            false,
            None
        ),
        Ok(build_codechat_for_web(
            "javascript",
            "\n",
            vec![build_codemirror_doc_block(
                0,
                1,
                "",
                "//",
                "<p>See <xref contenteditable=false data-content><span class=cc-error>a cross-reference requires a \"ref\" attribute</span></xref>."
            )]
        ))
    );
}

// Verify that hydration strips every attribute of a cross-reference but `ref`
// -- including an `id`, even one requesting auto-assignment, since a
// cross-reference is never a target -- and overwrites any `data-content`, so
// that saving the result writes back only the `ref`.
#[test]
fn test_hydrate_xref_strips_attributes() {
    let cache = Arc::new(Mutex::new(Cache::default()));
    let translation = source_to_codechat_for_web(
        "// <h1 id=\"t\">Title</h1>\nlet x = 1;\n// See <xref id=\"x\" title=\"Tip\" data-content=\"Old\" ref=\"t\"></xref> and <xref id=\"*\" ref=\"t\"></xref>.\n",
        Path::new("foo.js"),
        0.0,
        false,
        Some(cache.clone()),
    )
    .unwrap();
    assert_eq!(
        translation,
        build_codechat_for_web(
            "javascript",
            "\nlet x = 1;\n\n",
            vec![
                build_codemirror_doc_block(0, 1, "", "//", "<h1 id=t>Title</h1>"),
                build_codemirror_doc_block(
                    12,
                    13,
                    "",
                    "//",
                    concat!(
                        "<p>See ",
                        "<xref contenteditable=false data-content ref=t><a href=#t>Title</a></xref>",
                        " and ",
                        "<xref contenteditable=false data-content ref=t><a href=#t>Title</a></xref>",
                        "."
                    )
                )
            ]
        )
    );
    // Neither `id` was cached.
    assert_eq!(cache.lock().unwrap().ids.keys().collect::<Vec<_>>(), ["t"]);
    assert_eq!(
        codechat_for_web_to_source(&translation).unwrap(),
        "// <h1 id=\"t\">Title</h1>\nlet x = 1;\n// See <xref ref=\"t\"></xref> and <xref ref=\"t\"></xref>.\n"
    );
}

// Verify that a cross-reference's optional content, once cleaned and hydrated,
// becomes the text of its link, while `data-content` holds that content as
// parsed, attribute-encoded. Cleaning leaves only the permitted content of an
// `<a>`: it unwraps a link and a nested cross-reference, strips `id` and
// `data-gather`, and removes a fragment, so that the optional content defines
// nothing. It keeps the attributes which give phrasing elements their meaning,
// along with `<del>` and `<ins>`. Hydration renders math, but a hand-written
// diagram unwraps to its source; this requires a tight list, since in a
// paragraph the parser ends the cross-reference at the diagram (see
// <xref ref="cc-HZyE2Bv-eI"></xref>). An element which renders nothing still
// counts as content, but whitespace alone doesn't, leaving the target's inner
// HTML as the link's text; since `<svg>` is removed along with its content, it
// leaves only whitespace too. An error replaces the optional content.
#[test]
fn test_hydrate_xref_optional_content() {
    let cache = Arc::new(Mutex::new(Cache::default()));
    let translation = source_to_codechat_for_web(
        indoc!(
            r#"
            // <h1 id="t">Title</h1>
            let x = 1;
            // * <xref ref="t">The *title*</xref>
            // * <xref ref="t"><a href="https://example.com">Link</a> <b id="b" data-gather="f">bold</b><fragment id="f"></fragment> <xref ref="u">nested</xref></xref>
            // * <xref ref="t">The $x^2$ value</xref>
            // * <xref ref="t"><pre><code class="language-mermaid">graph TD</code></pre></xref>
            // * <xref ref="t"><wc-mermaid>graph LR</wc-mermaid></xref>
            // * <xref ref="t"><data value="42">n</data> <time datetime="2026-11-04">today</time> <bdi dir="rtl">x</bdi> <del>old</del> <ins>new</ins></xref>
            // * <xref ref="t"><svg><title>Hidden</title></svg></xref>
            // * <xref ref="t"><span></span></xref>
            // * <xref ref="t"> </xref>
            // * <xref ref="nope">Missing</xref>
            "#
        ),
        Path::new("foo.js"),
        0.0,
        false,
        Some(cache.clone()),
    )
    .unwrap();
    let CodeMirrorDiffable::Plain(code_mirror) = translation.source else {
        panic!("No diff!");
    };
    assert_eq!(
        code_mirror.doc_blocks[1].contents,
        concat!(
            "<ul>",
            "<li><xref data-content=\"The &amp;lt;em&amp;gt;title&amp;lt;/em&amp;gt;\" contenteditable=false ref=t><a href=#t>The <em>title</em></a></xref>",
            "<li><xref data-content=\"&amp;lt;a href=&amp;quot;https://example.com&amp;quot;&amp;gt;Link&amp;lt;/a&amp;gt; &amp;lt;b id=&amp;quot;b&amp;quot; data-gather=&amp;quot;f&amp;quot;&amp;gt;bold&amp;lt;/b&amp;gt;&amp;lt;fragment id=&amp;quot;f&amp;quot;&amp;gt;&amp;lt;/fragment&amp;gt; &amp;lt;xref ref=&amp;quot;u&amp;quot;&amp;gt;nested&amp;lt;/xref&amp;gt;\" contenteditable=false ref=t><a href=#t>Link <b>bold</b> nested</a></xref>",
            "<li><xref data-content=\"The &amp;lt;span class=&amp;quot;math math-inline&amp;quot;&amp;gt;x^2&amp;lt;/span&amp;gt; value\" contenteditable=false ref=t><a href=#t>The <span class=\"math math-inline mceNonEditable\" contenteditable=false>\\(x^2\\)</span> value</a></xref>",
            "<li><xref data-content=\"&amp;lt;pre&amp;gt;&amp;lt;code class=&amp;quot;language-mermaid&amp;quot;&amp;gt;graph TD&amp;lt;/code&amp;gt;&amp;lt;/pre&amp;gt;\" contenteditable=false ref=t><a href=#t><code>graph TD</code></a></xref>",
            "<li><xref data-content=\"&amp;lt;wc-mermaid&amp;gt;graph LR&amp;lt;/wc-mermaid&amp;gt;\" contenteditable=false ref=t><a href=#t>graph LR</a></xref>",
            "<li><xref data-content=\"&amp;lt;data value=&amp;quot;42&amp;quot;&amp;gt;n&amp;lt;/data&amp;gt; &amp;lt;time datetime=&amp;quot;2026-11-04&amp;quot;&amp;gt;today&amp;lt;/time&amp;gt; &amp;lt;bdi dir=&amp;quot;rtl&amp;quot;&amp;gt;x&amp;lt;/bdi&amp;gt; &amp;lt;del&amp;gt;old&amp;lt;/del&amp;gt; &amp;lt;ins&amp;gt;new&amp;lt;/ins&amp;gt;\" contenteditable=false ref=t><a href=#t><data value=42>n</data> <time datetime=2026-11-04>today</time> <bdi dir=rtl>x</bdi> <del>old</del> <ins>new</ins></a></xref>",
            "<li><xref contenteditable=false data-content=&amp;lt;svg&amp;gt;&amp;lt;title&amp;gt;Hidden&amp;lt;/title&amp;gt;&amp;lt;/svg&amp;gt; ref=t><a href=#t>Title</a></xref>",
            "<li><xref contenteditable=false data-content=&amp;lt;span&amp;gt;&amp;lt;/span&amp;gt; ref=t><a href=#t><span></span></a></xref>",
            "<li><xref data-content=\" \" contenteditable=false ref=t><a href=#t>Title</a></xref>",
            "<li><xref contenteditable=false data-content=Missing ref=nope><span class=cc-error>id \"nope\" not found</span></xref>",
            "</ul>"
        )
    );
    // Only the target and the cross-reference outside of optional content
    // defined or referenced an id.
    let cache = cache.lock().unwrap();
    let mut ids: Vec<&String> = cache.ids.keys().collect();
    ids.sort();
    assert_eq!(ids, ["nope", "t"]);
}

// Verify that every cleaner keeps the attributes which give phrasing elements
// their meaning; a cleaner lacking them would strip them from all later output.
#[test]
fn test_clean_keeps_phrasing_attributes() {
    let html = concat!(
        r#"<data value="42">n</data> <time datetime="2026-11-04">today</time> "#,
        r#"<bdi dir="rtl">x</bdi>"#
    );
    for cleaner in [
        &*AMMONIA_OPTIONS,
        &*CLEAN_LINK_CONTENT_HTML,
        &*CLEAN_FRAGMENT_HTML,
    ] {
        assert_eq!(cleaner.clean(html).to_string(), html);
    }
}

// Verify that math in a target's inner HTML renders where that inner HTML is
// used: in a cross-reference without optional content, and in a fragment's
// backlink to the gather element listing it.
#[test]
fn test_hydrate_target_inner_html_math() {
    let translation = source_to_codechat_for_web(
        indoc!(
            r#"
            // ### The $x^2$ value {#t data-gather=f}
            let x = 1;
            // See <xref ref="t"></xref>.<fragment id="f"></fragment>
            "#
        ),
        Path::new("foo.js"),
        0.0,
        false,
        None,
    )
    .unwrap();
    let CodeMirrorDiffable::Plain(code_mirror) = translation.source else {
        panic!("No diff!");
    };
    assert_eq!(
        code_mirror.doc_blocks[1].contents,
        concat!(
            "<p>See <xref contenteditable=false data-content ref=t><a href=#t>The <span class=\"math math-inline mceNonEditable\" contenteditable=false>\\(x^2\\)</span> value</a></xref>.",
            "<fragment contenteditable=false id=f>See <a href=#t>The <span class=\"math math-inline mceNonEditable\" contenteditable=false>\\(x^2\\)</span> value</a></fragment>"
        )
    );
}

// Verify that a tag or comment left open in a doc block closes where that doc
// block ends, leaving the code and the later doc block intact, both when
// translated and after a save.
#[test]
fn test_hydrate_unclosed_tag() {
    for (unclosed_tag, saved_doc_block) in [
        ("<xref ref=\"t\">", "// <xref ref=\"t\"> </xref>"),
        ("See <xref ref=\"t\">", "// See <xref ref=\"t\"></xref>"),
        ("<fragment id=\"f\">", "// <fragment id=\"f\"></fragment>"),
        ("<!-- note", "// <!-- note\n//\n// -->"),
        ("<textarea>", "//"),
        ("<title>", "//"),
        ("<template>", "//"),
        ("<table>", "// <table></table>"),
    ] {
        let translation = source_to_codechat_for_web(
            &format!("// {unclosed_tag}\nlet a = 1;\n// Next.\nlet b = 2;\n"),
            Path::new("foo.js"),
            0.0,
            false,
            None,
        )
        .unwrap();
        assert_eq!(
            codechat_for_web_to_source(&translation).unwrap(),
            format!("{saved_doc_block}\nlet a = 1;\n// Next.\nlet b = 2;\n"),
            "{unclosed_tag}"
        );
        let CodeMirrorDiffable::Plain(code_mirror) = translation.source else {
            panic!("No diff!");
        };
        assert_eq!(code_mirror.doc_blocks[1].contents, "<p>Next.");
    }
}

/// Assert that prose following the given unclosed tag isn't silently lost:
/// either translation reports an error, or a save keeps the prose.
fn assert_unclosed_tag_keeps_prose(
    // The tag left unclosed at the start of a doc block.
    unclosed_tag: &str,
) {
    if let Ok(translation) = source_to_codechat_for_web(
        &format!("// {unclosed_tag}\n//\n// Important prose.\nlet a = 1;\n// Next.\nlet b = 2;\n"),
        Path::new("foo.js"),
        0.0,
        false,
        None,
    ) {
        let saved = codechat_for_web_to_source(&translation).unwrap();
        assert!(
            saved.contains("Important prose."),
            "{unclosed_tag} saved as {saved:?}"
        );
    }
}

#[test]
fn test_hydrate_unclosed_tag_keeps_prose() {
    for unclosed_tag in [
        "<xref ref=\"t\">",
        "See <xref ref=\"t\">",
        "<!-- note",
        "<textarea>",
        "<title>",
        "<table>",
    ] {
        assert_unclosed_tag_keeps_prose(unclosed_tag);
    }
}

// TODO: Move these tags to `test_hydrate_unclosed_tag_keeps_prose` and delete
// this test once hydration keeps this prose or translation reports an error.
#[test]
#[ignore = "an unclosed <template>, <style>, or <script> silently erases the prose after it"]
fn test_hydrate_unclosed_tag_keeps_prose_todo() {
    for unclosed_tag in ["<template>", "<style>", "<script>"] {
        assert_unclosed_tag_keeps_prose(unclosed_tag);
    }
}

// Verify that an unclosed `<table>` stays in its doc block. Parsing all doc
// blocks at once would move everything after it, including the separators, in
// front of it, leaving this doc block empty.
#[test]
fn test_hydrate_unclosed_table() {
    let translation = source_to_codechat_for_web(
        "// <table>\n// <tr><td>x</td></tr>\nlet a = 1;\n// Later.\nlet b = 2;\n",
        Path::new("foo.js"),
        0.0,
        false,
        None,
    )
    .unwrap();
    let CodeMirrorDiffable::Plain(code_mirror) = translation.source else {
        panic!("No diff!");
    };
    assert_eq!(
        code_mirror.doc_blocks[0].contents,
        "<table><tbody><tr><td>x</table>"
    );
    assert_eq!(code_mirror.doc_blocks[1].contents, "<p>Later.");
}

// Verify that a target following an unclosed `<xref>` stays in the cache. The
// fact collection walk doesn't descend into an `<xref>`, so a target which it
// absorbed would leave the cache, and the link below would be an error instead.
#[test]
fn test_hydrate_unclosed_xref_later_target() {
    let translation = source_to_codechat_for_web(
        "// <xref ref=\"t\">\nlet a = 1;\n// <h3 id=\"t\">Title</h3>\nlet b = 2;\n",
        Path::new("foo.js"),
        0.0,
        false,
        None,
    )
    .unwrap();
    let CodeMirrorDiffable::Plain(code_mirror) = translation.source else {
        panic!("No diff!");
    };
    assert_eq!(
        code_mirror.doc_blocks[0].contents,
        "<xref data-content=\"\n\" contenteditable=false ref=t><a href=#t>Title</a></xref>"
    );
    assert_eq!(code_mirror.doc_blocks[1].contents, "<h3 id=t>Title</h3>");
}

// Verify that Markdown which hides the separators of later doc blocks produces
// an error, not a panic or empty doc blocks, which the next save would write to
// the source. The separator's closing fence is 23 characters long, so it can't
// close the first of these. The others end an HTML block at the separator's
// first `>`, exposing a fence which hides the next separator. In the last of
// these, a later separator's fence closes it, leaving that separator visible.
#[test]
fn test_hydrate_hidden_separator() {
    let long_fence = "`".repeat(24);
    for source in [
        format!("// {long_fence}\nlet a = 1;\n// Next.\nlet b = 2;\n"),
        "// <!X\nlet a = 1;\n// Next.\nlet b = 2;\n".to_string(),
        "// <!X\nlet a = 1;\n// Next.\nlet b = 2;\n// Last.\nlet c = 3;\n".to_string(),
    ] {
        assert_eq!(
            source_to_codechat_for_web(&source, Path::new("foo.js"), 0.0, false, None),
            Err(SourceToCodeChatForWebError::ParseFailed(
                "because an unclosed code block or declaration hides later doc blocks".to_string()
            )),
            "{source}"
        );
    }
}

// Verify that a translation which fails on a hidden separator leaves the cache
// alone. Otherwise, a target in a hidden doc block would leave the cache, so
// links to it from other files would re-render as errors.
#[test]
fn test_hydrate_hidden_separator_keeps_cache() {
    let cache = Arc::new(Mutex::new(Cache::default()));
    source_to_codechat_for_web(
        "// Intro.\nlet a = 1;\n// <h3 id=\"t\">Title</h3>\nlet b = 2;\n",
        Path::new("foo.js"),
        0.0,
        false,
        Some(cache.clone()),
    )
    .unwrap();
    let long_fence = "`".repeat(24);
    source_to_codechat_for_web(
        &format!("// {long_fence}\nlet a = 1;\n// <h3 id=\"t\">Title</h3>\nlet b = 2;\n"),
        Path::new("foo.js"),
        0.0,
        false,
        Some(cache.clone()),
    )
    .unwrap_err();
    assert_eq!(cache.lock().unwrap().ids.keys().collect::<Vec<_>>(), ["t"]);
}

// Verify that a `following` value too large to add to the doc block's index
// covers the rest of the file, instead of overflowing.
#[test]
fn test_hydrate_fragment_following_overflow() {
    let translation = source_to_codechat_for_web(
        &formatdoc!(
            r#"
            // <h3 id="gath" data-gather="frag">Gathered</h3>
            let a = 1;
            // <fragment id="frag" following="{}"></fragment>Doc.
            let b = 2;
            "#,
            usize::MAX
        ),
        Path::new("foo.js"),
        0.0,
        false,
        None,
    )
    .unwrap();
    let CodeMirrorDiffable::Plain(code_mirror) = translation.source else {
        panic!("No diff!");
    };
    assert!(
        code_mirror.doc_blocks[0].contents.contains("let b = 2;"),
        "{}",
        code_mirror.doc_blocks[0].contents
    );
}

// Verify that a Markdown document may begin with a comment, which parsing
// places before the document's first element.
#[test]
fn test_hydrate_markdown_leading_comment() {
    let source = "<!-- c -->\n\nHeading\n=======\n";
    let translation =
        source_to_codechat_for_web(source, Path::new("foo.md"), 0.0, false, None).unwrap();
    assert_eq!(codechat_for_web_to_source(&translation).unwrap(), source);
    let CodeMirrorDiffable::Plain(code_mirror) = translation.source else {
        panic!("No diff!");
    };
    assert_eq!(code_mirror.doc, "<!-- c --><h1>Heading</h1>");
}

// Verify that cleaning removes `contenteditable` from optional content, which
// would otherwise make part of a non-editable cross-reference editable, while
// saving still restores that content.
#[test]
fn test_hydrate_xref_content_not_editable() {
    let source = "// <h1 id=\"t\">Title</h1>\nlet x = 1;\n// <xref ref=\"t\"><span contenteditable=\"true\">x</span></xref>\n";
    let translation =
        source_to_codechat_for_web(source, Path::new("foo.js"), 0.0, false, None).unwrap();
    assert_eq!(codechat_for_web_to_source(&translation).unwrap(), source);
    let CodeMirrorDiffable::Plain(code_mirror) = translation.source else {
        panic!("No diff!");
    };
    assert_eq!(
        code_mirror.doc_blocks[1].contents,
        "<p><xref data-content=\"&amp;lt;span contenteditable=&amp;quot;true&amp;quot;&amp;gt;x&amp;lt;/span&amp;gt;\" contenteditable=false ref=t><a href=#t><span>x</span></a></xref>"
    );
}

// Verify that saving hydrated cross-references restores the optional content of
// each -- including math, and a nested cross-reference, which was never
// hydrated -- so that their source survives a round trip through the Client.
#[test]
fn test_xref_optional_content_round_trip() {
    let source = indoc!(
        r#"
        // <h1 id="t">Title</h1>
        let x = 1;
        // See <xref ref="t">the *title*</xref>, <xref ref="t"></xref>,
        // <xref ref="t">$x^2$ and <xref ref="t">more</xref></xref>, and
        // <xref ref="nope">nothing</xref>.
        "#
    );
    let translation =
        source_to_codechat_for_web(source, Path::new("foo.js"), 0.0, false, None).unwrap();
    assert_eq!(codechat_for_web_to_source(&translation).unwrap(), source);

    // A Markdown document reaches dehydration by a separate path.
    let source = "<h1 id=\"t\">Title</h1>\n\nSee <xref ref=\"t\">the *title*</xref> and <xref ref=\"t\"></xref>.\n";
    let translation =
        source_to_codechat_for_web(source, Path::new("foo.md"), 0.0, false, None).unwrap();
    assert_eq!(codechat_for_web_to_source(&translation).unwrap(), source);
}

// Verify that each kind of error replaces the content of the cross-reference's
// link, but leaves its optional content in `data-content`, from which saving
// restores it.
#[test]
fn test_hydrate_xref_error_keeps_optional_content() {
    let source = indoc!(
        r#"
        // <h1 id="d">First</h1>
        let x = 1;
        // <h1 id="d">Second</h1>
        let y = 1;
        // <fragment id="f"></fragment><xref ref="d">*a*</xref> <xref ref="f">*b*</xref>
        // <xref ref="nope">*c*</xref> <xref ref="1 2">*d*</xref> <xref>*e*</xref>
        "#
    );
    let translation =
        source_to_codechat_for_web(source, Path::new("foo.js"), 0.0, false, None).unwrap();
    assert_eq!(codechat_for_web_to_source(&translation).unwrap(), source);
    let CodeMirrorDiffable::Plain(code_mirror) = translation.source else {
        panic!("No diff!");
    };
    assert_eq!(
        code_mirror.doc_blocks[2].contents,
        concat!(
            "<p><fragment contenteditable=false id=f></fragment>",
            "<xref contenteditable=false data-content=&amp;lt;em&amp;gt;a&amp;lt;/em&amp;gt; ref=d><span class=cc-error>id \"d\" is defined more than once</span></xref> ",
            "<xref contenteditable=false data-content=&amp;lt;em&amp;gt;b&amp;lt;/em&amp;gt; ref=f><span class=cc-error>id \"f\" names a fragment; a cross-reference must name a target or gather element</span></xref> ",
            "<xref contenteditable=false data-content=&amp;lt;em&amp;gt;c&amp;lt;/em&amp;gt; ref=nope><span class=cc-error>id \"nope\" not found</span></xref> ",
            "<xref ref=\"1 2\" contenteditable=false data-content=&amp;lt;em&amp;gt;d&amp;lt;/em&amp;gt;><span class=cc-error>\"1 2\" is not a valid CSS identifier</span></xref> ",
            "<xref contenteditable=false data-content=&amp;lt;em&amp;gt;e&amp;lt;/em&amp;gt;><span class=cc-error>a cross-reference requires a \"ref\" attribute</span></xref>"
        )
    );
}

// Verify that a gather element and the fragment it lists hydrate in a single
// pass over their common file: the gather element receives the fragment's
// contents (the fragment's doc block plus the following code block), and the
// fragment receives a backlink to the gather element.
#[test]
fn test_hydrate_gather_same_file() {
    let translation = source_to_codechat_for_web(
        "// <h3 id=\"gath\" data-gather=\"frag\">Gathered</h3>\nlet a = 1;\n// <fragment id=\"frag\"></fragment>Doc.\nlet b = 2;\n// End.",
        Path::new("foo.js"),
        0.0,
        false,
        None,
    )
    .unwrap();
    let CodeMirrorDiffable::Plain(code_mirror) = translation.source else {
        panic!("No diff!");
    };
    let contents: Vec<&str> = code_mirror
        .doc_blocks
        .iter()
        .map(|doc_block| doc_block.contents.as_str())
        .collect();
    // The gather element gains the `cc-gather` class and is followed by the
    // gathered list: a link to the fragment, then the fragment's contents. The
    // fragment's doc block is line 3 of `foo.js` and its code block begins on
    // line 4.
    assert_eq!(
        contents[0],
        "<h3 class=cc-gather data-gather=frag id=gath>Gathered</h3><div class=cc-gather-items contenteditable=false><p class=cc-gather-item-link>From <a href=#frag>foo.js</a>:<div class=cc-fragment-doc><pre class=cc-fragment-indent><span class=cc-line-number>3</span></pre><div class=cc-fragment-doc-contents><p>Doc.</div></div><pre class=cc-fragment-code><span class=cc-line-number>4</span>let b = 2;\n</pre></div>"
    );
    // The fragment renders a backlink to the gather element.
    assert_eq!(
        contents[1],
        "<p><fragment contenteditable=false id=frag>See <a href=#gath>Gathered</a></fragment>Doc."
    );
    assert_eq!(contents[2], "<p>End.");
}

// Verify that a gather list's copy of a fragment keeps the `contenteditable`
// which marks math non-editable, but not a hand-written one, which would make
// part of the non-editable list editable. The fragment's own doc block is
// unchanged.
#[test]
fn test_hydrate_gather_contenteditable() {
    let source = indoc!(
        r#"
        // <fragment id="f"></fragment><span contenteditable="true">a</span>
        // <span contenteditable="">b</span> <span contenteditable="false">c</span>
        // $x^2$
        let a = 1;
        // <h3 data-gather="f" id="g">Gathered</h3>
        "#
    );
    let translation =
        source_to_codechat_for_web(source, Path::new("foo.js"), 0.0, false, None).unwrap();
    assert_eq!(codechat_for_web_to_source(&translation).unwrap(), source);
    let CodeMirrorDiffable::Plain(code_mirror) = translation.source else {
        panic!("No diff!");
    };
    assert_eq!(
        code_mirror.doc_blocks[0].contents,
        "<p><fragment contenteditable=false id=f>See <a href=#g>Gathered</a></fragment><span contenteditable=true>a</span> <span contenteditable>b</span> <span contenteditable=false>c</span> <span class=\"math math-inline mceNonEditable\" contenteditable=false>\\(x^2\\)</span>"
    );
    assert_eq!(
        code_mirror.doc_blocks[1].contents,
        "<h3 class=cc-gather data-gather=f id=g>Gathered</h3><div class=cc-gather-items contenteditable=false><p class=cc-gather-item-link>From <a href=#f>foo.js</a>:<div class=cc-fragment-doc><pre class=cc-fragment-indent><span class=cc-line-number>1</span></pre><div class=cc-fragment-doc-contents><p><span>a</span> <span>b</span> <span contenteditable=false>c</span> <span class=\"math math-inline mceNonEditable\" contenteditable=false>\\(x^2\\)</span></div></div><pre class=cc-fragment-code><span class=cc-line-number>4</span>let a = 1;\n</pre></div>"
    );
}

// Verify that a rendered fragment preserves the layout of the source it came
// from: each doc block keeps its indent, and each line of a code block and the
// first line of each doc block are preceded by their line numbers in the source.
#[test]
fn test_hydrate_gather_indent_and_line_numbers() {
    let translation = source_to_codechat_for_web(
        indoc!(
            r#"
            // <h3 id="gath" data-gather="frag">Gathered</h3>
            let a = 1;
            // A doc block
            // spanning two lines.
            let b = 2;
              // <fragment id="frag" following="2"></fragment>Indented doc.
              let c = 3;
              let d = 4;
              // More docs.
            let e = 5;
            "#
        ),
        Path::new("foo.js"),
        0.0,
        false,
        None,
    )
    .unwrap();
    let CodeMirrorDiffable::Plain(code_mirror) = translation.source else {
        panic!("No diff!");
    };
    // The gathered list follows the gather element, in the same doc block. The
    // fragment covers its own doc block (line 6) plus the two blocks following
    // it: the code on lines 7-8, then the doc block on line 9 -- so the line
    // numbers must count the two lines of the doc block above as well. Both of
    // the fragment's doc blocks are indented two spaces in the source, so both
    // carry that indent here.
    assert_eq!(
        code_mirror.doc_blocks[0]
            .contents
            .split_once("</a>:")
            .expect("the gathered list must link to the fragment")
            .1,
        concat!(
            "<div class=cc-fragment-doc>",
            "<pre class=cc-fragment-indent><span class=cc-line-number>6</span>  </pre>",
            "<div class=cc-fragment-doc-contents><p>Indented doc.</div></div>",
            "<pre class=cc-fragment-code>",
            "<span class=cc-line-number>7</span>  let c = 3;\n",
            "<span class=cc-line-number>8</span>  let d = 4;\n",
            "</pre>",
            "<div class=cc-fragment-doc>",
            "<pre class=cc-fragment-indent><span class=cc-line-number>9</span>  </pre>",
            "<div class=cc-fragment-doc-contents><p>More docs.</div></div>",
            "</div>"
        )
    );
}

// Verify that cross-file hydration works through a shared project cache: hrefs
// lead from the referring file to the target's file, and reprocessing the
// referring file after the target changed picks up the new content.
#[test]
fn test_hydrate_xref_cross_file() {
    let cache = Arc::new(Mutex::new(Cache::default()));

    // Define the target in one file...
    source_to_codechat_for_web(
        "// <h1 id=\"t\">Title</h1>",
        Path::new("a.js"),
        0.0,
        false,
        Some(cache.clone()),
    )
    .unwrap();
    // ...and reference it from another.
    let reference = "// See <xref ref=\"t\"></xref>";
    let translation = source_to_codechat_for_web(
        reference,
        Path::new("b.js"),
        0.0,
        false,
        Some(cache.clone()),
    )
    .unwrap();
    let CodeMirrorDiffable::Plain(code_mirror) = translation.source else {
        panic!("No diff!");
    };
    assert_eq!(
        code_mirror.doc_blocks[0].contents,
        "<p>See <xref contenteditable=false data-content ref=t><a href=a.js#t>Title</a></xref>"
    );

    // Change the target's inner HTML, then reprocess the referencing file: the
    // link text must update.
    source_to_codechat_for_web(
        "// <h1 id=\"t\">New title</h1>",
        Path::new("a.js"),
        0.0,
        false,
        Some(cache.clone()),
    )
    .unwrap();
    let translation =
        source_to_codechat_for_web(reference, Path::new("b.js"), 0.0, false, Some(cache)).unwrap();
    let CodeMirrorDiffable::Plain(code_mirror) = translation.source else {
        panic!("No diff!");
    };
    assert_eq!(
        code_mirror.doc_blocks[0].contents,
        "<p>See <xref contenteditable=false data-content ref=t><a href=a.js#t>New title</a></xref>"
    );
}

// Verify that a cross-reference within a gathered fragment links from the
// gathering file, not from the fragment's file: here the target shares the
// fragment's file, where the link is just `#t`, while the gathering file lies
// in another directory. Its optional content survives the cleaning of the
// fragment's content, so it still supplies the link's text. A cross-reference
// which hydrated to an error keeps it.
#[test]
fn test_hydrate_gathered_xref() {
    let cache = Arc::new(Mutex::new(Cache::default()));
    source_to_codechat_for_web(
        "// <h1 id=\"t\">Title</h1>\nlet a = 1;\n// <fragment id=\"frag\"></fragment>See <xref ref=\"t\"></xref>, <xref ref=\"t\">the title</xref>, and <xref ref=\"nope\"></xref>.\nlet b = 2;",
        Path::new("b.js"),
        0.0,
        false,
        Some(cache.clone()),
    )
    .unwrap();
    let translation = source_to_codechat_for_web(
        "// <h3 id=\"gath\" data-gather=\"frag\">Gathered</h3>",
        Path::new("sub/a.js"),
        0.0,
        false,
        Some(cache),
    )
    .unwrap();
    let CodeMirrorDiffable::Plain(code_mirror) = translation.source else {
        panic!("No diff!");
    };
    assert_eq!(
        code_mirror.doc_blocks[0].contents,
        concat!(
            "<h3 class=cc-gather data-gather=frag id=gath>Gathered</h3>",
            "<div class=cc-gather-items contenteditable=false>",
            "<p class=cc-gather-item-link>From <a href=../b.js#frag>../b.js</a>:",
            "<div class=cc-fragment-doc>",
            "<pre class=cc-fragment-indent><span class=cc-line-number>3</span></pre>",
            "<div class=cc-fragment-doc-contents>",
            "<p>See <xref contenteditable=false data-content ref=t><a href=../b.js#t>Title</a></xref>",
            ", <xref data-content=\"the title\" contenteditable=false ref=t><a href=../b.js#t>the title</a></xref>",
            ", and <xref contenteditable=false data-content ref=nope><span class=cc-error>id \"nope\" not found</span></xref>.",
            "</div></div>",
            "<pre class=cc-fragment-code><span class=cc-line-number>4</span>let b = 2;</pre>",
            "</div>"
        )
    );
}

// Verify that optional content holding markup and math survives storage in a
// fragment's content: the gathering file's re-hydrated link renders it, and its
// `data-content` still holds that content as parsed.
#[test]
fn test_hydrate_gathered_xref_markup() {
    let cache = Arc::new(Mutex::new(Cache::default()));
    source_to_codechat_for_web(
        "// <h1 id=\"t\">Title</h1>\nlet a = 1;\n// <fragment id=\"frag\"></fragment><xref ref=\"t\">*the* $x^2$</xref>\nlet b = 2;",
        Path::new("b.js"),
        0.0,
        false,
        Some(cache.clone()),
    )
    .unwrap();
    let translation = source_to_codechat_for_web(
        "// <h3 id=\"gath\" data-gather=\"frag\">Gathered</h3>",
        Path::new("sub/a.js"),
        0.0,
        false,
        Some(cache),
    )
    .unwrap();
    let CodeMirrorDiffable::Plain(code_mirror) = translation.source else {
        panic!("No diff!");
    };
    assert_eq!(
        code_mirror.doc_blocks[0].contents,
        concat!(
            "<h3 class=cc-gather data-gather=frag id=gath>Gathered</h3>",
            "<div class=cc-gather-items contenteditable=false>",
            "<p class=cc-gather-item-link>From <a href=../b.js#frag>../b.js</a>:",
            "<div class=cc-fragment-doc>",
            "<pre class=cc-fragment-indent><span class=cc-line-number>3</span></pre>",
            "<div class=cc-fragment-doc-contents>",
            "<p><xref data-content=\"&amp;lt;em&amp;gt;the&amp;lt;/em&amp;gt; &amp;lt;span class=&amp;quot;math math-inline&amp;quot;&amp;gt;x^2&amp;lt;/span&amp;gt;\" contenteditable=false ref=t>",
            "<a href=../b.js#t><em>the</em> <span class=\"math math-inline mceNonEditable\" contenteditable=false>\\(x^2\\)</span></a></xref>",
            "</div></div>",
            "<pre class=cc-fragment-code><span class=cc-line-number>4</span>let b = 2;</pre>",
            "</div>"
        )
    );
}

// Verify that, of two fragments sharing an id in one file, the first in
// document order supplies the cached content, and that re-processing the file
// unchanged doesn't outdate the file gathering that id.
#[test]
fn test_duplicate_fragment_in_one_file() {
    let cache = Arc::new(Mutex::new(Cache::default()));
    let translate = |source: &str, path: &str| {
        source_to_codechat_for_web(source, Path::new(path), 0.0, false, Some(cache.clone()))
            .unwrap();
    };
    let gathering = "// <h3 id=\"gath\" data-gather=\"frag\">Gathered</h3>";
    let fragments = "// <fragment id=\"frag\"></fragment>First.\nlet a = 1;\n// <fragment id=\"frag\"></fragment>Second.\nlet b = 2;";
    translate(gathering, "a.js");
    translate(fragments, "b.js");
    // Storing the fragment's content outdated the gathering file; bring it up
    // to date, then re-process the fragments' file.
    translate(gathering, "a.js");
    translate(fragments, "b.js");

    let cache = cache.lock().unwrap();
    assert_eq!(cache.files[Path::new("a.js")].status, FileStatus::UpToDate);
    let content = &cache.files[Path::new("b.js")].fragments["frag"].content;
    assert!(content.contains("First.") && !content.contains("Second."));
}

// Verify that a fragment in a Markdown document is an error.
#[test]
fn test_hydrate_fragment_in_markdown() {
    assert_eq!(
        source_to_codechat_for_web(
            "<fragment id=\"f\"></fragment>",
            Path::new("foo.md"),
            0.0,
            false,
            None
        ),
        Ok(build_codechat_for_web(
            MARKDOWN_MODE,
            "<p><fragment contenteditable=false id=f><span class=cc-error>fragments are not allowed in Markdown documents</span></fragment>",
            vec![]
        ))
    );
}

// Verify that a fragment without an id -- whether its `id` attribute is missing
// or empty -- hydrates to an error message requesting the id. Dehydration
// removes the message, so saving the hydrated result writes back a bare
// `<fragment></fragment>`. Since a fragment is cached under its id, the cache
// records nothing for one which lacks an id.
#[test]
fn test_hydrate_fragment_without_id() {
    // Both cases produce the same output. Hydration sees the empty `id`, which
    // it must treat as missing rather than as an invalid id; the minifier then
    // discards the empty attribute as redundant, so the Client never receives
    // it and saving can't restore it.
    for fragment in ["<fragment></fragment>", "<fragment id=\"\"></fragment>"] {
        let cache = Arc::new(Mutex::new(Cache::default()));
        let translation = source_to_codechat_for_web(
            &format!("# {fragment}Doc.\nx = 1\n"),
            Path::new("foo.py"),
            0.0,
            false,
            Some(cache.clone()),
        )
        .unwrap();
        assert_eq!(
            translation,
            build_codechat_for_web(
                "python",
                "\nx = 1\n",
                vec![build_codemirror_doc_block(
                    0,
                    1,
                    "",
                    "#",
                    "<p><fragment contenteditable=false><span class=cc-error>a fragment requires an \"id\" attribute</span></fragment>Doc."
                )]
            ),
            "hydrating {fragment}"
        );
        assert_eq!(
            codechat_for_web_to_source(&translation).unwrap(),
            "# <fragment></fragment>Doc.\nx = 1\n",
            "saving {fragment}"
        );
        // The commit recorded the file, but neither a fragment in it nor any
        // id.
        let cache_guard = cache.lock().unwrap();
        assert_eq!(cache_guard.files.len(), 1, "caching {fragment}");
        assert!(
            cache_guard
                .files
                .values()
                .all(|file_entry| file_entry.fragments.is_empty()),
            "caching {fragment}"
        );
        assert!(cache_guard.ids.is_empty(), "caching {fragment}");
    }
}

// Verify auto-assignment of ids: `id="*"` is replaced by a generated, valid CSS
// identifier which is *not* recorded in the cache, and which the cache picks up
// only once the file carrying it is written and processed again.
#[test]
fn test_auto_assign_id() {
    let cache = Arc::new(Mutex::new(Cache::default()));
    let translation = source_to_codechat_for_web(
        "// <h1 id=\"*\">Title</h1>",
        Path::new("a.js"),
        0.0,
        false,
        Some(cache.clone()),
    )
    .unwrap();
    let CodeMirrorDiffable::Plain(code_mirror) = translation.source else {
        panic!("No diff!");
    };
    // Recover the generated id from the hydrated output.
    let contents = &code_mirror.doc_blocks[0].contents;
    let id = Regex::new("<h1 id=([^>]+)>")
        .unwrap()
        .captures(contents)
        .expect("the `id` attribute must survive hydration")[1]
        .to_string();
    assert!(is_css_identifier(&id));
    // Nothing is cached until the file holding the new id is written: a write
    // which never happens (or fails) must not leave the cache describing an id
    // no file contains.
    assert!(!cache.lock().unwrap().ids.contains_key(&id));

    // Saving the file writes the generated id to disk; processing what was
    // written records it, so cross-references to it now resolve.
    source_to_codechat_for_web(
        &format!("// <h1 id=\"{id}\">Title</h1>"),
        Path::new("a.js"),
        0.0,
        false,
        Some(cache.clone()),
    )
    .unwrap();
    let translation = source_to_codechat_for_web(
        &format!("// See <xref ref=\"{id}\"></xref>"),
        Path::new("b.js"),
        0.0,
        false,
        Some(cache),
    )
    .unwrap();
    let CodeMirrorDiffable::Plain(code_mirror) = translation.source else {
        panic!("No diff!");
    };
    assert_eq!(
        code_mirror.doc_blocks[0].contents,
        format!(
            "<p>See <xref contenteditable=false data-content ref={id}><a href=a.js#{id}>Title</a></xref>"
        )
    );
}

// Verify that dehydration removes all hydration artifacts, so that saving
// hydrated content writes clean source: each `<xref>`'s contents are replaced
// by the optional content saved, attribute-encoded, in its `data-content`, or
// emptied if it has none; `<fragment>` contents are emptied, the gather list is
// removed, and the `cc-gather` class and the `contenteditable` and
// `data-content` attributes are dropped.
#[test]
fn test_dehydrate_hydration_artifacts() {
    assert_eq!(
        codechat_for_web_to_source(&build_codechat_for_web(
            "javascript",
            "\nlet b = 2;",
            vec![
                build_codemirror_doc_block(
                    0,
                    1,
                    "",
                    "//",
                    // The gather list holds a rendered fragment: an indented,
                    // line-numbered doc block and a line-numbered code block.
                    "<h3 id=\"gath\" data-gather=\"frag\" class=\"cc-gather\">Gathered</h3><div class=\"cc-gather-items\" contenteditable=\"false\"><div class=\"cc-fragment-doc\"><pre class=\"cc-fragment-indent\"><span class=\"cc-line-number\">3</span>  </pre><div class=\"cc-fragment-doc-contents\"><p>Doc.</p></div></div><pre class=\"cc-fragment-code\"><span class=\"cc-line-number\">4</span>let b = 2;\n</pre></div><p>See <xref ref=\"t\" contenteditable=\"false\" data-content=\"\"><a href=\"#t\">Title</a></xref> and <fragment id=\"frag\" contenteditable=\"false\">See <a href=\"#gath\">Gathered</a></fragment>too.</p><p>Also <xref ref=\"t\" contenteditable=\"false\" data-content=\"the &amp;lt;em&amp;gt;title&amp;lt;/em&amp;gt;\"><a href=\"#t\">the <em>title</em></a></xref> and <xref ref=\"t\">dropped</xref>.</p>"
                ),
            ]
        ))
        .unwrap(),
        "// <h3 id=\"gath\" data-gather=\"frag\">Gathered</h3>\n//\n// See <xref ref=\"t\"></xref> and <fragment id=\"frag\"></fragment>too.\n//\n// Also <xref ref=\"t\">the *title*</xref> and <xref ref=\"t\"></xref>.\nlet b = 2;"
    );
}

// Verify that the `contenteditable` attribute TinyMCE's anchor plugin adds to
// an empty named anchor (`<a id="foo"></a>`) is dropped when the anchor is
// saved, in both a Markdown document and a doc block. Only the plugin's
// serializer removes that attribute, and the Client's raw-format save bypasses
// it; see `remove_tinymce_data`.
#[test]
fn test_dehydrate_named_anchor() {
    /**** TODO: re-enable this when dprint-markdown is fixed.
    assert_eq!(
        codechat_for_web_to_source(&build_codechat_for_web(
            MARKDOWN_MODE,
            "<h2><a id=\"notes\" contenteditable=\"false\"></a>Notes</h2><p>Read the <a href=\"#notes\" contenteditable=\"false\">notes</a>.</p>",
            vec![]
        ))
        .unwrap(),
        "<a id=\"notes\"></a>Notes\n-----------------------\n\nRead the [notes](#notes).\n"
    );
    */

    assert_eq!(
        codechat_for_web_to_source(&build_codechat_for_web(
            "javascript",
            "\nlet a = 1;",
            vec![build_codemirror_doc_block(
                0,
                1,
                "",
                "//",
                "<p><a id=\"notes\" contenteditable=\"false\"></a>Notes</p>"
            )]
        ))
        .unwrap(),
        "// <a id=\"notes\"></a>Notes\nlet a = 1;"
    );
}

fn dehydrate_html(html: &str) -> io::Result<Rc<Node>> {
    let tree = html_to_dom(html, None)?;
    dehydrating_walk_node(&tree)?;
    Ok(tree)
}

#[test]
fn test_dehydrate_html_1() {
    // Each document needs its own conversion, since `convert` consumes it.
    let html_to_markdown = new_html_to_markdown();
    assert_eq!(
        HtmlToMarkdownWrapped::new(&html_to_markdown)
            .convert(
                &dehydrate_html(indoc!(
                    "
                    <wc-mermaid>flowchart LR
                        start --&gt; stop
                    </wc-mermaid>
                    "
                ))
                .unwrap()
            )
            .unwrap(),
        indoc!(
            "
            ```mermaid
            flowchart LR
                start --> stop
            ```
            "
        )
    );

    assert_eq!(
        HtmlToMarkdownWrapped::new(&html_to_markdown)
            .convert(
                &dehydrate_html(indoc!(
                    "
                    <graphviz-graph>digraph {
                        start -&gt; stop
                    }
                    </graphviz-graph>
                    "
                ))
                .unwrap()
            )
            .unwrap(),
        indoc!(
            "
            ```graphviz
            digraph {
                start -> stop
            }
            ```
            "
        )
    );

    assert_eq!(
        HtmlToMarkdownWrapped::new(&html_to_markdown)
            .convert(
                &dehydrate_html(indoc!(
                    r#"
                    <p><span class="math math-inline mceNonEditable" contenteditable="false">\({a}_1, b_{2}\)</span>
                    <span class="math math-inline mceNonEditable" contenteditable="false">\(a*1, b*2\)</span>
                    <span class="math math-inline mceNonEditable" contenteditable="false">\([a](b)\)</span>
                    <span class="math math-inline mceNonEditable" contenteditable="false">\(3 &lt;a&gt; b\)</span>
                    <span class="math math-inline mceNonEditable" contenteditable="false">\(a \; b\)</span></p>
                    <p><span class="math math-display mceNonEditable" contenteditable="false">$${a}_1, b_{2}, a*1, b*2, [a](b), 3 &lt;a&gt; b, a \; b$$</span></p>
                    "#
                ))
                .unwrap()
            )
            .unwrap(),
        indoc!(
            "
            ${a}_1, b_{2}$ $a*1, b*2$ $[a](b)$ $3 <a> b$ $a \\; b$

            $${a}_1, b_{2}, a*1, b*2, [a](b), 3 <a> b, a \\; b$$
            "
        )
    );

    assert_eq!(
        HtmlToMarkdownWrapped::new(&html_to_markdown)
            .convert(
                &dehydrate_html(indoc!(
                    "
                    <ol>
                    <li>foo&nbsp;</li>
                    </ol>
                    "
                ))
                .unwrap()
            )
            .unwrap(),
        "1. foo\u{a0}\n"
    );

    assert_eq!(
        HtmlToMarkdownWrapped::new(&html_to_markdown)
            .convert(
                &dehydrate_html(indoc!(
                    r#"
                    <pre><code class="language-html"><br>&lt;!DOCTYPE html&gt;<br>
                    &lt;html lang="en"&gt;
                    &lt;head&gt;
                        &lt;meta charset="UTF-8"&gt;
                        &lt;title&gt;TinyMCE Dirty Event Test&lt;/title&gt;
                    &lt;/head&gt;
                    &lt;body&gt;
                        &lt;h1&gt;TinyMCE Dirty Event Test&lt;/h1&gt;
                    &lt;/body&gt;
                    &lt;/html&gt;<br>
                    </code></pre>
                    "#
                ))
                .unwrap()
            )
            .unwrap(),
        indoc!(
            r#"
                ```html

                <!DOCTYPE html>

                <html lang="en">
                <head>
                    <meta charset="UTF-8">
                    <title>TinyMCE Dirty Event Test</title>
                </head>
                <body>
                    <h1>TinyMCE Dirty Event Test</h1>
                </body>
                </html>

                ```
        "#
        )
    );

    // A trailing empty paragraph (`<p><br></p>`) is converted to
    // `<p><br></p>` by `dehydrating_walk_node`, preserving it as a
    // non-breaking space.
    assert_eq!(
        HtmlToMarkdownWrapped::new(&html_to_markdown)
            .convert(
                &dehydrate_html(indoc!(
                    "
                    <p>1</p><p><br data-mce-bogus=\"1\"></p>
                    "
                ))
                .unwrap()
            )
            .unwrap(),
        indoc!(
            "
            1

            <p><br></p>
            "
        )
    );
}
