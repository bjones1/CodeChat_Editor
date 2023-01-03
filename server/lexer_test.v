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
// <h1><code>test_lexer.v</code>&mdash;Test <code>lexer.v</code></h1>
// <h2>Imports</h2>
// This marks these tests as internal, giving them access to private functions. See the <a href="https://github.com/vlang/v/blob/master/doc/docs.md#test-files">docs</a>.
module main

import regex

fn test_escape_regex() {
	// Check the empty case.
	assert escape_regex('') == ''

	// Check the no-replacement case.
	assert escape_regex('#') == '#'

	// Test a basic C opening block comment delimiter.
	assert escape_regex(r'/*') == r'/\*'

	// Make sure multiple replacements happen.
	assert escape_regex(r'/. ./') == r'/\. \./'
}

fn test_regex_builder() {
	// If there are no strings to add, the regex array and index should remain unchanged.
	mut regex_builder := RegexBuilder{}
	assert regex_builder.append([]) == -1

	// Add one items
	regex_builder = RegexBuilder{['one', 'two'], -1}
	assert regex_builder.append(['three']) == 1
	assert regex_builder.regex_strings == ['one', 'two', '(three)']

	// Add multiple items.
	regex_builder = RegexBuilder{['one', 'two'], 2}
	assert regex_builder.append(['three', 'four']) == 5
	assert regex_builder.regex_strings == ['one', 'two', '(three)|(four)']
}

fn test_source_lexer_1() {
	mut r := regex.regex_opt(r'b*') or { panic(err) }
	println(r.matches_string('a'))
	println(r.matches_string(''))

	python_lexer := language_lexer_arr[4]
	assert source_lexer('', python_lexer) == []
	assert source_lexer('a = 1\n# Testing', python_lexer) == []
	assert false
}
