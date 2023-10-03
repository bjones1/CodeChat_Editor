Copyright (C) 2022 Bryan A. Jones.

This file is part of the CodeChat Editor.

The CodeChat Editor is free software: you can redistribute it and/or modify it
under the terms of the GNU General Public License as published by the Free
Software Foundation, either version 3 of the License, or (at your option) any
later version.

The CodeChat Editor is distributed in the hope that it will be useful, but
WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more
details.

You should have received a [copy](LICENSE.html) of the GNU General Public
License along with the CodeChat Editor. If not, see
[https://www.gnu.org/licenses/](https://www.gnu.org/licenses/).

# Lexer walkthrough

This walkthrough shows how the lexer parses the following Python code fragment:

<code>print(<span style="color: rgb(224, 62, 45);">"""¶</span></code>\
<code><span style="color: rgb(224, 62, 45);"># This is not a comment! It's a multi-line
string.¶</span></code>\
<code><span style="color: rgb(224, 62, 45);">"""</span>)¶</code>\
<code><span style="color: rgb(45, 194, 107);"># This is a comment.</span></code>

Paragraph marks (the ¶ character) are included to show how the lexer handles
newlines. To explain the operation of the lexer, the code will be highlighted
in yellow to represent the
<span style="background-color: rgb(251, 238, 184);">unlexed source code</span>,
represented by the contents of the
variable `source_code[source_code_unlexed_index..]` and in green for the
<span style="background-color: rgb(191, 237, 210);">current code block</span>,
defined by `source_code[current_code_block_index..source_code_unlexed_index]`.
Code that is classified by the lexer will be placed in the `classified_code`
array.

## Start of parse

The <span style="background-color: rgb(251, 238, 184);">unlexed source
code</span> holds all the code (everything is highlighted in yellow); the
<span style="background-color: rgb(191, 237, 210);">current code block</span>
is empty (there is no green highlight).

<span style="background-color: rgb(251, 238, 184);"><code>print(<span style="color: rgb(224, 62, 45);">"""¶</span></code></span>\
<span style="background-color: rgb(251, 238, 184);"><code><span style="color: rgb(224, 62, 45);">#
This is not a comment! It's a multi-line string.¶</span></code></span>\
<span style="background-color: rgb(251, 238, 184);"><code><span style="color: rgb(224, 62, 45);">"""</span>)¶</code></span>\
<code><span style="background-color: rgb(251, 238, 184);">&nbsp; <span style="color: rgb(45, 194, 107);">#
This is a comment.</span></span></code>

```
classified_code = [
]
```

## Search for a token

The lexer begins by searching for the regex in
`language_lexer_compiled.next_token`, which is `(\#)|(""")|(''')|(")|(')`. The
first token found is
<span style="color: rgb(224, 62, 45);"><code>"""</code></span>. Everything up
to the match is moved from the unlexed source code to the current code block,
giving:

<code><span style="background-color: rgb(191, 237, 210);">print(</span><span style="color: rgb(224, 62, 45); background-color: rgb(251, 238, 184);">"""¶</span></code>\
<span style="background-color: rgb(251, 238, 184);"><code><span style="color: rgb(224, 62, 45);">#
This is not a comment! It's a multi-line string.¶</span></code></span>\
<span style="background-color: rgb(251, 238, 184);"><code><span style="color: rgb(224, 62, 45);">"""</span>)¶</code></span>\
<code><span style="background-color: rgb(251, 238, 184);">&nbsp; <span style="color: rgb(45, 194, 107);">#
This is a comment.</span></span></code>

```
classified_code = [
]
```

## String processing

The regex is accompanied by a map named `language_lexer_compiled.map`, which
connects the mapped group to which token it matched (see
`struct RegexDelimType`):

```
Regex:           (#)       |  (""") | (''')  |  (")   |  (')
Mapping:    Inline comment   String   String   String   String
Group:            1            2        3        4        5
```

Since group 2 matched, looking up this group in the map tells the lexer it’s a
string, and also gives a regex which identifies the end of the string . This
regex identifies the end of the string, moving it from the
<span style="background-color: rgb(251, 238, 184);">(unclassified) source
code</span> to the (classified)
<span style="background-color: rgb(191, 237, 210);">current code block</span>.
It correctly skips what looks like a comment but is not a comment. After this
step, the lexer’s state is:

<span style="background-color: rgb(191, 237, 210);"><code>print(<span style="color: rgb(224, 62, 45);">"""¶</span></code></span>\
<span style="background-color: rgb(191, 237, 210);"><code><span style="color: rgb(224, 62, 45);">#
This is not a comment! It's a multi-line string.¶</span></code></span>\
<code><span style="color: rgb(224, 62, 45); background-color: rgb(191, 237, 210);">"""</span><span style="background-color: rgb(251, 238, 184);">)¶</span></code>\
<code><span style="background-color: rgb(251, 238, 184);">&nbsp; <span style="color: rgb(45, 194, 107);">#
This is a comment.</span></span></code>

```
classified_code = [
]
```

## Search for a token (second time)

Now, the lexer is back to its state of looking through code (as opposed to
looking inside a string, comment, etc.). It uses the `next_token` regex as
before to identify the next token
<span style="color: rgb(45, 194, 107);"><code>#</code></span> and moves all the
preceding characters from source code to the current code block. The lexer
state is now:

<code><span style="background-color: rgb(191, 237, 210);">print(<span style="color: rgb(224, 62, 45);">"""¶</span></span></code>\
<span style="background-color: rgb(191, 237, 210);"><code><span style="color: rgb(224, 62, 45);">#
This is not a comment! It's a multi-line string.¶</span></code></span>\
<span style="background-color: rgb(191, 237, 210);"><code><span style="color: rgb(224, 62, 45);">"""</span>)¶</code></span>\
<code><span style="background-color: rgb(191, 237, 210);">&nbsp; </span><span style="color: rgb(45, 194, 107);"><span style="background-color: rgb(251, 238, 184);"><code>#
This is a comment.</code></span></span></code>

```
classified_code = [
]
```

## Inline comment lex

Based on the map, the lexer identifies this as an inline comment. The inline
comment lexer first identifies the end of the comment (the next newline or, as
in this case, the end of the file), putting the entire inline comment except
for the comment opening delimiter
<span style="color: rgb(45, 194, 107);"><code>#</code></span> into
<span style="background-color: rgb(236, 240, 241);"><code>full_comment</code></span>.
It then splits the current code block into two
groups: <span style="background-color: rgb(236, 202, 250);"><code>code_lines_before_comment</code></span>
(lines in the current code block which come before the current line) and the
<span style="background-color: rgb(194, 224, 244);"><code>comment_line_prefix</code></span>
(the current line up to the start of the comment). The classification is:

<code><span style="background-color: rgb(236, 202, 250);">print(<span style="color: rgb(224, 62, 45);">"""¶</span></span></code>\
<span style="background-color: rgb(236, 202, 250);"><code><span style="color: rgb(224, 62, 45);">#
This is not a comment! It's a multi-line string.¶</span></code></span>\
<span style="background-color: rgb(236, 202, 250);"><code><span style="color: rgb(224, 62, 45);">"""</span>)¶</code></span>\
<code><span style="background-color: rgb(194, 224, 244);">&nbsp; </span><span style="color: rgb(45, 194, 107);">#<span style="background-color: rgb(236, 240, 241);">
This is a comment.</span></span></code>

```
classified_code = [
]
```

## Code/doc block classification

Because
<code><span style="background-color: rgb(194, 224, 244);">comment_line_prefix</span></code>
contains only whitespace and
<span style="background-color: rgb(236, 240, 241);">full_comment</span> has a
space after the comment delimiter, the lexer classifies this as a doc block. It
adds <span style="background-color: rgb(236, 202, 250);">code_lines_before_comment</span>
as a code block, then the text of the comment as a doc block:

```
classified_code = [
  Item 0 = CodeDocBlock {
    indent: "", delimiter: "", contents = "print("""¶
# This is not a comment! It's a multi-line string.¶
""")¶
"},
  Item 1 = CodeDocBlock {
    indent: "  ", delimiter: "#", contents = "This is a comment"
  },
]
```

## Done

After this, the unlexed source code is empty since the inline comment
classified moved the remainder of its contents into `classified_code`. The
function exits.
