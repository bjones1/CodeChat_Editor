Project 2 submission
====================

Team name: (team name)

The specification consists of the file [spec.md](spec.md). (Update this with the
name of your specification.)

Draft submission
----------------

### Muzanbekova Kamila

GitHub userid: kmuzanbekova

#### Contributions

(Kamila's contributions)

#### Build and run test

(Summarized LLM output)

#### Specification review rounds

(Table showing spec review rounds)

#### Last specification review

(LLM and prompt used)

(LLM output, possibly with strikethrough)

### Danial Baitakov

GitHub userid: exekerey

#### Contributions

(Danial's contributions)

#### Build and run test

(Summarized LLM output)

#### Specification review rounds

(Table showing spec review rounds)

#### Last specification review

(LLM and prompt used)

(LLM output, possibly with strikethrough)

### Nurlybek Aimagambetov

GitHub userid: nurlybek-aimagambetov

#### Contributions

(Nurlybek's contributions)

#### Build and run test

(Summarized LLM output)

#### Specification review rounds

(Table showing spec review rounds)

#### Last specification review

(LLM and prompt used)

(LLM output, possibly with strikethrough)


Final submission
----------------

### Muzanbekova Kamila

GitHub userid: kmuzanbekova

#### Contributions

Reviewed the Tag format section (`spec.md`, lines 50–77) against `server/src/processing.rs` and `server/src/processing/cache.rs`, identifying 10 edge cases and semantic gaps that were all resolved in the updated specification.

#### Specification review rounds

| Round | Scope | Findings | Resolved |
| :--- | :--- | :--- | :--- |
| 1 | `Tag format` (`spec.md`, lines 50–77) | 10 (5 important, 1 moderate, 2 minor, 2 nit) | 10 (9 applied directly; 1 resolved via warning + save-time strip) |

#### Last specification review

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

#### Implementation review rounds

(Table showing implementation review rounds)

#### Last implementation review

(LLM and prompt used)

(LLM output, possibly with strikethrough)

### Danial Baitakov

GitHub userid: exekerey

#### Contributions

Reviewed the Dehydration, Re-translation & diffs, and Testing sections (`spec.md`) against `server/src/processing.rs`, `server/src/translation.rs`, and `client/src/CodeMirror-integration.mts` over two review rounds, revising them to use a separate tag-removal pre-pass, state how long tags stay stale, scope diff amplification to source files, and add discriminating test cases.

#### Specification review rounds

| Round | Scope | Findings | Main changes made |
| :--- | :--- | :--- | :--- |
| 1 | Dehydration, Re-translation & diffs, Testing | 9 (4 important, 2 moderate, 2 minor, 1 nit) | Tag removal moved to a separate pre-pass that merges split text nodes; staleness window stated (corrected only by IDE-originated edits); shared `strip_line_number_tags` helper and `translation.rs` tests; diff amplification limited to source files with a measurable TODO; tests added for dehydration edge cases, comparison, and diff amplification |
| 2 | Dehydration, Re-translation & diffs, Testing | 10 (2 important, 7 moderate, 1 minor) | Line-offset test corrected for `render_fragment_content`'s `o_i` difference; diff-amplification test now checks `contents`, not just an `Update`; pre-pass call sites named and `<xref>` case dropped; cursor-marker cases clarified and made unit-testable; strip order fixed; Consumers marked out of scope; partly rejected (#7 premise, most of #8, parts of #9 and #10) |
| 3 | Dehydration, Re-translation & diffs, Testing | 7 (1 important, 2 moderate, 3 minor, 1 nit) | Existing exact-HTML tests in `overall_*.rs` and `ide/vscode/tests.rs` are updated to include tags; cursor-marker test states the expected text after re-wrapping; comparison covers a tag missing on one side; staleness wording fixed (any sent re-translation corrects tags); test that both dehydration paths run the pre-pass; dropped the unworkable gutter-number check (all 7 applied) |

#### Last specification review

**LLM used:** Claude Opus 5.5 (Claude Code)  
**Prompt used:** `Review the sections named "Dehydration", "Re-translation & diffs" (including its "Consumers" subsection), and "Testing" in spec.md (lines 231–313), writing findings to review.md`, followed by a paragraph limiting scope to those sections, listing the source files to check claims against, and asking for only relevant findings (no redesigns).

All findings in this round were applied; none were deemed incorrect or irrelevant.


Review of "Dehydration", "Re-translation & diffs", and "Testing" (spec.md, lines 231–313)
==========================================================================================

Scope
-----

This review covers only "Dehydration" (spec.md:231–234), "Re-translation &
diffs" with its "Consumers" subsection (spec.md:236–252), and "Testing"
(spec.md:255–312). Other sections were read only for context. Statements about
existing code were checked against the `line-number-support-2` branch, mainly
`server/src/processing.rs`, `server/src/translation.rs`,
`client/src/CodeMirror-integration.mts`, `client/src/CodeChatEditor.mts`, and
the existing test files. Nothing was built or run.

Summary
-------

The three sections are mostly sound. Every claim about current code checks
out: the two callers of `dehydrating_walk_node`, the replacements it doesn't
descend into, where `html_to_dom` places the marker, the order of the
`compare_html` normalizations, `DocBlockWidget.eq` ignoring positions, and
Markdown documents using `setContent`. The main gap is in Testing. Several
existing tests outside the three listed files assert tag-free HTML and will
fail, and the spec's rule for such tests can't be applied to them. The other
findings are one ambiguous expected result, two missing test cases for
behavior these sections define, and two small inaccuracies.

Findings
--------

### 1. Existing tests outside listed files break; remedy unusable (important)

spec.md:257–263 limits new tests to three files and says "Tests not about line
numbers should compare HTML after deleting tags with `strip_line_number_tags`".
spec.md:288 requires only that "every existing round-trip test still
reproduces its source". However, several existing tests assert the exact HTML
the Server sends, which will now contain tags:

* `server/tests/overall/overall_1.rs:104, 349, 395, 416, 490` and
  `overall_2.rs:503` compare a doc block's `inner_html()` in the browser with
  strings like `"<p>Test</p>"`. The TinyMCE DOM will now hold
  `<p><l-n>1</l-n>Test</p>`. These are integration tests in a separate crate,
  so a `pub(crate)` helper (spec.md:240) isn't visible to them.
* `server/src/ide/vscode/tests.rs:1866` and `:1930` expect Server-to-Client
  translations (`"<p>See <xref …"`) inside whole-`EditorMessage`
  `assert_eq!`s. Deleting tags from just the HTML would mean taking these
  structs apart. The file is also missing from the list of test locations.

An implementer will hit these failures with no guidance. The rule that is
given can't be followed for the integration tests, and it is awkward to follow
for the vscode tests.

**Suggest:** Name these files and pick one approach. Either update their
expected strings to include the tags, or make the helper reachable (for
example `pub` with `#[doc(hidden)]`) and use it there. For the browser checks
in `overall_*.rs`, compare against an expected string that includes the tag.

### 2. Expected marker position for a cursor in a tag is ambiguous (moderate)

spec.md:292–294 requires that "for a `dom_location` inside a tag,
`doc_block_html_to_markdown`'s output holds the marker at the start of that
tag's line." Dehydration (spec.md:234) defines the behavior differently: the
marker takes the removed tag's place and merges with the adjacent text.
`doc_block_html_to_markdown` then re-wraps the paragraph
(`processing.rs:739–755`), so "that tag's line" usually doesn't exist in the
output. For example, `<p>a <l-n>2</l-n>b</p>` with the cursor in the tag's
text becomes `a \u{E83B}b` on one line, so the marker is mid-line. An
implementer can't tell whether that output passes.

**Suggest:** State the result in terms of the text, not the line: the marker
directly precedes the text that followed the tag. Give one input/output pair,
such as `<p>a <l-n>2</l-n>b</p>` → `a \u{E83B}b\n`.

### 3. No comparison test for a tag present on one side only (moderate)

Staleness (spec.md:241) says lines typed in the Client "have no tag at all"
and that "when only tags differ, no re-translation is sent". In practice,
that is the most common difference: after the save round trip, the Server's
re-translation (`translation.rs:1317–1328`) tags a line the Client sent
without one. The Comparison tests (spec.md:299–302) only cover tags that
differ "only in tag numbers", plus a tag removed together with its text. A
`compare_html` that stripped tags only when both sides had one would pass
them all.

**Suggest:** Add a case where `compare_html` reports equal for
`<p><l-n>1</l-n>a <l-n>2</l-n>b</p>` against the Client's `<p>a b</p>`.

### 4. "Corrected only by the next IDE-originated edit" is inaccurate (minor)

spec.md:241 says this, but any re-translation that is actually sent replaces
the Client's HTML, and its tags with it. That covers a Client save whose round
trip differs in more than tags (`translation.rs:1317–1328`; for example, a new
doc block or added math) and a cache-driven re-translation
(`translation.rs:1594–1615`). The conclusion that consumers must tolerate
stale tags is still right. Only "only" is wrong.

**Suggest:** "Tags are corrected by the next translation sent to the Client:
an IDE-originated edit, or a re-translation sent for a non-tag difference."

### 5. Tests don't check that both dehydration callers run the pre-pass (minor)

spec.md:232 requires the pre-pass in both the Markdown-document path
(`processing.rs:500–501`) and `doc_block_html_to_markdown`
(`processing.rs:736–737`). The required equality test, "Markdown from tagged
HTML equals Markdown from the same HTML with its tags deleted" (spec.md:297–
298), doesn't say which path it runs through. If the pre-pass were missing
from one caller, every test could still pass. Existing round-trip tests won't
catch this either, because their HTML has no tags.

**Suggest:** Require the equality test to run through both
`codechat_for_web_to_source` with a Markdown document and a source file's doc
block.

### 6. "First tag equals gutter + o_i" fails for some required blocks (minor)

spec.md:282–285 says "A doc block's first tag equals the
`render_fragment_content` gutter number for that block plus $o_i$." This holds
only when the block's first content line has inline content. The same
required file includes an empty doc block and a doc block with an unterminated
fence (spec.md:279–281). Neither has a tag on its first line. In addition,
`render_fragment_content` emits a gutter number only for a doc block inside a
fragment (`processing.rs:2199`), so the test also needs `<fragment>`s around
those blocks.

**Suggest:** "For each doc block whose first content line has inline content,
wrapped in a fragment, the first tag equals …"

### 7. Rationale for the marker landing in a tag is overstated (nit)

spec.md:234 says the marker reaches an `l-n` "only through `html_to_dom`'s
fallback to the last child." `restoreSelection`
(`CodeChatEditor.mts:564–598`) also re-applies a saved index path after every
`setContent`/`innerHTML` update (`CodeMirror-integration.mts:538–545`,
`CodeChatEditor.mts:402–412`). When tags are added or shifted, that path can
put the caret programmatically inside an `l-n`'s text. The required behavior
is unaffected, because the pre-pass handles a marker anywhere inside a tag.

**Suggest:** Drop "only", or add "or a programmatic selection restore".

What's sound
------------

* The callers named in spec.md:232 are the only ones
  (`processing.rs:501, 737`, `processing/tests.rs:3098`), and
  `dehydrating_walk_node` does skip the children of the nodes it replaces and
  of `<xref>` (`processing.rs:3247–3262`). The Mermaid/GraphViz (`:3131–3142`),
  `<pre>`/`<br>` (`:3172–3201`), and math (`:2912–2915`) checks do require a
  single text child, or only text and `<br>` children. That makes the case for
  a separate pass, followed by merging adjacent text nodes, correct.
* The marker is inserted during `html_to_dom` (`processing.rs:1489–1515`),
  before any later pass, and only into a text node, as spec.md:234 says.
  Leaving a marker-only text node in place of a removed tag keeps the cursor
  line computation (`translation.rs:1482–1491`) working.
* The `compare_html` details are accurate. The first argument arrives already
  minified, and the raw side is minified inside the function
  (`translation.rs:1793–1794`). Stripping before the `<br> `/`<p><iframe `
  rewrites (`:1799`, `:1806`) is the right order. `compare_html_walker` only
  removes TinyMCE data. The pattern `<l-n>[0-9]+</l-n>` leaves escaped
  code-span text alone and correctly treats malformed Client tags as a
  difference.
* The diff-amplification analysis matches the code.
  `diff_code_mirror_doc_blocks` tokenizes on `contents`
  (`processing.rs:3392–3396`), `DocBlockWidget.eq` ignores `from`/`to`
  (`CodeMirror-integration.mts:461–469`), and `updateDOM` resets the contents,
  MathJax, and TinyMCE (`:512–552`). Markdown documents always call
  `setContent` (`CodeChatEditor.mts:409`). The diff test correctly checks
  `contents`, not just whether an `Update` exists.
* The `render_fragment_content` gutter numbers a doc block from its first
  source line, including a stripped `/*` line (`processing.rs:2185–2189`), as
  spec.md:284–285 states.
* The test locations work: `compare_html` is private, but `mod tests` in
  `translation.rs` can reach it, and `processing/tests.rs` is a child module.
  The Consumers subsection is clearly marked informative. Its targets exist
  (`TargetFact::line`, `FragmentFact::line` in `processing/cache.rs:303, 319`),
  and the two TODOs come with concrete trigger conditions.

#### Implementation review rounds

(Table showing implementation review rounds)

#### Last implementation review

(LLM and prompt used)

(LLM output, possibly with strikethrough)

### Nurlybek Aimagambetov

GitHub userid: nurlybek-aimagambetov

#### Contributions

Replaced the single content-line offset with per-line source offsets that
preserve gaps in merged comments; required both lexers to compute block heights
from full normalized source spans; specified tag removal and text-node merging
inside math and diagram containers before hydration; defined byte-offset
mapping for LF, CRLF, and lone CR; expanded lexer, line-offset, and hydration
test requirements.

#### Specification review rounds

| Round | Scope | Findings | Main changes made |
| :--- | :--- | :--- | :--- |
| 1 | Source line offsets, Sanitization & hydration | 4 (all P2) | Corrected offsets, source heights, and newline handling; specified hydration cleanup and tests |
| 2 | Source line offsets, Sanitization & hydration | 1 (P3) | No changes made |

#### Last specification review

**LLM used:** GPT-6 (Codex)  
**Prompt used:** ``Review **`Source line offsets`** and **`Sanitization`** in spec.md``

One minor ambiguity found:

* **[P3] “No attributes” needs clarification** — `spec.md:251`. Adding `l-n`
  to `add_tags` still permits generic attributes. The existing configuration
  allows `id` and `data-gather`; Ammonia also defaults to allowing `lang` and
  `title`. If all attributes must be stripped, specify an attribute filter for
  `l-n`. Otherwise, say “add no tag-specific attributes.” See
  [Ammonia's attribute handling](https://docs.rs/ammonia/4.1.2/src/ammonia/lib.rs.html#1905-1945).

No substantive issues found in **Source line offsets**. The per-line mapping
handles stripped delimiters, merged comments, empty comments, and final lines
without a newline consistently.

#### Implementation review rounds

(Table showing implementation review rounds)

#### Last implementation review

(LLM and prompt used)

(LLM output, possibly with strikethrough)
