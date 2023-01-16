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

// <h2>Define lexers for each supported language.</h2>
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
        //     supporting C or older C++ using another lexer entry, since the raw
        //     string syntax in C++11 and newer is IMHO so rare we won't encounter
        //     it in older code. See the <a
        //         href="https://en.cppreference.com/w/cpp/language/string_literal">C++
        // <p>string literals docs for the reasoning behind the start body
        //     regex.</p>
        // <p>&nbsp;</p>
        heredoc_delim: Some(&HeredocDelim {
            start_prefix: "R\"",
            delim_ident_regex: "[^()\\\\[[:space:]]]*",
            start_suffix: "(",
            stop_prefix: ")",
            stop_suffix: "\"",
        }),
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
    // JavaScript
    LanguageLexer {
        ace_mode: "javascript",
        ext_arr: &["js", "mjs"],
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
        template_literal: true,
    },
    // JSON5
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
    // Python
    LanguageLexer {
        ace_mode: "python",
        ext_arr: &["py"],
        inline_comment_delim_arr: &["#"],
        block_comment_delim_arr: &[],
        string_delim_spec_arr: &[
            // Note that raw strings still allow escaping the single/double quote.
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
    // <a href="https://doc.rust-lang.org/reference/tokens.html#literals">Rust</a>
    LanguageLexer {
        ace_mode: "rust",
        ext_arr: &["rs"],
        // <p>Since Rust complains about <code>///</code> comments on items that
        //     rustdoc ignores, support both styles.</p>
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
        // Likewise, raw byte strings behave identically to raw strings from this lexer's perspective.
        heredoc_delim: Some(&HeredocDelim {
            start_prefix: "r",
            delim_ident_regex: "#+",
            start_suffix: "\"",
            stop_prefix: "\"",
            stop_suffix: "",
        }),
        template_literal: false,
    },
    // <a href="https://toml.io/en/">TOML</a>
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
            // Basic strings
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
    // Verilog
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
    // V
    LanguageLexer {
        // Ace doesn't support V yet.
        ace_mode: "",
        ext_arr: &["v"],
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
        template_literal: false,
    },
    // YAML
    LanguageLexer {
        ace_mode: "yaml",
        ext_arr: &["yaml"],
        inline_comment_delim_arr: &["#"],
        block_comment_delim_arr: &[],
        string_delim_spec_arr: &[
            // See <a href="https://yaml.org/spec/1.2.2/#double-quoted-style">double-quoted style</a>. Something I don't understand and will probably ignore: "Single- and double-quoted scalars are restricted to a single line when contained inside an implicit key."
            StringDelimiterSpec {
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
            // See <a href="https://yaml.org/spec/1.2.2/#single-quoted-style">single-quoted style</a>. Single-quoted strings escape a single quote by repeating it twice: <code>'That''s unusual.'</code> Rather than try to parse this, treat it as two back-to-back strings: <code>'That'</code> and <code>'s unusual.'</code> We don't care about getting the correct value for strings; the only purpose is to avoid interpreting string contents as inline or block comments.
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
