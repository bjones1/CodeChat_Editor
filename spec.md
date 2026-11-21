The CodeChat Editor, a word processor for software developers.<br>Copyright (C)
2024-2026 Bryan A. Jones.

This file is part of the CodeChat Editor.

The CodeChat Editor is free software: you can redistribute it and/or modify it
under the terms of the GNU General Public License as published by the Free
Software Foundation, either version 3 of the License, or (at your option) any
later version.

The CodeChat Editor is distributed in the hope that it will be useful, but
WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more
details.

You should have received a copy of the GNU General Public License along with the
CodeChat Editor. If not, see
[http://www.gnu.org/licenses](http://www.gnu.org/licenses).

Line-number tags spec
=====================

During Markdown-to-HTML translation (for a Markdown document or a source file's
doc blocks), the Server inserts `<l-n>N</l-n>` before the rendered content of
each source line containing inline content (`N` = IDE line number). These tags
map rendered HTML back to source lines for scroll sync and `cache.rs` `Target`
`_line` fields (currently hardcoded to 0).

* <strong>Scope:</strong> Tags exist only in Server intermediate HTML and the
  Client DOM; dehydration strips them before saving Markdown.
* <strong>Terminology:</strong> *Fenced/indented code block* refers to Markdown
  `Tag::CodeBlock`. *Code block* refers to a CodeChat Editor code block (which
  never receives `<l-n>` tags; tags appear only in Markdown documents and doc
  blocks).
* <strong>Sidebar TOC:</strong> `source_to_codechat_for_web` emits no tags when
  `is_toc` is true. A TOC file opened directly as the active document has
  `is_toc = false` and receives tags.

<h2 id="cc-T4yjQxfetF">Tag format &amp; lookup</h2>

In `markdown_to_html` output:

* <strong>Format:</strong> `<l-n>N</l-n>` with no attributes or extra content.
  `N` is 1-based, matching the IDE, CodeMirror gutter, and
  `render_fragment_content` (ASCII digits only; no signs, leading zeros, or
  whitespace).
* <strong>Semantics:</strong> Marks where line `N`'s inline content starts.
  Content between two tags belongs to the first tag's line or a subsequent
  untagged line. Leading content before the first tag (HTML blocks, fenced code,
  doc block separators) maps to no line.
* <strong>Omitted tags:</strong> Lines with no inline content, opaque content,
  or continuing an inline event from an earlier line
  (<xref ref="cc-4KkOVSDENA"></xref>) get no tag.
* <strong>Monotonicity:</strong> `N` is strictly increasing; skipped lines are
  allowed.
* <strong>Lookup rule:</strong> Within a single container (Markdown body or one
  doc block), line `N` resolves to the last tag whose value is $\le N$. If none
  exists, the line has no position in that container (CodeMirror locates code
  block lines).
* <strong>Client DOM resilience:</strong> TinyMCE copy/paste, typing, cursor
  markers, stale edits (<xref ref="cc-qSbTxGyL2n"></xref>), and `<xref>`
  hydration (<xref ref="cc-e7tpKZV9sK"></xref>) can duplicate, reorder, or
  corrupt tags. Client DOM consumers must ignore invalid `N` values and must not
  assume ordering or uniqueness.
* <strong>Collisions \& registration:</strong> `l-n` is a valid
  [custom element name](https://html.spec.whatwg.org/multipage/custom-elements.html#valid-custom-element-name)
  (preventing standard element collisions) and is distinct from `<span
  class="cc-line-number">` (used for gathered fragment gutters). The Client must
  never call `customElements.define` on it.
* <strong>Hand-written tags:</strong> `l-n` is reserved. Any `l-n` start tag
  (ASCII case-insensitive) in an `InlineHtml` or `Html` event triggers a
  non-blocking warning (matching `parse_doc_block`'s hidden-text warning),
  renders normally, and is stripped (along with its content) during dehydration
  on the next save.

<h2 id="cc-gpOIpo2gjZ">Placement</h2>

`markdown_to_html` parses with `into_offset_iter` and runs a streaming adapter
that injects `Event::InlineHtml("<l-n>N</l-n>")` before eligible events using a
byte-offset-to-line map (<xref ref="cc-dl60mkSR4A"></xref>).

### Trigger rules

**Eligible triggers:**
* Leaf inline events: `Text`, `Code`, `InlineMath`, `DisplayMath`, `InlineHtml`,
  `FootnoteReference`, `TaskListMarker`.
* Container inline `Start` events: `Emphasis`, `Strong`, `Strikethrough`,
  `Superscript`, `Subscript`, `Link`, `Image`.

Insert `<l-n>N</l-n>` before a trigger **iff**:
1. Opaque depth is `0` and raw-text suppression is inactive
   (<xref ref="cc-4KkOVSDENA"></xref>);
2. The byte offset maps to a valid source line `N`; and
3. `N` is strictly greater than the last emitted tag's line.

**Excluded events (non-triggers):**
* **Block `Start` events** (`Paragraph`, `Heading`, `BlockQuote`, `List`,
  `Item`, `Table*`, `FootnoteDefinition`, `DefinitionList*`, `HtmlBlock`,
  `CodeBlock`, `MetadataBlock`): placing tags between blocks causes TinyMCE's
  `forced_root_block` to wrap them in empty `<p>` tags. Waiting places tags
  inside `<p>`, `<h1>`, `<li>`, `<td>`, `<dt>`, etc.
* **`SoftBreak`, `HardBreak`:** Their byte ranges start on the preceding line;
  break whitespace remains outside the tag (`a *b*\n<l-n>2</l-n>c`) so `minify`
  collapses `\n` into a preserving space.
* **`End` events:** Share the `Start` event's byte range, which would point to
  the wrong line.
* **`Html`, `Rule`:** Block-level constructs.

### Example

```markdown
# Title

Some *text*
continues here.

* [ ] A task
```

renders (after minification) as:

```html
<h1><l-n>1</l-n>Title</h1><p><l-n>3</l-n>Some <em>text</em> <l-n>4</l-n>continues here.<ul><li><l-n>6</l-n><input disabled type=checkbox>A task</ul>
```

<h3 id="cc-4KkOVSDENA">Exceptions</h3>

**Opaque depth:** An integer counter initialized to `0`. An opaque `Start`
increments depth *after* the trigger check (so `Image` gets a tag before it,
none inside); its matching `End` decrements it. No tags emit while depth $> 0$.

| Opaque element | Reason |
| :--- | :--- |
| `Tag::CodeBlock` | `Text` events ignore line boundaries; `hydrate_child` requires a single text node for Mermaid/GraphViz; syntax highlighters read `innerHTML`; `DOC_BLOCK_SEPARATOR_*` markers must match. |
| `Tag::Image` | Inner events serialize into the `alt` attribute. |
| `Tag::HtmlBlock` | Emits one `Html` event per line; tags could corrupt tags, comments, `<textarea>`/`<script>`, or doc block separators. |
| `Tag::MetadataBlock` | Unrendered, though events still pass through the writer. |

**Raw-text suppression:** When an `InlineHtml` event opens one of the tags below
without closing it in the same event, suppress tag emission across block
boundaries until an `Html` or `InlineHtml` event contains the matching closing
tag (ASCII case-insensitive):
* **Raw-text elements** (`script`, `style`, `textarea`, `title`, `xmp`,
  `iframe`, `noembed`, `noframes`, `noscript`, `plaintext`): html5ever parses
  their contents as literal text, which dehydration cannot strip. `plaintext`
  never closes, suppressing until EOF (`parse_doc_block` already warns on
  hidden text).
* **`template`:** Contents live in a `DocumentFragment` outside normal child
  traversal, so dehydration skips them.
* **`select`:** html5ever strips unknown start tags inside `<select>` while
  leaving the text digits behind.

**Other constructs:**
* **Tables:** No special handling needed. Each row's tag lands in its first
  non-empty cell; empty rows and delimiter rows (`|---|`) get no tags. (Raw
  inline HTML tables inside a paragraph may foster-parent a tag above the
  table; dehydration still strips it.)
* **Untagged lines:** Blank lines, fences, code block lines, thematic breaks,
  setext underlines, link reference definitions, table delimiters, HTML/metadata
  blocks, continuation lines of multi-line code spans/inline math/inline HTML,
  and doc block separator lines.
* **`<xref>`:** Multi-line `<xref>` elements may receive an inner tag, which
  hydration removes (<xref ref="cc-e7tpKZV9sK"></xref>).
* **Sidebar TOC:** `source_to_codechat_for_web` omits tags when `is_toc` is
  `true`, and includes them when opened as the current file (`is_toc` is `false`).


<h2 id="cc-dl60mkSR4A">Source line offsets</h2>

To map a byte offset in `markdown_to_html` input to a 1-based source line (`Option<usize>`):
1. Binary-search line-start byte offsets for the 0-based Markdown line $m$
   (split on `\n`; `\r` stays with the line it terminates).
2. Translate $m$ to a source line:

* **Markdown documents:** Return `Some(m + 1)`.
* **Source files:** `source_to_codechat_for_web` concatenates separators and
  doc block contents into `doc_contents`. Because separators shift line
  numbers and the lexer strips leading/trailing block comment delimiter lines
  (`/*` and `*/`—dedenting only strips columns, so internal lines map 1:1),
  `DocBlock` adds `contents_line_offset` ($o_i$):
  * $o_i = 1$ if an opening block comment delimiter is immediately followed by a
    stripped newline; `0` otherwise (and `0` in
    `code_mirror_to_code_doc_blocks`).
  * Lexer tests must assert $o_i + c_i \le h_i$ for every doc block.

For doc block $i$ in `code_doc_block_arr`, 0-based content line $k$
($0 \le k < c_i$, where $c_i = \text{contents.split\_inclusive('\textbackslash n').count()}$)
maps to source line:

$$L = 1 + \sum_{j < i} h_j + o_i + k$$

where $h_j$ is `code_doc_block_lines` of block $j$ (code or doc).

While folding blocks into `doc_contents`, record each block's segment
`(start_md_line, c_i, L_0)`. To resolve Markdown line $m$:
1. Find the last segment with $\text{start\_md\_line} \le m$; set
   $k = m - \text{start\_md\_line}$.
2. If no segment exists or $k \ge c_i$, $m$ is a separator line $\rightarrow$
   return `None`.
3. Otherwise return `Some(L_0 + k)`.

| Source line | Text | Block | Calculation |
| :--- | :--- | :--- | :--- |
| 1 | `int x;` | 0: code ($h_0 = 1$) | — |
| 2–3 | `// Intro text\n// continues.` | 1: doc ($h_1 = 2, o_1 = 0$) | Lines 2, 3 |
| 4–6 | `/*\n   Block text.\n */` | 2: doc ($h_2 = 3, o_2 = 1$) | $1 + (1 + 2) + 1 + 0 = 5$ |


<h2 id="cc-e7tpKZV9sK">Sanitization & hydration</h2>

HTML sanitizers must never unwrap `<l-n>` (Ammonia's default), which turns
`a\n<l-n>2</l-n>b` into saved text `a 2b`.

| Component | Rule |
| :--- | :--- |
| `AMMONIA_OPTIONS` | Add `l-n` to `add_tags` with no attributes. |
| `CLEAN_LINK_CONTENT_HTML` | Add `l-n` to `add_clean_content_tags` so stored target and `<xref>` text stays clean (<xref ref="cc-2dkkF9YJnL"></xref>). |
| `CLEAN_FRAGMENT_HTML` | Add `l-n` to `add_clean_content_tags` so line shifts don't dirty gathered fragments (fragments use `cc-line-number`). |
| `MINIFY_OPTIONS` | Unchanged (keeps `<l-n>` and collapses adjacent whitespace to one space). |
| TinyMCE (`client/src/tinymce-config.mts`) | Add `l-n` to `extended_valid_elements` and `~l-n` (inline) to `custom_elements` so TinyMCE doesn't unwrap it. |
| Client CSS (`client/src/css/CodeChatEditor.css`) | Add unlayered `l-n { display: none; }` outside theme layers to hide digits and block caret entry. |

Hydration invariants:

* `hydrate_dom` strips `<l-n>` inside `<xref>` during `hoist_fragment_content`, before `hydrate_source_xref` captures `data-content` for dehydration (<xref ref="cc-HZyE2Bv-eI"></xref>).
* `<l-n>` inside `<fragment>` is naturally moved after the fragment by `hoist_fragment_content`.
* Hydrated gather lists hold cleaned fragment HTML and never contain `<l-n>`.
* Mermaid, GraphViz, and math elements retain the single text child required by `hydrate_child`.


### Dehydration
* **Removal:** `dehydrating_walk_node` deletes every `l-n` element and its content—matching on local name across all depths and namespaces, including foreign `<svg>`/`<math>` contexts—alongside hydrated gather lists. This runs before TinyMCE `<br>`/`<p>` fix-ups and custom-element replacements, leaving the walk index unchanged.
* **Fidelity:** Because break whitespace stays outside tags, `htmd` sees identical DOM trees with or without tags, producing byte-identical Markdown. Hand-written or pasted `<l-n>` tags are stripped on save.
* **Cursor marker:** `html_to_dom` injects `UNICODE_CURSOR_MARKER` before dehydration so Client and Server node counts match. If the marker lands inside an `<l-n>` text node, replace the `<l-n>` element with a text node containing only the marker.
* **`compare_html_walker`:** Does not strip tags (handled by `compare_html`).

## Re-translation & diffs

Line numbers shift when lines are added/removed above them or when save-time word wrapping reflows edits.

* **Comparison (`compare_html`):** Strips `<l-n>[0-9]+</l-n>` from both minified inputs so stale line numbers alone don't trigger a re-translation. Tags update on the next IDE sync.
  * *TODO:* If a future consumer needs exact line numbers immediately after an edit, trigger re-translation when only tags differ (throttled to autosave).
* **Diff amplification:** Inserting a line renumbers all later tags, causing `diff_code_mirror_doc_blocks` to re-render every subsequent doc block.
  * *TODO:* If benchmarking large files shows slowdowns, switch to block-relative line numbers plus a per-block start offset.

### Consumers *(informative)*

* **Cache line facts:** In `hydrating_walk_node`, track the last seen `<l-n>` tag. Set `TargetFact::line` and `FragmentFact::line` to the first descendant `<l-n>` tag, falling back to the last preceding tag (giving a lower bound for raw HTML blocks).
* **Scroll sync:** Client uses `querySelectorAll("l-n")` per container to match IDE cursor positions, tolerating malformed or out-of-order tags.


<h2 id="cc-wPpJhnQKLq">Testing</h2>

Use unit tests in `server/src/processing/tests.rs` and
`server/src/lexer/tests.rs`; don't add browser tests to `overall.rs` (they are
slow). Tests not about line numbers should compare HTML after deleting tags with
`compare_html`'s regex.

Required cases:

* <strong>Placement:</strong> paragraph with soft and hard breaks; code span and
  inline math spanning lines (second line untagged); lines starting with
  emphasis, a link, an image, inline code, and inline math; setext and ATX
  headings; tight, loose, task, and definition lists; block quote; footnote
  definition; table with an empty cell, an all-empty row, and a header row.
* <strong>Exceptions:</strong> fenced code closed with backticks or tildes,
  unterminated fences, indented code; Mermaid and GraphViz still hydrate; image
  alt text spanning lines; raw HTML blocks, including a multi-line comment;
  front matter; each raw-text element, `template`, and `select` opened by inline
  HTML across lines, including a close in a later paragraph; multi-line `<xref>`
  whose `data-content` holds no tag.
* <strong>Line offsets:</strong> Markdown document with CRLF endings; source
  file mixing inline and block comments, with `/*` on its own line, an
  unterminated fence in one doc block, an empty doc block, and a final doc block
  without trailing newline. Numbers must match `render_fragment_content`'s.
* <strong>Sanitization and dehydration:</strong> `minify` keeps each tag and
  adjacent whitespace; stored target inner HTML and fragment content hold no
  tags; every existing round-trip test still reproduces its source; an `l-n`
  start tag in inline HTML or an HTML block produces a warning and still
  renders, but one in a code span doesn't warn; malformed or out-of-order Client
  tags are removed; a cursor marker inside a tag survives.
* <strong>Comparison:</strong> `compare_html` reports equal for HTML differing
  only in tag numbers, unequal otherwise.
* <strong>Client:</strong> in `CodeChatEditor-test.mts`, load
 `<p><l-n>3</l-n>a</p>` into TinyMCE, read it back in the Client's raw save
  format, and check the tag is intact and hidden.

Implementation checklist
------------------------

1. `server/src/lexer.rs`: Compute `DocBlock::contents_line_offset`.
2. `server/src/processing.rs`: Implement line map/segments, `markdown_to_html` tag injection + warnings, Ammonia allowlists (`AMMONIA_OPTIONS`, `CLEAN_LINK_CONTENT_HTML`, `CLEAN_FRAGMENT_HTML`), `<xref>` tag stripping in `hydrate_dom`, and `<l-n>` removal in `dehydrating_walk_node`.
3. `server/src/translation.rs`: Update `compare_html` to strip `<l-n>[0-9]+</l-n>`.
4. `client/src/tinymce-config.mts` & `client/src/css/CodeChatEditor.css`: Configure TinyMCE custom elements and unlayered `display: none`.
5. Tests: Add coverage for the changes above (per <xref ref="cc-wPpJhnQKLq"></xref>).
