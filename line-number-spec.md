Markdown source line numbers
============================

Goal
----

The Server includes source line markers in the HTML it sends to the Client
for Markdown documents and for the doc blocks of source files. A marker such
as `<l-n>24</l-n>` says that the content after it starts on line 24 of the
original file. For example, a paragraph on lines 24 and 25 is sent as:

```html
<p><l-n>24</l-n>First line
<l-n>25</l-n>Second line</p>
```

The markers are hidden metadata for later features. `TargetFact.line` and
`FragmentFact.line` stay 0 (see the comment in `processing.rs`, "Line numbers
aren't available ..."); filling them is out of scope, as is any visible
line-number interface.

Placement
---------

Lines count from 1 in the original file, including blank lines and code.
Every caller passes LF-normalized text.

Insert a marker before the first *inline event* which starts on a line not
yet marked. The inline events are `Text`, `Code`, `InlineMath`,
`DisplayMath`, `InlineHtml`, `FootnoteReference`, `TaskListMarker`, and the
`Start` of `Emphasis`, `Strong`, `Strikethrough`, `Superscript`, `Subscript`,
`Link` and `Image`. These always sit inside a paragraph, heading, list item,
table cell or definition, so a marker never lands between `<tr>` and `<td>`,
where html5ever would move it.

Insert nothing inside an image (its text becomes the `alt` attribute), a code
block, a raw HTML block or a metadata block. Insert nothing either after a
tag, inline or in an HTML block, which opens one of these elements, up to the
tag which closes it or the end of the doc block:

* text html5ever doesn't parse as HTML: `iframe`, `noembed`, `noframes`,
  `noscript`, `script`, `style`, `textarea`, `title`, `xmp`;
* text a diagram reads as its source: `wc-mermaid`, `graphviz-graph`;
* `xref`, whose content `hydrate_source_xref` copies into `data-content`,
  which a save writes back unchanged.

A marker there would become visible text, or be saved.

A line with no inline event of its own gets no marker: a blank line, a code
fence, a setext underline, a thematic break, a table delimiter row, a link
reference definition, or the second line of a multi-line inline code span,
inline HTML tag or display math.

This is the prototype `add_line_comments` (book repo,
`project_notes/md-ast-line-numbers`), with `<l-n>` in place of the HTML
comment and the marker moved from the first event of a line to its first
inline event.

Server changes
--------------

All in `server/src/processing.rs` unless noted.

1. `markdown_to_html` gains a parameter: the byte ranges of its input which
   come from the file, each with the file line of its first line. It inserts
   `InlineHtml("<l-n>N</l-n>")` events as described above. Text outside the
   ranges (the doc block separators) gets no marker; an empty list gives the
   old output.
2. Markdown documents pass one range, the whole input, starting at line 1.
3. Source files: the loop which builds `doc_contents` records each doc
   block's range and first line, which is 1 plus the sum of
   `code_doc_block_lines` over the blocks before it (the count
   `render_fragment_content` uses for its gutter).
4. `AMMONIA_OPTIONS`: add `l-n` to `add_tags`, so `minify` keeps markers.
5. `CLEAN_LINK_CONTENT_HTML` and `CLEAN_FRAGMENT_HTML`: add `l-n` to
   `add_clean_content_tags`. Link text copied from a heading, and gathered
   fragments, then carry no markers from another place.
6. `dehydrating_walk_node`: remove every `<l-n>` with its number. `l-n` is
   reserved for generated markers; one written by hand is removed too.
7. `compare_html` (`translation.rs`): remove markers from both inputs before
   comparing. Otherwise every Client edit which shifts lines makes the Server
   send a re-translation.
8. `send_re_translation` (`translation.rs`): when a re-translation is sent
   anyway (something besides markers differs), keep the Client's contents for
   each doc block which `compare_html` finds equal. A new marker in the doc
   block being edited would otherwise mark TinyMCE dirty and shift the child
   index the Client uses to restore the caret. Markers in the Client may be
   stale after a Client edit until the next translation which isn't a
   re-translation.

Client changes
--------------

* `tinymce-config.mts`: add `l-n` to `extended_valid_elements` and `~l-n`
  (inline) to `custom_elements`; TinyMCE drops unknown tags but keeps their
  text.
* `css/CodeChatEditor.css`: `l-n { display: none; }`, outside the cascade
  layer so no theme can show it. The TOC page loads this file too.
* `tinymce-config.mts`, `handleLineMarkers`: a block holding only markers
  gets the `<br data-mce-bogus="1">` TinyMCE gives an empty block. TinyMCE
  counts the marker's number as content, so without this the block has no
  height and is lost on save. Such a block appears when Enter splits a
  paragraph just after its marker (`NewBlock` event) and when TinyMCE parses
  contents (a parser node filter on `l-n`), since the parser removes the
  `<br>` which ends a block it doesn't consider empty.
* `tinymce-config.mts`, `handleLineMarkers`: remove markers from selection
  contents (`GetContent` with `selection`), which TinyMCE puts on the
  clipboard on copy or cut.
* `CodeMirror-integration.mts`: the walk which puts the caret at the start of
  a doc block entered from above skips a marker, so the caret doesn't land in
  hidden text.

Known deviation
---------------

For a block comment whose text starts on the line after the opening
delimiter (`/*⏎`), the generic lexer (every language but C, C++ and Python)
drops that newline, so the markers in it are one line early, and each such
comment moves every later marker in the file one more line early. The
fragment gutter numbers have the same offset; fixing the lexer is out of
scope.

Tests
-----

In `server/src/processing/tests.rs` (exact expected HTML) and
`server/src/translation.rs`:

| Input | Expected |
| --- | --- |
| Markdown: two prose lines, blank line, `# Head` | Markers 1 and 2 in the `<p>`, 4 in the `<h1>` |
| Prose on lines 1 and 7 around a fenced code block on 3-5; a metadata block on lines 1-3, then prose | Markers 1 and 7 only; same with a raw HTML block and with `$$` display math (marker on its first line); marker 5 only after the metadata block |
| Hard break (two spaces, `\`, `<br>`), list continuation, block quote | One marker per line, inside the `<p>`, `<li>` or quote |
| Table with an empty first cell; task list | One marker per row, in its first cell with content; marker before the checkbox |
| Image whose alt text spans two lines, or holds `<script>`; link label spanning two lines | No marker in `alt`, and the line after the image is marked; marker for the label's second line inside the link text |
| JavaScript file: 23 code lines, then two doc block lines, code, another doc block | Markers 24 and 25; the later block uses its file line |
| `<xref>` to a heading in the same file, with text after a two-line one; gathered fragment | No marker in the link text, `data-content` or gathered copy; the text after the two-line `<xref>` is marked |
| `<iframe` tag spanning two lines, then prose; `</iframe>` on a line of its own; an `<iframe>` line, a blank line, more text; `<wc-mermaid>` over three lines; a JavaScript doc block which leaves `<wc-mermaid>` open | No marker inside any of these elements; the text after each `<iframe>` is marked, as is the next doc block |
| Save a translated file with markers (Markdown and JavaScript) | Source identical to the input |
| `compare_html` on HTML that differs only in markers, also with a marker before an `<iframe>` which TinyMCE wrapped in a paragraph | Equal |
| Re-translation whose doc block differs only in markers | The Client's contents are kept; browser tests `overall_1::test_client_updates` and `overall_3::test_8` fail without this |
| Browser: Enter at the start of a paragraph, then Enter in the empty paragraph it leaves (`overall_3::test_8`) | The empty paragraph keeps its height and can be entered |

Existing expected HTML in `processing/tests.rs`, `ide/vscode/tests.rs` and
`server/tests/overall*.rs` gains markers; removing them must give the old
expectations exactly, except that diff lengths grow by the markers' length.
Tests that call `markdown_to_html` directly pass an empty list and don't
change. The clipboard change in the Client is checked by hand; the caret
change has no test.

<h3 id="cc-FekMIBZOuZ" data-gather="cc-Ux5oCqGB58 cc-ehJC2b6tRN cc-LVzecWlvge cc-3Q7ZgTnUbT cc-fHkfDh9IcD cc-AZYIpZE6s0 cc-fI5iKBnJat">Implementation of Markdown source line numbers</h3>
