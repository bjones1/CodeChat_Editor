/// <details>
///     <summary>Copyright (C) 2022 Bryan A. Jones.</summary>
///     <p>This file is part of the CodeChat Editor.</p>
///     <p>The CodeChat Editor is free software: you can redistribute it and/or
///         modify it under the terms of the GNU General Public License as
///         published by the Free Software Foundation, either version 3 of the
///         License, or (at your option) any later version.</p>
///     <p>The CodeChat Editor is distributed in the hope that it will be
///         useful, but WITHOUT ANY WARRANTY; without even the implied warranty
///         of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
///         General Public License for more details.</p>
///     <p>You should have received a copy of the GNU General Public License
///         along with the CodeChat Editor. If not, see <a
///             href="http://www.gnu.org/licenses/">http://www.gnu.org/licenses/</a>.
///     </p>
/// </details>
/// <h1><code>supported_languages.rs</code> &mdash; Provide lexer info for all
///     supported languages</h1>
/// <p>Note that the lexers here should be complemented by the appropriate Ace
///     mode in <a
///         href="../../../client/src/ace-webpack.mts">ace-webpack.mts</a>.</p>
/// <p>Ordering matters: all these delimiters end up in a large regex separated
///     by an or operator. The regex or operator matches from left to right. So,
///     longer Python string delimiters must be specified first (leftmost):
///     <code>"""</code> (a multi-line Python string) must come before
///     <code>"</code>. The resulting regex will then have <code>"""|"</code>,
///     which will first search for the multi-line triple quote, then if that's
///     not found, the single quote. A regex of <code>"|"""</code> would never
///     match the triple quote, since the single quote would match first.</p>
/// <h2>Imports</h2>
/// <h3>Local</h3>
use super::BlockCommentDelim;
use super::HeredocDelim;
use super::LanguageLexer;
use super::NewlineSupport;
use super::StringDelimiterSpec;

// <h2>Define lexers for each supported language</h2>
pub const LANGUAGE_LEXER_ARR: &[LanguageLexer] = &[
    // <p>C/C++</p>
    LanguageLexer {
        ace_mode: "c_cpp",
        ext_arr: &["c", "cc", "cpp"],
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
        // <p>Note: the C/C++ support expects C++11 or newer. Don't worry about
        //     supporting C or older C++ using another lexer entry, since the
        //     raw string syntax in C++11 and newer is IMHO so rare we won't
        //     encounter it in older code. See the C++ <a
        //         href="https://en.cppreference.com/w/cpp/language/string_literal">string
        //         literals docs for the reasoning behind the start body
        //         regex.</a></p>
        heredoc_delim: Some(&HeredocDelim {
            start_prefix: "R\"",
            delim_ident_regex: "[^()\\\\[[:space:]]]*",
            start_suffix: "(",
            stop_prefix: ")",
            stop_suffix: "\"",
        }),
        template_literal: false,
    },
    // <p>TODO: C# and its <a
    //         href="https://learn.microsoft.com/en-us/dotnet/csharp/language-reference/builtin-types/reference-types#string-literals">String
    //         literals</a></p>
    // <p>CSS</p>
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
        template_literal: false,
    },
    // <p>Go</p>
    LanguageLexer {
        ace_mode: "golang",
        ext_arr: &["go"],
        // <p>See <a href="https://go.dev/ref/spec">The Go Programming Language
        //         Specification</a> on <a
        //         href="https://go.dev/ref/spec#Comments">Comments</a>.</p>
        inline_comment_delim_arr: &[],
        block_comment_delim_arr: &[BlockCommentDelim {
            opening: "/*",
            closing: "*/",
            is_nestable: false,
        }],
        // <p>See <a href="https://go.dev/ref/spec#String_literals">String
        //         literals</a>.</p>
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
        template_literal: false,
    },
    // <p>HTML</p>
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
        template_literal: false,
    },
    // <p>Java</p>
    LanguageLexer {
        ace_mode: "java",
        ext_arr: &["java"],
        // <p>See the <a
        //         href="https://docs.oracle.com/javase/specs/jls/se19/html/index.html">Java
        //         Language Specification, Java SE 19 edition</a>, <a
        //         href="https://docs.oracle.com/javase/specs/jls/se19/html/jls-3.html#jls-3.7">&sect;3.7.
        //         Comments</a>. The end of this section notes that <q>comments
        //         do not occur within character literals, string literals, or
        //         text blocks,</q> which describes the approach of this lexer
        //     nicely.</p>
        inline_comment_delim_arr: &["//"],
        block_comment_delim_arr: &[BlockCommentDelim {
            opening: "/*",
            closing: "*/",
            is_nestable: false,
        }],
        string_delim_spec_arr: &[
            // <p>See <a
            //         href="https://docs.oracle.com/javase/specs/jls/se19/html/jls-3.html#jls-3.10.5">&sect;3.10.5.
            //         String Literals</a>.</p>
            StringDelimiterSpec {
                delimiter: "\"",
                escape_char: "\\",
                // <p>Per the previous link, <q>It is a compile-time error for a
                //         line terminator (&sect;3.4) to appear after the
                //         opening " and before the matching closing "."</q></p>
                newline_support: NewlineSupport::None,
            },
            // <p>See <a
            //         href="https://docs.oracle.com/javase/specs/jls/se19/html/jls-3.html#jls-3.10.6">&sect;3.10.6.
            //         Text Blocks</a>.</p>
            StringDelimiterSpec {
                delimiter: "\"\"\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
        ],
        heredoc_delim: None,
        template_literal: false,
    },
    // <p>JavaScript</p>
    LanguageLexer {
        ace_mode: "javascript",
        ext_arr: &["js", "mjs"],
        // <p>See <a
        //         href="https://262.ecma-international.org/13.0/#sec-comments">&sect;12.4
        //         Comments</a></p>
        inline_comment_delim_arr: &["//"],
        block_comment_delim_arr: &[BlockCommentDelim {
            opening: "/*",
            closing: "*/",
            is_nestable: false,
        }],
        string_delim_spec_arr: &[
            // <p>See <a
            //         href="https://262.ecma-international.org/13.0/#prod-StringLiteral">&sect;12.8.4
            //         String Literals</a>.</p>
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
        template_literal: true,
    },
    // <p>JSON5</p>
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
        template_literal: false,
    },
    // <p>Python</p>
    LanguageLexer {
        ace_mode: "python",
        ext_arr: &["py"],
        inline_comment_delim_arr: &["#"],
        block_comment_delim_arr: &[],
        string_delim_spec_arr: &[
            // <p>Note that raw strings still allow escaping the single/double
            //     quote. See the <a
            //         href="https://docs.python.org/3/reference/lexical_analysis.html#literals">language
            //         reference</a>.</p>
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
        template_literal: false,
    },
    // <p><a
    //         href="https://doc.rust-lang.org/reference/tokens.html#literals">Rust</a>
    // </p>
    LanguageLexer {
        ace_mode: "rust",
        ext_arr: &["rs"],
        // <p>Support both rustdoc-style comments and plain Rust comments.</p>
        inline_comment_delim_arr: &["///", "//"],
        block_comment_delim_arr: &[BlockCommentDelim {
            opening: "/*",
            closing: "*/",
            is_nestable: true,
        }],
        string_delim_spec_arr: &[
            // <p>Byte strings behave like strings for this lexer.</p>
            StringDelimiterSpec {
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
        ],
        // <p>Likewise, raw byte strings behave identically to raw strings from
        //     this lexer's perspective.</p>
        heredoc_delim: Some(&HeredocDelim {
            start_prefix: "r",
            delim_ident_regex: "#+",
            start_suffix: "\"",
            stop_prefix: "\"",
            stop_suffix: "",
        }),
        template_literal: false,
    },
    // <p><a href="https://toml.io/en/">TOML</a></p>
    LanguageLexer {
        ace_mode: "toml",
        ext_arr: &["toml"],
        inline_comment_delim_arr: &["#"],
        block_comment_delim_arr: &[],
        string_delim_spec_arr: &[
            // <p>Multi-line literal strings (as described by the link above).
            // </p>
            StringDelimiterSpec {
                delimiter: "'''",
                escape_char: "",
                newline_support: NewlineSupport::Unescaped,
            },
            // <p>Multi-line basic strings</p>
            StringDelimiterSpec {
                delimiter: "\"\"\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
            // <p>Basic strings</p>
            StringDelimiterSpec {
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::None,
            },
            // <p>Literal strings</p>
            StringDelimiterSpec {
                delimiter: "'",
                escape_char: "\\",
                newline_support: NewlineSupport::Escaped,
            },
        ],
        heredoc_delim: None,
        template_literal: false,
    },
    // <p>TypeScript</p>
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
        template_literal: true,
    },
    // <p>Verilog</p>
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
        template_literal: false,
    },
    // <p><a href="https://vlang.io/">V</a></p>
    LanguageLexer {
        // <p>Ace doesn't support V yet.</p>
        ace_mode: "",
        ext_arr: &["v"],
        // <p>See <a
        //         href="https://github.com/vlang/v/blob/master/doc/docs.md#comments">Comments</a>.
        // </p>
        inline_comment_delim_arr: &["//"],
        block_comment_delim_arr: &[BlockCommentDelim {
            opening: "/*",
            closing: "*/",
            is_nestable: false,
        }],
        // <p>See <a
        //         href="https://github.com/vlang/v/blob/master/doc/docs.md#strings">Strings</a>.
        // </p>
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
        template_literal: false,
    },
    // <p>YAML</p>
    LanguageLexer {
        ace_mode: "yaml",
        ext_arr: &["yaml", "yml"],
        inline_comment_delim_arr: &["#"],
        block_comment_delim_arr: &[],
        string_delim_spec_arr: &[
            // <p>See <a
            //         href="https://yaml.org/spec/1.2.2/#double-quoted-style">double-quoted
            //         style</a>. Something I don't understand and will probably
            //     ignore: "Single- and double-quoted scalars are restricted to
            //     a single line when contained inside an implicit key."</p>
            StringDelimiterSpec {
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
            // <p>See <a
            //         href="https://yaml.org/spec/1.2.2/#single-quoted-style">single-quoted
            //         style</a>. Single-quoted strings escape a single quote by
            //     repeating it twice: <code>'That''s unusual.'</code> Rather
            //     than try to parse this, treat it as two back-to-back strings:
            //     <code>'That'</code> and <code>'s unusual.'</code> We don't
            //     care about getting the correct value for strings; the only
            //     purpose is to avoid interpreting string contents as inline or
            //     block comments.</p>
            StringDelimiterSpec {
                delimiter: "'",
                escape_char: "",
                newline_support: NewlineSupport::Unescaped,
            },
        ],
        heredoc_delim: None,
        template_literal: false,
    },
    // <p>CodeChat HTML</p>
    LanguageLexer {
        ace_mode: "codechat-html",
        ext_arr: &["cchtml"],
        inline_comment_delim_arr: &[],
        block_comment_delim_arr: &[],
        string_delim_spec_arr: &[],
        heredoc_delim: None,
        template_literal: false,
    },
];
