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

Reviewed the `Tag format & lookup` (`spec.md`, lines 39–75) and `Placement`
(`spec.md`, lines 76–170) sections against `server/src/processing.rs`,
`server/src/translation.rs`, `server/src/processing/cache.rs`,
`client/src/tinymce-config.mts`, and `pulldown-cmark` 0.13.6 across three
review rounds, resolving 25 total findings (including escaping hand-written
`<l-n>` tags to prevent content hiding and save-time data loss, defining
value-based lookup with explicit container selection, clarifying mid-line
continuation semantics, raw-text suppression across HTML block boundaries, and
pre-injection warning scanning).

#### Specification review rounds

| Round | Scope                                          | Findings                                     | Resolved                                                                                                                                                                                                                                 |
| ----- | ---------------------------------------------- | -------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1     | `Tag format` (`spec.md`, lines 50–77)          | 10 (5 important, 1 moderate, 2 minor, 2 nit) | 10 (9 applied directly; 1 resolved via warning + save-time strip)                                                                                                                                                                        |
| 2     | `Placement` (`spec.md`, lines 76–170)          | 7 (2 important, 1 moderate, 3 minor, 1 nit)  | 7 (unified `Html`/`InlineHtml` raw-text \& warning scanner, clarified continuation-line tags, specified post-check suppression timing and `range.start` mapping, fixed `parse_doc_block` error wording)                                  |
| 3     | `Tag format & lookup` (`spec.md`, lines 39–75) | 8 (3 important, 2 moderate, 2 minor, 1 nit)  | 8 (escaped hand-written `<l-n` tags with `log::warn!`, defined value-based lookup and container selection, updated mid-line semantics, fixed `cc-qSbTxGyL2n` cross-reference, defined valid tag syntax, and tightened wording/wrapping) |


#### Last specification review

**LLM used:** Claude Opus 5.5 Medium (Cursor)
**Prompt used:** `Review the section named "Tag format & lookup" in @spec.md,
writing findings to @review.md`

All 8 findings in this round were resolved in `spec.md`.

Review of "Tag format & lookup" (spec.md, lines 39–75)
=====================================================

Scope
-----

This review covers only the `<h2 id="cc-T4yjQxfetF">Tag format &amp;
lookup</h2>` section (spec.md:39–75). Other sections were read only for
context, or are cited where they conflict with this one. Claims about existing
code were checked against the `line-number-support-2` branch:
`server/src/processing.rs` (`markdown_to_html`, `parse_doc_block`,
`doc_blocks_to_dom`, `hoist_fragment_content`, `render_fragment_content`),
`server/src/translation.rs`, `server/src/processing/cache.rs`, and
`client/src/tinymce-config.mts`. Nothing was built or run.

Summary
-------

Since round 1, the section has gained a per-container lookup rule, a
Client-resilience bullet, and a hand-written-tag policy. The syntax rules are
still solid. The new material has four real problems:

* An unclosed hand-written `<l-n>` hides the rest of its container and then
  deletes it on save. A closed one is hidden too, so "renders normally" is
  wrong.
* The lookup rule says "the last tag", but the next bullet says Client
  consumers can't assume ordering. In the Client, "last" has no defined
  meaning.
* The per-container lookup doesn't say how a consumer picks the container.
  Applied as written, every earlier doc block resolves a line that belongs to
  a later block.
* The "Semantics" bullet is false for the mid-line tags that the round 2
  Placement fix introduced.

The remaining findings are about how warnings are reported and detected, a
broken cross-reference, and wording.

Findings
--------

~~### 1. Hand-written `<l-n>`: hidden content and data loss (important)~~

spec.md:72–75 says a hand-written `l-n` start tag "renders normally, and is
stripped (along with its content) during dehydration on the next save".

~~* **It doesn't render.** The Client stylesheet rule `l-n { display: none; }` (Sanitization table, spec.md:258) applies to every `l-n`. A hand-written `<l-n>important</l-n>` disappears from the Client view. Then the next save deletes it from the file. The user gets a warning about text they can't see.~~
~~* **An unclosed tag hides everything after it.** `l-n` is an unknown element to html5ever. A later `<p>` start tag closes an open `p` but not an open  `l-n`. Given `Intro <l-n> oops` followed by more paragraphs, every later  paragraph (and every generated tag in them) nests inside the hand-written  element. `parse_doc_block` accepts this: the sentinel ends up inside the  open element, so `contains_sentinel` is true and the HTML is reparsed  without error (`processing.rs:1625–1640`). The rest of the doc block or  document is invisible in the Client, and dehydration deletes all of it on  the next save.~~
~~* **It breaks this section's own guarantees.** The bullets at spec.md:41–57 apply to "`markdown_to_html` output". A hand-written `<l-n>999</l-n>` (or `<l-n class=x>`, or `<l-n>abc</l-n>`) appears in that output and violates Format and Monotonicity.~~

**Suggest:** Don't let hand-written tags reach the DOM. The adapter already
scans every `Html` and `InlineHtml` event for them, so it can escape the
`<l-n` (emit `&lt;l-n`) or drop the tag in the same pass. The user then sees
the literal text, nothing is hidden, and a save writes it back unchanged.
Keep the warning. If the current policy stays, at least (a) make an unclosed
hand-written `l-n` a translation error, as `parse_doc_block` already does for
other text-hiding constructs, (b) replace "renders normally" with "is hidden
in the Client and deleted with its content on the next save", and (c) limit
the Format and Monotonicity bullets to *generated* tags.

~~### 2. "Last tag" is undefined once ordering isn't guaranteed (important)~~

The lookup rule (spec.md:58–61) resolves line `N` to "the last tag whose value
is $\le N$". That relies on the Monotonicity bullet: in Server output, the
last such tag in document order is also the one with the largest value. The
next bullet (spec.md:62–66) says Client consumers "must not assume ordering or
uniqueness". The scroll-sync consumer (spec.md:299) runs in the Client, so the
only consumer that uses the lookup rule is the one for which the rule's
premise doesn't hold. For example, take the tags `2, 9, 5` after a paste, with
`N = 6`. "Last in document order with value $\le 6$" gives `5`. "Largest value
$\le 6$" also gives `5`. With `N = 9`, the first reading gives `5` and the
second gives `9`. A duplicate `5, 5` leaves it unclear which element to scroll to.

**Suggest:** Define the lookup on values, not positions: "the valid tag with
the greatest value $\le N$; if several share that value, the first in document
order". State that in Server output this is the same as the last such tag.

~~### 3. The lookup doesn't say how to pick the container (important)~~

"Within a single container (Markdown body or one doc block), line `N` resolves
to the last tag whose value is $\le N$" (spec.md:58–61). In a source file,
tag values are global source lines (spec.md:214), so every doc block before
line `N` has a tag $\le N$. A consumer that searches block 0 for a line in
block 3, or for a line in a code block, gets a match. Only a consumer that
already knows which container holds `N` gets the right answer. The parenthesis
"CodeMirror locates code block lines" hints at this, but doesn't say how.

**Suggest:** Make the first step explicit: "First find the block containing
`N` from the block line ranges ($1 + \sum_{j<i} h_j$ up to
$\sum_{j \le i} h_j$, see <xref ref="cc-dl60mkSR4A"></xref>, or the doc block
positions in CodeMirror). Apply the tag lookup only if that block is a doc
block, and only to that block's tags." For a Markdown document, the body is
the only container.

~~### 4. "Semantics" is false for mid-line tags (moderate)~~

Round 2 (Placement finding 2) kept tags in the middle of a line after a
multi-line inline event, and the Omitted-tags parenthesis (spec.md:53–55)
describes them. The Semantics bullet wasn't updated:

* "Marks where line `N`'s inline content starts" (spec.md:48). For
  `` `a\nb` c ``, the output is `<l-n>1</l-n><code>a b</code><l-n>2</l-n> c`.
  Line 2's content starts at `b`, inside the code span, not at tag 2.
* "Content between two tags belongs to the first tag's line or a subsequent
  untagged line" (spec.md:48–49). The `b` lies between tags 1 and 2 and comes
  from line 2. Line 2 is neither the first tag's line nor untagged.

**Suggest:** "Marks the first trigger which starts on line `N`. Content
between two tags belongs to the first tag's line, to a later untagged line,
or (for the continuation of a multi-line inline event) to the second tag's
line." A tag for line `N` then gives a position *on* line `N`, not its start,
which is all scroll sync needs.

~~### 5. The warning has no channel and no detection rule (moderate)~~

spec.md:72–75 requires "a non-blocking warning". It doesn't say where the
warning goes. `markdown_to_html` returns a bare `String`
(`processing.rs:1446`), `source_to_codechat_for_web` returns
`Result<CodeChatForWeb, SourceToCodeChatForWebError>` with no warning field,
and the only existing mechanism is `log::warn!`. A log line is easy to
implement but the user never sees it, which matters because their text is
deleted on save (finding 1). Implementers will choose differently, and the
required test ("produces a warning", spec.md:358) can't be written until the
channel is known.

"Any `l-n` start tag ... in an incoming `InlineHtml` or `Html` event" also
needs a matching rule. An `Html` event is raw text, not tokens. A substring
search for `<l-n` also fires on `<!-- <l-n> -->`, on
`<span title="<l-n>">`, and on `<l-n>` inside a `<script>` or `<textarea>`.
html5ever parses none of these as an element, so dehydration does *not* strip
them, which contradicts "is stripped ... on the next save".

**Suggest:** Name the channel (for example, `log::warn!` with the file path
and source line, plus whatever user-facing route the project prefers). Define
the match, for example the regex `(?i)<l-n[\s/>]`. Say that false positives
inside comments, attribute values, and raw-text elements are accepted, and
that only an actual `l-n` element is stripped on save.

~~### 6. Broken cross-reference `cc-qSbTxGyL2n` (minor)~~

spec.md:63 cites stale edits via `<xref ref="cc-qSbTxGyL2n"></xref>`. No
element in the project has that id. `## Re-translation & diffs`
(spec.md:283) has no id. Hydration will render an error in place of the link.

**Suggest:** Give that heading the id: `<h2 id="cc-qSbTxGyL2n">Re-translation
&amp; diffs</h2>`. This matches the other `<h2 id=...>` headings in the spec.

~~### 7. Client-resilience bullet: wrong sources, undefined "invalid" (minor)~~

spec.md:62–66 lists what can corrupt tags in the Client DOM. Two items don't
belong in the list:

* **Cursor markers** are inserted on the Server, during save
  (`translation.rs:1465–1482`, then `html_to_dom`). They never appear in the
  Client DOM. `client/src` doesn't reference `UNICODE_CURSOR_MARKER`. They
  matter for dehydration, which spec.md:281 already covers.
* **`<xref>` hydration** runs on the Server and only *removes* tags
  (spec.md:262), and stored xref text is cleaned by `CLEAN_LINK_CONTENT_HTML`.
  It can leave lines without a tag, but it can't duplicate or reorder tags.

"Must ignore invalid `N` values" also needs a definition, since every consumer
has to apply the same one.

**Suggest:** Remove the cursor marker from the list. Change the xref item to
"can remove tags". Define validity: "a tag is valid if it has no element
children and its `textContent` matches `^[1-9][0-9]*$`; consumers skip every
other tag."

~~### 8. Smaller wording gaps (nit)~~

* **`N` ≥ 1.** "1-based ... no leading zeros" (spec.md:44–46) still allows
  `0`, which has no leading zero. `cache.rs` uses `0` for an unknown line
  (`cache.rs:2011, 2029`; spec.md:27). Say "`N` ≥ 1" so that a tag can't be
  confused with "unknown".
* **Omitted tags** (spec.md:51–53) lists "opaque content", which means
  opaque depth. It doesn't list lines inside a raw-text suppression region,
  which also get no tag. Add "or content inside a raw-text suppression
  region".
* **Line length.** spec.md:74–75 run past 80 columns, against the wrapping
  rule in `AGENTS.md`.

What's sound
------------

* The Format rule is clear and testable. It matches `render_fragment_content`,
  which counts from 1 and sums `code_doc_block_lines`
  (`processing.rs:2152–2241`).
* `l-n` is a valid custom element name: it starts with a lowercase ASCII
  letter, contains a hyphen, and isn't one of the reserved names. So it can't
  collide with a standard element. The ban on `customElements.define` (added
  after round 1) closes the upgrade risk. No Client code defines `l-n`
  (`tinymce-config.mts:153` registers only `graphviz-graph`, `wc-mermaid`,
  `xref`, and `fragment`).
* Strict monotonicity holds for generated tags in Server output, including
  after hydration. Hydration only removes tags, and `hoist_fragment_content`
  (`processing.rs:1811–1827`) moves a fragment's children to just after the
  fragment without reordering them.
* The new "leading content maps to no line" sentence and the "no position"
  fallback in the lookup rule close round 1 findings 2 and 6 for Markdown
  documents.
* Checking hand-written tags "before tag injection" (spec.md:74) correctly
  keeps generated tags from setting off the warning (round 2 Placement
  finding 6).

Out of scope, noted in passing
------------------------------

The introduction (spec.md:35–37) relies on `source_to_codechat_for_web`
seeing `is_toc`. That parameter is currently `_is_toc` and unused
(`processing.rs:972`), so the implementation checklist should include passing
it to `markdown_to_html`.


#### Implementation review rounds

(Table showing implementation review rounds)

#### Last implementation review

(LLM and prompt used)

(LLM output, possibly with strikethrough)

### Danial Baitakov

GitHub userid: exekerey

#### Contributions

Reviewed the Dehydration, Re-translation \& diffs, and Testing sections
(`spec.md`) against `server/src/processing.rs`, `server/src/translation.rs`, and
`client/src/CodeMirror-integration.mts` over two review rounds, revising them to
use a separate tag-removal pre-pass, state how long tags stay stale, scope diff
amplification to source files, and add discriminating test cases.

#### Specification review rounds

| Round | Scope                                         | Findings                                    | Main changes made                                                                                                                                                                                                                                                                                                                                                                             |
| ----- | --------------------------------------------- | ------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1     | Dehydration, Re-translation \& diffs, Testing | 9 (4 important, 2 moderate, 2 minor, 1 nit) | Tag removal moved to a separate pre-pass that merges split text nodes; staleness window stated (corrected only by IDE-originated edits); shared `strip_line_number_tags` helper and `translation.rs` tests; diff amplification limited to source files with a measurable TODO; tests added for dehydration edge cases, comparison, and diff amplification                                     |
| 2     | Dehydration, Re-translation \& diffs, Testing | 10 (2 important, 7 moderate, 1 minor)       | Line-offset test corrected for `render_fragment_content`'s `o_i` difference; diff-amplification test now checks `contents`, not just an `Update`; pre-pass call sites named and `<xref>` case dropped; cursor-marker cases clarified and made unit-testable; strip order fixed; Consumers marked out of scope; partly rejected (#7 premise, most of #8, parts of #9 and #10)                  |
| 3     | Dehydration, Re-translation \& diffs, Testing | 7 (1 important, 2 moderate, 3 minor, 1 nit) | Existing exact-HTML tests in `overall_*.rs` and `ide/vscode/tests.rs` are updated to include tags; cursor-marker test states the expected text after re-wrapping; comparison covers a tag missing on one side; staleness wording fixed (any sent re-translation corrects tags); test that both dehydration paths run the pre-pass; dropped the unworkable gutter-number check (all 7 applied) |

#### Last specification review

<strong>LLM used:</strong> Claude Opus 5.5 (Claude Code)<br><strong>Prompt
used:</strong> `Review the sections named "Dehydration", "Re-translation &
diffs" (including its "Consumers" subsection), and "Testing" in spec.md (lines
231–313), writing findings to review.md`, followed by a paragraph limiting scope
to those sections, listing the source files to check claims against, and asking
for only relevant findings (no redesigns).

All findings in this round were applied; none were deemed incorrect or
irrelevant.

Review of "Dehydration", "Re-translation \& diffs", and "Testing" (spec.md,
lines 231–313)
===========================================================================

Scope
-----

This review covers only "Dehydration" (spec.md:231–234), "Re-translation \&
diffs" with its "Consumers" subsection (spec.md:236–252), and "Testing"
(spec.md:255–312). Other sections were read only for context. Statements about
existing code were checked against the `line-number-support-2` branch, mainly
`server/src/processing.rs`, `server/src/translation.rs`,
`client/src/CodeMirror-integration.mts`, `client/src/CodeChatEditor.mts`, and
the existing test files. Nothing was built or run.

Summary
-------

The three sections are mostly sound. Every claim about current code checks out:
the two callers of `dehydrating_walk_node`, the replacements it doesn't descend
into, where `html_to_dom` places the marker, the order of the `compare_html`
normalizations, `DocBlockWidget.eq` ignoring positions, and Markdown documents
using `setContent`. The main gap is in Testing. Several existing tests outside
the three listed files assert tag-free HTML and will fail, and the spec's rule
for such tests can't be applied to them. The other findings are one ambiguous
expected result, two missing test cases for behavior these sections define, and
two small inaccuracies.

Findings
--------

### 1\. Existing tests outside listed files break; remedy unusable (important)

spec.md:257–263 limits new tests to three files and says "Tests not about line
numbers should compare HTML after deleting tags with `strip_line_number_tags`".
spec.md:288 requires only that "every existing round-trip test still reproduces
its source". However, several existing tests assert the exact HTML the Server
sends, which will now contain tags:

* `server/tests/overall/overall_1.rs:104, 349, 395, 416, 490` and
  `overall_2.rs:503` compare a doc block's `inner_html()` in the browser with
  strings like `"<p>Test</p>"`. The TinyMCE DOM will now hold
  `<p><l-n>1</l-n>Test</p>`. These are integration tests in a separate crate, so
  a `pub(crate)` helper (spec.md:240) isn't visible to them.
* `server/src/ide/vscode/tests.rs:1866` and `:1930` expect Server-to-Client
  translations (`"<p>See <xref …"`) inside whole-`EditorMessage` `assert_eq!`s.
  Deleting tags from just the HTML would mean taking these structs apart. The
  file is also missing from the list of test locations.

An implementer will hit these failures with no guidance. The rule that is given
can't be followed for the integration tests, and it is awkward to follow for the
vscode tests.

<strong>Suggest:</strong> Name these files and pick one approach. Either update
their expected strings to include the tags, or make the helper reachable (for
example `pub` with `#[doc(hidden)]`) and use it there. For the browser checks in
`overall_*.rs`, compare against an expected string that includes the tag.

### 2\. Expected marker position for a cursor in a tag is ambiguous (moderate)

spec.md:292–294 requires that "for a `dom_location` inside a tag,
`doc_block_html_to_markdown`'s output holds the marker at the start of that
tag's line." Dehydration (spec.md:234) defines the behavior differently: the
marker takes the removed tag's place and merges with the adjacent text.
`doc_block_html_to_markdown` then re-wraps the paragraph
(`processing.rs:739–755`), so "that tag's line" usually doesn't exist in the
output. For example, `<p>a <l-n>2</l-n>b</p>` with the cursor in the tag's text
becomes `a \u{E83B}b` on one line, so the marker is mid-line. An implementer
can't tell whether that output passes.

<strong>Suggest:</strong> State the result in terms of the text, not the line:
the marker directly precedes the text that followed the tag. Give one
input/output pair, such as `<p>a <l-n>2</l-n>b</p>` → `a \u{E83B}b\n`.

### 3\. No comparison test for a tag present on one side only (moderate)

Staleness (spec.md:241) says lines typed in the Client "have no tag at all" and
that "when only tags differ, no re-translation is sent". In practice, that is
the most common difference: after the save round trip, the Server's
re-translation (`translation.rs:1317–1328`) tags a line the Client sent without
one. The Comparison tests (spec.md:299–302) only cover tags that differ "only in
tag numbers", plus a tag removed together with its text. A `compare_html` that
stripped tags only when both sides had one would pass them all.

<strong>Suggest:</strong> Add a case where `compare_html` reports equal for
`<p><l-n>1</l-n>a <l-n>2</l-n>b</p>` against the Client's `<p>a b</p>`.

### 4\. "Corrected only by the next IDE-originated edit" is inaccurate (minor)

spec.md:241 says this, but any re-translation that is actually sent replaces the
Client's HTML, and its tags with it. That covers a Client save whose round trip
differs in more than tags (`translation.rs:1317–1328`; for example, a new doc
block or added math) and a cache-driven re-translation
(`translation.rs:1594–1615`). The conclusion that consumers must tolerate stale
tags is still right. Only "only" is wrong.

<strong>Suggest:</strong> "Tags are corrected by the next translation sent to
the Client: an IDE-originated edit, or a re-translation sent for a non-tag
difference."

### 5\. Tests don't check that both dehydration callers run the pre-pass (minor)

spec.md:232 requires the pre-pass in both the Markdown-document path
(`processing.rs:500–501`) and `doc_block_html_to_markdown`
(`processing.rs:736–737`). The required equality test, "Markdown from tagged
HTML equals Markdown from the same HTML with its tags deleted" (spec.md:297–
298), doesn't say which path it runs through. If the pre-pass were missing from
one caller, every test could still pass. Existing round-trip tests won't catch
this either, because their HTML has no tags.

<strong>Suggest:</strong> Require the equality test to run through both
`codechat_for_web_to_source` with a Markdown document and a source file's doc
block.

### 6\. "First tag equals gutter + o\_i" fails for some required blocks (minor)

spec.md:282–285 says "A doc block's first tag equals the
`render_fragment_content` gutter number for that block plus $o_i$." This holds
only when the block's first content line has inline content. The same required
file includes an empty doc block and a doc block with an unterminated fence
(spec.md:279–281). Neither has a tag on its first line. In addition,
`render_fragment_content` emits a gutter number only for a doc block inside a
fragment (`processing.rs:2199`), so the test also needs `<fragment>`s around
those blocks.

<strong>Suggest:</strong> "For each doc block whose first content line has
inline content, wrapped in a fragment, the first tag equals …"

### 7\. Rationale for the marker landing in a tag is overstated (nit)

spec.md:234 says the marker reaches an `l-n` "only through `html_to_dom`'s
fallback to the last child." `restoreSelection` (`CodeChatEditor.mts:564–598`)
also re-applies a saved index path after every `setContent`/`innerHTML` update
(`CodeMirror-integration.mts:538–545`, `CodeChatEditor.mts:402–412`). When tags
are added or shifted, that path can put the caret programmatically inside an
`l-n`'s text. The required behavior is unaffected, because the pre-pass handles
a marker anywhere inside a tag.

<strong>Suggest:</strong> Drop "only", or add "or a programmatic selection
restore".

What's sound
------------

* The callers named in spec.md:232 are the only ones (`processing.rs:501, 737`,
  `processing/tests.rs:3098`), and `dehydrating_walk_node` does skip the
  children of the nodes it replaces and of `<xref>` (`processing.rs:3247–3262`).
  The Mermaid/GraphViz (`:3131–3142`), `<pre>`/`<br>` (`:3172–3201`), and math
  (`:2912–2915`) checks do require a single text child, or only text and `<br>`
  children. That makes the case for a separate pass, followed by merging
  adjacent text nodes, correct.
* The marker is inserted during `html_to_dom` (`processing.rs:1489–1515`),
  before any later pass, and only into a text node, as spec.md:234 says. Leaving
  a marker-only text node in place of a removed tag keeps the cursor line
  computation (`translation.rs:1482–1491`) working.
* The `compare_html` details are accurate. The first argument arrives already
  minified, and the raw side is minified inside the function
  (`translation.rs:1793–1794`). Stripping before the `<br>`/`<p><iframe`
  rewrites (`:1799`, `:1806`) is the right order. `compare_html_walker` only
  removes TinyMCE data. The pattern `<l-n>[0-9]+</l-n>` leaves escaped code-span
  text alone and correctly treats malformed Client tags as a difference.
* The diff-amplification analysis matches the code.
  `diff_code_mirror_doc_blocks` tokenizes on `contents`
  (`processing.rs:3392–3396`), `DocBlockWidget.eq` ignores `from`/`to`
  (`CodeMirror-integration.mts:461–469`), and `updateDOM` resets the contents,
  MathJax, and TinyMCE (`:512–552`). Markdown documents always call `setContent`
  (`CodeChatEditor.mts:409`). The diff test correctly checks `contents`, not
  just whether an `Update` exists.
* The `render_fragment_content` gutter numbers a doc block from its first source
  line, including a stripped `/*` line (`processing.rs:2185–2189`), as
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

(Nurlybek's contributions)

#### Specification review rounds

| Round | Scope                                          | Findings   | Main changes made                                                                                                                                                                                                                                                                                                                                                                                                 |
| ----- | ---------------------------------------------- | ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1     | Source line offsets, Sanitization \& hydration | 4 (all P2) | Replaced the single content-line offset with per-line source offsets that preserve gaps in merged comments; required both lexers to compute block heights from full normalized source spans; specified tag removal and text-node merging inside math and diagram containers before hydration; defined byte-offset mapping for LF, CRLF, and lone CR; expanded lexer, line-offset, and hydration test requirements |

#### Last specification review

<strong>LLM used:</strong> GPT-6 (Codex)<br><strong>Prompt used:</strong>
``Review **`Source line offsets`** and **`Sanitization`** in spec.md``

All four findings were addressed in `spec.md`. The review below records the
findings before those changes.

Review of "Source line offsets" and "Sanitization \& hydration" (spec.md)
=========================================================================

Scope: the "Source line offsets" and "Sanitization \& hydration" sections in
`spec.md`. Findings were checked against `server/src/lexer.rs`,
`server/src/lexer/pest_parser.rs`, and `server/src/processing.rs`, with focused
lexer/parser probes.

Findings
--------

### 1\. One offset cannot represent merged block comments (P2)

At `spec.md:186`, the formula assumes contiguous source lines. The JavaScript
lexer merges `/*\nFirst\n*/\n/*\nSecond\n*/\n` into one doc block. With `o_i =
1`, it maps `Second` to line 4 instead of 5.

<strong>Suggest:</strong> Preserve removed boundary lines or record separate
mapping segments for each original comment.

### 2\. Source heights need an explicit lexer correction (P2)

At `spec.md:192`, `code_doc_block_lines` returns `DocBlock.lines`, but both
lexer paths currently derive that field from processed contents. For
`/*\nA\n*/\n// B\n`, the JavaScript lexer reports the first block's height as 2;
it occupies 3 source lines.

<strong>Suggest:</strong> Specify correcting source-height accounting alongside
adding `contents_line_offset`, or subsequent tags remain displaced.

### 3\. Raw math HTML can violate the hydration invariant (P2)

At `spec.md:228`, consider `A <span class="math math-inline">x\ny</span>.`.
Placement inserts a tag before `y`, giving the math span three children.
`replace_math_node` then rejects it because it requires one text child. The
dehydration pre-pass happens too late to prevent this rendering regression.

<strong>Suggest:</strong> Require suppression or removal and text-node merging
before hydration, with a regression case.

### 4\. Splitting only on LF misses lone-CR line endings (P2)

At `spec.md:171`, `a\rsecond\rthird` produces three Markdown lines in
pulldown-cmark, but the proposed map assigns all content to line 1.

<strong>Suggest:</strong> Recognize lone CR as well as LF and CRLF, or normalize
before parsing while preserving the offset correspondence.

#### Implementation review rounds

(Table showing implementation review rounds)

#### Last implementation review

(LLM and prompt used)

(LLM output, possibly with strikethrough)
