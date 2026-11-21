**LLM used:** Claude Opus 5.5 Medium (Cursor)  
**Prompt used:** `Review the section named "Tag format" in spec.md, writing findings to review.md`

Review of "Tag format" (`spec.md`, lines 50–77)
===============================================

Scope: the `<h2 id="cc-T4yjQxfetF">Tag format</h2>` section only. Other
sections are cited where they contradict it or where it depends on them.
Statements about the code were checked against `server/src/processing.rs`
(`markdown_to_html`, `render_fragment_content`) and `server/src/processing/cache.rs`.

Summary
-------

The section's syntax rules hold up: the digits, the lack of attributes, the
element name, and the difference from `cc-line-number`. The two
*semantic* bullets (lines 59–65) promise more than the Placement rules
deliver. Those two bullets are what consumers will rely on, so they need the
most work. Findings are ordered most important first.

Findings
--------

### 1. "Content between tags" claim is false for multi-line inline events (important)

Lines 60–62 say content between two tags came from the first tag's line "or a
later one with no inline content of its own". Placement inserts a tag only
before a trigger whose range *starts* on a new line (Placement, condition 2).
A single trigger event can span lines, so the line where it continues has
inline content but no tag:

* a code span: `` `a`` + newline + ``b` `` is one `Code` event;
* inline math spanning lines (one `InlineMath`/`DisplayMath` event);
* inline HTML split across lines, e.g. `<span` + newline + `class="x">`;
* inline HTML suppressed by the raw-text rule (`<textarea>` … across lines);
* an image whose alt text spans lines (opaque, per Exceptions).

Lines inside opaque *block* contexts are also mis-described. A fenced code
block's lines, and an HTML block's lines (including the doc block
separators), have rendered content but no tags. "No inline content of its own"
is a stretch for them.

**Suggest:** say "or a later line which received no tag", and point to
Exceptions for the reasons a line receives none.

### 2. Content before the first tag and after the last tag is not described (important)

The "between tags" bullet covers only content that has a tag on both sides.
Two other cases need a rule:

* Content before the first tag. Examples are an HTML block or fenced code
  block at the top of a document, and, in source files, the separator HTML
  that `source_to_codechat_for_web` puts before each doc block.
* Content after the last tag.

Lookup (line 64) has the same gap. For a requested line `N` lower than every
tag's number, the "last tag whose number is at most `N`" doesn't exist, and
the section doesn't say what a consumer should do.

**Suggest:** say that content after the last tag belongs to that tag's line or
later lines. Say that content before the first tag, and a lookup that finds
no tag, map to "no position" (or to the start of the container), and that
consumers must handle that case.

### 3. Ordering and uniqueness are asserted, not guaranteed by Placement (important)

Line 63 promises "increasing order of `N`, and at most once for each `N`". The
only Placement rule behind this is condition 3, "that line *differs* from the
line of the last tag inserted". That rule prevents a repeat only on
consecutive triggers. The guarantee therefore depends on an unstated
assumption: pulldown-cmark emits trigger events with non-decreasing start
offsets, and the line map is monotonic. Both are probably true with
`Options::all()` today. The spec should still either:

* make condition 3 "is greater than the line of the last tag inserted", so
  the guarantee holds by construction; or
* state the monotonicity assumption, and require a test that checks the
  ordering over the whole test corpus.

### 4. The guarantees don't hold in the Client, where the consumers run (important)

Line 32 says tags exist "in the Client and in the Server's intermediate HTML".
The scroll-sync consumer (Consumers, line 422) reads the Client's DOM. The
Client's DOM can break every bullet in this section:

* **Copy and paste in TinyMCE** duplicates or reorders `l-n` elements, because
  they are valid elements (`extended_valid_elements`). That breaks "at most
  once" and "increasing order".
* **Typed text:** Sanitization (line 340) admits that text can be typed into a
  tag. Dehydration (line 375) describes a cursor marker inside one. Both break
  "no content other than `N`".
* **Stale numbers** after Client edits (Re-translation, line 396).
* **Hydration removes tags** inside `<xref>` content (line 343), so the
  Client's HTML is not the same as `markdown_to_html` output.

**Suggest:** limit the bullets explicitly to HTML produced by
`markdown_to_html`. Add a sentence saying that consumers of the Client DOM
must tolerate out-of-order, duplicate, stale, or non-numeric tags. For
example: parse `textContent` defensively and skip tags that fail to parse.

### 5. Hand-written `l-n` elements break the guarantees and are silently deleted (important)

The reservation paragraph (lines 67–68) says only that dehydration removes a
hand-written `l-n`. Until the next save, though, a hand-written
`<l-n>999</l-n>` passes through as `InlineHtml` and survives `AMMONIA_OPTIONS`
(which now allow it). It then sits among the generated tags, so ordering and
uniqueness are broken for every consumer. On save, its content is silently
deleted, which is data loss with no warning. The `plaintext` case (line 195)
sets a precedent here: there, `parse_doc_block` reports what it hides.

**Suggest:** have the Placement adapter drop `InlineHtml`/`Html` events that
are `l-n` elements, or strip them in hydration, so only generated tags exist
in the HTML. Have `parse_doc_block` report hand-written ones. Mention the
user-visible consequence here.

### 6. Lookup scope is undefined for source files (moderate)

"A consumer looking up line `N` uses the last tag whose number is at most `N`"
doesn't say *where* to search. In a source file, each doc block's HTML is a
separate container, and code-block lines (shown by CodeMirror) have no tags.
A whole-document search for a code line `N` returns a tag in an earlier doc
block, which is the wrong pane. A per-doc-block search can find nothing.

**Suggest:** define the lookup per container (a doc block, or a Markdown
document's body). State that lines belonging to code blocks are resolved by
CodeMirror, not by tags.

### 7. Contradiction with the introduction (minor)

The intro (line 24) says a tag appears "before the rendered content of each
source line". The section says "Lines may be skipped" (line 63–64), and
Exceptions lists many lines that get none. Reword the intro to say "of each
source line with inline content, with the exceptions listed below", so the
two statements agree.

### 8. "Matching CodeMirror's gutter" needs a qualifier for Markdown documents (minor)

Line 55 ties `N` to CodeMirror's gutter. A Markdown document has no CodeMirror
view in the Client, so the reference that applies there is the IDE's line
number. The claim is correct for source files: `render_fragment_content` and
the gutter both count from 1 and sum `code_doc_block_lines` (checked at
`processing.rs:2185`). Suggest "matching the IDE's line numbers, CodeMirror's
gutter, and `render_fragment_content`".

### 9. Custom-element rationale: add the constraint it implies (nit)

Being a valid custom element name means `l-n` parses as `HTMLElement`, not
`HTMLUnknownElement`, and that the name *could* be registered. Add a sentence
saying that the Client must never call `customElements.define("l-n", …)`.
Upgrading the element could render content or change how TinyMCE handles it.

### 10. Digit format vs. the comparison regex (nit)

Line 56 forbids leading zeros and requires at least one digit. The
`compare_html` regex (line 394), `<l-n>[0-9]*</l-n>`, also matches `<l-n></l-n>`
and `<l-n>007</l-n>`. That is harmless, but say whether the looseness is
intended (so hand-written or emptied tags are tolerated) or use `[0-9]+` for
consistency.

What's sound
------------

* The syntax rules (ASCII digits, no attributes, a short name) are clear and
  testable.
* The name choice is justified correctly: the hyphen guarantees no collision
  with a standard HTML element.
* The section separates `l-n` tags from the `cc-line-number` spans clearly.
  The description of those spans matches `render_fragment_content`
  (`processing.rs:2152–2241`).
* The cache claim in the intro matches `cache.rs`: `_line` fields exist at
  lines 258 and 277, filled from `fact.line`.
