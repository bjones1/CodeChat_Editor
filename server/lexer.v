// <!-- CodeChat-lexer: vlang -->
// <details>
//     <summary>Copyright (C) 2022 Bryan A. Jones.</summary>
//     <p>This file is part of the CodeChat Editor.</p>
//     <p>The CodeChat Editor is free software: you can redistribute it and/or
//         modify it under the terms of the GNU General Public License as
//         published by the Free Software Foundation, either version 3 of the
//         License, or (at your option) any later version.</p>
//     <p>The CodeChat Editor is distributed in the hope that it will be useful,
//         but WITHOUT ANY WARRANTY; without even the implied warranty of
//         MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
//         General Public License for more details.</p>
//     <p>You should have received a copy of the GNU General Public License
//         along with the CodeChat Editor. If not, see <a
//             href="http://www.gnu.org/licenses/">http://www.gnu.org/licenses/</a>.
//     </p>
// </details>
// <h1><code>lexer.v</code>&mdash;Lex source code into code blocks and doc
//     blocks</h1>
// <h2>Imports</h2>
import regex

// <h2>Data structures</h2>
// <p>This struct defines the delimiters for a block comment.</p>
struct BlockCommentDelim {
	// <p>A string specifying the opening comment delimiter for a block comment.
	// </p>
	opening string
	// <p>A string specifying the closing comment delimiter for a block comment.
	// </p>
	closing string
}

// <p>This defines the delimiters for a <a
//         href="https://en.wikipedia.org/wiki/Here_document">heredoc</a> (or
//     heredoc-like literal).</p>
struct HeredocDelim {
	// <p>The prefix before the heredoc's delimiting identifier.</p>
	start_prefix string
	// <p>A regex which matches the delimiting identifier.</p>
	delim_ident_regex string
	// <p>The suffix after the delimiting identifier.</p>
	start_suffix string
	// <p>The prefix before the second (closing) delimiting identifier.</p>
	stop_prefix string
	// <p>The suffix after the heredoc's closing delimiting identifier.</p>
	stop_suffix string
}


enum TemplateLiteral {
	// This language does not contain template literals.
	no
	// This language does contain template literals.
	yes
	// Indicates the lexer is inside a nested template literal; for internal use only.
	nested
}

struct LanguageLexer {
	// <p>The Ace mode to use for this language</p>
	ace_mode string
	// <p>An array of file extensions for this language. They begin with a period,
	//     such as <code>.v</code>.</p>
	ext_arr	[]string
	// <p>An array of strings which specify inline comment delimiters.</p>
	inline_comment_delim_arr []string
	// <p>An array which specifies opening and closing block comment delimiters.
	// </p>
	block_comment_delim_arr []BlockCommentDelim
	// <p>An array of strings which specify the beginning/end of a long string.
	//     Long strings may contain newlines.</p>
	long_string_delim_arr []string
	// <p>An array of strings which specify the beginning/end of a short string.
	//     Short strings may only contain escaped newlines, such as
	//     <code>\n</code>, or newlines with a line continuation character (such as
	//     <code>\</code> followed by a newline).</p>
	short_string_delim_arr []string
	// <p>An array of heredocs delimiters.</p>
	heredoc_delim_arr []HeredocDelim
	// <p>Template literal support (for languages such as JavaScript, TypeScript,
	//     etc.). A value of <code>none</code> indicates the lexer is inside a template; this should only be used by the <code>source_lexer</code> itself.</p>
	template_literal TemplateLiteral
}

const language_lexer_arr = [
    // <p>Note: the C/C++ support expects C++11 or newer. Don't worry about
    //     supporting C or older C++ using another lexer entry, since the raw
    //     string syntax in C++11 and newer is IMHO so rare we won't encounter
    //     it in older code. See the&nbsp;<a
    //         href="https://en.cppreference.com/w/cpp/language/string_literal">C++
    //         string literals docs</a> for the reasoning behind the start body
    //     regex.</p>
    ///            Language name File extensions     IC      Block comment       					Long string     Short str   Heredoc JS tmpl lit
    LanguageLexer{"c_cpp",       [".cc", ".cpp"],    ["//"], [BlockCommentDelim{"/*", "*/"}],     	[],             ['"'],      [HeredocDelim{'R"', "[^()\\ ]", "(", ")", ""}], TemplateLiteral.no},
    LanguageLexer{"html",        [".html"],          [],     [BlockCommentDelim{"<!--", "-->"}],	[],             [],         [],     TemplateLiteral.no},
    LanguageLexer{"javascript",  [".js", ".mjs"],    ["//"], [BlockCommentDelim{"/*", "*/"}],     	[],             ['"', "'"], [],     TemplateLiteral.yes},
    LanguageLexer{"json5",       [".json"],          ["//"], [BlockCommentDelim{"/*", "*/"}],     	[],             ['"', "'"], [],     TemplateLiteral.no},
    LanguageLexer{"python",      [".py"],            ["#"],  [],                 					['"""', "'''"], ['"', "'"], [],     TemplateLiteral.no},
    LanguageLexer{"typescript",  [".ts", ".mts"],    ["//"], [BlockCommentDelim{"/*", "*/"}],     	[],             ['"', "'"], [],     TemplateLiteral.yes},
    LanguageLexer{"verilog",     [".v"],             ["//"], [BlockCommentDelim{"/*", "*/"}],     	[],             ['"'],      [],     TemplateLiteral.no},
    LanguageLexer{"vlang",       [".v"],             ["//"], [BlockCommentDelim{"/*", "*/"}],     	[],             ['"', "'"], [],     TemplateLiteral.no},
    LanguageLexer{"yaml",        [".yaml",".yml"],   ["#"],  [],                 					[],           	['"', "'"], [],     TemplateLiteral.no},
    LanguageLexer{"codechat-html", [".cchtml"],      [""],   [],                 					[],            	[],         [],     TemplateLiteral.no},
]

// This describes either a code block or a doc bock.
struct CodeDocBlock {
	is_doc_block bool
	// For a doc block, the whitespace characters which created the indent for this doc block. For a code block, <code>none</code>.
	indent string
	// The contents of this block -- documentation (with the comment delimiters removed) or code.
	contents string
}

struct RegexBuilder {
	mut:
	// An array of regular expressions, each of which matches one type of identifier. Initialize it to an empty array.
	regex_strings []string = []
	// The group index of the last group when <code>regex_strings</code> is combined into a single regex. Initialize it to -1 (just before group 0, since there are no groups yet).
	group_index int = -1
}


// <h2>Source lexer</h2>
// <p>This lexer categorizes source code into code blocks or doc blocks.
fn source_lexer(
    // <p>The source code to lex.</p>
    source_code_ string,
	// A description of the language, used to lexer the <code>source_code</code>.
	language_lexer LanguageLexer,
) []CodeDocBlock {
    // <p>Rather than attempt to lex the entire language, this lexer's only goal
    //     is to categorize all the source code into code blocks or doc blocks.
    //     To do it, it only needs to:</p>
    // <ul>
    //     <li>Recognize where comments can't be&mdash;inside strings, <a
    //             href="https://en.wikipedia.org/wiki/Here_document">here
    //             text</a>, or <a
    //             href="https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Template_literals">template
    //             literals</a>. These are always part of a code block and can
    //         never contain a comment or (by implication) a doc block.</li>
    //     <li>Outside of these special cases, look for inline or block
    //         comments, categorizing everything else as plain code.</li>
    //     <li>After finding either an inline or block comment, determine if
    //         this is a doc block.</li>
    // </ul>
    // <p>To accomplish this goal, construct a <a
    //         href="https://developer.mozilla.org/en-US/docs/Web/JavaScript/Guide/Regular_Expressions">regex</a>
    //     named <code>classify_regex</code> and associated indices from the
    //     language information provided (<code>language_name</code>,
    //     <code>extension_strings</code>, etc.). It&nbsp;divides source code
    //     into two categories: plain code and special cases. The special cases
    //     consist of:</p>
    // <ul>
    //     <li>String-like code (strings, here text, template literals). In this
    //         case, the lexer must find the end of the string-like element
    //         before it can return to plain code.</li>
    //     <li>Comments (inline or block). In this case, the lexer must find the
    //         end of the comment before it can return to plain code.</li>
    // </ul>
    // <p>This regex assumes the string it analyzes was preceded by plain code;
    //     its purpose is to identify the start of the next special case.
    //     <strong>This code makes heavy use of regexes -- read the previous
    //         link thoroughly.</strong></p>
    // <p>Use an index, since we need to know which special case (a string,
    //     inline comment, etc.) the regex found.</p>
    // <p>Produce the overall regex from regexes which find a specific special
    //     case.</p>
    // <p>Given an array of strings containing unescaped characters which
    //     identifies the start of one of the special cases, combine them into a
    //     single string separated by an or operator. Return the index of the
    //     resulting string in <code>regex_strings</code>, or -1
    //     if the array is empty (indicating that this language doesn't support
    //     the provided special case).</p>

	mut regex_builder := RegexBuilder{}
    // <p>Order these statements by length of the expected strings, since the
    //     regex with an or expression will match left to right.</p>
    // <p>Include only the opening block comment string (element 0) in the
    //     regex.</p>
    block_comment_index := regex_builder.append(language_lexer.block_comment_delim_arr.map(it.opening))
    long_string_index := regex_builder.append(language_lexer.long_string_delim_arr)
    inline_comment_index := regex_builder.append(language_lexer.inline_comment_delim_arr)
    short_string_index := regex_builder.append(language_lexer.short_string_delim_arr)
    // <p>Template literals only exist in JavaScript. No other language (that I
    //     know of) allows comments inside these, or nesting of template
    //     literals.</p>
	template_literal_index := if language_lexer.template_literal == .yes || language_lexer.template_literal == .nested {
        // <p>If inside a template literal, look for a nested template literal
        //     (<code>`</code>) or the end of the current expression
        //     (<code>}</code>).</p>
        regex_builder.regex_strings << if language_lexer.template_literal == .yes { "`" } else { "`|}" }
        short_string_index + 1
    } else { -1 }
    mut classify_regex := regex.regex_opt("(" + regex_builder.regex_strings.join(")|(") + ")") or { panic(err) }

	// Prepare regexes for other points in the code.
	newline_regex_fragment := "((\r\n)|\n|\r)"
	mut newline_regex := regex.regex_opt(newline_regex_fragment) or { panic(err) }
	// This regex matches to the end of the current line, treating line continuations as part of the current line.
	mut end_of_line_regex := regex.regex_opt(
		// Look for
		"(" +
			// a line continution character followed by a newline (note the double slash, to avoid escaping the next character in the regex)
			r"\\" + newline_regex_fragment +
			// or anything except a newline
			r"|[^\n\r]" +
		// zero or more times,
		r")*" +
		// followed by a newline.
		newline_regex_fragment
	) or { panic(err) }
	mut whitespace_only_regex := regex.regex_opt(r"^\s*$") or { panic(err) }

	mut source_code := source_code_
    mut classified_source := []CodeDocBlock{}
    // <p>An accumulating array of strings composing the current code block.</p>
    mut code_block_array := []string{}
    for source_code.len != 0 {
        // <p>Look for the next special case. Per the earlier discussion, this
        //     assumes that the text immediately
        //     preceding&nbsp;<code>source_code</code> was plain code.</p>
        classify_match := re_find(mut classify_regex, source_code)

        if classify_match.matched() {
            // <p>Move everything preceding this match from
            //     <code>source_code</code> to the current code block, since per
            //     the assumptions this is code. Per the <a
            //         href="https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/RegExp/exec#return_value">docs</a>,
            //     <code>m.index</code> is the index of the beginning of the
            //     match.</p>
            code_block_array << source_code[..classify_match.start]
            source_code = source_code[classify_match.start..]

            // <h3>Determine which special case matched</h3>
            // <p>Was this special case a comment? If so, then
            //     <code>m[inline_comment_index]</code> will be
            //     non-empty.</p>
			inline_comment_string := classify_match.group(inline_comment_index)
            if inline_comment_string != "" {
                // <p>An inline comment delimiter matched.</p>
                // <p><strong>First</strong>, find the end of this comment: a
                //     newline that's not escaped by a line continuation
                //     character (which is <code>\</code> in C/C++/many
                //     languages). Note that using a negative lookbehind
                //     assertion would make this much simpler:
                //     <code>/(?&lt;!\\)(\n|\r\n|\r)/</code>. However, V doesn't
                //     support this.</p>
                end_of_comment := re_find(mut end_of_line_regex, source_code)

                // <p>Assign <code>full_comment</code> to contain the entire
                //     comment, from the inline comment delimiter until the
                //     newline which ends the comment. No matching newline means
                //     we're at the end of the file, so the comment is all the
                //     remaining <code>source_code</code>.</p>
                full_comment := if end_of_comment.matched() { source_code[0..end_of_comment.stop] } else { source_code }

                // <p>Move to the next block of source code to be lexed.</p>
                source_code = source_code[full_comment.len..]

				// Currently, <code>code_block_array</code> contains preceding code
                //     (which might be multiple lines) until the inline comment
                //     delimiter. Split this on newlines, grouping all the lines before the last line into <code>code_lines_before_comment</code> (which is all code), and everything else (from the beginning of the last line to where the inline comment delimiter appears) into <code>comment_line_prefix</code>. For example, consider the fragment <code>a = 1\nb = 2 // Doc</code>. After processing, <code>code_lines_before_comment == "a = 1\n"</code> and <code>comment_line_prefix == "b = 2 "</code>.
                code_block_string := code_block_array.join("")
				// This horrible syntax is like arr[-1], but negative indexes aren't directly supported (only negative indexes for a slice).
                comment_line_prefix := newline_regex.split(code_block_string)#[-1..][0]
				// We can't use <code>[..-code_block_string.len]</code>: if it's empty, then <code>[..0]</code> produces an empty string, instead of the entire string.
				code_lines_before_comment := code_block_string#[..code_block_string.len - comment_line_prefix.len]

                // <p><strong>Next</strong>, determine if this comment is a doc
                //     block. Criteria for doc blocks for an inline comment:</p>
                // <ul>
                //     <li>All characters preceding the comment on the line
                //         containing the comment must be whitespace.</li>
                //     <li>Either:
                //         <ul>
                //             <li>The inline comment delimiter is immediately
                //                 followed by a space, or</li>
                //             <li>the inline comment delimiter is followed by a
                //                 newline or the end of the file.</li>
                //         </ul>
                //     </li>
                // </ul>
                // <p>With this last line located, apply the doc block criteria.
                // </p>
				ws := re_find(mut whitespace_only_regex, comment_line_prefix)
  				println(ws.matched())
				println(comment_line_prefix)
				// Criteria 1 -- the whitespace matched. BUG in regex: the <code>whitespace_only_regex</code> doesn't match an empty string.
				if (ws.matched() || comment_line_prefix == "") && (
					// Criteria 2.1
					full_comment.starts_with(inline_comment_string + " ") ||
					// Criteria 2.2
					(full_comment == inline_comment_string + if end_of_comment.matched()
						// Compare with a newline if it was found; the group of the found newline is 8.
						{ end_of_line_regex.get_group_by_id(source_code, 8) } else
						// Compare with an empty string if there's no newline.
						{ "" }
					)
				) {
                    // <p>This is a doc block. Transition from a code block to
                    //     this doc block.</p>
                    if code_lines_before_comment != "" {
                        // <p>Save only code blocks with some content.</p>
                        classified_source << CodeDocBlock{false, "", code_lines_before_comment}
                    }
                    code_block_array = []

                    // <p>Add this doc block by pushing the array [whitespace
                    //     before the inline comment, inline comment contents,
                    //     inline comment delimiter]. Since it's a doc block,
                    //     then <code>comment_line_prefix</code> contains
                    //     the whitespace before this comment.
                    //     <code>inline_comment_string</code> contains the
                    //     inline comment delimiter. For the contents, omit the
                    //     leading space it it's there (this might be just a
                    //     newline or an EOF).</p>
                    has_space_after_comment := full_comment.starts_with(inline_comment_string + " ")
					classified_source << CodeDocBlock{true, comment_line_prefix, full_comment[inline_comment_string.len + if has_space_after_comment { 1 } else { 0 }..]}
				}
			}
		}
		break
	}
	/*
                } else {
                    // <p>This is still code.</p>
                    code_block_array.push(full_comment);
                }
            } else if (m[block_comment_index]) {
                // <p>A block comment. Find the end of it.</p>
                // for now just match c++ style comments Start with /* and end with */
                const end_of_comment_match = source_code.match(/\*\//);
                // <p>Assign <code>full_comment</code> to contain the entire
                //     comment, from the block comment start until the block
                //     comment end. No matching end means we're at the end of the
                //     file, so the comment is all the remaining
                //     <code>source_code</code>.</p>

                const full_comment = end_of_comment_match
                    ? source_code.substring(
                          0,
                          end_of_comment_match.index +
                              end_of_comment_match[0].length
                      )
                    : source_code;

                // starting at the block comment closing delimiter add everything until the next newline
                const after_close = source_code
                    .substring(full_comment.length)
                    .match(/(\\\r\n|\\\n|\\\r|[^\\\n\r])*(\n|\r\n|\r)/);
                // <p>Move to the next block of source code to be lexed.</p>
                source_code = source_code.substring(full_comment.length);

                let code_block = code_block_array.join("");
                const comment_line_prefix = code_block
                    .split(/\n|\r\n|\r/)
                    .at(-1);
                // <p>With this last line located, apply the doc block criteria.
                // </p>
                const block_comment_string = m[block_comment_index];

                // doc block criteria for a block comment:
                // 1. must have whitespace after the opening comment delimiter
                // 2. must not have anything besides whitespace before the opening comment delimiter on the same line
                // 3. must not have anything besides whitespace after the closing comment delimiter on the same line
                // 4. MAY have whitespace before the closing comment delimiter on the same line

                // check after_close for non-whitespace characters

                if (
                    comment_line_prefix.match(/^\s*$/) &&
                    full_comment.startsWith(block_comment_string + " ") &&
                    full_comment.endsWith("*delme/") &&
                    (!after_close || after_close[0].match(/^\s*$/))
                ) {
                    // <p>This is a doc block. Transition from a code block to
                    //     this doc block.</p>
                    code_block = code_block.substring(
                        0,
                        code_block.length - comment_line_prefix.length
                    );
                    if (code_block) {
                        // <p>Save only code blocks with some content.</p>
                        classified_source.push([null, code_block, ""]);
                    }
                    code_block_array = [];
                    const has_space_after_comment =
                        full_comment[block_comment_string.length] === " ";
                    // don't add the closing *delme/ to the comment
                    classified_source.push([
                        comment_line_prefix,
                        full_comment.substring(
                            block_comment_string.length +
                                (has_space_after_comment ? 1 : 0),
                            full_comment.length - 2
                        ),
                        block_comment_string,
                    ]);
                } else {
                    // <p>This is still code.</p>
                    code_block_array.push(full_comment);
                }
            } else if (m[long_string_index]) {
                // <p>A long string. Find the end of it.</p>
                code_block_array.push(m[long_string_index]);
                source_code = source_code.substring(
                    m[long_string_index].length
                );
                const string_m = source_code.match(m[long_string_index]);
                // <p>Add this to the code block, then move forward. If it's not
                //     found, the quote wasn't properly closed; add the rest of
                //     the code.</p>
                if (string_m) {
                    const index = string_m.index + string_m[0].length;
                    code_block_array.push(source_code.substring(0, index));
                    source_code = source_code.substring(index);
                } else {
                    code_block_array.push(source_code);
                    source_code = "";
                }
            } else if (m[short_string_index]) {
                // <p>A short string. Find the end of it.</p>
                code_block_array.push(m[short_string_index]);
                source_code = source_code.substring(
                    m[short_string_index].length
                );
                // prettier-ignore
                const string_m = source_code.match(
                    // <p>Use <a
                    //         href="https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/String/raw"><code>String.raw</code></a>
                    //     so we don't have to double the number of backslashes
                    //     in this regex. Joining regex literals doesn't work
                    //     &ndash; <code>/.a/ +
                    //         /b/</code> produces the string
                    //     <code>'/.a//b/'</code>, not a regex. The regex is:
                    // </p>
                    // <p>Look for anything that doesn't terminate a string:</p>
                    "(" +
                        // <p>a backslash followed by a newline (in all three
                        //     newline styles);</p>
                        String.raw`\\\r\n|\\\n|\\\r|` +
                        // <p>a backslash followed by any non-newline character
                        //     (note that the <code>.</code> character class <a
                        //         href="https://developer.mozilla.org/en-US/docs/Web/JavaScript/Guide/Regular_Expressions/Character_Classes#types">doesn't
                        //         match newlines</a>; using the <code>s</code>
                        //     or <code>dotAll</code> flag causes it to match <a
                        //         href="https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Lexical_grammar#line_terminators">line
                        //         terminators</a> that we don't recognize, plus
                        //     not match a <code>\r\n</code> sequence);</p>
                        String.raw`\\.|` +
                        // <p>anything that's not a backslash, quote mark, or
                        //     newline.</p>
                        String.raw`[^\\${m[short_string_index]}\n\r]` +
                        // <p>Find as many of these as possible. Therefore, the next
                        //     token will be the end of the string.</p>
                    ")*" +
                    // <p>A string is terminated by either a quote mark or a
                    //     newline. (We can't just put <code>.</code>, because
                    //     one flavor of newline is two characters; in addition,
                    //     that character class doesn't match newlines, as
                    //     stated above.) Terminating strings at a newline helps
                    //     avoid miscategorizing large chunks of code that the
                    //     compiler likewise flags as a syntax error.</p>
                    String.raw`(${m[short_string_index]}|\r\n|\n|\r)`
                );
                if (string_m) {
                    const index = string_m.index + string_m[0].length;
                    code_block_array.push(source_code.substring(0, index));
                    source_code = source_code.substring(index);
                } else {
                    code_block_array.push(source_code);
                    source_code = "";
                }
            } else if (m[template_literal_index]) {
                // <p>TODO! For now, just assume there's no comments in
                //     here...dangerous!!!</p>
                code_block_array.push(m[template_literal_index]);
                source_code = source_code.substring(
                    m[template_literal_index].length
                );
            } else {
                console.assert(false);
                debugger;
            }
        } else {
            // <p>The rest of the source code is in the code block.</p>
            code_block_array.push(source_code);
            source_code = "";
        }
    }

    // <p>Include any accumulated code in the classification.</p>
    const code = code_block_array.join("");
    if (code) {
        classified_source.push([null, code, ""]);
    }
	*/
	return classified_source
}

// A language is typically specified using several alternative delimiters; for exaple, a JavaScript string may start with a single quote or a double quote. Given a <code>string_arr</code> of these, this function joins them with a regex or operator.
//
// Goal:
// - produce an array of regex strings
// - Produce an index or a "not present" for each type of delimiter.
//
// Is there some way to write this functionally? Need two index return values. But this seems silly, using state is simpler.
//
// Internal state:
fn (mut regex_builder RegexBuilder) append(
	// An array of alternative delimiters, which will be combined with a regex or (<code>|</code>) operator
	string_arr []string)
	// The index of the group which corresponds to <code>string_arr</code>, or -1 if <code>string_arr</code> is empty.
	int
{
	if string_arr == [] {
		return -1
	}
	regex_builder.regex_strings <<
		// <p>Escape any regex characters in these strings. Wrap each string in parens, since the V regex treats an expression such as <code>"""|'''</code> as "look for two double quotes, then a double quote or a single quote, then two single quotes." Instead, this must be <code>(""")|(''')</code> to express "look for three double quotes or three single quotes."</p>
		"(" + string_arr.map(escape_regex(it)).join(")|(") + ")"
	// Adjust the regex index:
	regex_builder.group_index +=
		// this entire expression will (eventually) be wrapped in parens,
		1 +
		// plus we added a group for each element in <code>string_arr</code>.
		string_arr.len
	return regex_builder.group_index
}


// Given a string, insert escapes so this string can be used in a regex. For example, the C/C++ opening block comment of <code>/*</code> must be escaped to <code>/\*</code>, so that <code>*</code> matches an asterisk, not any character.

fn escape_regex(str_to_escape string) string {
	mut escape_regex_re := regex.regex_opt(r"([.*+?^${}()|[\]\\])") or { panic(err) }
	return escape_regex_re.replace(str_to_escape, r"\\0")
}


// Make regexes easier to use by providing a match object.
struct RegexMatch {
	re regex.RE
	in_txt string
	start int
	stop int
}


// Ideally, this would be part of the regex module instead.
fn re_find(mut re regex.RE, in_txt string) (RegexMatch) {
	start, stop := re.find(in_txt)
	return RegexMatch{re, in_txt, start, stop}
}


fn (rm RegexMatch) matched() bool {
	return rm.start != regex.no_match_found
}


fn (rm RegexMatch) group(index int) string {
	return rm.re.get_group_by_id(rm.in_txt, index)
}
