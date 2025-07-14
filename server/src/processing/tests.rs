// Copyright (C) 2023 Bryan A. Jones.
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
use std::{path::PathBuf, str::FromStr};

// ### Third-party
use predicates::prelude::predicate::str;
use pretty_assertions::assert_eq;

// ### Local
use super::{
    CodeChatForWeb, CodeMirror, CodeMirrorDocBlock, SourceFileMetadata, StringDiff,
    TranslationResults, find_path_to_toc,
};
use crate::{
    lexer::{CodeDocBlock, DocBlock, compile_lexers, supported_languages::get_language_lexer_vec},
    prep_test_dir,
    processing::{
        CodeMirrorDiffable, CodeMirrorDocBlockDelete, CodeMirrorDocBlockTransaction,
        CodeMirrorDocBlockUpdate, code_doc_block_vec_to_source, code_mirror_to_code_doc_blocks,
        codechat_for_web_to_source, diff_code_mirror_doc_blocks, diff_str,
        source_to_codechat_for_web,
    },
    test_utils::stringit,
};

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
// Since it just invokes `code_mirror_to_code_doc_blocks` and
// `code_doc_block_vec_to_source`, both of which have their own set of tests, we
// just need to do a bit of testing.
#[test]
fn test_codechat_for_web_to_source() {
    let codechat_for_web = build_codechat_for_web("python", "", vec![]);
    assert_eq!(
        codechat_for_web_to_source(&codechat_for_web),
        Result::Ok("".to_string())
    );

    let codechat_for_web = build_codechat_for_web("undefined", "", vec![]);
    assert_eq!(
        codechat_for_web_to_source(&codechat_for_web),
        Result::Err("Invalid mode".to_string())
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

    // Pass one doc block containing Unicode.
    assert_eq!(
        run_test(
            "python",
            "σ\n",
            vec![build_codemirror_doc_block(1, 2, "", "#", "Test")],
        ),
        vec![build_code_block("σ"), build_doc_block("", "#", "Test")]
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
}

#[test]
fn test_codemirror_to_code_doc_blocks_cpp() {
    // Pass an inline comment.
    assert_eq!(
        run_test(
            "c_cpp",
            "\n",
            vec![build_codemirror_doc_block(0, 1, "", "//", "Test")]
        ),
        vec![build_doc_block("", "//", "Test")]
    );

    // Pass a block comment.
    assert_eq!(
        run_test(
            "c_cpp",
            "\n",
            vec![build_codemirror_doc_block(0, 1, "", "/*", "Test")]
        ),
        vec![build_doc_block("", "/*", "Test")]
    );

    // Two back-to-back doc blocks.
    assert_eq!(
        run_test(
            "c_cpp",
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
    assert_eq!(code_doc_block_vec_to_source(&vec![], py_lexer).unwrap(), "");
    // A one-line comment.
    assert_eq!(
        code_doc_block_vec_to_source(&vec![build_doc_block("", "#", "Test")], py_lexer).unwrap(),
        "# Test"
    );
    assert_eq!(
        code_doc_block_vec_to_source(&vec![build_doc_block("", "#", "Test\n")], py_lexer).unwrap(),
        "# Test\n"
    );
    // Check empty doc block lines and multiple lines.
    assert_eq!(
        code_doc_block_vec_to_source(
            &vec![build_doc_block("", "#", "Test 1\n\nTest 2")],
            py_lexer
        )
        .unwrap(),
        "# Test 1\n#\n# Test 2"
    );

    // Repeat the above tests with an indent.
    assert_eq!(
        code_doc_block_vec_to_source(&vec![build_doc_block(" ", "#", "Test")], py_lexer).unwrap(),
        " # Test"
    );
    assert_eq!(
        code_doc_block_vec_to_source(&vec![build_doc_block("  ", "#", "Test\n")], py_lexer)
            .unwrap(),
        "  # Test\n"
    );
    assert_eq!(
        code_doc_block_vec_to_source(
            &vec![build_doc_block("   ", "#", "Test 1\n\nTest 2")],
            py_lexer
        )
        .unwrap(),
        "   # Test 1\n   #\n   # Test 2"
    );

    // Basic code.
    assert_eq!(
        code_doc_block_vec_to_source(&vec![build_code_block("Test")], py_lexer).unwrap(),
        "Test"
    );

    // An incorrect delimiter.
    assert_eq!(
        code_doc_block_vec_to_source(&vec![build_doc_block("", "?", "Test")], py_lexer)
            .unwrap_err(),
        "Unknown comment opening delimiter '?'."
    );

    // Empty doc blocks separated by an empty code block.
    assert_eq!(
        code_doc_block_vec_to_source(
            &vec![
                build_doc_block("", "#", "\n"),
                build_code_block("\n"),
                build_doc_block("", "#", ""),
            ],
            py_lexer
        )
        .unwrap(),
        "#\n\n#"
    );

    assert_eq!(
        code_doc_block_vec_to_source(
            &vec![
                build_doc_block("", "#", "σ\n"),
                build_code_block("σ\n"),
                build_doc_block("", "#", "σ"),
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
    assert_eq!(
        code_doc_block_vec_to_source(&vec![], css_lexer).unwrap(),
        ""
    );
    // A one-line comment.
    assert_eq!(
        code_doc_block_vec_to_source(&vec![build_doc_block("", "/*", "Test\n")], css_lexer)
            .unwrap(),
        "/* Test */\n"
    );
    assert_eq!(
        code_doc_block_vec_to_source(&vec![build_doc_block("", "/*", "Test")], css_lexer).unwrap(),
        "/* Test */"
    );
    // Check empty doc block lines and multiple lines.
    assert_eq!(
        code_doc_block_vec_to_source(
            &vec![
                build_code_block("Test_0\n"),
                build_doc_block("", "/*", "Test 1\n\nTest 2\n")
            ],
            css_lexer
        )
        .unwrap(),
        r#"Test_0
/* Test 1

   Test 2 */
"#
    );

    // Repeat the above tests with an indent.
    assert_eq!(
        code_doc_block_vec_to_source(&vec![build_doc_block("  ", "/*", "Test\n")], css_lexer)
            .unwrap(),
        "  /* Test */\n"
    );
    assert_eq!(
        code_doc_block_vec_to_source(
            &vec![
                build_code_block("Test_0\n"),
                build_doc_block("   ", "/*", "Test 1\n\nTest 2\n")
            ],
            css_lexer
        )
        .unwrap(),
        r#"Test_0
   /* Test 1

      Test 2 */
"#
    );

    // Basic code.
    assert_eq!(
        code_doc_block_vec_to_source(&vec![build_code_block("Test")], css_lexer).unwrap(),
        "Test"
    );

    // An incorrect delimiter.
    assert_eq!(
        code_doc_block_vec_to_source(&vec![build_doc_block("", "?", "Test")], css_lexer)
            .unwrap_err(),
        "Unknown comment opening delimiter '?'."
    );
}

// A language with multiple inline and block comment styles.
#[test]
fn test_code_doc_blocks_to_source_csharp() {
    let llc = compile_lexers(get_language_lexer_vec());
    let csharp_lexer = llc.map_mode_to_lexer.get(&stringit("csharp")).unwrap();

    // An empty document.
    assert_eq!(
        code_doc_block_vec_to_source(&vec![], csharp_lexer).unwrap(),
        ""
    );

    // An invalid comment.
    assert_eq!(
        code_doc_block_vec_to_source(&vec![build_doc_block("", "?", "Test\n")], csharp_lexer)
            .unwrap_err(),
        "Unknown comment opening delimiter '?'."
    );

    // Inline comments.
    assert_eq!(
        code_doc_block_vec_to_source(&vec![build_doc_block("", "//", "Test\n")], csharp_lexer)
            .unwrap(),
        "// Test\n"
    );
    assert_eq!(
        code_doc_block_vec_to_source(&vec![build_doc_block("", "///", "Test\n")], csharp_lexer)
            .unwrap(),
        "/// Test\n"
    );

    // Block comments.
    assert_eq!(
        code_doc_block_vec_to_source(&vec![build_doc_block("", "/*", "Test\n")], csharp_lexer)
            .unwrap(),
        "/* Test */\n"
    );
    assert_eq!(
        code_doc_block_vec_to_source(&vec![build_doc_block("", "/**", "Test\n")], csharp_lexer)
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
        source_to_codechat_for_web("", &".xxx".to_string(), false, false),
        TranslationResults::Unknown
    );

    // A file with an invalid lexer specification. Obscure this, so that this
    // file can be successfully lexed by the CodeChat editor.
    let lexer_spec = format!("{}{}", "CodeChat Editor ", "lexer: ");
    assert_eq!(
        source_to_codechat_for_web(
            &format!("{lexer_spec}unknown"),
            &".xxx".to_string(),
            false,
            false,
        ),
        TranslationResults::Err("<p>Unknown lexer type unknown.</p>".to_string())
    );

    // A CodeChat Editor document via filename.
    assert_eq!(
        source_to_codechat_for_web("", &"md".to_string(), false, false),
        TranslationResults::CodeChat(build_codechat_for_web("markdown", "", vec![]))
    );

    // A CodeChat Editor document via lexer specification.
    assert_eq!(
        source_to_codechat_for_web(
            &format!("{lexer_spec}markdown"),
            &"xxx".to_string(),
            false,
            false,
        ),
        TranslationResults::CodeChat(build_codechat_for_web(
            "markdown",
            &format!("<p>{lexer_spec}markdown</p>\n"),
            vec![]
        ))
    );

    // An empty source file.
    assert_eq!(
        source_to_codechat_for_web("", &"js".to_string(), false, false),
        TranslationResults::CodeChat(build_codechat_for_web("javascript", "", vec![]))
    );

    // A zero doc block source file.
    assert_eq!(
        source_to_codechat_for_web("let a = 1;", &"js".to_string(), false, false),
        TranslationResults::CodeChat(build_codechat_for_web("javascript", "let a = 1;", vec![]))
    );

    // One doc block source files.
    assert_eq!(
        source_to_codechat_for_web("// Test", &"js".to_string(), false, false),
        TranslationResults::CodeChat(build_codechat_for_web(
            "javascript",
            "\n",
            vec![build_codemirror_doc_block(0, 1, "", "//", "<p>Test</p>\n")]
        ))
    );
    assert_eq!(
        source_to_codechat_for_web("let a = 1;\n// Test", &"js".to_string(), false, false,),
        TranslationResults::CodeChat(build_codechat_for_web(
            "javascript",
            "let a = 1;\n\n",
            vec![build_codemirror_doc_block(
                11,
                12,
                "",
                "//",
                "<p>Test</p>\n"
            )]
        ))
    );
    assert_eq!(
        source_to_codechat_for_web("// Test\nlet a = 1;", &"js".to_string(), false, false,),
        TranslationResults::CodeChat(build_codechat_for_web(
            "javascript",
            "\nlet a = 1;",
            vec![build_codemirror_doc_block(0, 1, "", "//", "<p>Test</p>\n")]
        ))
    );

    // A two doc block source file. This also tests references in one block to a
    // target in another block.
    assert_eq!(
        source_to_codechat_for_web(
            "// [Link][1]\nlet a = 1;\n/* [1]: http://b.org */",
            &"js".to_string(),
            false,
            false,
        ),
        TranslationResults::CodeChat(build_codechat_for_web(
            "javascript",
            "\nlet a = 1;\n\n",
            vec![
                build_codemirror_doc_block(
                    0,
                    1,
                    "",
                    "//",
                    "<p><a href=\"http://b.org\">Link</a></p>\n"
                ),
                build_codemirror_doc_block(12, 13, "", "/*", "")
            ]
        ))
    );

    // Trigger special cases:
    //
    // *   An empty doc block at the beginning of the file.
    // *   A doc block in the middle of the file
    // *   A doc block with no trailing newline at the end of the file.
    assert_eq!(
        source_to_codechat_for_web("//\n\n//\n\n//", &"cpp".to_string(), false, false),
        TranslationResults::CodeChat(build_codechat_for_web(
            "c_cpp",
            "\n\n\n\n",
            vec![
                build_codemirror_doc_block(0, 1, "", "//", ""),
                build_codemirror_doc_block(2, 3, "", "//", ""),
                build_codemirror_doc_block(4, 5, "", "//", "")
            ]
        ))
    );
    assert_eq!(
        source_to_codechat_for_web("// ~~~\n\n//\n\n//", &"cpp".to_string(), false, false),
        TranslationResults::CodeChat(build_codechat_for_web(
            "c_cpp",
            "\n\n\n\n",
            vec![
                build_codemirror_doc_block(0, 1, "", "//", "<pre><code>\n</code></pre>\n"),
                build_codemirror_doc_block(2, 3, "", "//", ""),
                build_codemirror_doc_block(4, 5, "", "//", "")
            ]
        ))
    );

    // Test Unicode characters in code.
    assert_eq!(
        source_to_codechat_for_web("; // σ\n//", &"cpp".to_string(), false, false),
        TranslationResults::CodeChat(build_codechat_for_web(
            "c_cpp",
            "; // σ\n",
            vec![build_codemirror_doc_block(7, 8, "", "//", ""),]
        ))
    );

    // Test Unicode characters in strings.
    assert_eq!(
        source_to_codechat_for_web("\"σ\";\n//", &"cpp".to_string(), false, false),
        TranslationResults::CodeChat(build_codechat_for_web(
            "c_cpp",
            "\"σ\";\n",
            vec![build_codemirror_doc_block(5, 6, "", "//", ""),]
        ))
    );

    // Test a fenced code block that's unterminated. See [fence
    // mending](#fence-mending).
    assert_eq!(
        source_to_codechat_for_web("/* ``` foo\n*/\n// Test", &"cpp".to_string(), false, false),
        TranslationResults::CodeChat(build_codechat_for_web(
            "c_cpp",
            "\n\n\n",
            vec![
                build_codemirror_doc_block(
                    0,
                    2,
                    "",
                    "/*",
                    "<pre><code class=\"language-foo\">\n\n</code></pre>\n"
                ),
                build_codemirror_doc_block(2, 3, "", "//", "<p>Test</p>\n"),
            ]
        ))
    );
    // Test the other code fence character (the tilde).
    assert_eq!(
        source_to_codechat_for_web(
            "/* ~~~~~~~ foo\n*/\n// Test",
            &"cpp".to_string(),
            false,
            false
        ),
        TranslationResults::CodeChat(build_codechat_for_web(
            "c_cpp",
            "\n\n\n",
            vec![
                build_codemirror_doc_block(
                    0,
                    2,
                    "",
                    "/*",
                    "<pre><code class=\"language-foo\">\n\n</code></pre>\n"
                ),
                build_codemirror_doc_block(2, 3, "", "//", "<p>Test</p>\n"),
            ]
        ))
    );
    // Test multiple unterminated fenced code blocks.
    assert_eq!(
        source_to_codechat_for_web("// ```\n // ~~~", &"cpp".to_string(), false, false),
        TranslationResults::CodeChat(build_codechat_for_web(
            "c_cpp",
            "\n\n",
            vec![
                build_codemirror_doc_block(0, 1, "", "//", "<pre><code>\n</code></pre>\n"),
                build_codemirror_doc_block(1, 2, " ", "//", "<pre><code></code></pre>\n"),
            ]
        ))
    );

    // Test an unterminated HTML block.
    assert_eq!(
        source_to_codechat_for_web("// <foo>\n // Test", &"cpp".to_string(), false, false),
        TranslationResults::CodeChat(build_codechat_for_web(
            "c_cpp",
            "\n\n",
            vec![
                build_codemirror_doc_block(0, 1, "", "//", "<foo>\n"),
                build_codemirror_doc_block(1, 2, " ", "//", "<p>Test</p>\n"),
            ]
        ))
    );

    // Test an unterminated `<pre>` block. Ensure that markdown after this is
    // still parsed.
    assert_eq!(
        source_to_codechat_for_web("// <pre>\n // *Test*", &"cpp".to_string(), false, false),
        TranslationResults::CodeChat(build_codechat_for_web(
            "c_cpp",
            "\n\n",
            vec![
                build_codemirror_doc_block(0, 1, "", "//", "<pre>\n\n"),
                build_codemirror_doc_block(1, 2, " ", "//", "<p><em>Test</em></p>\n"),
            ]
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

fn apply_str_diff(before: &str, diffs: &[StringDiff]) -> String {
    let mut before = before.to_string();
    // Walk from the last diff to the first.
    for diff in diffs.iter().rev() {
        // Convert from a character index to a byte index. If the index is past
        // the end of the string, report the length of the string.
        let from_index = before
            .char_indices()
            .nth(diff.from)
            .unwrap_or((before.len(), 'x'))
            .0;
        if let Some(to) = diff.to {
            let to_index = before
                .char_indices()
                .nth(to)
                .unwrap_or((before.len(), 'x'))
                .0;
            before.replace_range(from_index..to_index, &diff.insert);
        } else {
            before.insert_str(diff.from, &diff.insert);
        };
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
            insert: "".to_string(),
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
            insert: "".to_string(),
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
            insert: "".to_string(),
        }],
    );

    // Test with unicode.
    test_diff(
        "①\n②③④\n⑤⑥",
        "①\n❷❸\n⑤⑥",
        &[StringDiff {
            from: 2,
            to: Some(6),
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
                from_new: 10,
                to: 12,
                indent: None,
                delimiter: "#".to_string(),
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
                from_new: 10,
                to: 11,
                indent: Some(" ".to_string()),
                delimiter: "#".to_string(),
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
                from_new: 10,
                to: 11,
                indent: None,
                delimiter: "*".to_string(),
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
                from_new: 10,
                to: 11,
                indent: None,
                delimiter: "#".to_string(),
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
            indent: "".to_string(),
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
            CodeMirrorDocBlockTransaction::Add(CodeMirrorDocBlock {
                from: 11,
                to: 12,
                indent: "".to_string(),
                delimiter: "#".to_string(),
                contents: "test".to_string()
            }),
            CodeMirrorDocBlockTransaction::Update(CodeMirrorDocBlockUpdate {
                from: 11,
                from_new: 10,
                to: 11,
                indent: None,
                delimiter: "#".to_string(),
                contents: vec![]
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
            indent: "".to_string(),
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
            indent: "".to_string(),
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
            CodeMirrorDocBlockDelete { from: 10, to: 11 }
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
            CodeMirrorDocBlockDelete { from: 11, to: 12 }
        )]
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
            CodeMirrorDocBlockDelete { from: 11, to: 12 }
        )]
    );
}
