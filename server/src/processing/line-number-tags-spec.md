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
  `N` is a 1-based integer ($N \ge 1$), matching the IDE, CodeMirror gutter, and
  `render_fragment_content` (ASCII digits only; no signs, leading zeros, or
  whitespace).
* <strong>Semantics:</strong> Marks the first eligible inline element that
  starts on line `N` (giving scroll sync a DOM position on that line). Content
  between two tags belongs to the first tag's line, a subsequent untagged line,
  or—when continuing a multi-line inline span—the second tag's line. Leading
  content before the first tag (HTML blocks, fenced code, doc block separators)
  maps to no line.
* <strong>Omitted tags:</strong> Lines with no inline content, opaque content,
  content inside a raw-text suppression region, or only the continuation of an
  inline event from an earlier line (<xref ref="cc-4KkOVSDENA"></xref>) get no
  tag. (If new inline content follows a multi-line span on that same line, a tag
  is placed before that new content.)
* <strong>Monotonicity:</strong> `N` is strictly increasing; skipped lines are
  allowed.
* <strong>Lookup rule:</strong> To locate source line `N`:
  1. Find the block containing `N` using block line ranges
     (<xref ref="cc-dl60mkSR4A"></xref>) or CodeMirror block positions. If `N`
     is in a code block, CodeMirror locates it directly.
  2. If `N` is in a doc block (or a Markdown document body), search only that
     container's valid `<l-n>` tags for the **greatest value $\le N$** (breaking
     ties with the first tag in document order). In ordered Server output, this
     is the last tag $\le N$.
  3. If no tag has a value $\le N$, the line has no position in that container.
* <strong>Client DOM resilience:</strong> TinyMCE copy/paste, typing, and
  stale edits (<xref ref="cc-qSbTxGyL2n"></xref>) can duplicate, reorder, or
  corrupt tags, while `<xref>` hydration (<xref ref="cc-e7tpKZV9sK"></xref>)
  can remove them. Client DOM consumers must not assume ordering or uniqueness
  and must skip invalid tags. A tag is valid only if it has no element
  children and its `textContent` matches `^[1-9][0-9]*$`.
* <strong>Collisions \& registration:</strong> `l-n` is a valid
  [custom element name](https://html.spec.whatwg.org/multipage/custom-elements.html#valid-custom-element-name)
  (preventing standard element collisions) and is distinct from `<span
  class="cc-line-number">` (used for gathered fragment gutters). The Client must
  never call `customElements.define` on it.
* <strong>Hand-written tags:</strong> `l-n` is reserved. Before tag
  injection (even when the line map is `None`, such as in the sidebar TOC), scan
  `Html` and `InlineHtml` events for `(?i-u)<(/?l-n)([\t\n\x0C\r />])`, escape
  each match to `&lt;$1$2`, and log via `log::warn!` with the file path and
  source line (omitting the line when the line map is `None`). Escaping keeps
  text visible despite `l-n { display: none; }`, prevents unclosed tags from
  swallowing later blocks, and preserves the text on save. (False positives in
  comments, attributes, and raw-text elements are accepted.)

<h2 id="cc-gpOIpo2gjZ">Placement</h2>

Update `markdown_to_html` to accept `line_map: Option<&LineMap>` and `file_path: &Path`, where `LineMap` is either `Document` (maps Markdown line `m` to `m + 1`) or `Segments(Vec<(usize, Vec<usize>)>)` (built during `source_to_codechat_for_web`'s fold). Pass `None` when `is_toc` is `true` to emit no tags.

### Trigger rules

**Eligible triggers:**
* Leaf inline events: `Text`, `Code`, `InlineMath`, `DisplayMath`, `InlineHtml`,
  `FootnoteReference`, `TaskListMarker`.
* Container inline `Start` events: `Emphasis`, `Strong`, `Strikethrough`,
  `Superscript`, `Subscript`, `Link`, `Image`.

Insert `<l-n>N</l-n>` before a trigger **iff**:
1. Opaque depth is `0` and raw-text suppression is inactive
   (<xref ref="cc-4KkOVSDENA"></xref>);
2. The event's `range.start` maps to a valid source line `N`; and
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
<h1><l-n>1</l-n>Title</h1><p><l-n>3</l-n>Some <em>text</em> <l-n>4</l-n>continues here.<ul><li><l-n>6</l-n><input disabled type=checkbox> A task</ul>
```

<h3 id="cc-4KkOVSDENA">Exceptions</h3>

**Opaque depth:** An integer counter initialized to `0`. An opaque `Start`
increments depth *after* the trigger check (so `Image` gets a tag before it,
none inside); its matching `End` decrements it. No tags emit while depth $> 0$.

| Opaque element | Reason |
| :--- | :--- |
| `Tag::CodeBlock` | `Text` events ignore line boundaries; `hydrate_child` requires a single text node for Mermaid/GraphViz; syntax highlighters read `innerHTML`; `DOC_BLOCK_SEPARATOR_*` markers must match. |
| `Tag::Image` | Inner events serialize into the `alt` attribute. |
| `Tag::HtmlBlock` | Defensive; contains only non-trigger `Html` events. |
| `Tag::MetadataBlock` | Unrendered, though events still pass through the writer. |

**Raw-text suppression:** CommonMark can end an `HtmlBlock` at a blank line or
container boundary while a tag or comment is still open. Because `html5ever`
parses the contents below as literal text or separate fragments, dehydration
cannot strip `<l-n>` tags inside them. Note that targets opened at the start of
a line normally form an HTML block; suppression only matters if that block ends
early or if the target opens mid-paragraph.

Scan incoming `Html` and `InlineHtml` events before injecting `<l-n>` tags
(after escaping hand-written `l-n` tags). Run a two-state scanner across each
event, applying state changes *after* the event's trigger check so a closing tag
at the start of a line does not receive an inner `<l-n>` tag:

* **Unsuppressed:** Scan for the earliest opener matching
  `(?i-u)<(script|style|textarea|title|xmp|iframe|noembed|noframes|noscript|plaintext|template|select)[\t\n\x0C\r />]`
  or `<!--`.
  * In `InlineHtml` events, only match at **byte offset 0** (ignores tags inside attribute values).
  * In `Html` events, search the entire event.
  * Record the opener's source line upon matching.
* **Suppressed:** Ignore new openers and scan the entire event for the active closer:
  `(?i-u)</name[\t\n\x0C\r />]` (`plaintext` never closes), or `-->` / `--!>`
  starting from the first byte of `<!--` (allowing `<!-->` and `<!--->` to close
  immediately). Resume normal scanning after any closer.

Reset suppression at the end of input and when the first event at or after the
next segment's `start_md_line` arrives (after scanning the preceding separator's
closing `Html` events). If suppression is still active at a reset, log a warning
via `log::warn!` with the file path and recorded opener line. *(Prevents unclosed
tags or attribute false positives from leaking across doc-block boundaries).*

**Other constructs:**
* **Tables:** No special handling needed. Each row's tag lands in its first
  non-empty cell; empty rows and delimiter rows (`|---|`) get no tags.
* **Untagged lines:** Blank lines, fences, code block lines, thematic breaks,
  setext underlines, link reference definitions, table delimiters, HTML/metadata
  blocks, continuation lines of multi-line code spans, math, link destinations
  or titles, inline HTML, or image alt text with no subsequent inline events,
  lines inside raw-text suppression, and doc block separator lines.
* **`<xref>`:** Multi-line `<xref>` elements may receive an inner tag, which
  hydration removes (<xref ref="cc-e7tpKZV9sK"></xref>).
* **Hand-written math and diagram HTML:** Tags emitted inside raw math spans,
  `<pre>`/`<code>`, `wc-mermaid`, or `graphviz-graph` are removed before
  hydration, with adjacent text nodes merged
  (<xref ref="cc-e7tpKZV9sK"></xref>).

<h3 id="cc-fWDWpSp7n2" data-gather="cc-f9V6VecYSP cc-CJIPQ2ftG9 cc-YPPU9sUNNZ">Implementation of Placement</h3>

<h2 id="cc-dl60mkSR4A">Source line offsets</h2>

To map a byte offset in `markdown_to_html` input to a 1-based source line
(`Option<usize>`):
1. Scan the exact input bytes to record line-start offsets, beginning with
   `0`. LF (`\n`), lone CR (`\r`), and CRLF (`\r\n`) each end one line;
   record the next start after the entire terminator. Binary-search these
   offsets for the 0-based Markdown line $m$. Do not normalize this input
   after recording offsets: they must match `into_offset_iter`'s byte ranges.
2. Translate $m$ to a source line:

* **Markdown documents:** Return `Some(m + 1)`.
* **Source files:** `source_to_codechat_for_web` concatenates separators and
  doc block contents into `doc_contents`. Both lexer paths normalize source
  line endings to LF before extracting comments. They must retain the
  original source height and a mapping for every retained content line:
  * `DocBlock.lines` is the number of lines in the full normalized source span
    consumed by that block, including delimiter-only lines and empty comments.
    Compute it before stripping delimiters or dedenting, using
    `source_span.split_inclusive('\n').count()`. Apply this to the regex lexer
    and the Pest parser in `server/src/lexer/pest_parser.rs`; neither may
    derive source height from processed contents. A non-empty final span
    without a trailing newline still occupies a line.
  * Add `DocBlock::contents_line_offsets: Vec<usize>`. Entry $s_i[k]$ is the
    0-based source-line offset, relative to this doc block's start, of content
    line $k$. Construct it from retained source spans as delimiters and
    indentation are removed. A retained blank line maps to the source line
    supplying its newline; empty contents have an empty map.
  * When adjacent comments merge, add the accumulated source height to each
    incoming content-line offset before appending its map, then add the
    incoming source height. Removed delimiter lines may create gaps in the
    map; one initial offset cannot represent them.
  * This metadata is valid only for source-to-HTML translation. Set the map
    to an empty vector in `code_mirror_to_code_doc_blocks`, where source
    heights are already invalid and neither is consulted.

For doc block $i$ in `code_doc_block_arr`, 0-based content line $k$
($0 \le k < c_i$, where $c_i$ is
`contents.split_inclusive('\n').count()`) maps to source line:

$$L = 1 + \sum_{j < i} h_j + s_i[k]$$

Here $h_j$ is `code_doc_block_lines` of block $j$ (code or doc), using the
corrected source heights. Lexer tests must assert that the map length equals
$c_i$, entries are strictly increasing and less than $h_i$, and the sum of
all block heights equals the normalized source's
`split_inclusive('\n').count()`. This count excludes the empty line after a
final newline.

While folding blocks into `doc_contents`, record each block's segment
`(start_md_line, source_lines)`, where `source_lines[k]` is $L$ above. Compute
`start_md_line` from the actual accumulated text after appending that block's
separator and before appending its contents; a preceding block without a final
newline must not create a fictitious extra line. To resolve Markdown line $m$:
1. Find the last segment with $\text{start\_md\_line} \le m$; set
   $k = m - \text{start\_md\_line}$.
2. If no segment exists or $k$ is outside `source_lines`, $m$ is a separator
   line $\rightarrow$ return `None`.
3. Otherwise return `Some(source_lines[k])`.

| Source line | Text | Block | Calculation |
| :--- | :--- | :--- | :--- |
| 1 | `int x;` | 0: code ($h_0 = 1$) | — |
| 2–3 | `// Intro text\n// continues.` | 1: doc ($h_1 = 2$) | $s_1 = [0, 1]$: lines 2, 3 |
| 4–6 | `/*\n   Block text.\n */` | 2: doc ($h_2 = 3$) | For `Block text.`, $s_2[k] = 1$: $1 + (1 + 2) + 1 = 5$ |

For adjacent comments `/*\nFirst\n*/\n/*\nSecond\n*/\n`, a merged doc block
occupies six source lines. `First` has source-line offset `1` and `Second`
has offset `4`, so their tags are `2` and `5` regardless of which blank lines
the lexer retains in the contents.

**Client document height.** `source_to_codechat_for_web` replaces each doc
block in `CodeMirror.doc` with one newline per *source* line it occupies
(`DocBlock.lines`), not one per content line, so that CodeMirror line numbers
match IDE line numbers. Each `CodeMirrorDocBlock` therefore spans
`to - from = DocBlock.lines` newlines, and a comment with delimiter-only lines
(such as `/*` or `*/` on its own line) occupies more Client lines than its
contents, shifting the `from`/`to` of every later block. One exception keeps
round trips exact: a doc block whose contents are empty, which the lexers
produce only for an empty comment ending the file without a final newline,
emits `lines - 1` newlines, so the missing final newline stays missing. As
before, `to` is at least `from + 1`. `render_fragment_content`'s gutter numbers
use the same source heights through `code_doc_block_lines`. The Client→IDE
direction is unaffected: `code_mirror_to_code_doc_blocks` regenerates
delimiters from `contents` and ignores heights.

<h3 id="cc-675iCUV0TK" data-gather="cc-2RR1Hkiqvd cc-Pgf8BNFlSN cc-QAaRjE3zO2 cc-1HKC5Ndgmt cc-U5oZkvs4iD cc-Ad95J3JIlK cc-4sw3FiojeX cc-EnMVWxNfWT">Implementation of Source line offsets</h3>

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

* **Caret placement:** `l-n { display: none; }` blocks browser caret movement
  into `<l-n>` tags, but script-driven selection (such as entering a doc block
  from above in `client/src/CodeMirror-integration.mts`) must explicitly skip
  `l-n` subtrees when walking for the first text node so typed text is neither
  hidden nor deleted on save.

Hydration invariants:

* The pre-pass at the start of `hydrate_dom` strips `<l-n>` inside `<xref>`, before `hydrate_source_xref` captures `data-content` for dehydration (<xref ref="cc-HZyE2Bv-eI"></xref>).
* `<l-n>` inside `<fragment>` is naturally moved after the fragment by `hoist_fragment_content`.
* Hydrated gather lists hold cleaned fragment HTML and never contain `<l-n>`.
* At the start of `hydrate_dom`, before `hoist_fragment_content`, fact
  collection, or any call to `hydrate_child` or `replace_math_node`, remove
  every descendant `<l-n>` and its content from `<pre>`, `<code>`,
  `wc-mermaid`, `graphviz-graph`, `<xref>`, and `<span>` elements whose
  `class` token list contains `math` and either `math-inline` or `math-display`.
  Use the same subtree removal and merging of adjacent text nodes required by the
  dehydration pre-pass. Select these containers by tag and class before
  inspecting their children. Placement can insert tags inside hand-written HTML
  even though Markdown code blocks and math events keep their generated
  wrappers free of tags. This cleanup preserves the single text child of
  otherwise valid math and diagram content without removing tags elsewhere in
  the document.

<h3 id="cc-Wm3Zt9Kq2B" data-gather="cc-Hx4rQ7mWbN">Implementation of Sanitization &amp; hydration</h3>

### Dehydration
* **Removal:** A separate pre-pass deletes every `l-n` element and its content—matching on local name across all depths and namespaces (including foreign `<svg>`/`<math>` contexts and `template_contents`). Both callers of `dehydrating_walk_node` (the Markdown document path in `codechat_for_web_to_source` and `doc_block_html_to_markdown`) and the test helper `dehydrate_html` run it first. It must be a separate pass because `dehydrating_walk_node` doesn't descend into the nodes it replaces (Mermaid/GraphViz, math, `<pre>` with `<br>`), and a stray tag would block those replacements, which require a single text child or only text and `<br>` children.
* **Fidelity:** After removing a tag, the pre-pass merges the text nodes on either side of it, so `htmd` sees the same DOM tree as for HTML without tags. Because break whitespace stays outside tags, the resulting Markdown is byte-identical. Pasted `<l-n>` DOM elements are stripped on save, while hand-written `<l-n>` tags in Markdown were escaped during translation and are preserved.
* **Cursor marker:** `html_to_dom` injects `UNICODE_CURSOR_MARKER` before the pre-pass, so Client and Server node indices match. The marker rarely lands in an `l-n` (for example, through `html_to_dom`'s fallback to the last child, or a restored selection). When the pre-pass removes an `l-n` containing the marker in any descendant text, it leaves a text node containing only the marker in its place, which is then merged with adjacent text like any other. If the DOM path ends on an `l-n` element rather than a text node, `html_to_dom` inserts no marker, as it does today for any non-text node.

<h4 id="cc-h2RGug0SiP" data-gather="cc-4AJLIwQ1g7">Implementation of Dehydration</h4>

<h2 id="cc-qSbTxGyL2n">Re-translation &amp; diffs</h2>

Line numbers shift when lines are added/removed above them or when save-time word wrapping reflows edits.

* **Comparison (`compare_html`):** Strips tags from both minified inputs using `strip_line_number_tags`, a `pub(crate)` helper in `processing.rs` that removes `<l-n>[0-9]+</l-n>`, so stale line numbers alone don't trigger a re-translation. The strip runs before the `<br> ` and `<p><iframe ` normalizations. `compare_html_walker` doesn't strip tags.
* **Staleness:** When only tags differ, no re-translation is sent, and the Server keeps the Client's HTML. Tags are corrected only when the Server next sends new HTML: after an IDE-originated edit, or a re-translation that differs for another reason (for example, a cache change). Until then, Client tags after an edited line (including lines shifted by word-wrap reflow) may be stale, and lines typed in the Client have no tag at all. Client consumers must tolerate both (see Consumers).
  * *TODO:* If scroll sync proves inaccurate while editing in the Client, re-translate after a Client edit whenever the saved Markdown's line count changed.
* **Diff amplification (source files only):** Inserting a line renumbers all later tags, so `diff_code_mirror_doc_blocks` emits a contents update for every later doc block. Without tags, only their positions change, and `DocBlockWidget.eq` (which ignores position) skips `updateDOM`. With tags, each later block resets its `innerHTML`, re-runs MathJax, re-creates Mermaid/GraphViz elements, and (if focused) calls TinyMCE `setContent`. Markdown documents are unaffected, since the Client already calls `setContent` on the whole document for every update.
  * *TODO:* If a source file with 50 doc blocks containing math takes more than 100 ms from an IDE line insert to the Client update, switch to block-relative line numbers plus a per-block start offset.

<h3 id="cc-FaTzUELlSl" data-gather="cc-TRSJgb1Wta cc-C9BUjQM51i">Implementation of Re-translation &amp; diffs</h3>

### Consumers *(informative)*

These are not part of this project's implementation; they motivate the tag
format and define what Client consumers must tolerate.

* **Cache line facts:** In `hydrating_walk_node`, track the last seen `<l-n>` tag. Set `TargetFact::line` and `FragmentFact::line` to the first descendant `<l-n>` tag, falling back to the last preceding tag (giving a lower bound for raw HTML blocks).
* **Scroll sync:** Client uses `querySelectorAll("l-n")` per container to match IDE cursor positions, tolerating malformed, out-of-order, stale, or missing tags.


<h2 id="cc-wPpJhnQKLq">Testing</h2>

Use unit tests in `server/src/processing/tests.rs`,
`server/src/lexer/tests.rs`, `server/src/lexer/pest_parser.rs`'s test module,
and `translation.rs`'s `mod tests` (where the private `compare_html` is
reachable); don't add browser tests to `overall.rs` (they are slow). The
Client test below is the only exception: it runs in
the browser harness, as `CodeChatEditor-test.mts` already does. Tests not about
line numbers should compare HTML after deleting tags with
`strip_line_number_tags`. Existing exact-HTML assertions outside these files
(`server/tests/overall/overall_1.rs`, `overall_2.rs`, and
`server/src/ide/vscode/tests.rs`), which can't reach a `pub(crate)` helper or
compare whole messages, are updated to include the expected tags. The
hand-written-tag warning test lives in `server/tests/line_number_warnings.rs`,
its own test binary, because `log::set_logger` can be set only once per process
and the `ide::vscode` lib tests install log4rs.

Required cases:

* <strong>Placement:</strong> paragraph with soft and hard breaks; code span and
  inline math spanning lines (both when ending the line with no tag on line 2,
  and when followed by text on line 2 with a tag before that text); lines starting with
  emphasis, a link, an image, inline code, and inline math; setext and ATX
  headings; tight, loose, task, and definition lists; block quote; footnote
  definition; table with an empty cell, an all-empty row, and a header row.
* <strong>Exceptions:</strong> fenced code closed with backticks or tildes,
  unterminated fences, indented code; Mermaid and GraphViz still hydrate; image
  alt text spanning lines; raw HTML blocks; front matter; each raw-text element,
  `template`, `select`,  and HTML comment opened by inline HTML or an HTML block
  and closed across lines, blank lines, or container boundaries, including
  `<!-->`, `<!--->`, `</textarea><!--`, and a closing tag at the start of a
  line (such as `x <textarea>a\n</textarea> b`, where `<l-n>2</l-n>` must
  appear after `</textarea>`); multi-line `<xref>` whose
  `data-content` holds no tag.
* <strong>Lexer metadata:</strong> In both the regex and Pest lexer tests,
  assert source heights and content-line maps against original source spans,
  including delimiter-only lines, empty comments, adjacent block comments,
  and a final block without a newline. Check map lengths, ordering, bounds,
  and total source height as specified in
  <xref ref="cc-dl60mkSR4A"></xref>.
* <strong>Line offsets:</strong> Markdown documents with LF, CRLF, lone CR,
  and mixed line endings; `a\rsecond\rthird` must receive tags `1`, `2`, and
  `3`. A source file mixing inline and block comments must cover `/*` on its
  own line, an unterminated fence in one doc block, an empty doc block, and a
  final doc block without a trailing newline. Each tag must equal $L$ from
  <xref ref="cc-dl60mkSR4A"></xref>. In JavaScript (regex lexer) and C/C++
  (Pest lexer), `/*\nA\n*/\n// B\n` must tag `B` as line `4`, and
  `/*\nFirst\n*/\n/*\nSecond\n*/\n` must tag `First` and `Second` as `2`
  and `5`, including when the comments merge.
* <strong>Hydration cleanup:</strong> Translate
  `A <span class="math math-inline">x\ny</span>.` through
  `source_to_codechat_for_web`. It must hydrate to delimited, non-editable
  math with one text child and no inner `<l-n>`, while retaining the tag
  before `A`. Repeat for `math math-display` and hand-written multiline
  Mermaid/GraphViz `<pre><code class="language-...">` and custom-element
  HTML. Verify the diagram wrappers and text match translation without line
  tags, and stored target and fragment content also stay free of inner tags.
* <strong>Sanitization and dehydration:</strong> `minify` keeps each tag and
  adjacent whitespace; stored target inner HTML and fragment content hold no
  tags; every existing round-trip test still reproduces its source; an `l-n`
  start or end tag in inline HTML or an HTML block produces a warning and
  escapes to literal `&lt;l-n` text that survives save, while one in a code
  span doesn't warn; malformed, out-of-order, or pasted `<l-n>` DOM elements in Client HTML 
  are removed on save; tags inside `<svg>` and `template_contents` are removed; for a
  `dom_location` inside a tag, `doc_block_html_to_markdown`'s output holds the
  marker immediately before the text that followed the tag
  (`<p>a\n<l-n>2</l-n>b</p>` yields `a \u{E83B}b`); the pre-pass runs through both
  `codechat_for_web_to_source` paths (Markdown document and source file); a stray tag inside `wc-mermaid`, a
  `<pre>` with `<br>`, and math is removed and the replacement still happens;
  text after a removed tag starting with `2) `, `#`, `+ `, `>`, or spaces
  round-trips unchanged; Markdown from tagged HTML equals 
  Markdown from the same HTML with its tags deleted.
* <strong>Comparison:</strong> `compare_html` reports equal for HTML differing
  only in tag numbers, and for a tag present on one side and missing on the
  other (a line typed in the Client); unequal for a text change next to a tag
  (`<l-n>2</l-n>b` vs `<l-n>3</l-n>c`) and for a tag removed with its line's
  text; equal across the `<br> ` normalization next to a tag.
* <strong>Diff amplification:</strong> translate a source file with doc blocks
  before and after a code line, then the same file with one code line inserted
  above that line. `diff_code_mirror_doc_blocks` on the two results emits a
  non-empty `contents` diff for each later doc block (positions shift even
  without tags, so check `contents`, not just the presence of an `Update`) and
  none for earlier ones.
* <strong>Client:</strong> in `CodeChatEditor-test.mts`, load
  `<p><l-n>3</l-n>a</p>` into TinyMCE, verify the tag stays inside `<p>` with `display: none`, enter the
  tagged doc block from the line above, type a character, and confirm it is
  visible and survives a save.

Implementation checklist
------------------------

1. `server/src/lexer.rs` and `server/src/lexer/pest_parser.rs`: Compute
   `DocBlock.lines` from full normalized source spans and add
   `DocBlock::contents_line_offsets`, preserving offsets and heights when
   merging adjacent comments. Initialize the reverse-translation map in
   `code_mirror_to_code_doc_blocks` to an empty vector.
2. `server/src/processing.rs`: Implement the LF/CRLF/lone-CR byte-to-line map,
   per-block source-line segments, `markdown_to_html` signature and tag
   injection (passing `None` when `is_toc` is `true`), warnings, and Ammonia allowlists (`AMMONIA_OPTIONS`,
   `CLEAN_LINK_CONTENT_HTML`, `CLEAN_FRAGMENT_HTML`). Add tag removal and text
   merging inside math and diagram containers before hydration, `<xref>` tag
   stripping in `hydrate_dom`, the `<l-n>` removal pre-pass before
   `dehydrating_walk_node`, and `strip_line_number_tags`.
3. `server/src/translation.rs`: Update `compare_html` to call `strip_line_number_tags`.
4. `client/src/tinymce-config.mts`, `client/src/css/CodeChatEditor.css`, &
   `client/src/CodeMirror-integration.mts`: Configure TinyMCE custom elements,
   add unlayered `display: none`, and skip `l-n` subtrees when placing the caret.
5. Tests: Add coverage for the changes above (per <xref ref="cc-wPpJhnQKLq"></xref>).
