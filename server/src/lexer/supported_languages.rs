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
/// # `supported_languages.rs` - Provide lexer info for all supported languages
///
/// This file contains a data structure which describes all supported languages;
/// the [lexer](../lexer.rs) uses this lex a given language.
///
/// ## Lexer implementation
///
/// Ordering matters: all these delimiters end up in a large regex separated by
/// an or operator. The regex or operator matches from left to right. So, longer
/// Python string delimiters must be specified first (leftmost): `"""` (a
/// multi-line Python string) must come before `"`. The resulting regex will
/// then have `"""|"`, which will first search for the multi-line triple quote,
/// then if that's not found, the single quote. A regex of `"|"""` would never
/// match the triple quote, since the single quote would match first.
///
/// Note that the lexers here should be complemented by the appropriate Ace mode
/// in [ace-webpack.mts](../../../client/src/ace-webpack.mts).
///
/// ### <a id="string_delimiter_doubling"></a>String delimiter doubling
///
/// Some languages allow inserting the string delimiter within a string by
/// putting two back-to-back delimiters in the string. For example, SQL's string
/// delimiter is a single quote. To insert a single quote in a string, double
/// it: `'She''s here.'`, for example. From a lexer perspective, we don't need
/// extra logic to handle this; instead, it's treated as two back-to-back
/// strings. In this case, they would be `'She'` and `'s here.'`. While this
/// doesn't parse the string correctly, it does correctly identify where
/// comments can't be, which is all that the lexer needs to do.
// ## Imports
//
// ### Standard library
use std::sync::Arc;

// ### Local
use super::{
    BlockCommentDelim, HeredocDelim, LanguageLexer, NewlineSupport, SpecialCase,
    StringDelimiterSpec,
};

// ## Helper functions
//
// These functions simplify the syntax needed to create a `LanguageLexer`.
fn make_language_lexer(
    lexer_name: &str,
    ext_arr: &[&str],
    inline_comment_delim_arr: &[&str],
    block_comment_delim_arr: &[BlockCommentDelim],
    string_delim_spec_arr: &[StringDelimiterSpec],
    heredoc_delim: Option<HeredocDelim>,
    special_case: SpecialCase,
) -> LanguageLexer {
    LanguageLexer {
        lexer_name: Arc::new(lexer_name.to_string()),
        ext_arr: ext_arr.iter().map(|x| Arc::new(x.to_string())).collect(),
        inline_comment_delim_arr: inline_comment_delim_arr
            .iter()
            .map(|x| x.to_string())
            .collect(),
        block_comment_delim_arr: block_comment_delim_arr.to_vec(),
        string_delim_spec_arr: string_delim_spec_arr.to_vec(),
        heredoc_delim,
        special_case,
    }
}

fn make_string_delimiter_spec(
    delimiter: &str,
    escape_char: &str,
    newline_support: NewlineSupport,
) -> StringDelimiterSpec {
    StringDelimiterSpec {
        delimiter: delimiter.to_string(),
        escape_char: escape_char.to_string(),
        newline_support,
    }
}

fn make_heredoc_delim(
    start_prefix: &str,
    delim_ident_regex: &str,
    start_suffix: &str,
    stop_prefix: &str,
    stop_suffix: &str,
) -> Option<HeredocDelim> {
    Some(HeredocDelim {
        start_prefix: start_prefix.to_string(),
        delim_ident_regex: delim_ident_regex.to_string(),
        start_suffix: start_suffix.to_string(),
        stop_prefix: stop_prefix.to_string(),
        stop_suffix: stop_suffix.to_string(),
    })
}

fn make_block_comment_delim(opening: &str, closing: &str, is_nestable: bool) -> BlockCommentDelim {
    BlockCommentDelim {
        opening: opening.to_string(),
        closing: closing.to_string(),
        is_nestable,
    }
}

// ## Define lexers for each supported language.
pub fn get_language_lexer_vec() -> Vec<LanguageLexer> {
    vec![
        // ### Linux shell scripts
        make_language_lexer(
            "sh",
            &["sh"],
            &["#"],
            &[],
            &[
                make_string_delimiter_spec("\"", "\\", NewlineSupport::Unescaped),
                make_string_delimiter_spec("'", "\\", NewlineSupport::Unescaped),
            ],
            // This doesn't quite match the spec (search for here documents in
            // the bash man page), since it doesn't correctly handle unmatched
            // or mismatched quote; for example, `TODO`.
            make_heredoc_delim("<<-?('|\")?", "\\w+", "('|\")?", "", ""),
            SpecialCase::None,
        ),
        // ### C/C++
        make_language_lexer(
            "c_cpp",
            // Note that the `.ino` extension is for Arduino source files.
            &["c", "cc", "cpp", "h", "hh", "hpp", "ino"],
            &["//"],
            &[make_block_comment_delim("/*", "*/", false)],
            &[make_string_delimiter_spec(
                "\"",
                "\\",
                NewlineSupport::Escaped,
            )],
            // Note: the C/C++ support expects C++11 or newer. Don't worry about
            // supporting C or older C++ using another lexer entry, since the
            // raw string syntax in C++11 and newer is IMHO so rare we won't
            // encounter it in older code. See the C++
            // [string literals docs for the reasoning behind the start body regex.](https://en.cppreference.com/w/cpp/language/string_literal)
            make_heredoc_delim("R\"", "[^()\\\\[[:space:]]]*", "(", ")", "\""),
            SpecialCase::None,
        ),
        // ### C#
        make_language_lexer(
            "csharp",
            &["cs"],
            // See
            // [6.3.3 Comments](https://learn.microsoft.com/en-us/dotnet/csharp/language-reference/language-specification/lexical-structure#633-comments).
            // Also provide support for
            // [documentation comments](https://learn.microsoft.com/en-us/dotnet/csharp/language-reference/language-specification/documentation-comments).
            &["//", "///"],
            &[
                make_block_comment_delim("/*", "*/", false),
                make_block_comment_delim("/**", "*/", false),
            ],
            &[make_string_delimiter_spec(
                // See
                // [6.4.5.6 String literals](https://learn.microsoft.com/en-us/dotnet/csharp/language-reference/language-specification/lexical-structure#6456-string-literals).
                "\"",
                "\\",
                NewlineSupport::None,
            )],
            None,
            SpecialCase::CSharpVerbatimStringLiteral,
        ),
        // ### CSS
        make_language_lexer(
            "css",
            &["css"],
            &[],
            &[make_block_comment_delim("/*", "*/", false)],
            &[
                make_string_delimiter_spec("\"", "\\", NewlineSupport::Unescaped),
                make_string_delimiter_spec("'", "\\", NewlineSupport::Unescaped),
            ],
            None,
            SpecialCase::None,
        ),
        // ### Go
        make_language_lexer(
            "golang",
            &["go"],
            // See
            // [The Go Programming Language Specification](https://go.dev/ref/spec)
            // on [Comments](https://go.dev/ref/spec#Comments).
            &[],
            &[make_block_comment_delim("/*", "*/", false)],
            // See [String literals](https://go.dev/ref/spec#String_literals).
            &[
                make_string_delimiter_spec("\"", "\\", NewlineSupport::None),
                make_string_delimiter_spec("`", "", NewlineSupport::Unescaped),
            ],
            None,
            SpecialCase::None,
        ),
        // ### HTML
        make_language_lexer(
            "html",
            &["html", "htm"],
            &[],
            &[make_block_comment_delim("<!--", "-->", false)],
            &[
                make_string_delimiter_spec("\"", "\\", NewlineSupport::Unescaped),
                make_string_delimiter_spec("'", "\\", NewlineSupport::Unescaped),
            ],
            None,
            SpecialCase::None,
        ),
        // ### Java
        make_language_lexer(
            "java",
            &["java"],
            // See the
            // [Java Language Specification, Java SE 19 edition](https://docs.oracle.com/javase/specs/jls/se19/html/index.html),
            // [§3.7. Comments](https://docs.oracle.com/javase/specs/jls/se19/html/jls-3.html#jls-3.7).
            // The end of this section notes that <q>comments do not occur
            // within character literals, string literals, or text blocks,</q>
            // which describes the approach of this lexer nicely.
            &["//"],
            &[make_block_comment_delim("/*", "*/", false)],
            // See
            // [§3.10.5. String Literals](https://docs.oracle.com/javase/specs/jls/se19/html/jls-3.html#jls-3.10.5).
            &[
                make_string_delimiter_spec(
                    "\"",
                    "\\",
                    // Per the previous link, <q>It is a compile-time error for
                    // a line terminator (§3.4) to appear after the opening "
                    // and before the matching closing "."</q>
                    NewlineSupport::None,
                ),
                // See
                // [§3.10.6. Text Blocks](https://docs.oracle.com/javase/specs/jls/se19/html/jls-3.html#jls-3.10.6).
                make_string_delimiter_spec("\"\"\"", "\\", NewlineSupport::Unescaped),
            ],
            None,
            SpecialCase::None,
        ),
        // ### JavaScript
        make_language_lexer(
            "javascript",
            &[
                "js", "mjs",
                // Note that
                // [Qt's QML language](https://doc.qt.io/qt-6/qtqml-syntax-basics.html)
                // is basically JSON with some embedded JavaScript. Treat it as
                // JavaScript, since those rules include template literals.
                "qml",
            ],
            // See
            // [§12.4 Comments](https://262.ecma-international.org/13.0/#sec-comments)
            &["//"],
            &[make_block_comment_delim("/*", "*/", false)],
            &[
                // See
                // [§12.8.4 String Literals](https://262.ecma-international.org/13.0/#prod-StringLiteral).
                make_string_delimiter_spec("\"", "\\", NewlineSupport::Escaped),
                make_string_delimiter_spec("'", "\\", NewlineSupport::Escaped),
            ],
            None,
            SpecialCase::TemplateLiteral,
        ),
        // ### JSON5
        make_language_lexer(
            "json5",
            &["json"],
            &["//"],
            &[make_block_comment_delim("/*", "*/", false)],
            &[
                make_string_delimiter_spec("\"", "\\", NewlineSupport::Escaped),
                make_string_delimiter_spec("'", "\\", NewlineSupport::Escaped),
            ],
            None,
            SpecialCase::None,
        ),
        // ### MATLAB
        make_language_lexer(
            "matlab",
            &["m"],
            // See the
            // [MATLAB docs on comments](https://www.mathworks.com/help/matlab/matlab_prog/comments.html).
            // Block comments are a special case, so they're not included here.
            &["%", "..."],
            &[],
            // Per the
            // [MATLAB docs](https://www.mathworks.com/help/matlab/matlab_prog/represent-text-with-character-and-string-arrays.html),
            // there are two types of strings. Although MATLAB supports
            // [standard escape sequences](https://www.mathworks.com/help/matlab/matlab_prog/matlab-operators-and-special-characters.html#bvg44q6)
            // (scroll to the bottom of the page), these don't affect quotes;
            // instead, doubled quotes are used to insert a single quote. See
            // [string delimiter doubling](#string_delimiter_doubling).
            &[
                make_string_delimiter_spec("\"", "", NewlineSupport::None),
                make_string_delimiter_spec("'", "", NewlineSupport::None),
            ],
            None,
            SpecialCase::Matlab,
        ),
        // ### Python
        make_language_lexer(
            "python",
            &["py"],
            &["#"],
            &[],
            &[
                // Note that raw strings still allow escaping the single/double
                // quote. See the
                // [language reference](https://docs.python.org/3/reference/lexical_analysis.html#literals).
                make_string_delimiter_spec("\"\"\"", "\\", NewlineSupport::Unescaped),
                make_string_delimiter_spec("'''", "\\", NewlineSupport::Unescaped),
                make_string_delimiter_spec("\"", "\\", NewlineSupport::Escaped),
                make_string_delimiter_spec("'", "\\", NewlineSupport::Escaped),
            ],
            None,
            SpecialCase::None,
        ),
        // ### [Rust](https://doc.rust-lang.org/reference/tokens.html#literals)
        make_language_lexer(
            "rust",
            &["rs"],
            // Support both rustdoc-style comments and plain Rust comments.
            &["///", "//!", "//"],
            &[make_block_comment_delim("/*", "*/", true)],
            &[
                // Byte strings behave like strings for this lexer.
                make_string_delimiter_spec("\"", "\\", NewlineSupport::Unescaped),
            ],
            // Likewise, raw byte strings behave identically to raw strings from
            // this lexer's perspective.
            make_heredoc_delim("r", "#+", "\"", "\"", ""),
            SpecialCase::None,
        ),
        // ### SQL
        make_language_lexer(
            "sql",
            &["sql"],
            // See
            // [Wikipedia](https://en.wikipedia.org/wiki/SQL_syntax#Comments).
            // The
            // [SQL specification isn't free](https://en.wikipedia.org/wiki/SQL#Standardization_history),
            // sadly. Oracle publishes their flavor of the 2016 spec; see
            // [Comments within SQL statements](https://docs.oracle.com/database/121/SQLRF/sql_elements006.htm#SQLRF51099).
            // Postgresql defines
            // [comments](https://www.postgresql.org/docs/15/sql-syntax-lexical.html#SQL-SYNTAX-COMMENTS)
            // as well.
            &["--"],
            &[make_block_comment_delim("/*", "*/", false)],
            &[
                // SQL standard strings allow newlines and don't provide an
                // escape character. This language uses
                // [string delimiter doubling](#string_delimiter_doubling).
                // Unfortunately, each variant of SQL also supports their custom
                // definition of strings; these must be handled by
                // vendor-specific flavors of this basic lexer definition.
                make_string_delimiter_spec("'", "", NewlineSupport::Unescaped),
            ],
            None,
            SpecialCase::None,
        ),
        // ### [Swift](https://docs.swift.org/swift-book/documentation/the-swift-programming-language/)
        make_language_lexer(
            "swift",
            &["swift"],
            // See
            // [comments](https://docs.swift.org/swift-book/documentation/the-swift-programming-language/thebasics#Comments).
            &["//"],
            &[make_block_comment_delim("/*", "*/", true)],
            // See
            // [Strings and Characters](https://docs.swift.org/swift-book/documentation/the-swift-programming-language/stringsandcharacters).
            &[
                // Technically, this would include optional whitespace after the
                // triple quotes then a newlines then end with a newline before
                // the closing triple quotes. However, not doing this is a
                // syntax error, so we ignore this subtlety.
                make_string_delimiter_spec("\"\"\"", "\\", NewlineSupport::Unescaped),
                make_string_delimiter_spec("\"", "\\", NewlineSupport::None),
            ],
            // Swift supports
            // [extended string delimiters](https://docs.swift.org/swift-book/documentation/the-swift-programming-language/stringsandcharacters#Extended-String-Delimiters)
            // in both string literal and multiline string flavors. Since this
            // parser only supports a single heredoc type, we ignore the string
            // literal flavor. This is a bug: consider the string
            // `#"Not a comment "/*"#`. This would parse as a code block
            // containing just `#`, then the string `"Not a comment "` then a
            // comment starting with `/*"#`.
            make_heredoc_delim("", "#+", "\"\"\"", "\"\"\"", ""),
            SpecialCase::None,
        ),
        // ### [TOML](https://toml.io/en/)
        make_language_lexer(
            "toml",
            &["toml"],
            &["#"],
            &[],
            &[
                // Multi-line literal strings (as described by the link above).
                make_string_delimiter_spec("'''", "", NewlineSupport::Unescaped),
                // Multi-line basic strings
                make_string_delimiter_spec("\"\"\"", "\\", NewlineSupport::Unescaped),
                // Basic strings
                make_string_delimiter_spec("\"", "\\", NewlineSupport::None),
                // Literal strings
                make_string_delimiter_spec("'", "\\", NewlineSupport::Escaped),
            ],
            None,
            SpecialCase::None,
        ),
        // ### TypeScript
        make_language_lexer(
            "typescript",
            &["ts", "mts"],
            &["//"],
            &[make_block_comment_delim("/*", "*/", false)],
            &[
                make_string_delimiter_spec("\"", "\\", NewlineSupport::Unescaped),
                make_string_delimiter_spec("'", "\\", NewlineSupport::Unescaped),
            ],
            None,
            SpecialCase::TemplateLiteral,
        ),
        // ### VHDL
        make_language_lexer(
            // See the IEEE Standard VHDL Language Reference Manual (IEEE Std
            // 1076-2008)
            "vhdl",
            // `bsd(l)` files are boundary scan files.
            &["vhdl", "vhd", "bsd", "bsdl"],
            // See section 15.9 of the standard.
            &["--"],
            &[make_block_comment_delim("/*", "*/", false)],
            // Per section 15.7 of the standard, strings may not contain
            // newlines. This language uses
            // [string delimiter doubling](#string_delimiter_doubling).
            &[make_string_delimiter_spec("\"", "", NewlineSupport::None)],
            None,
            SpecialCase::None,
        ),
        // ### Verilog
        make_language_lexer(
            "verilog",
            &["v", "sv"],
            &["//"],
            &[make_block_comment_delim("/*", "*/", false)],
            &[make_string_delimiter_spec(
                "\"",
                "\\",
                NewlineSupport::Escaped,
            )],
            None,
            SpecialCase::None,
        ),
        // ### [V](https://vlang.io/)
        make_language_lexer(
            // Ace doesn't support V yet.
            "",
            &["v"],
            // See
            // [Comments](https://github.com/vlang/v/blob/master/doc/docs.md#comments).
            &["//"],
            &[make_block_comment_delim("/*", "*/", false)],
            // See
            // [Strings](https://github.com/vlang/v/blob/master/doc/docs.md#strings).
            &[
                make_string_delimiter_spec("\"", "\\", NewlineSupport::Unescaped),
                make_string_delimiter_spec("'", "\\", NewlineSupport::Unescaped),
            ],
            None,
            SpecialCase::None,
        ),
        // ### YAML
        make_language_lexer(
            "yaml",
            &["yaml", "yml"],
            &["#"],
            &[],
            &[
                // See
                // [double-quoted style](https://yaml.org/spec/1.2.2/#double-quoted-style).
                // Something I don't understand and will probably ignore:
                // "Single- and double-quoted scalars are restricted to a single
                // line when contained inside an implicit key."
                make_string_delimiter_spec("\"", "\\", NewlineSupport::Unescaped),
                // See
                // [single-quoted style](https://yaml.org/spec/1.2.2/#single-quoted-style).
                // Single-quoted strings escape a single quote by repeating it
                // twice: `'That''s unusual.'` Rather than try to parse this,
                // treat it as two back-to-back strings: `'That'` and
                // `'s unusual.'` We don't care about getting the correct value
                // for strings; the only purpose is to avoid interpreting string
                // contents as inline or block comments.
                make_string_delimiter_spec("'", "", NewlineSupport::Unescaped),
            ],
            None,
            SpecialCase::None,
        ),
        // ### Markdown
        make_language_lexer("markdown", &["md"], &[], &[], &[], None, SpecialCase::None),
    ]
}
