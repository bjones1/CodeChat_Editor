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
/// # `test.rs` -- Unit tests for the lexer
// ## Imports
use super::supported_languages::get_language_lexer_vec;
use super::{compile_lexers, source_lexer, CodeDocBlock, DocBlock};

// ## Utilities
//
// Provide a compact way to create a `CodeDocBlock`.
fn build_doc_block(indent: &str, delimiter: &str, contents: &str) -> CodeDocBlock {
    return CodeDocBlock::DocBlock(DocBlock {
        indent: indent.to_string(),
        delimiter: delimiter.to_string(),
        contents: contents.to_string(),
        lines: contents.matches("\n").count()
            + (if contents.chars().last().unwrap_or('\n') == '\n' {
                0
            } else {
                1
            }),
    });
}

fn build_code_block(contents: &str) -> CodeDocBlock {
    return CodeDocBlock::CodeBlock(contents.to_string());
}

// ### Source lexer tests
#[test]
fn test_py() {
    let llc = compile_lexers(get_language_lexer_vec());
    let py = llc.map_mode_to_lexer.get(&"python".to_string()).unwrap();

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
    // An empty string, follow by a comment which ignores the fake multi-line
    // string.
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
    let llc = compile_lexers(get_language_lexer_vec());
    let js = llc
        .map_mode_to_lexer
        .get(&"javascript".to_string())
        .unwrap();

    // JavaScript tests.
    //
    // A simple inline comment.
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

    // Indented block comments.
    assert_eq!(
        source_lexer(
            r#"test_1();
/* Test
   2 */"#,
            js
        ),
        [
            build_code_block("test_1();\n"),
            build_doc_block("", "/*", "Test\n2"),
        ]
    );

    assert_eq!(
        source_lexer(
            r#"test_1();
  /* Test
     2 */"#,
            js
        ),
        [
            build_code_block("test_1();\n"),
            build_doc_block("  ", "/*", "Test\n2"),
        ]
    );

    assert_eq!(
        source_lexer(
            r#"test_1();
/* Test
   2
 */"#,
            js
        ),
        [
            build_code_block("test_1();\n"),
            build_doc_block("", "/*", "Test\n2\n"),
        ]
    );

    assert_eq!(
        source_lexer(
            r#"test_1();
  /* Test
     2
   */"#,
            js
        ),
        [
            build_code_block("test_1();\n"),
            build_doc_block("  ", "/*", "Test\n2\n"),
        ]
    );

    assert_eq!(
        source_lexer(
            r#"test_1();
  /* Test
     2

     3
   */"#,
            js
        ),
        [
            build_code_block("test_1();\n"),
            build_doc_block("  ", "/*", "Test\n2\n\n3\n"),
        ]
    );

    // Mis-indented block comments.
    assert_eq!(
        source_lexer(
            r#"test_1();
/* Test
  2 */"#,
            js
        ),
        [
            build_code_block("test_1();\n"),
            build_doc_block("", "/*", "Test\n  2"),
        ]
    );

    assert_eq!(
        source_lexer(
            r#"test_1();
 /* Test
   2 */"#,
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
    let llc = compile_lexers(get_language_lexer_vec());
    let cpp = llc.map_mode_to_lexer.get(&"c_cpp".to_string()).unwrap();

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
    let llc = compile_lexers(get_language_lexer_vec());
    let csharp = llc.map_mode_to_lexer.get(&"csharp".to_string()).unwrap();

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
    let llc = compile_lexers(get_language_lexer_vec());
    let matlab = llc.map_mode_to_lexer.get(&"matlab".to_string()).unwrap();

    // Test both inline comment styles. Verify that escaped quotes are ignored,
    // and that doubled quotes are handled correctly.
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
            // TODO: currently, whitespace on the line containing the closing
            // block delimiter isn't captured. Fix this.
            build_doc_block("  ", "%{", "a = 2\n"),
        ]
    );
}

#[test]
fn test_rust() {
    let llc = compile_lexers(get_language_lexer_vec());
    let rust = llc.map_mode_to_lexer.get(&"rust".to_string()).unwrap();

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

    assert_eq!(
        source_lexer(
            r#"/* Depth 1
  /* Depth 2 comment */
  /* Depth 2
    /* Depth 3 */ */
More depth 1 */"#,
            rust
        ),
        [
            build_code_block("/* Depth 1\n"),
            build_doc_block("  ", "/*", "Depth 2 comment\n"),
            build_code_block(
                r#"  /* Depth 2
    /* Depth 3 */ */
More depth 1 */"#
            ),
        ]
    );
}

#[test]
fn test_sql() {
    let llc = compile_lexers(get_language_lexer_vec());
    let sql = llc.map_mode_to_lexer.get(&"sql".to_string()).unwrap();

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
fn test_swift() {
    let llc = compile_lexers(get_language_lexer_vec());
    let swift = llc.map_mode_to_lexer.get(&"swift".to_string()).unwrap();

    // Test comments.
    assert_eq!(
        source_lexer(" // An inline comment\nsome_code()", swift),
        [
            build_doc_block(" ", "//", "An inline comment\n"),
            build_code_block("some_code()")
        ]
    );
    assert_eq!(
        source_lexer("  /* A block comment */\nsome_code()", swift),
        [
            build_doc_block("  ", "/*", "A block comment\n"),
            build_code_block("some_code()")
        ]
    );

    // Test strings.
    assert_eq!(
        source_lexer(
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
        source_lexer(
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
        source_lexer(
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
    let toml = llc.map_mode_to_lexer.get(&"toml".to_string()).unwrap();
    assert_eq!(toml.language_lexer.lexer_name.as_str(), "toml");

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
    let llc = compile_lexers(get_language_lexer_vec());

    let c_ext_lexer_arr = llc.map_ext_to_lexer_vec.get(&"c".to_string()).unwrap();
    assert_eq!(c_ext_lexer_arr.len(), 1);
    assert_eq!(
        c_ext_lexer_arr[0].language_lexer.lexer_name.as_str(),
        "c_cpp"
    );
    assert_eq!(
        llc.map_mode_to_lexer
            .get(&"verilog".to_string())
            .unwrap()
            .language_lexer
            .lexer_name
            .as_str(),
        "verilog"
    );
}
