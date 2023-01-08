/// <details>
///      <summary>Copyright (C) 2022 Bryan A. Jones.</summary>
///      <p>This file is part of the CodeChat Editor.</p>
///      <p>The CodeChat Editor is free software: you can redistribute it and/or
///          modify it under the terms of the GNU General Public License as
///          published by the Free Software Foundation, either version 3 of the
///          License, or (at your option) any later version.</p>
///      <p>The CodeChat Editor is distributed in the hope that it will be useful,
///          but WITHOUT ANY WARRANTY; without even the implied warranty of
///          MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
///          General Public License for more details.</p>
///      <p>You should have received a copy of the GNU General Public License
///          along with the CodeChat Editor. If not, see <a
///              href="http://www.gnu.org/licenses/">http://www.gnu.org/licenses/</a>.
///      </p>
///  </details>
/// <h1><code>supported_languages.rs</code> &mdash; Provide lexer info for all supported languages</h1>
// Define lexers for each supported language.
use super::BlockCommentDelim;
use super::HeredocDelim;
use super::LanguageLexer;
use super::NewlineSupport;
use super::StringDelimiterSpec;
use super::TemplateLiteral;

// <p>Ordering matters: all these delimiters end up in a large regex separated by an or operator. The regex or operator matches from left to right. So, longer Python string delimiters must be specified first (leftmost): <code>"""</code> (a multi-line Python string) must come before <code>"</code>. The resulting regex will then have <code>"""|"</code>, which will first search for the multi-line triple quote, then if that's not found, the single quote. A regex of <code>"|"""</code> would never match the triple quote, since the single quote would match first.
pub const LANGUAGE_LEXER_ARR: &[LanguageLexer] = &[
    // C/C++
    LanguageLexer {
        ace_mode: "c_cpp",
        ext_arr: &[".c", ".cc", ".cpp"],
        line_continuation: "\\",
        inline_comment_delim_arr: &["//"],
        block_comment_delim_arr: &[BlockCommentDelim {
            opening: "/*",
            closing: "*/",
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
        //         string literals docs</a> for the reasoning behind the start body
        //     regex.</p>
        heredoc_delim: Some(&HeredocDelim {
            start_prefix: "R\"",
            delim_ident_regex: "[^()\\ ]",
            start_suffix: "(",
            stop_prefix: ")",
            stop_suffix: "",
        }),
        template_literal: TemplateLiteral::No,
    },
    // HTML
    LanguageLexer {
        ace_mode: "html",
        ext_arr: &[".html", ".htm"],
        line_continuation: "",
        inline_comment_delim_arr: &[],
        block_comment_delim_arr: &[BlockCommentDelim {
            opening: "<!--",
            closing: "-->",
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
        template_literal: TemplateLiteral::No,
    },
    // JavaScript
    LanguageLexer {
        ace_mode: "javascript",
        ext_arr: &[".js", ".mjs"],
        line_continuation: "",
        inline_comment_delim_arr: &["//"],
        block_comment_delim_arr: &[BlockCommentDelim {
            opening: "/*",
            closing: "*/",
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
        template_literal: TemplateLiteral::Yes,
    },
    // JSON5
    LanguageLexer {
        ace_mode: "json5",
        ext_arr: &[".json"],
        line_continuation: "",
        inline_comment_delim_arr: &["//"],
        block_comment_delim_arr: &[BlockCommentDelim {
            opening: "/*",
            closing: "*/",
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
        template_literal: TemplateLiteral::No,
    },
    // Python
    LanguageLexer {
        ace_mode: "python",
        ext_arr: &[".py"],
        line_continuation: "\\",
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
        template_literal: TemplateLiteral::No,
    },
    // <a href="https://doc.rust-lang.org/reference/tokens.html#literals">Rust</a>
    LanguageLexer {
        ace_mode: "rust",
        ext_arr: &[".rs"],
        line_continuation: "\\",
        // Since Rust complains about <code>///</code> comments on items that rustdoc ignores, support both styles.
        inline_comment_delim_arr: &["///", "//"],
        block_comment_delim_arr: &[BlockCommentDelim {
            opening: "/*",
            closing: "*/",
        }],
        string_delim_spec_arr: &[
            // Note that raw byte strings behave identically to raw strings from this lexer's perspective.
            StringDelimiterSpec {
                delimiter: "r#\"",
                escape_char: "",
                newline_support: NewlineSupport::Unescaped,
            },
            // Likewise, byte strings behave like strings for this lexer.
            StringDelimiterSpec {
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
        ],
        heredoc_delim: None,
        template_literal: TemplateLiteral::No,
    },
    // <a href="https://toml.io/en/">TOML</a>
    LanguageLexer {
        ace_mode: "toml",
        ext_arr: &[".toml"],
        line_continuation: "",
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
        template_literal: TemplateLiteral::No,
    },
    // TypeScript
    LanguageLexer {
        ace_mode: "typescript",
        ext_arr: &[".ts", ".mts"],
        line_continuation: "",
        inline_comment_delim_arr: &["//"],
        block_comment_delim_arr: &[BlockCommentDelim {
            opening: "/*",
            closing: "*/",
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
        template_literal: TemplateLiteral::Yes,
    },
    // Verilog
    LanguageLexer {
        ace_mode: "verilog",
        ext_arr: &[".v"],
        line_continuation: "",
        inline_comment_delim_arr: &["//"],
        block_comment_delim_arr: &[BlockCommentDelim {
            opening: "/*",
            closing: "*/",
        }],
        string_delim_spec_arr: &[StringDelimiterSpec {
            delimiter: "\"",
            escape_char: "\\",
            newline_support: NewlineSupport::Escaped,
        }],
        heredoc_delim: None,
        template_literal: TemplateLiteral::No,
    },
    // V
    LanguageLexer {
        // Ace doesn't support V yet.
        ace_mode: "",
        ext_arr: &[".v"],
        line_continuation: "",
        inline_comment_delim_arr: &["//"],
        block_comment_delim_arr: &[BlockCommentDelim {
            opening: "/*",
            closing: "*/",
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
        template_literal: TemplateLiteral::No,
    },
    // YAML
    LanguageLexer {
        ace_mode: "yaml",
        ext_arr: &[".yaml"],
        line_continuation: "",
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
        template_literal: TemplateLiteral::No,
    },
    // CodeChat HTML
    LanguageLexer {
        ace_mode: "codechat-html",
        ext_arr: &[".cchtml"],
        line_continuation: "",
        inline_comment_delim_arr: &[],
        block_comment_delim_arr: &[],
        string_delim_spec_arr: &[],
        heredoc_delim: None,
        template_literal: TemplateLiteral::No,
    },
];
