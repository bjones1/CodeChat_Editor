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
//! `test.rs` -- Unit tests for the lexer
//! =====================================
// Imports
// -------
use super::supported_languages::get_language_lexer_vec;
use super::{CodeDocBlock, DocBlock, LanguageLexerCompiled, compile_lexers, source_lexer};
use indoc::indoc;
use pretty_assertions::assert_eq;
use test_utils::test_utils::stringit;

// Utilities
// ---------
//
// Provide a compact way to create a `CodeDocBlock` whose line metadata was
// stripped by `lex`.
fn build_doc_block(indent: &str, delimiter: &str, contents: &str) -> CodeDocBlock {
    build_doc_block_with_lines(indent, delimiter, contents, 0, &[])
}

// Create a `CodeDocBlock` including its line metadata.
fn build_doc_block_with_lines(
    indent: &str,
    delimiter: &str,
    contents: &str,
    // The height of the doc block's source span.
    lines: usize,
    // The doc block's `contents_line_offsets`.
    contents_line_offsets: &[usize],
) -> CodeDocBlock {
    CodeDocBlock::DocBlock(DocBlock {
        indent: indent.to_string(),
        delimiter: delimiter.to_string(),
        contents: contents.to_string(),
        lines,
        contents_line_offsets: contents_line_offsets.to_vec(),
    })
}

fn build_code_block(contents: &str) -> CodeDocBlock {
    CodeDocBlock::CodeBlock(contents.to_string())
}

/// Assert that the line metadata of `code_doc_blocks` agrees with the `source`
/// they were lexed from, per
/// [Source line offsets](../processing/line-number-tags-spec.md#cc-dl60mkSR4A).
/// Removing delimiters and indents only strips text from a source line, so
/// each content line's trimmed text must appear in the source line it maps
/// to. Shared with the Pest parser's tests.
pub(super) fn check_line_metadata(
    // The source code, before line-ending normalization.
    source: &str,
    // The result of lexing `source`.
    code_doc_blocks: &[CodeDocBlock],
) {
    let normalized_source = source.replace("\r\n", "\n").replace('\r', "\n");
    let source_lines: Vec<&str> = normalized_source.split_inclusive('\n').collect();
    // The 0-based source line on which the current block begins.
    let mut block_start = 0;
    for code_doc_block in code_doc_blocks {
        match code_doc_block {
            CodeDocBlock::CodeBlock(code) => block_start += code.split_inclusive('\n').count(),
            CodeDocBlock::DocBlock(doc_block) => {
                let offsets = &doc_block.contents_line_offsets;
                let content_lines: Vec<&str> = doc_block.contents.split_inclusive('\n').collect();
                assert_eq!(offsets.len(), content_lines.len(), "{doc_block:?}");
                assert!(
                    offsets.windows(2).all(|pair| pair[0] < pair[1]),
                    "{doc_block:?}"
                );
                for (content_line, offset) in content_lines.iter().zip(offsets) {
                    assert!(*offset < doc_block.lines, "{doc_block:?}");
                    let source_line = source_lines[block_start + offset];
                    assert!(
                        source_line.contains(content_line.trim()),
                        "Content line {content_line:?} isn't in source line {source_line:?}."
                    );
                }
                block_start += doc_block.lines;
            }
        }
    }
    assert_eq!(block_start, source_lines.len());
}

/// Lex `source`, check its line metadata, then strip that metadata so the
/// result can be compared with blocks from `build_doc_block`.
fn lex(source: &str, language_lexer_compiled: &LanguageLexerCompiled) -> Vec<CodeDocBlock> {
    let mut code_doc_blocks = source_lexer(source, language_lexer_compiled);
    check_line_metadata(source, &code_doc_blocks);
    for code_doc_block in &mut code_doc_blocks {
        if let CodeDocBlock::DocBlock(doc_block) = code_doc_block {
            doc_block.lines = 0;
            doc_block.contents_line_offsets.clear();
        }
    }
    code_doc_blocks
}

// ### Source lexer tests
#[test]
fn test_py() {
    let llc = compile_lexers(get_language_lexer_vec());
    let py = llc.map_mode_to_lexer.get(&stringit("python")).unwrap();

    // Try basic cases: make sure than newlines are processed correctly.
    assert_eq!(lex("", py), []);
    assert_eq!(lex("\n", py), [build_code_block("\n")]);
    assert_eq!(lex("\r", py), [build_code_block("\n")]);
    assert_eq!(lex("\r\n", py), [build_code_block("\n")]);

    // Look at a code to doc transition, checking various newline combos.
    assert_eq!(
        lex("\n# Test", py),
        [build_code_block("\n"), build_doc_block("", "#", "Test")]
    );
    assert_eq!(
        lex("\n# Test\n", py),
        [build_code_block("\n"), build_doc_block("", "#", "Test\n")]
    );
    assert_eq!(
        lex("\n# Test\n\n", py),
        [
            build_code_block("\n"),
            build_doc_block("", "#", "Test\n"),
            build_code_block("\n"),
        ]
    );

    // Source followed by a comment.
    assert_eq!(
        lex("a = 1\n# Test", py),
        [
            build_code_block("a = 1\n"),
            build_doc_block("", "#", "Test")
        ]
    );

    // Comments that aren't in doc blocks.
    assert_eq!(lex("a = 1 # Test", py), [build_code_block("a = 1 # Test"),]);
    assert_eq!(
        lex("\na = 1 # Test", py),
        [build_code_block("\na = 1 # Test"),]
    );
    assert_eq!(
        lex("a = 1 # Test\n", py),
        [build_code_block("a = 1 # Test\n"),]
    );
    assert_eq!(lex("#Test\n", py), [build_code_block("#Test\n"),]);

    // Doc blocks
    assert_eq!(lex("#", py), [build_doc_block("", "#", ""),]);
    assert_eq!(lex("#\n", py), [build_doc_block("", "#", "\n"),]);
    assert_eq!(lex("  # Test", py), [build_doc_block("  ", "#", "Test")]);
    assert_eq!(
        lex("  # Test\n", py),
        [build_doc_block("  ", "#", "Test\n")]
    );
    assert_eq!(
        lex("\n  # Test", py),
        [build_code_block("\n"), build_doc_block("  ", "#", "Test")]
    );
    assert_eq!(
        lex("# Test1\n # Test2", py),
        [
            build_doc_block("", "#", "Test1\n"),
            build_doc_block(" ", "#", "Test2")
        ]
    );

    // Doc blocks with empty comments
    assert_eq!(
        lex("# Test 1\n#\n# Test 2", py),
        [build_doc_block("", "#", "Test 1\n\nTest 2"),]
    );
    assert_eq!(
        lex("  # Test 1\n  #\n  # Test 2", py),
        [build_doc_block("  ", "#", "Test 1\n\nTest 2"),]
    );

    // Single-line strings
    assert_eq!(lex("''", py), [build_code_block("''"),]);
    // An unterminated string before EOF.
    assert_eq!(lex("'", py), [build_code_block("'"),]);
    assert_eq!(lex("\"\"", py), [build_code_block("\"\""),]);
    assert_eq!(lex("a = 'test'\n", py), [build_code_block("a = 'test'\n"),]);
    // Terminate a string with a newline
    assert_eq!(lex("a = 'test\n", py), [build_code_block("a = 'test\n"),]);
    assert_eq!(lex(r"'\''", py), [build_code_block(r"'\''"),]);
    assert_eq!(lex("'\\\n'", py), [build_code_block("'\\\n'"),]);
    // This is `\\` followed by a newline, which terminates the string early
    // (syntax error -- unescaped newline in a single-line string).
    assert_eq!(
        lex("'\\\\\n# Test'", py),
        [
            build_code_block("'\\\\\n"),
            build_doc_block("", "#", "Test'")
        ]
    );
    // This is `\\\` followed by a newline, which puts a `\` followed by a
    // newline in the string, so there's no comment.
    assert_eq!(
        lex("'\\\\\\\n# Test'", py),
        [build_code_block("'\\\\\\\n# Test'"),]
    );
    assert_eq!(lex("'\\\n# Test'", py), [build_code_block("'\\\n# Test'"),]);
    assert_eq!(
        lex("'\n# Test'", py),
        [build_code_block("'\n"), build_doc_block("", "#", "Test'")]
    );

    // Multi-line strings
    assert_eq!(
        lex("'''\n# Test'''", py),
        [build_code_block("'''\n# Test'''"),]
    );
    assert_eq!(
        lex("\"\"\"\n#Test\"\"\"", py),
        [build_code_block("\"\"\"\n#Test\"\"\""),]
    );
    assert_eq!(
        lex("\"\"\"Test 1\n\"\"\"\n# Test 2", py),
        [
            build_code_block("\"\"\"Test 1\n\"\"\"\n"),
            build_doc_block("", "#", "Test 2")
        ]
    );
    // Quotes nested inside a multi-line string.
    assert_eq!(
        lex("'''\n# 'Test' 1'''\n# Test 2", py),
        [
            build_code_block("'''\n# 'Test' 1'''\n"),
            build_doc_block("", "#", "Test 2")
        ]
    );
    // An empty string, follow by a comment which ignores the fake multi-line
    // string.
    assert_eq!(
        lex("''\n# Test 1'''\n# Test 2", py),
        [
            build_code_block("''\n"),
            build_doc_block("", "#", "Test 1'''\nTest 2")
        ]
    );
    assert_eq!(
        lex("'''\n# Test 1\\'''\n# Test 2", py),
        [build_code_block("'''\n# Test 1\\'''\n# Test 2"),]
    );
    assert_eq!(
        lex("'''\n# Test 1\\\\'''\n# Test 2", py),
        [
            build_code_block("'''\n# Test 1\\\\'''\n"),
            build_doc_block("", "#", "Test 2")
        ]
    );
    assert_eq!(
        lex("'''\n# Test 1\\\\\\'''\n# Test 2", py),
        [build_code_block("'''\n# Test 1\\\\\\'''\n# Test 2"),]
    );
}

#[test]
fn test_js() {
    let llc = compile_lexers(get_language_lexer_vec());
    let js = llc.map_mode_to_lexer.get(&stringit("javascript")).unwrap();

    // JavaScript tests.
    //
    // A simple inline comment.
    assert_eq!(lex("// Test", js), [build_doc_block("", "//", "Test"),]);

    // An empty block comment.
    assert_eq!(lex("/* */", js), [build_doc_block("", "/*", ""),]);
    assert_eq!(lex("/*\n*/", js), [build_doc_block("", "/*", ""),]);

    // basic test
    assert_eq!(
        lex("/* Basic Test */", js),
        [build_doc_block("", "/*", "Basic Test"),]
    );

    // no space after opening delimiter (criteria 1)
    assert_eq!(lex("/*Test */", js), [build_code_block("/*Test */"),]);

    // no space after closing delimiter
    assert_eq!(lex("/* Test*/", js), [build_doc_block("", "/*", "Test"),]);

    // extra spaces after opening delimiter (ok, drop 1)
    assert_eq!(
        lex("/*   Extra Space */", js),
        [build_doc_block("", "/*", "  Extra Space"),]
    );

    // code before opening delimiter (criteria 2)
    assert_eq!(
        lex("a = 1 /* Code Before */", js),
        [build_code_block("a = 1 /* Code Before */"),]
    );

    // 4 spaces before opening delimiter (criteria 2 ok)
    assert_eq!(
        lex("    /* Space Before */", js),
        [build_doc_block("    ", "/*", "Space Before"),]
    );

    // newline in comment
    assert_eq!(
        lex("/* Newline\nIn Comment */", js),
        [build_doc_block("", "/*", "Newline\nIn Comment"),]
    );

    // 3 trailing whitespaces (criteria 3 ok)
    assert_eq!(
        lex("/* Trailing Whitespace  */  ", js),
        [build_doc_block("", "/*", "Trailing Whitespace   "),]
    );

    // code after closing delimiter (criteria 3)
    assert_eq!(
        lex("/* Code After */ a = 1", js),
        [build_code_block("/* Code After */ a = 1"),]
    );

    // Another important case:
    assert_eq!(
        lex("/* Another Important Case */\n", js),
        [build_doc_block("", "/*", "Another Important Case\n"),]
    );

    // No closing delimiter
    assert_eq!(
        lex("/* No Closing Delimiter", js),
        [build_code_block("/* No Closing Delimiter"),]
    );

    // Two closing delimiters
    assert_eq!(
        lex("/* Two Closing Delimiters */ \n */", js),
        [
            build_doc_block("", "/*", "Two Closing Delimiters \n"),
            build_code_block(" */"),
        ]
    );
    // Code before a block comment.
    assert_eq!(
        lex("bears();\n/* Bears */\n", js),
        [
            build_code_block("bears();\n"),
            build_doc_block("", "/*", "Bears\n"),
        ]
    );

    // A newline after the opening comment delimiter.
    assert_eq!(
        lex("test_1();\n/*\nTest 2\n*/", js),
        [
            build_code_block("test_1();\n"),
            build_doc_block("", "/*", "Test 2\n"),
        ]
    );

    // Indented block comments.
    assert_eq!(
        lex(
            r"test_1();
/* Test
   2 */",
            js
        ),
        [
            build_code_block("test_1();\n"),
            build_doc_block("", "/*", "Test\n2"),
        ]
    );

    assert_eq!(
        lex(
            r"test_1();
  /* Test
     2 */",
            js
        ),
        [
            build_code_block("test_1();\n"),
            build_doc_block("  ", "/*", "Test\n2"),
        ]
    );

    assert_eq!(
        lex(
            r"test_1();
/* Test
   2
 */",
            js
        ),
        [
            build_code_block("test_1();\n"),
            build_doc_block("", "/*", "Test\n2\n"),
        ]
    );

    assert_eq!(
        lex(
            r"test_1();
  /* Test
     2
   */",
            js
        ),
        [
            build_code_block("test_1();\n"),
            build_doc_block("  ", "/*", "Test\n2\n"),
        ]
    );

    assert_eq!(
        lex(
            r"test_1();
  /* Test
     2

     3
   */",
            js
        ),
        [
            build_code_block("test_1();\n"),
            build_doc_block("  ", "/*", "Test\n2\n\n3\n"),
        ]
    );

    // Mis-indented block comments.
    assert_eq!(
        lex(
            r"test_1();
/* Test
  2 */",
            js
        ),
        [
            build_code_block("test_1();\n"),
            build_doc_block("", "/*", "Test\n  2"),
        ]
    );

    assert_eq!(
        lex(
            r"test_1();
 /* Test
   2 */",
            js
        ),
        [
            build_code_block("test_1();\n"),
            build_doc_block(" ", "/*", "Test\n   2"),
        ]
    );

    // Some basic template literal tests. Comments inside template literal
    // expressions aren't parsed correctly; neither are nested template
    // literals.
    assert_eq!(lex("``", js), [build_code_block("``"),]);
    assert_eq!(lex("`", js), [build_code_block("`"),]);
    assert_eq!(lex("`\n// Test`", js), [build_code_block("`\n// Test`"),]);
    assert_eq!(
        lex("`\\`\n// Test`", js),
        [build_code_block("`\\`\n// Test`"),]
    );
    assert_eq!(
        lex("`\n// Test 1`\n// Test 2", js),
        [
            build_code_block("`\n// Test 1`\n"),
            build_doc_block("", "//", "Test 2")
        ]
    );
    assert_eq!(
        lex("`\n// Test 1\\`\n// Test 2`\n// Test 3", js),
        [
            build_code_block("`\n// Test 1\\`\n// Test 2`\n"),
            build_doc_block("", "//", "Test 3")
        ]
    );
}

#[ignore = "Re-enable this test where heredocs are supported."]
#[test]
fn test_cpp() {
    let llc = compile_lexers(get_language_lexer_vec());
    let cpp = llc.map_mode_to_lexer.get(&stringit("cpp")).unwrap();

    // Try out a C++ heredoc.
    assert_eq!(
        lex("R\"heredoc(\n// Test 1)heredoc\"\n// Test 2", cpp),
        [
            build_code_block("R\"heredoc(\n// Test 1)heredoc\"\n"),
            build_doc_block("", "//", "Test 2")
        ]
    );
}

#[test]
fn test_c() {
    let llc = compile_lexers(get_language_lexer_vec());
    let c = llc.map_mode_to_lexer.get(&stringit("cpp")).unwrap();

    // Test logical lines.
    let s = indoc!(
        r#"
        "A string that\
        // spans two lines but is not a comment."
        "#
    );
    assert_eq!(lex(s, c), [build_code_block(s),]);

    // Test other line endings.
    assert_eq!(lex("a\r\nb\rc", c), [build_code_block("a\nb\nc"),]);
    assert_eq!(
        lex("// a\r\n//\r// c", c),
        [build_doc_block("", "//", "a\n\nc"),]
    );

    // Test end of file cases.
    assert_eq!(lex("// a", c), [build_doc_block("", "//", "a"),]);
    assert_eq!(lex("/* a */", c), [build_doc_block("", "/*", "a"),]);
}

#[test]
fn test_csharp() {
    let llc = compile_lexers(get_language_lexer_vec());
    let csharp = llc.map_mode_to_lexer.get(&stringit("csharp")).unwrap();

    // Try out a verbatim string literal with embedded double quotes.
    assert_eq!(
        lex("// Test 1\n@\"\n// Test 2\"\"\n// Test 3\"", csharp),
        [
            build_doc_block("", "//", "Test 1\n"),
            build_code_block("@\"\n// Test 2\"\"\n// Test 3\"")
        ]
    );
}

#[test]
fn test_matlab() {
    let llc = compile_lexers(get_language_lexer_vec());
    let matlab = llc.map_mode_to_lexer.get(&stringit("matlab")).unwrap();

    // Test both inline comment styles. Verify that escaped quotes are ignored,
    // and that doubled quotes are handled correctly.
    assert_eq!(
        lex(
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
        lex(
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
            // TODO: currently, whitespace on the line containing the closing
            // block delimiter isn't captured. Fix this.
            build_doc_block("  ", "%{", "a = 2\n"),
        ]
    );
}

#[test]
fn test_rust() {
    let llc = compile_lexers(get_language_lexer_vec());
    let rust = llc.map_mode_to_lexer.get(&stringit("rust")).unwrap();

    // Test Rust raw strings.
    assert_eq!(
        lex("r###\"\n// Test 1\"###\n// Test 2", rust),
        [
            build_code_block("r###\"\n// Test 1\"###\n"),
            build_doc_block("", "//", "Test 2")
        ]
    );

    // Test Rust comments, which can be nested but aren't here. TODO: test
    // nested comments.
    assert_eq!(
        lex("test_1();\n/* Test 2 */\n", rust),
        [
            build_code_block("test_1();\n"),
            build_doc_block("", "/*", "Test 2\n")
        ]
    );

    assert_eq!(
        lex(
            r" /* Depth 1
  /* Depth 2 comment */
  /* Depth 2
    /* Depth 3 */ */
  /* Depth 2
    /* Depth 3 comment */
   */
More depth 1 */",
            rust
        ),
        [
            build_code_block(" /* Depth 1\n"),
            build_doc_block("  ", "/*", "Depth 2 comment\n"),
            build_code_block(
                r"  /* Depth 2
    /* Depth 3 */ */
  /* Depth 2
"
            ),
            build_doc_block("    ", "/*", "Depth 3 comment\n"),
            build_code_block(
                r"   */
More depth 1 */"
            ),
        ]
    );
}

#[test]
fn test_sql() {
    let llc = compile_lexers(get_language_lexer_vec());
    let sql = llc.map_mode_to_lexer.get(&stringit("sql")).unwrap();

    // Test strings with embedded single quotes.
    assert_eq!(
        lex("-- Test 1\n'\n-- Test 2''\n-- Test 3'", sql),
        [
            build_doc_block("", "--", "Test 1\n"),
            build_code_block("'\n-- Test 2''\n-- Test 3'")
        ]
    );
}

#[test]
fn test_swift() {
    let llc = compile_lexers(get_language_lexer_vec());
    let swift = llc.map_mode_to_lexer.get(&stringit("swift")).unwrap();

    // Test comments.
    assert_eq!(
        lex(" // An inline comment\nsome_code()", swift),
        [
            build_doc_block(" ", "//", "An inline comment\n"),
            build_code_block("some_code()")
        ]
    );
    assert_eq!(
        lex("  /* A block comment */\nsome_code()", swift),
        [
            build_doc_block("  ", "/*", "A block comment\n"),
            build_code_block("some_code()")
        ]
    );

    // Test strings.
    assert_eq!(
        lex(
            r#"// Test 1
foo("// a string\"")"#,
            swift
        ),
        [
            build_doc_block("", "//", "Test 1\n"),
            build_code_block(r#"foo("// a string\"")"#)
        ]
    );

    assert_eq!(
        lex(
            r#"// Test 1
foo("""
// Test 2
)""""#,
            swift
        ),
        [
            build_doc_block("", "//", "Test 1\n"),
            build_code_block(
                r#"foo("""
// Test 2
)""""#
            )
        ]
    );

    // Test extended string delimiters for a string literal.
    assert_eq!(
        lex(
            r##"// Test 1
foo(#"""
// Test 2
"""
// Test 3
)"""#"##,
            swift
        ),
        [
            build_doc_block("", "//", "Test 1\n"),
            build_code_block(
                r##"foo(#"""
// Test 2
"""
// Test 3
)"""#"##
            )
        ]
    );
}

#[test]
fn test_toml() {
    let llc = compile_lexers(get_language_lexer_vec());
    let toml = llc.map_mode_to_lexer.get(&stringit("toml")).unwrap();
    assert_eq!(toml.language_lexer.lexer_name.as_str(), "toml");

    // Multi-line literal strings don't have escapes.
    assert_eq!(
        lex("'''\n# Test 1\\'''\n# Test 2", toml),
        [
            build_code_block("'''\n# Test 1\\'''\n"),
            build_doc_block("", "#", "Test 2")
        ]
    );
    // Basic strings have an escape, but don't allow newlines.
    assert_eq!(
        lex("\"\\\n# Test 1\"", toml),
        [
            build_code_block("\"\\\n"),
            build_doc_block("", "#", "Test 1\"")
        ]
    );
}

// ### Line metadata tests
//
// Check source heights and content-line maps against the original source
// spans, using the regex lexer (JavaScript) and the Pest parser (C/C++, Python).
fn assert_lexes_with_lines(
    source: &str,
    language_lexer_compiled: &LanguageLexerCompiled,
    expected: &[CodeDocBlock],
) {
    let code_doc_blocks = source_lexer(source, language_lexer_compiled);
    check_line_metadata(source, &code_doc_blocks);
    assert_eq!(code_doc_blocks, expected);
}

#[test]
fn test_line_metadata_regex_lexer() {
    let llc = compile_lexers(get_language_lexer_vec());
    let js = llc.map_mode_to_lexer.get(&stringit("javascript")).unwrap();

    // A delimiter-only opening line leaves a gap before the first content
    // line; the blank content line is supplied by the closing delimiter's
    // line. `B` is therefore on source line 4.
    assert_lexes_with_lines(
        "/*\nA\n*/\n// B\n",
        js,
        &[
            build_doc_block_with_lines("", "/*", "A\n\n", 3, &[1, 2]),
            build_doc_block_with_lines("", "//", "B\n", 1, &[0]),
        ],
    );

    // Adjacent block comments merge; the second comment's offsets follow the
    // first's full height, placing `First` and `Second` on lines 2 and 5.
    assert_lexes_with_lines(
        "/*\nFirst\n*/\n/*\nSecond\n*/\n",
        js,
        &[build_doc_block_with_lines(
            "",
            "/*",
            "First\n\nSecond\n\n",
            6,
            &[1, 2, 4, 5],
        )],
    );

    // Empty comments. Each still occupies its full source height; a comment
    // with empty contents has an empty map.
    assert_lexes_with_lines(
        "//\n/* */\n/*\n*/\ncode;\n//",
        js,
        &[
            build_doc_block_with_lines("", "//", "\n", 1, &[0]),
            build_doc_block_with_lines("", "/*", "\n\n", 3, &[0, 2]),
            build_code_block("code;\n"),
            build_doc_block_with_lines("", "//", "", 1, &[]),
        ],
    );
    assert_lexes_with_lines(
        "/*\n*/",
        js,
        &[build_doc_block_with_lines("", "/*", "", 2, &[])],
    );

    // A final doc block without a trailing newline still occupies its last
    // line, including when dedenting empties that line.
    assert_lexes_with_lines(
        "code;\n/* a\n   b */",
        js,
        &[
            build_code_block("code;\n"),
            build_doc_block_with_lines("", "/*", "a\nb", 2, &[0, 1]),
        ],
    );
    assert_lexes_with_lines(
        "/* a\n */",
        js,
        &[build_doc_block_with_lines("", "/*", "a\n", 2, &[0])],
    );
    assert_lexes_with_lines(
        "// a\n// b",
        js,
        &[build_doc_block_with_lines("", "//", "a\nb", 2, &[0, 1])],
    );

    // Heights are measured after line-ending normalization.
    assert_lexes_with_lines(
        "/*\r\nA\r*/\r\n",
        js,
        &[build_doc_block_with_lines("", "/*", "A\n\n", 3, &[1, 2])],
    );
}

#[test]
fn test_line_metadata_pest_parser() {
    let llc = compile_lexers(get_language_lexer_vec());
    let c = llc.map_mode_to_lexer.get(&stringit("cpp")).unwrap();
    let py = llc.map_mode_to_lexer.get(&stringit("python")).unwrap();

    // The Pest parser keeps the newline after a bare opening delimiter as a
    // blank content line, supplied by that delimiter's line. `B` is still on
    // source line 4.
    assert_lexes_with_lines(
        "/*\nA\n*/\n// B\n",
        c,
        &[
            build_doc_block_with_lines("", "/*", "\nA\n\n", 3, &[0, 1, 2]),
            build_doc_block_with_lines("", "//", "B\n", 1, &[0]),
        ],
    );

    // The Pest parser doesn't merge adjacent block comments; `First` and
    // `Second` are still on lines 2 and 5.
    assert_lexes_with_lines(
        "/*\nFirst\n*/\n/*\nSecond\n*/\n",
        c,
        &[
            build_doc_block_with_lines("", "/*", "\nFirst\n\n", 3, &[0, 1, 2]),
            build_doc_block_with_lines("", "/*", "\nSecond\n\n", 3, &[0, 1, 2]),
        ],
    );

    // Empty comments.
    assert_lexes_with_lines(
        "//\n/* */\ncode;\n//",
        c,
        &[
            build_doc_block_with_lines("", "//", "\n", 1, &[0]),
            build_doc_block_with_lines("", "/*", "\n", 1, &[0]),
            build_code_block("code;\n"),
            build_doc_block_with_lines("", "//", "", 1, &[]),
        ],
    );
    assert_lexes_with_lines("#", py, &[build_doc_block_with_lines("", "#", "", 1, &[])]);

    // Final doc blocks without a trailing newline.
    assert_lexes_with_lines(
        "code;\n/* a\n   b */",
        c,
        &[
            build_code_block("code;\n"),
            build_doc_block_with_lines("", "/*", "a\nb", 2, &[0, 1]),
        ],
    );
    assert_lexes_with_lines(
        "/* a\n */",
        c,
        &[build_doc_block_with_lines("", "/*", "a\n", 2, &[0])],
    );
    assert_lexes_with_lines(
        "# a\n#\n# b",
        py,
        &[build_doc_block_with_lines("", "#", "a\n\nb", 3, &[0, 1, 2])],
    );
}

// ### Compiler tests
#[test]
fn test_compiler() {
    let llc = compile_lexers(get_language_lexer_vec());

    let c_ext_lexer_arr = llc.map_ext_to_lexer_vec.get(&stringit("c")).unwrap();
    assert_eq!(c_ext_lexer_arr.len(), 1);
    assert_eq!(c_ext_lexer_arr[0].language_lexer.lexer_name.as_str(), "cpp");
    assert_eq!(
        llc.map_mode_to_lexer
            .get(&stringit("verilog"))
            .unwrap()
            .language_lexer
            .lexer_name
            .as_str(),
        "verilog"
    );
}

#[test]
fn test_utf8_indent_no_panic() {
    // Regression test: a multi-byte UTF-8 character in a block comment's
    // continuation line must not cause a byte-index char-boundary panic when
    // computing/removing the comment's indentation.
    let llc = compile_lexers(get_language_lexer_vec());
    // Use SQL, since (unlike C/C++) it's lexed by the regex-based lexer
    // rather than the PEG-based parser.
    let sql = llc.map_mode_to_lexer.get(&stringit("sql")).unwrap();
    let s = "/* Test\n \u{65E5}\u{672C}\u{8A9E} text\n*/\n";
    // This must not panic; also verify the contents are preserved intact
    // (since the multi-byte character prevents this comment from being
    // recognized as consistently indented, it should be left unchanged).
    assert_eq!(
        lex(s, sql),
        [build_doc_block(
            "",
            "/*",
            "Test\n \u{65E5}\u{672C}\u{8A9E} text\n\n"
        )]
    );
}
