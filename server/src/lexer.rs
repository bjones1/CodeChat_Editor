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
/// <h1><code>lexer.rs</code> &mdash; Lex source code into code and doc blocks</h1>
/// <h2>Data structures</h2>
/// <p>This struct defines the delimiters for a block comment.</p>
struct BlockCommentDelim {
	// <p>A string specifying the opening comment delimiter for a block comment.
	// </p>
	opening: &'static str,
	// <p>A string specifying the closing comment delimiter for a block comment.
	// </p>
	closing: &'static str,
}


enum NewlineSupport {
    // This string delimiter allows unescaped newlines. This is a multiline string.
    Unescaped,
    // This string delimiter only allows newlines when preceded by the string escape character. This is (mostly) a single-line string.
    Escaped,
    // This string delimiter does not allow newlines. This is strictly a single-line string.
    None
}


struct StringDelimiterSpec {
    // Delimiter to indicate the start and end of a string.
    delimiter: &'static str,
    // Escape character, to allow inserting the string delimiter into the string. Empty if this string delimiter doesn't provide an escape character.
    escape_char: &'static str,
    // <p>Newline handling. This value cannot be <code>Escaped</code> if the <code>escape_char</code> is empty.
    newline_support: NewlineSupport,
}


// <p>This defines the delimiters for a <a
//         href="https://en.wikipedia.org/wiki/Here_document">heredoc</a> (or
//     heredoc-like literal).</p>
struct HeredocDelim {
	// <p>The prefix before the heredoc's delimiting identifier.</p>
	start_prefix: &'static str,
	// <p>A regex which matches the delimiting identifier.</p>
	delim_ident_regex: &'static str,
	// <p>The suffix after the delimiting identifier.</p>
	start_suffix: &'static str,
	// <p>The prefix before the second (closing) delimiting identifier.</p>
	stop_prefix: &'static str,
	// <p>The suffix after the heredoc's closing delimiting identifier.</p>
	stop_suffix: &'static str,
}


enum TemplateLiteral {
	// This language does not contain template literals.
    No,
	// This language does contain template literals.
    Yes,
	// Indicates the lexer is inside a nested template literal; for internal use only.
    Nested,
}


struct LanguageLexer {
	// <p>The Ace mode to use for this language</p>
	ace_mode: &'static str,
	// <p>An array of file extensions for this language. They begin with a period,
	//     such as <code>.rs</code>.</p>
	ext_arr: &'static[&'static str],
    // A string specifying the line continuation character; an empty string if this language doesn't contain it.
    line_continuation: &'static str,
	// <p>An array of strings which specify inline comment delimiters. Empty if this language doesn't provide inline comments.</p>
	inline_comment_delim_arr: &'static[&'static str],
	// <p>An array which specifies opening and closing block comment delimiters. Empty if this language doesn't provide block comments.
	// </p>
	block_comment_delim_arr: &'static[BlockCommentDelim],
    // Specify the strings supported by this language. While this could be empty, such a language would be very odd.
    string_delim_spec_arr: &'static[StringDelimiterSpec],
	// <p>An array of heredoc delimiters; empty if heredocs aren't supported.</p>
	heredoc_delim_arr: &'static[HeredocDelim],
	// <p>Template literal support (for languages such as JavaScript, TypeScript,
	//     etc.). A value of <code>none</code> indicates the lexer is inside a template; this should only be used by the <code>source_lexer</code> itself.</p>
	template_literal: TemplateLiteral,
}

const LANGUAGE_LEXER_ARR : &[LanguageLexer] = &[
    // C/C++
    LanguageLexer {
        ace_mode: "c_cpp",
        ext_arr: &[".c", ".cc", ".cpp"],
        line_continuation: "\\",
        inline_comment_delim_arr: &["//"],
        block_comment_delim_arr: &[BlockCommentDelim{ opening: "/*", closing: "*/"}],
        string_delim_spec_arr: &[
            StringDelimiterSpec{
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Escaped,
            }
        ],
        heredoc_delim_arr: &[],
        template_literal: TemplateLiteral::No,
    },

    // HTML
    LanguageLexer {
        ace_mode: "html",
        ext_arr: &[".html", ".htm"],
        line_continuation: "",
        inline_comment_delim_arr: &[],
        block_comment_delim_arr: &[BlockCommentDelim{ opening: "<!--", closing: "-->"}],
        string_delim_spec_arr: &[
            StringDelimiterSpec{
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
            StringDelimiterSpec{
                delimiter: "'",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
        ],
        heredoc_delim_arr: &[],
        template_literal: TemplateLiteral::No,
    },

    // JavaScript
    LanguageLexer {
        ace_mode: "javascript",
        ext_arr: &[".js", ".mjs"],
        line_continuation: "",
        inline_comment_delim_arr: &["//"],
        block_comment_delim_arr: &[BlockCommentDelim{ opening: "/*", closing: "*/"}],
        string_delim_spec_arr: &[
            StringDelimiterSpec{
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
            StringDelimiterSpec{
                delimiter: "'",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
        ],
        heredoc_delim_arr: &[],
        template_literal: TemplateLiteral::Yes,
    },

    // JSON5
    LanguageLexer {
        ace_mode: "json5",
        ext_arr: &[".json"],
        line_continuation: "",
        inline_comment_delim_arr: &["//"],
        block_comment_delim_arr: &[BlockCommentDelim{ opening: "/*", closing: "*/"}],
        string_delim_spec_arr: &[
            StringDelimiterSpec{
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Escaped,
            },
            StringDelimiterSpec{
                delimiter: "'",
                escape_char: "\\",
                newline_support: NewlineSupport::Escaped,
            },
        ],
        heredoc_delim_arr: &[],
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
            StringDelimiterSpec{
                delimiter: "\"\"\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
            StringDelimiterSpec{
                delimiter: "'''",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
            StringDelimiterSpec{
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Escaped,
            },
            StringDelimiterSpec{
                delimiter: "'",
                escape_char: "\\",
                newline_support: NewlineSupport::Escaped,
            },
        ],
        heredoc_delim_arr: &[],
        template_literal: TemplateLiteral::No,
    },

    // <a href="https://doc.rust-lang.org/reference/tokens.html#literals">Rust</a>
    LanguageLexer {
        ace_mode: "rust",
        ext_arr: &[".rs"],
        line_continuation: "\\",
        // Since Rust complains about <code>///</code> comments on items that rustdoc ignores, support both styles.
        inline_comment_delim_arr: &["///", "//"],
        block_comment_delim_arr: &[BlockCommentDelim{ opening: "/*", closing: "*/"}],
        string_delim_spec_arr: &[
            // Note that raw byte strings behave identically to raw strings from this lexer's perspective.
            StringDelimiterSpec{
                delimiter: "r#\"",
                escape_char: "",
                newline_support: NewlineSupport::Unescaped,
            },
            // Likewise, byte strings behave like strings for this lexer.
            StringDelimiterSpec{
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            }
        ],
        heredoc_delim_arr: &[],
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
            StringDelimiterSpec{
                delimiter: "'''",
                escape_char: "",
                newline_support: NewlineSupport::Unescaped,
            },
            // Multi-line basic strings
            StringDelimiterSpec{
                delimiter: "\"\"\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
            // Basic strings
            StringDelimiterSpec{
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::None,
            },
            // Literal strings
            StringDelimiterSpec{
                delimiter: "'",
                escape_char: "\\",
                newline_support: NewlineSupport::Escaped,
            },
        ],
        heredoc_delim_arr: &[],
        template_literal: TemplateLiteral::No,
    },

    // TypeScript
    LanguageLexer {
        ace_mode: "typescript",
        ext_arr: &[".ts", ".mts"],
        line_continuation: "",
        inline_comment_delim_arr: &["//"],
        block_comment_delim_arr: &[BlockCommentDelim{ opening: "/*", closing: "*/"}],
        string_delim_spec_arr: &[
            StringDelimiterSpec{
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
            StringDelimiterSpec{
                delimiter: "'",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
        ],
        heredoc_delim_arr: &[],
        template_literal: TemplateLiteral::Yes,
    },

    // Verilog
    LanguageLexer {
        ace_mode: "verilog",
        ext_arr: &[".v"],
        line_continuation: "",
        inline_comment_delim_arr: &["//"],
        block_comment_delim_arr: &[BlockCommentDelim{ opening: "/*", closing: "*/"}],
        string_delim_spec_arr: &[
            StringDelimiterSpec{
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Escaped,
            }
        ],
        heredoc_delim_arr: &[],
        template_literal: TemplateLiteral::No,
    },

    // V
    LanguageLexer {
        // Ace doesn't support V yet.
        ace_mode: "",
        ext_arr: &[".v"],
        line_continuation: "",
        inline_comment_delim_arr: &["//"],
        block_comment_delim_arr: &[BlockCommentDelim{ opening: "/*", closing: "*/"}],
        string_delim_spec_arr: &[
            StringDelimiterSpec{
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
            StringDelimiterSpec{
                delimiter: "'",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
        ],
        heredoc_delim_arr: &[],
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
            StringDelimiterSpec{
                delimiter: "\"",
                escape_char: "\\",
                newline_support: NewlineSupport::Unescaped,
            },
            // See <a href="https://yaml.org/spec/1.2.2/#single-quoted-style">single-quoted style</a>. Single-quoted strings escape a single quote by repeating it twice: <code>'That''s unusual.'</code> Rather than try to parse this, treat it as two back-to-back strings: <code>'That'</code> and <code>'s unusual.'</code> We don't care about getting the correct value for strings; the only purpose is to avoid interpreting string contents as inline or block comments.
            StringDelimiterSpec{
                delimiter: "'",
                escape_char: "",
                newline_support: NewlineSupport::Unescaped,
            },
        ],
        heredoc_delim_arr: &[],
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
        heredoc_delim_arr: &[],
        template_literal: TemplateLiteral::No,
    },
];

struct CodeDocBlock<'a> {
	// For a doc block, the whitespace characters which created the indent for this doc block. For a code block, <code>None</code>.
    indent: Option<&'a str>,
	// The contents of this block -- documentation (with the comment delimiters removed) or code.
    contents: &'a str
}


pub fn source_lexer() {
    println!("Hello, world!");
}


// Rust <a href="https://doc.rust-lang.org/book/ch11-03-test-organization.html">almost mandates</a> putting tests in the same file as the source, which I dislike. Here's a <a href="http://xion.io/post/code/rust-unit-test-placement.html">good discussion</a> of how to place them in another file, for the time when I'm ready to adopt this more sane layout.
#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
