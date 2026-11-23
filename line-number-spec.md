Markdown source line numbers
============================

Goal
----

The Server includes source line markers in the HTML it sends to the Client
for standalone Markdown documents and Markdown extracted from source-code
documentation comments. Each eligible physical line contributes one marker,
such as `<l-n>24</l-n>`. The number identifies line 24 of the original file.

Observable behavior
-------------------

* Count lines from 1 in the original file, including blank and source-code
  lines. Treat LF and CRLF as one line break each. The final line follows the same placement rules
  whether or not it ends in a newline.
* A marker contains only the original line number in decimal. Place it at
  the first safe DOM position associated with the line, adjacent to its
  rendered content. Multiple parser events on a line yield one marker.
  Multiple source lines rendered in one paragraph or list item retain
  separate markers in source order. Visual wrapping adds no markers.
* Preserve rendering and editing behavior. Markers are hidden metadata.
  Track which elements came from the Server's marker generation through
  the render and save paths. Tag name and numeric content alone do not
  establish that provenance. Remove only generated markers, including their
  numeric text, before reconstructing Markdown. Preserve authored HTML such
  as a literal `<l-n>24</l-n>` according to existing save behavior. A visible
  line-number interface is out of scope.
* Emit markers for the current file's own content. Generated separators,
  fallback labels, and copies inserted by fragment gathering or
  cross-reference hydration do not contribute markers. Preserve normal
  hydration and minification.
* Rendered footnote definitions keep their original source line numbers.
  When the renderer moves a definition into a footnote section, its markers
  move with it. Marker numbers need not increase across the entire HTML.

For example, a paragraph spanning lines 24 and 25 may be sent as follows.

```html
<p><l-n>24</l-n>First line
<l-n>25</l-n>Second line</p>
```

### Safe marker placement

Omit a marker if the line has no rendered location or insertion would alter
the rendered content or HTML structure. Omitted lines still count.

| Case | Policy |
| --- | --- |
| Blank separators, recognized metadata with no rendered output, and reference definitions without their own output | Skip. Apply the metadata rule only to blocks recognized by the existing parser. |
| Setext heading and thematic break | Mark the heading text line and skip its underline. A thematic break renders as an element and can have a marker beside it where the parent permits one. |
| Fenced or indented code and source code outside documentation comments | Skip code and fences. Preserve literal code text. |
| Multiline link label | Mark each line with a safe rendered label position. A destination or title continuation alone has no marker. |
| Multiline image | A marker can sit beside the rendered image for its opening line. Continuations represented only in an attribute are skipped unless the line has other eligible content. |
| Raw HTML, multiline inline code, and math | Place a marker beside the rendered element when that position is safe and the element can be attributed to the source line, even if the line contains only that element. Skip continuation lines represented only inside the opaque value. Eligible prose on the same line may still receive a marker. Keep markers outside opaque content, tags, attributes, and expressions. |
| Lists, definition lists, block quotes, and tables | Use valid content containers. A table row with eligible content receives one marker in a cell, even if it has several cells. Skip alignment delimiter rows. |

Server integration
------------------

Both paths in `server/src/processing.rs` call `markdown_to_html`. For
standalone Markdown, derive line numbers directly from the input. For source
files, carry original positions through comment extraction and map positions
in the combined Markdown back to their original file lines. Synthetic doc
block separators have no original line. Preserve the combined parse so link
references can resolve across documentation blocks.

Use parser source spans or equivalent source-position information. Do not
infer line numbers from generated HTML newlines or inject markers into
Markdown before parsing. Keep the existing parser configuration, including
the smart punctuation setting. Verify markers survive hydration, doc-block
splitting, and minification in the final Client payload.

Acceptance tests
----------------

* Two prose lines, a blank line, then a heading produce markers 1, 2, and 4.
  Cover a list continuation, soft break, and both hard-break spellings.
* Prose on lines 1 and 7 around a fenced block on lines 3–5 produces only
  markers 1 and 7. Cover indented code, raw HTML, and display math too.
* Two documentation lines after 23 lines of source code produce markers
  24 and 25. Verify later doc blocks use absolute file lines.
* LF and CRLF inputs containing UTF-8 text and an unterminated final line
  produce the same marker numbers.
* Test recognized YAML and TOML metadata in separate fixtures. Test a setext
  heading and a thematic break separately to distinguish their markers.
* Test multiline link and image labels separately. Assert that link text
  continuations can be marked and image attributes stay intact.
* Test nested quotes, task and definition lists, and aligned tables.
  Assert one marker per eligible row and valid DOM placement.
* A rendered footnote definition keeps its original line number even when
  rendered after later prose. Cross-block links still resolve and gathered
  content contributes no duplicate or foreign markers.
* Open and save a document that contains both a generated marker and an
  authored raw HTML element with the same markup, such as `<l-n>24</l-n>`.
  Compare the saved result with the same document saved without generated
  markers. The authored element follows existing save behavior and the
  generated marker contributes neither text nor whitespace.
* In the final Server payload, verify adjacent markers for lines containing
  only inline code, inline math, or an opaque HTML element when placement is
  safe. Check that opaque continuation lines and code blocks stay unmarked.
* In the Client, open an annotated document and enter edit mode. Verify that
  generated marker numbers are invisible in both states. Include an authored
  raw HTML `<l-n>` element and compare its display and editing behavior with
  the same document opened without generated markers.
* Verify that an unterminated final prose line receives a marker, while an
  unterminated final line inside a fenced code block is skipped.

Use exact expected marker numbers in focused Server cases for both input
kinds. Use the professor's full Markdown sample as a broader regression
fixture and compare rendering with the existing baseline after removing
generated markers. Also assert exact original line numbers for selected
markers in that fixture, including a rendered continuation line and the
first prose line after a skipped block. Run the existing test suite after
implementation.
