Project 2 submission
====================

Team name: Line number support 2

The specification consists of the file [spec.md](spec.md).

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

Reviewed and finalized the `Tag format & lookup` and `Placement` sections of
`spec.md` across five review rounds (resolving 37 findings). Implemented Server
`<l-n>` tag injection, the two-state raw-text scanner, hand-written tag
escaping with `log::warn!`, LF/CRLF/lone-CR byte-to-line mapping, per-block
source line offsets across both lexers, Ammonia sanitizer allowlists,
`compare_html` tag stripping, the dehydration pre-pass, and associated unit
tests, then resolved all Server/spec review findings (D1, D2, and minor items)
during implementation review.

#### Specification review rounds

| Round | Scope                                               | Findings                                     | Resolved                                                                                                                                                                                                                                                                  |
| ----- | --------------------------------------------------- | -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1     | `Tag format` (`spec.md`, lines 50–77)               | 10 (5 important, 1 moderate, 2 minor, 2 nit) | 10 (9 applied directly; 1 resolved via warning + save-time strip)                                                                                                                                                                                                         |
| 2     | `Placement` (`spec.md`, lines 76–170)               | 7 (2 important, 1 moderate, 3 minor, 1 nit)  | 7 (unified `Html`/`InlineHtml` raw-text \& warning scanner, clarified continuation-line tags, specified post-check suppression timing and `range.start` mapping, fixed `parse_doc_block` error wording)                                                                    |
| 3     | `Tag format & lookup` (`spec.md`, lines 39–75)      | 8 (3 important, 2 moderate, 2 minor, 1 nit)  | 8 (escaped hand-written `<l-n` tags with `log::warn!`, defined value-based lookup and container selection, updated mid-line semantics, fixed `cc-qSbTxGyL2n` cross-reference, defined valid tag syntax, and tightened wording/wrapping)                                   |
| 4     | `Placement` (`spec.md`, lines 88–183)               | 6 (2 important, 3 moderate, 1 nit)           | 6 (added rule and test to skip `<l-n>` subtrees on Client caret entry, reset raw-text suppression at boundaries with `log::warn!`, defined two-state scanner rules, fixed start-of-line `<textarea>` test example, specified `markdown_to_html` signature and `is_toc` wiring, and tightened wording/wrapping) |
| 5     | `Tag format & lookup` \& `Placement` (lines 39–194) | 6 (4 moderate, 2 nit)                        | 6 (replaced lookaheads and Unicode regex flags with ASCII-only `(?i-u)` patterns, restricted `InlineHtml` openers to offset 0, specified suppression reset timing and recorded opener line warning, defined `LineMap`, clarified `None` escaping behavior, and added `--!>` comment closer) |

#### Last specification review

**LLM used:** Claude Opus 5.5 Medium (Cursor)
**Prompt used:** `Review the sections named "Tag format & lookup" and "Placement" in @spec.md together, and writing findings to @review.md. Report only real bugs, contradictions, or missing details (no redesigns).`

All 6 findings in this round were resolved in `spec.md`.

Review of "Tag format & lookup" and "Placement" (spec.md, lines 39–194)
=====================================================================

Scope
-----

This review covers `<h2 id="cc-T4yjQxfetF">Tag format &amp; lookup</h2>`
(spec.md:39–86) and `<h2 id="cc-gpOIpo2gjZ">Placement</h2>` (spec.md:88–194),
read together. Other sections were read only for context. Claims about the
code were checked against the `line-number-support-2` branch:
`server/src/processing.rs` (`markdown_to_html`, `source_to_codechat_for_web`,
the `DOC_BLOCK_SEPARATOR_*` constants, `doc_blocks_to_dom`,
`parse_doc_block`, `render_fragment_content`), `server/Cargo.toml`, and
`Cargo.lock`, which pins pulldown-cmark 0.13.6 (its `html.rs` writer was
read). Nothing was built or run.

Summary
-------

All six findings from the previous round are fixed. Four problems remain:

* The regexes in both sections use lookahead, which the project's only regex
  crate (`regex = "1"`) rejects. They also use Unicode `\s` and Unicode case
  folding, so the scanner can decide a raw-text element has closed when
  html5ever hasn't. Tags then land inside RCDATA, where dehydration can't
  strip them, so they get saved.
* An opener inside an inline HTML attribute value still turns suppression on.
  In a Markdown document, nothing resets it before the end of the input. In
  one ordering it also gets the scanner out of step with html5ever and
  corrupts saved text.
* The suppression reset at doc block separators doesn't say where it happens,
  and the required warning has no source line to report.
* `LineMap` is part of the new signature but is defined nowhere.

Findings
--------

~~### 1. The regexes can't compile, and their Unicode matching disagrees with html5ever (moderate)~~

The hand-written-tag regex `(?i)<(/?l-n(?=[\s/>]))` (spec.md:81) and the
scanner's opener and closer patterns (spec.md:165, 168) all use `(?=…)`.
`server/Cargo.toml:84` depends only on `regex = "1"`. That crate has no
lookaround, so `Regex::new` fails, and the `LazyLock::new(|| …unwrap())`
pattern used for every other regex in `processing.rs` would panic the first
time it runs. `Cargo.lock` has no `fancy-regex`.

Getting rid of the lookahead isn't enough by itself. With the `regex` crate's
default Unicode mode, the patterns match things html5ever doesn't:

* `\s` matches U+000B and U+00A0, among others. html5ever ends a tag name
  only at tab, LF, FF, space, `/`, or `>`.
* `(?i)` uses simple case folding, so `s` also matches U+017F (`ſ`). html5ever
  lowercases only ASCII.

If this happens on an opener, the only effect is missing tags. If it happens
on a closer, the result is corruption. Take `x <textarea>a` followed on the
next line by `</textarea\u{A0}>b c`. The scanner sees a closer and ends
suppression. html5ever is still in RCDATA, so the tag placed before `b` is
textarea text. The dehydration pre-pass can't remove it, and the literal
`<l-n>2</l-n>` is saved.

**Suggest:** Give the patterns in a form the `regex` crate accepts, with ASCII
semantics. For example, `(?i-u)<(/?l-n)([\t\n\x0C\r />])` replaced with
`&lt;$1$2`. Consuming the boundary byte is harmless here: the scanner only
needs match positions, and the escape writes that byte back. Use the same
`(?i-u)` and `[\t\n\x0C\r />]` in the opener and closer patterns. (CR can't
appear after the lexer's normalization, but Markdown documents aren't
normalized, so keep it.)

~~### 2. An opener in an inline attribute value still disables tags, and can lead to corruption (moderate)~~

The reset added in the previous round only works at doc block separators
(spec.md:173–178). A Markdown document has no separators. So
`<a title="<select> menu">x</a>` drops every later tag in the document, with
only a warning at the end of the input. The parenthesis in the spec says the
reset stops such false positives "from dropping tags in later blocks". For
Markdown documents, which include `spec.md` itself, that isn't true.

The same false positive can also get the scanner out of step with html5ever
so that tags land in the wrong place:

```markdown
<a title="<!--">x</a> <textarea>a
b <!-- c --> d
</textarea>
```

1. The scanner treats the attribute's `<!--` as an open comment, so it skips
   the real `<textarea>` opener.
2. It closes the "comment" at `-->` on line 2.
3. Line 3's `</textarea>` is inline HTML (only an *opening* `<textarea` starts
   a type 1 HTML block), and suppression is now off, so it gets
   `<l-n>3</l-n>`. html5ever is still in RCDATA at that point, so the tag
   becomes textarea text and is saved.

**Suggest:** In the Unsuppressed state, accept an opener in an `InlineHtml`
event only at byte offset 0. pulldown-cmark emits each inline HTML construct
(one tag, one comment, one processing instruction, and so on) as its own
`InlineHtml` event. So an opener later in that event is always inside an
attribute value, and html5ever never treats it as an opener. Keep the current
anywhere-in-the-event search for closers in `InlineHtml`, because RCDATA and
raw text end at any `</name`, even one inside a quoted value. Also keep it for
openers in `Html` events, where one line can hold several tags. Correct the
parenthesis so it doesn't claim coverage for Markdown documents.

~~### 3. Where the reset happens and what the warning reports aren't defined (moderate)~~

spec.md:173–175 says to "reset suppression at every doc-block separator" and
to warn "with the file path and line number". Three things are missing:

* **How a separator is recognized.** No pulldown-cmark event marks a
  separator. The adapter has to match the separator text or use the segment
  boundaries in the line map. Neither method is named.
* **Whether the reset happens before or after the separator's own `Html`
  events are scanned.** The order matters. Line 3 of the separator
  (`processing.rs:305`) is `</pre></script></style></textarea>-->?>]]>`. If
  the reset happens when the first `<CodeChatEditor-fence>` line arrives, then
  any open `script`, `style`, or `textarea` target, or a comment from an HTML
  block, is still open at that point, so the reset warns. The spec says
  comments "are closed by the separator itself", and `parse_doc_block`
  accepts such a comment (`processing.rs:1635–1637`), so this warning is
  wrong. If the reset happens after the separator has been scanned, those
  targets close normally and only real false positives warn.
* **Which line the warning reports.** Every separator line maps to `None`
  (spec.md:252–253), and end of input has no line. The only line worth
  reporting is the opener's line.

**Suggest:** Record the opener's source line when suppression starts and
report that line in the warning. Reset when the first event at or after the
next segment's `start_md_line` arrives. At that point all of the separator's
`Html` events have been scanned. Also reset after the last event.

~~### 4. `LineMap` is used in the signature but never defined (moderate)~~

spec.md:90 adds `Option<&LineMap>`, but no section defines `LineMap`. The
section on source line offsets (spec.md:198–254) describes three different
things: a byte-offset-to-Markdown-line table built from "the exact input
bytes", the identity-plus-one mapping for Markdown documents, and per-block
segments for source files. The spec doesn't say:

* whether `LineMap` holds the line-start offsets, or `markdown_to_html`
  computes them from its `markdown` argument. If the caller computes them, it
  must use exactly the string that is passed.
* what the Markdown document caller (`processing.rs:1024`) passes when
  `is_toc` is false. It has no segments, so it needs an identity variant or
  segments built specially.
* who builds the segments. The natural place is the fold at
  `processing.rs:1072–1076`, which is where spec.md:245 says to record them.

**Suggest:** Define the type, for example
`enum LineMap { Document, Segments(Vec<(usize, Vec<usize>)>) }`, with the
line-start table computed inside `markdown_to_html`. State what each caller
passes.

~~### 5. What `None` turns off isn't stated (nit)~~

According to spec.md:91, `None` means "emit no tags". That leaves two
questions open. Does the hand-written-tag escape (spec.md:80–86), which runs
"before tag injection", still run? And what source line does its required
warning report when there's no map? If the escape is skipped, a hand-written
`<l-n>` in the sidebar TOC is hidden by `l-n { display: none; }`. If it runs,
TOC output and the output of every test that passes `None` change. Say which
one applies. If the escape runs, also say that the warning leaves out the
line.

~~### 6. `--!>` closes a comment in html5ever but not in the scanner (nit)~~

The HTML tokenizer also ends a comment at `--!>`. The scanner looks only for
`-->` (spec.md:169), so `<!-- a --!>` in an HTML block leaves the scanner
suppressed until the next `-->` or the next reset. The result is only missing
tags. Either add `--!>` to the closer, or list it with the accepted false
positives.

What's sound
------------

* The tag format matches `render_fragment_content`'s 1-based numbering
  (`processing.rs:2179–2185`). Strict monotonicity holds for the whole
  `markdown_to_html` call: events arrive in source order, and the
  content-line maps are strictly increasing within and across blocks.
* The lookup rule is consistent with the placement rules. Every untagged line
  (fences, HTML blocks, suppressed text, continuation lines) resolves to the
  nearest earlier tag in its container, or to no position.
* Raw-text suppression doesn't need an opaque-depth reset. A doc block's
  unterminated fence ends at the separator's backtick or tilde fence, and an
  `Image` can't cross a blank line.
* The separator has no raw-text opener and no `<!--`, so scanning it can't
  turn suppression on.
* The trigger list and the opaque set match pulldown-cmark 0.13.6's writer:
  * `raw_text` (`html.rs:479`) pulls the alt-text events through the adapter,
    so depth counting sees them.
  * `in_non_writing_block` suppresses `Text` but not `InlineHtml`, which is
    why `MetadataBlock` must be opaque.
* The Example output matches existing minified output. For example,
  `translation.rs:1966` shows `<li><input checked type=checkbox>Task list`,
  with no space after the checkbox.


#### Implementation review rounds

| Round | Scope | Findings | Resolved |
| ----- | ----- | -------- | -------- |
| 1 | Server tagger, line offsets, lexers, sanitization/dehydration, `compare_html`, and spec/AGENTS compliance (`line-number-support-2` vs `main`) | 8 (3 blocking in teammate phases B1–B3, 2 spec/gather deviations D1–D2, 3 minor) | 5 resolved directly (documented `CodeMirrorDocBlock` source-line height in spec and added `test_source_to_codechat_for_web_delimiter_lines`, added 6 `<fragment>` tags and `data-gather` entries, replaced per-opener `Regex::new` with static table + ASCII byte matcher, and added doc comments); 3 (B1–B3) later resolved in teammate commits |
| 2 | Full branch (`main..7ccb56d`) after teammate phases B1–B3 (`<xref>` cleanup, math/diagram hydration cleanup, Client UI & tests) | 4 (all non-blocking: stray root files, unrun browser tests, 2 minor test notes) | Verified B1–B3, D1–D2, and all 186 unit tests passing with zero regressions |
| 3 | Full branch (`main..7ccb56d`) final verification against `line-number-tags-spec.md` and `AGENTS.md` | 4 (0 functional bugs; 1 missing save round-trip test for hand-written `<l-n>`, 3 missing parameter/return comments, stray root files, unrun browser tests) | Confirmed zero functional bugs across all phases (186 passed, 0 failed) |


#### Last implementation review

**LLM used:** Claude Opus 5.5 Medium (Cursor)
**Prompt used:** `Review the implementation against @server/src/processing/line-number-tags-spec.md and @AGENTS.md, writing findings to @review.md. Report only real bugs, spec deviations, or missing requirements.`

Produced the folowing output

All functional requirements across Server and Client are verified with zero bugs (186 unit tests passing). Remaining items are non-blocking documentation/test nits and keeping `submission.md` for project submission.

Implementation review, round 3: line-number tags (`line-number-support-2` vs `main`)
===================================================================================

This round re-reviews the whole branch (`main..7ccb56d`) against
`server/src/processing/line-number-tags-spec.md` and `AGENTS.md`. It lists only
real bugs, spec deviations, and missing requirements. No source files were
changed. (A temporary probe test was added to try the edge cases below, then
removed.)

**Verdict:** I found no functional bugs in tag placement, line mapping, lexer
metadata, sanitization, hydration, dehydration, or comparison. One required test
is missing (finding 1), and the browser tests still haven't been run. The
other items are small `AGENTS.md` deviations and housekeeping.

Checks run
----------

| Check | Result |
| :--- | :--- |
| `cargo test --lib` | pass: 186 passed, 0 failed, 1 ignored (`lexer::tests::test_cpp`, also ignored on `main`) |
| Probe: Markdown and source-file round trips (`source_to_codechat_for_web` → `codechat_for_web_to_source`) | Tags matched $L$ in every case: `/*\nA\n*/\n// B\n` → `2`, `4` (JS and C); `/*\nA\n*/\n/*\nB\n*/\n` → `2`, `5` (merged in JS, separate in C); a code span ending on line 2 → `<code>a b</code><l-n>2</l-n> c`; an unclosed `<!--` in one Python doc block doesn't suppress the next block's tag. CodeMirror `from`/`to` matched `DocBlock.lines`, including the empty-contents exception (`x;\n//`). |
| Browser tests (`server/tests/overall.rs`, the Mocha suite in `client/src/CodeChatEditor-test.mts`) | **not run** |

Findings
--------

### 1. Missing test: hand-written `<l-n>` "survives save" (missing requirement)

The spec's Sanitization and dehydration case requires that a hand-written
`l-n` tag "escapes to literal `&lt;l-n` text that survives save".
`test_line_number_tags_hand_written` and `tests/line_number_warnings.rs` check
only the escape and the warning. No test runs the escaped HTML through
`codechat_for_web_to_source`.

The probe shows the visible text survives, but the first save rewrites the
source:

| Source | Saved as |
| :--- | :--- |
| `a <l-n>5</l-n> b` (Markdown document) | `a \<l-n>5\</l-n> b` |
| `// a <l-n>5</l-n> b` (JS doc block) | `// a \<l-n>5\</l-n> b` |
| `<div>\n<l-n>x</l-n>\n</div>` (HTML block) | `<div>&lt;l-n&gt;x&lt;/l-n&gt;</div>` |

After that, later saves are stable. "Preserves the text on save" can be read
either as "the visible text survives" (met) or "the source survives" (not
met). **Fix:** add a round-trip test that asserts the saved Markdown. Then
either accept the one-time rewrite and say so in the spec's "Hand-written tags"
bullet, or change the implementation.

### 2. Signatures missing parameter or return comments (`AGENTS.md`)

`AGENTS.md` asks for a comment on each parameter and on the return type in new
and edited signatures. These were added or edited on this branch without them:

* `parse_checked` (`server/src/lexer/pest_parser.rs:310`): no parameter or
  return comments.
* `append_code_doc_block` (`server/src/lexer.rs:712-714`): `indent`,
  `delimiter`, and `contents` have no comments, though the other three
  parameters do.
* `LineMap::segment_starts` (`server/src/processing.rs:1604`): no
  return comment.

### 3. Stray root files (open since round 1)

`review.md` and `submission.md` are still tracked at the repository root.
Remove them before merging.

### 4. Browser tests not run (open since round 1)

The updated exact-HTML assertions in `overall_1.rs` and `overall_2.rs`, and the
new Client test in `CodeChatEditor-test.mts`, have never been run. Run them
before merging.

### Danial Baitakov

GitHub userid: exekerey

#### Contributions

Reviewed the Dehydration, Re-translation \& diffs, and Testing sections
(`spec.md`) against `server/src/processing.rs`, `server/src/translation.rs`, and
`client/src/CodeMirror-integration.mts` over two review rounds, revising them to
use a separate tag-removal pre-pass, state how long tags stay stale, scope diff
amplification to source files, and add discriminating test cases.

Implemented Phases 5 and 7: a pre-hydration pass in `hydrate_dom` that removes
`<l-n>` tags from `<pre>`, `<code>`, math, Mermaid/GraphViz, and `<xref>`
(fixing review items B1 and B2), plus the remaining required tests (hydration
cleanup, container-bounded raw-text blocks, hand-written tag warnings, and diff
amplification).

#### Specification review rounds

| Round | Scope                                         | Findings                                    | Main changes made                                                                                                                                                                                                                                                                                                                                                                             |
| ----- | --------------------------------------------- | ------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1     | Dehydration, Re-translation \& diffs, Testing | 9 (4 important, 2 moderate, 2 minor, 1 nit) | Tag removal moved to a separate pre-pass that merges split text nodes; staleness window stated (corrected only by IDE-originated edits); shared `strip_line_number_tags` helper and `translation.rs` tests; diff amplification limited to source files with a measurable TODO; tests added for dehydration edge cases, comparison, and diff amplification                                     |
| 2     | Dehydration, Re-translation \& diffs, Testing | 10 (2 important, 7 moderate, 1 minor)       | Line-offset test corrected for `render_fragment_content`'s `o_i` difference; diff-amplification test now checks `contents`, not just an `Update`; pre-pass call sites named and `<xref>` case dropped; cursor-marker cases clarified and made unit-testable; strip order fixed; Consumers marked out of scope; partly rejected (#7 premise, most of #8, parts of #9 and #10)                  |
| 3     | Dehydration, Re-translation \& diffs, Testing | 7 (1 important, 2 moderate, 3 minor, 1 nit) | Existing exact-HTML tests in `overall_*.rs` and `ide/vscode/tests.rs` are updated to include tags; cursor-marker test states the expected text after re-wrapping; comparison covers a tag missing on one side; staleness wording fixed (any sent re-translation corrects tags); test that both dehydration paths run the pre-pass; dropped the unworkable gutter-number check (all 7 applied) |

#### Last specification review

**LLM used:** Claude Opus 5.5 (Claude Code)
**Prompt used:** `Review the sections named "Dehydration", "Re-translation & diffs" (including its "Consumers" subsection), and "Testing" in spec.md (lines 231–313), writing findings to review.md`, 
followed by a paragraph limiting scope to those sections, listing 
the source files to check claims against, and asking
for only relevant findings (no redesigns).`

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
**Prompt used:**
``Review **`Source line offsets`** and **`Sanitization`** in spec.md``

One minor ambiguity found:

* <s>**[P3] “No attributes” needs clarification**</s> — `spec.md:251`. Adding `l-n`
  to `add_tags` still permits generic attributes. The existing configuration
  allows `id` and `data-gather`; Ammonia also defaults to allowing `lang` and
  `title`. If all attributes must be stripped, specify an attribute filter for
  `l-n`. Otherwise, say “add no tag-specific attributes.” See
  [Ammonia's attribute handling](https://docs.rs/ammonia/4.1.2/src/ammonia/lib.rs.html#1905-1945).

No substantive issues found in **Source line offsets**. The per-line mapping
handles stripped delimiters, merged comments, empty comments, and final lines
without a newline consistently.

#### Implementation review rounds

| Round | Scope | Findings | Resolved |
| :--- | :--- | :--- | :--- |
| 1 | Implementation against `server/src/processing/line-number-tags-spec.md`: placement, source offsets, lexer metadata, sanitization/hydration, dehydration, comparison, and Client configuration | 2 (1 P3 warning-line bug, 1 P3 missing required regression test) | 0; both remain open. Passed 22 line-number unit tests, 2 lexer metadata tests, and the warning integration test. |

#### Last implementation review

**LLM used:** GPT-6 (Codex)
**Prompt used:** `review server/src/processing/line-number-tags-spec.md`

Reviewed the complete specification against `server/src/processing.rs`, both
lexer paths and their metadata tests, `server/src/translation.rs`, the Client's
TinyMCE configuration, CSS and caret placement, and associated regression tests.
The Consumers subsection is explicitly informative and was not treated as an
implementation requirement.

1. <s>**[P3] Count lone CR when locating warnings inside HTML events.**</s>
   `escape_hand_written_tags` (`server/src/processing.rs:1901`) computes a
   match's line by counting only `\n` in the event prefix; `scan_raw_text`
   (`:1968`) uses the same calculation for an opener. The spec explicitly
   counts lone `\r` as a line ending (`line-number-tags-spec.md:198-204`) and
   requires warnings to name the source line (`:80-84`, `:177-181`). A temporary
   probe translating `<a\rtitle="<l-n>">a</a>` reported `review.md:1` for the
   reserved tag on line 2; `<div>\r<l-n>x</l-n>\r</div>` also reported line 1
   for both tags on line 2. The unclosed opener in
   `<div>\r<textarea>\n\na` likewise produced an opener warning for line 1
   instead of line 2, before hydration rejected the unclosed HTML. Apply the
   same LF/CRLF/lone-CR rules as `LineStarts` to offsets within HTML events.
   Extend `server/tests/line_number_warnings.rs` to assert file paths and actual
   line numbers for mixed endings; its current assertions check warning counts
   and the word `reserved`, so they miss this bug.

2. <s>**[P3] Add the required save regression for hand-written tags.**</s>
   The specification requires escaped hand-written tags to survive save
   (`line-number-tags-spec.md:411-415`). `test_line_number_tags_hand_written`
   (`server/src/processing/tests.rs:3951-3972`) checks only forward translation,
   and `server/tests/line_number_warnings.rs:76-96` checks warnings. The
   dehydration tests exercise injected or pasted DOM tags, which must be
   deleted, rather than escaped hand-written tags, whose text must survive.
   A regression that deleted this literal text during save would therefore
   pass these tests. Add round-trip assertions through
   `source_to_codechat_for_web` and `codechat_for_web_to_source` for inline HTML
   and HTML blocks in Markdown documents and source-file doc blocks.
   The temporary probe confirmed that `a <l-n>5</l-n> b\n` saves as
   `a \<l-n>5\</l-n> b\n`; the JavaScript doc-block path preserves the same
   escaped text after `// `. An HTML-block input
   `<div>\r<l-n>x</l-n>\r</div>` saves as
   `<div>&lt;l-n&gt;x&lt;/l-n&gt;</div>\n`. These results preserve literal text
   with normalized Markdown spelling; they are not evidence of text loss.
   The finding is missing required regression coverage.

No other functional deviations were found in the inspected placement rules,
source-line mapping, source heights, hydration/dehydration passes, comparison,
or Client configuration. Client behavior was inspected but not browser-tested.

Validation (all commands run with `-p codechat-editor-server` and `--offline`):

* `cargo test --lib line_number`: 22 passed, 165 filtered out.
* `cargo test --lib line_metadata`: 2 passed, 185 filtered out.
* `cargo test --test line_number_warnings`: 1 passed.
* A temporary integration probe reproduced the incorrect warning lines and
  checked save behavior in Markdown-document and JavaScript doc-block paths;
  it was removed after review.

The full suite and browser tests were not run. Only `submission.md` remains
changed; neither finding was fixed during this review.
