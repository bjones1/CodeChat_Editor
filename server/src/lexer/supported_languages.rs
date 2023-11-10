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
///
/// ## Imports
///
/// ### Local
use super::BlockCommentDelim;
use super::HeredocDelim;
use super::LanguageLexer;
use super::NewlineSupport;
use super::SpecialCase;
use super::StringDelimiterSpec;

// ## Define lexers for each supported language
pub const LANGUAGE_LEXER_ARR: &[LanguageLexer] = &[
    // ### Linux shell scripts
    LanguageLexer {
        ace_mode: "sh",
        ext_arr: &["sh"],
        inline_comment_delim_arr: &["#"],
        block_comment_delim_arr: &[],
        string_delim_spec_arr: &[
            StringDelimiterSpec {
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
            StringDelimiterSpec {
                delimiter: "'",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
        ],
        // This doesn't quite match the spec (search for here documents in the
        // bash man page), since it doesn't correctly handle unmatched or
        // mismatched quote; for example, `TODO`.
        heredoc_delim: Some(&HeredocDelim {
            start_prefix: "<<-?('|\")?",
            delim_ident_regex: "\\w+",
            start_suffix: "('|\")?",
            stop_prefix: "",
            stop_suffix: "",
        }),
        special_case: SpecialCase::None,
    },
    // ### C/C++
    LanguageLexer {
        ace_mode: "c_cpp",
        // Note that the `.ino` extension is for Arduino source files.
        ext_arr: &["c", "cc", "cpp", "h", "hh", "hpp", "ino"],
        inline_comment_delim_arr: &["//"],
        block_comment_delim_arr: &[BlockCommentDelim {
            opening: "/*",
            closing: "*/",
            is_nestable: false,
        }],
        string_delim_spec_arr: &[StringDelimiterSpec {
            delimiter: "\"",
            escape_char: "\\",
            newline_support: NewlineSupport::Escaped,
        }],
        // Note: the C/C++ support expects C++11 or newer. Don't worry about
        // supporting C or older C++ using another lexer entry, since the raw
        // string syntax in C++11 and newer is IMHO so rare we won't encounter
        // it in older code. See the C++
        // [string literals docs for the reasoning behind the start body regex.](https://en.cppreference.com/w/cpp/language/string_literal)
        heredoc_delim: Some(&HeredocDelim {
            start_prefix: "R\"",
            delim_ident_regex: "[^()\\\\[[:space:]]]*",
            start_suffix: "(",
            stop_prefix: ")",
            stop_suffix: "\"",
        }),
        special_case: SpecialCase::None,
    },
    // ### C#
    LanguageLexer {
        ace_mode: "csharp",
        ext_arr: &["cs"],
        // See
        // [6.3.3 Comments](https://learn.microsoft.com/en-us/dotnet/csharp/language-reference/language-specification/lexical-structure#633-comments).
        // Also provide support for
        // [documentation comments](https://learn.microsoft.com/en-us/dotnet/csharp/language-reference/language-specification/documentation-comments).
        inline_comment_delim_arr: &["//", "///"],
        block_comment_delim_arr: &[
            BlockCommentDelim {
                opening: "/*",
                closing: "*/",
                is_nestable: false,
            },
            BlockCommentDelim {
                opening: "/**",
                closing: "*/",
                is_nestable: false,
            },
        ],
        // See
        // [6.4.5.6 String literals](https://learn.microsoft.com/en-us/dotnet/csharp/language-reference/language-specification/lexical-structure#6456-string-literals).
        string_delim_spec_arr: &[StringDelimiterSpec {
            delimiter: "\"",
            escape_char: "\\",
            newline_support: NewlineSupport::None,
        }],
        heredoc_delim: None,
        special_case: SpecialCase::CSharpVerbatimStringLiteral,
    },
    // ### CSS
    LanguageLexer {
        ace_mode: "css",
        ext_arr: &["css"],
        inline_comment_delim_arr: &[],
        block_comment_delim_arr: &[BlockCommentDelim {
            opening: "/*",
            closing: "*/",
            is_nestable: false,
        }],
        string_delim_spec_arr: &[
            StringDelimiterSpec {
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
            StringDelimiterSpec {
                delimiter: "'",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
        ],
        heredoc_delim: None,
        special_case: SpecialCase::None,
    },
    // ### Go
    LanguageLexer {
        ace_mode: "golang",
        ext_arr: &["go"],
        // See
        // [The Go Programming Language Specification](https://go.dev/ref/spec)
        // on [Comments](https://go.dev/ref/spec#Comments).
        inline_comment_delim_arr: &[],
        block_comment_delim_arr: &[BlockCommentDelim {
            opening: "/*",
            closing: "*/",
            is_nestable: false,
        }],
        // See [String literals](https://go.dev/ref/spec#String_literals).
        string_delim_spec_arr: &[
            StringDelimiterSpec {
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::None,
            },
            StringDelimiterSpec {
                delimiter: "`",
                escape_char: "",
                newline_support: NewlineSupport::Unescaped,
            },
        ],
        heredoc_delim: None,
        special_case: SpecialCase::None,
    },
    // ### HTML
    LanguageLexer {
        ace_mode: "html",
        ext_arr: &["html", "htm"],
        inline_comment_delim_arr: &[],
        block_comment_delim_arr: &[BlockCommentDelim {
            opening: "<!--",
            closing: "-->",
            is_nestable: false,
        }],
        string_delim_spec_arr: &[
            StringDelimiterSpec {
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
            StringDelimiterSpec {
                delimiter: "'",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
        ],
        heredoc_delim: None,
        special_case: SpecialCase::None,
    },
    // ### Java
    LanguageLexer {
        ace_mode: "java",
        ext_arr: &["java"],
        // See the
        // [Java Language Specification, Java SE 19 edition](https://docs.oracle.com/javase/specs/jls/se19/html/index.html),
        // [§3.7. Comments](https://docs.oracle.com/javase/specs/jls/se19/html/jls-3.html#jls-3.7).
        // The end of this section notes that <q>comments do not occur within
        // character literals, string literals, or text blocks,</q> which
        // describes the approach of this lexer nicely.
        inline_comment_delim_arr: &["//"],
        block_comment_delim_arr: &[BlockCommentDelim {
            opening: "/*",
            closing: "*/",
            is_nestable: false,
        }],
        string_delim_spec_arr: &[
            // See
            // [§3.10.5. String Literals](https://docs.oracle.com/javase/specs/jls/se19/html/jls-3.html#jls-3.10.5).
            StringDelimiterSpec {
                delimiter: "\"",
                escape_char: "\\",
                // Per the previous link, <q>It is a compile-time error for a
                // line terminator (§3.4) to appear after the opening " and
                // before the matching closing "."</q>
                newline_support: NewlineSupport::None,
            },
            // See
            // [§3.10.6. Text Blocks](https://docs.oracle.com/javase/specs/jls/se19/html/jls-3.html#jls-3.10.6).
            StringDelimiterSpec {
                delimiter: "\"\"\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
        ],
        heredoc_delim: None,
        special_case: SpecialCase::None,
    },
    // ### JavaScript
    LanguageLexer {
        ace_mode: "javascript",
        ext_arr: &[
            "js", "mjs",
            // Note that
            // [Qt's QML language](https://doc.qt.io/qt-6/qtqml-syntax-basics.html)
            // is basically JSON with some embedded JavaScript. Treat it as
            // JavaScript, since those rules include template literals.
            "qml",
        ],
        // See
        // [§12.4 Comments](https://262.ecma-international.org/13.0/#sec-comments)
        inline_comment_delim_arr: &["//"],
        block_comment_delim_arr: &[BlockCommentDelim {
            opening: "/*",
            closing: "*/",
            is_nestable: false,
        }],
        string_delim_spec_arr: &[
            // See
            // [§12.8.4 String Literals](https://262.ecma-international.org/13.0/#prod-StringLiteral).
            StringDelimiterSpec {
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Escaped,
            },
            StringDelimiterSpec {
                delimiter: "'",
                escape_char: "\\",
                newline_support: NewlineSupport::Escaped,
            },
        ],
        heredoc_delim: None,
        special_case: SpecialCase::TemplateLiteral,
    },
    // ### JSON5
    LanguageLexer {
        ace_mode: "json5",
        ext_arr: &["json"],
        inline_comment_delim_arr: &["//"],
        block_comment_delim_arr: &[BlockCommentDelim {
            opening: "/*",
            closing: "*/",
            is_nestable: false,
        }],
        string_delim_spec_arr: &[
            StringDelimiterSpec {
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Escaped,
            },
            StringDelimiterSpec {
                delimiter: "'",
                escape_char: "\\",
                newline_support: NewlineSupport::Escaped,
            },
        ],
        heredoc_delim: None,
        special_case: SpecialCase::None,
    },
    // ### MATLAB
    LanguageLexer {
        ace_mode: "matlab",
        ext_arr: &["m"],
        // See the
        // [MATLAB docs on comments](https://www.mathworks.com/help/matlab/matlab_prog/comments.html).
        // Block comments are a special case, so they're not included here.
        inline_comment_delim_arr: &["%", "..."],
        block_comment_delim_arr: &[],
        // Per the
        // [MATLAB docs](https://www.mathworks.com/help/matlab/matlab_prog/represent-text-with-character-and-string-arrays.html),
        // there are two types of strings. Although MATLAB supports
        // [standard escape sequences](https://www.mathworks.com/help/matlab/matlab_prog/matlab-operators-and-special-characters.html#bvg44q6)
        // (scroll to the bottom of the page), these don't affect quotes;
        // instead, doubled quotes are used to insert a single quote. See
        // [string delimiter doubling](#string_delimiter_doubling).
        string_delim_spec_arr: &[
            StringDelimiterSpec {
                delimiter: "\"",
                escape_char: "",
                newline_support: NewlineSupport::None,
            },
            StringDelimiterSpec {
                delimiter: "'",
                escape_char: "",
                newline_support: NewlineSupport::None,
            },
        ],
        heredoc_delim: None,
        special_case: SpecialCase::Matlab,
    },
    // ### Python
    LanguageLexer {
        ace_mode: "python",
        ext_arr: &["py"],
        inline_comment_delim_arr: &["#"],
        block_comment_delim_arr: &[],
        string_delim_spec_arr: &[
            // Note that raw strings still allow escaping the single/double
            // quote. See the
            // [language reference](https://docs.python.org/3/reference/lexical_analysis.html#literals).
            StringDelimiterSpec {
                delimiter: "\"\"\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
            StringDelimiterSpec {
                delimiter: "'''",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
            StringDelimiterSpec {
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Escaped,
            },
            StringDelimiterSpec {
                delimiter: "'",
                escape_char: "\\",
                newline_support: NewlineSupport::Escaped,
            },
        ],
        heredoc_delim: None,
        special_case: SpecialCase::None,
    },
    // ### [Rust](https://doc.rust-lang.org/reference/tokens.html#literals)
    LanguageLexer {
        ace_mode: "rust",
        ext_arr: &["rs"],
        // Support both rustdoc-style comments and plain Rust comments.
        inline_comment_delim_arr: &["///", "//"],
        block_comment_delim_arr: &[BlockCommentDelim {
            opening: "/*",
            closing: "*/",
            is_nestable: true,
        }],
        string_delim_spec_arr: &[
            // Byte strings behave like strings for this lexer.
            StringDelimiterSpec {
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
        ],
        // Likewise, raw byte strings behave identically to raw strings from
        // this lexer's perspective.
        heredoc_delim: Some(&HeredocDelim {
            start_prefix: "r",
            delim_ident_regex: "#+",
            start_suffix: "\"",
            stop_prefix: "\"",
            stop_suffix: "",
        }),
        special_case: SpecialCase::None,
    },
    // ### SQL
    LanguageLexer {
        ace_mode: "sql",
        ext_arr: &["sql"],
        // See [Wikipedia](https://en.wikipedia.org/wiki/SQL_syntax#Comments).
        // The
        // [SQL specification isn't free](https://en.wikipedia.org/wiki/SQL#Standardization_history),
        // sadly. Oracle publishes their flavor of the 2016 spec; see
        // [Comments within SQL statements](https://docs.oracle.com/database/121/SQLRF/sql_elements006.htm#SQLRF51099).
        // Postgresql defines
        // [comments](https://www.postgresql.org/docs/15/sql-syntax-lexical.html#SQL-SYNTAX-COMMENTS)
        // as well.
        inline_comment_delim_arr: &["--"],
        block_comment_delim_arr: &[BlockCommentDelim {
            opening: "/*",
            closing: "*/",
            is_nestable: false,
        }],
        string_delim_spec_arr: &[
            // SQL standard strings allow newlines and don't provide an escape
            // character. This language uses
            // [string delimiter doubling](#string_delimiter_doubling).
            // Unfortunately, each variant of SQL also supports their custom
            // definition of strings; these must be handled by vendor-specific
            // flavors of this basic lexer definition.
            StringDelimiterSpec {
                delimiter: "'",
                escape_char: "",
                newline_support: NewlineSupport::Unescaped,
            },
        ],
        heredoc_delim: None,
        special_case: SpecialCase::None,
    },
    // ### [TOML](https://toml.io/en/)
    LanguageLexer {
        ace_mode: "toml",
        ext_arr: &["toml"],
        inline_comment_delim_arr: &["#"],
        block_comment_delim_arr: &[],
        string_delim_spec_arr: &[
            // Multi-line literal strings (as described by the link above).
            StringDelimiterSpec {
                delimiter: "'''",
                escape_char: "",
                newline_support: NewlineSupport::Unescaped,
            },
            // Multi-line basic strings
            StringDelimiterSpec {
                delimiter: "\"\"\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
            // Basic strings
            StringDelimiterSpec {
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::None,
            },
            // Literal strings
            StringDelimiterSpec {
                delimiter: "'",
                escape_char: "\\",
                newline_support: NewlineSupport::Escaped,
            },
        ],
        heredoc_delim: None,
        special_case: SpecialCase::None,
    },
    // ### TypeScript
    LanguageLexer {
        ace_mode: "typescript",
        ext_arr: &["ts", "mts"],
        inline_comment_delim_arr: &["//"],
        block_comment_delim_arr: &[BlockCommentDelim {
            opening: "/*",
            closing: "*/",
            is_nestable: false,
        }],
        string_delim_spec_arr: &[
            StringDelimiterSpec {
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
            StringDelimiterSpec {
                delimiter: "'",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
        ],
        heredoc_delim: None,
        special_case: SpecialCase::TemplateLiteral,
    },
    // ### VHDL
    LanguageLexer {
        // See the IEEE Standard VHDL Language Reference Manual (IEEE Std
        // 1076-2008)
        ace_mode: "vhdl",
        // `bsd(l)` files are boundary scan files.
        ext_arr: &["vhdl", "vhd", "bsd", "bsdl"],
        // See section 15.9 of the standard.
        inline_comment_delim_arr: &["--"],
        block_comment_delim_arr: &[BlockCommentDelim {
            opening: "/*",
            closing: "*/",
            is_nestable: false,
        }],
        // Per section 15.7 of the standard, strings may not contain newlines.
        // This language uses
        // [string delimiter doubling](#string_delimiter_doubling).
        string_delim_spec_arr: &[StringDelimiterSpec {
            delimiter: "\"",
            escape_char: "",
            newline_support: NewlineSupport::None,
        }],
        heredoc_delim: None,
        special_case: SpecialCase::None,
    },
    // ### Verilog
    LanguageLexer {
        ace_mode: "verilog",
        ext_arr: &["v"],
        inline_comment_delim_arr: &["//"],
        block_comment_delim_arr: &[BlockCommentDelim {
            opening: "/*",
            closing: "*/",
            is_nestable: false,
        }],
        string_delim_spec_arr: &[StringDelimiterSpec {
            delimiter: "\"",
            escape_char: "\\",
            newline_support: NewlineSupport::Escaped,
        }],
        heredoc_delim: None,
        special_case: SpecialCase::None,
    },
    // ### [V](https://vlang.io/)
    LanguageLexer {
        // Ace doesn't support V yet.
        ace_mode: "",
        ext_arr: &["v"],
        // See
        // [Comments](https://github.com/vlang/v/blob/master/doc/docs.md#comments).
        inline_comment_delim_arr: &["//"],
        block_comment_delim_arr: &[BlockCommentDelim {
            opening: "/*",
            closing: "*/",
            is_nestable: false,
        }],
        // See
        // [Strings](https://github.com/vlang/v/blob/master/doc/docs.md#strings).
        string_delim_spec_arr: &[
            StringDelimiterSpec {
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
            StringDelimiterSpec {
                delimiter: "'",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
        ],
        heredoc_delim: None,
        special_case: SpecialCase::None,
    },
    // ### YAML
    LanguageLexer {
        ace_mode: "yaml",
        ext_arr: &["yaml", "yml"],
        inline_comment_delim_arr: &["#"],
        block_comment_delim_arr: &[],
        string_delim_spec_arr: &[
            // See
            // [double-quoted style](https://yaml.org/spec/1.2.2/#double-quoted-style).
            // Something I don't understand and will probably ignore: "Single-
            // and double-quoted scalars are restricted to a single line when
            // contained inside an implicit key."
            StringDelimiterSpec {
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
            // See
            // [single-quoted style](https://yaml.org/spec/1.2.2/#single-quoted-style).
            // Single-quoted strings escape a single quote by repeating it
            // twice: `'That''s unusual.'` Rather than try to parse this, treat
            // it as two back-to-back strings: `'That'` and `'s unusual.'` We
            // don't care about getting the correct value for strings; the only
            // purpose is to avoid interpreting string contents as inline or
            // block comments.
            StringDelimiterSpec {
                delimiter: "'",
                escape_char: "",
                newline_support: NewlineSupport::Unescaped,
            },
        ],
        heredoc_delim: None,
        special_case: SpecialCase::None,
    },
    // ### Markdown
    LanguageLexer {
        ace_mode: "markdown",
        ext_arr: &["md"],
        inline_comment_delim_arr: &[],
        block_comment_delim_arr: &[],
        string_delim_spec_arr: &[],
        heredoc_delim: None,
        special_case: SpecialCase::None,
    },
];
