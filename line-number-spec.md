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
  which a save writes back unchanged; it may hold another `xref`.

A marker there would become visible text, or be saved. Inside one of these
elements, a tag for another of them opens nothing.

A line with no inline event of its own gets no marker: a blank line, a code
fence, a setext underline, a thematic break, a table delimiter row, a link
reference definition, or the second line of a multi-line inline code span,
inline math, inline HTML tag or display math.

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
   old output. `track_unmarked_element` reads the tags of inline HTML and of
   HTML block lines from left to right to find which of the elements above is
   open.
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
   reserved for generated markers; one written by hand is removed too. Join
   the text on either side of a removed marker into one text node: htmd
   escapes block syntax, such as `2.`, at the start of each text node.
7. `compare_html` (`translation.rs`): remove markers from both inputs before
   normalizing and comparing. Otherwise every Client edit which shifts lines
   makes the Server send a re-translation, and `minify` keeps a space next to
   a marker which it trims without one.
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
* `tinymce-config.mts`, `handleLineMarkers`: when Enter splits a block
  (`NewBlock` event), move each marker with nothing after it in the first half
  to the start of the second half, which holds the text of its line, and give
  an emptied first half the `<br data-mce-bogus="1">` TinyMCE gives an empty
  block. TinyMCE counts the marker's number as content, so otherwise a first
  half holding only markers has no height once saved, and Enter in an
  empty-looking list item adds an item instead of leaving the list.
* `tinymce-config.mts`, `handleLineMarkers`: in the HTML TinyMCE returns
  (`GetContent`), turn a non-breaking space just before a marker into a space.
  TinyMCE makes the space at the end of a text node non-breaking when typing
  near it, and a marker ends a text node at each line.
* `tinymce-config.mts`, `handleLineMarkers`: remove markers from the HTML of
  selection contents (`GetContent` with `selection` and format `html`), which
  TinyMCE puts on the clipboard on copy or cut. The plain text already leaves
  out hidden markers.
* `CodeMirror-integration.mts`: the walk which puts the caret at the start of
  a doc block entered from above skips a marker, so the caret doesn't land in
  hidden text. So does `restoreSelection` in `CodeChatEditor.mts` after a
  re-translation, which can add a marker to a block made in the Client.

Known deviations
----------------

For a block comment whose text starts on the line after the opening
delimiter (`/*⏎`), the generic lexer (every language but C, C++ and Python)
drops that newline, so the markers in it are one line early, and each such
comment moves every later marker in the file one more line early. The
fragment gutter numbers have the same offset; fixing the lexer is out of
scope.

TinyMCE's Find and Replace searches the text of hidden markers: a phrase
which spans two lines isn't found, and a search for a number can find a
marker.

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
| `<xref>` to a heading in the same file, with text after a two-line one, also one holding another `<xref>`, inline or in an HTML block; gathered fragment | No marker in the link text, `data-content` or gathered copy; the text after the two-line `<xref>` is marked |
| `<iframe` tag spanning two lines, then prose; `</iframe>` on a line of its own; an `<iframe>` line, a blank line, more text; a line which closes one `<iframe>` and opens another; `<script>` or `<textarea>` whose text holds `<iframe`; a `<title_x>` line, which isn't `<title>`; `<wc-mermaid>` over three lines; a JavaScript doc block which leaves `<wc-mermaid>` open | No marker inside any of these elements; the text after each element is marked, as is the next doc block |
| Save a translated file with markers (Markdown and JavaScript), also with a line which starts with `2.` | The same source as a save without markers: the input, except that htmd moves `2.` off the start of a line |
| `compare_html` on HTML that differs only in markers, also with a marker before an `<iframe>` which TinyMCE wrapped in a paragraph, and with a space before a marker at the end of a block | Equal |
| Re-translation whose doc block differs only in markers | The Client's contents are kept; browser tests `overall_1::test_client_updates` and `overall_3::test_8` fail without this |
| Browser: Enter at the start of a paragraph, then Enter in the empty paragraph it leaves (`overall_3::test_8`) | The empty paragraph is saved as `<p><br></p>`, as without markers, and can be entered |

Existing expected HTML in `processing/tests.rs`, `ide/vscode/tests.rs` and
`server/tests/overall*.rs` gains markers; removing them must give the old
expectations exactly, except that diff lengths grow by the markers' length.
Tests that call `markdown_to_html` directly pass an empty list and don't
change. Checked by hand in TinyMCE: the clipboard change; Enter in a list
item, before a later line of a paragraph and before a marker; typing next to
a marker. The caret changes have no test.

<h2 id="cc-FekMIBZOuZ" data-gather="cc-Ux5oCqGB58 cc-xUdLzouspj cc-ehJC2b6tRN cc-LVzecWlvge cc-GYm4GGc5ha cc-3Q7ZgTnUbT cc-fHkfDh9IcD cc-AZYIpZE6s0 cc-fI5iKBnJat cc-yDqinUyTF3">Implementation of Markdown source line numbers</h2>
