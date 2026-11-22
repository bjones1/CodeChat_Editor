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

Reviewed the `Tag format` (`spec.md`, lines 50–77) and `Placement` (`spec.md`,
lines 76–170) sections against `server/src/processing.rs`,
`server/src/processing/cache.rs`, and `pulldown-cmark` 0.13.6 across two review
rounds, resolving 17 total findings (including raw-text suppression across HTML
block boundaries, multi-line inline continuation semantics, closing-tag
suppression timing, `range.start` offset mapping, and pre-injection warning
scanning).

#### Specification review rounds

| Round | Scope                                 | Findings                                     | Resolved                                                                                                                                                                                                |
| ----- | ------------------------------------- | -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1     | `Tag format` (`spec.md`, lines 50–77) | 10 (5 important, 1 moderate, 2 minor, 2 nit) | 10 (9 applied directly; 1 resolved via warning + save-time strip)                                                                                                                                       |
| 2     | `Placement` (`spec.md`, lines 76–170) | 7 (2 important, 1 moderate, 3 minor, 1 nit)  | 7 (unified `Html`/`InlineHtml` raw-text \& warning scanner, clarified continuation-line tags, specified post-check suppression timing and `range.start` mapping, fixed `parse_doc_block` error wording) |

#### Last specification review

<strong>LLM used:</strong> Claude Opus 5.5 Medium (Cursor)<br><strong>Prompt
used:</strong> `Review the section named "Placement" in @spec.md, writing
findings to @review.md`

All 7 findings in this round were resolved in `spec.md`.

Review of "Placement" (spec.md, lines 76–170)
=============================================

Scope
-----

This review covers only "Placement" (spec.md:76–170), including its "Trigger
rules", "Example", and "Exceptions" subsections. Other sections were read only
for context. Claims about existing code were checked against the
`line-number-support-2` branch, mainly `server/src/processing.rs`
(`markdown_to_html`, the doc block separator, `doc_blocks_to_dom`,
`parse_doc_block`), and against the locked pulldown-cmark 0.13.6 source
(`lib.rs`, `html.rs`, `scanners.rs`, `firstpass.rs`). Nothing was built or run.

Summary
-------

The trigger model is sound. The trigger and non-trigger lists together cover
every `Event` and `Tag` variant in pulldown-cmark 0.13.6, so no event is left
unclassified. The worked example matches the rules. There are four real
problems:

* Raw-text suppression is only triggered by `InlineHtml`. An HTML *block* can
  leave the same elements open past its end.
* The trigger rules tag continuation lines of multi-line inline events, while
  three other places in the spec say those lines get no tag.
* The spec doesn't say whether suppression ends before or after the closing
  event's own trigger check.
* The `plaintext` rule is based on an inaccurate description of
  `parse_doc_block`.

The remaining findings are small gaps in the wording.

Findings
--------

<del>\### 1. Raw-text elements opened by an HTML block aren't suppressed
(important)</del>

<del>spec.md:139–142 starts raw-text suppression only "when an `InlineHtml`
event opens one of the tags below". The spec assumes that an HTML block is
covered by opaque depth. It isn't, because opaque depth ends when the
`HtmlBlock` ends, and html5ever doesn't stop there. CommonMark ends many HTML
blocks at a blank line or at the end of their container, even if an element they
opened is still open:</del>

* `iframe`, `noframes`, and `title` start type-6 blocks, which end at a blank
  line (pulldown-cmark `scanners.rs:70, 78, 93`).
* `xmp`, `noembed`, `noscript`, `plaintext`, `template`, and `select` alone on a
  line start type-7 blocks, which also end at a blank line.
* Type-1 blocks (`script`, `style`, `textarea`, `pre`; `firstpass.rs:2182`) and
  type-2 comment blocks end at their container's end, for example inside a list
  item or block quote.

For example:

```markdown
<iframe>

Some text

</iframe>
```

This renders as `<iframe>\n<p><l-n>3</l-n>Some
text</p>\n<p></iframe></p>`.`parse_doc_block` accepts it because the element is
closed before the sentinel. html5ever then keeps `<l-n>3</l-n>` as literal
iframe text. This is the same corruption spec.md:143–147 describes for the
`InlineHtml` case: dehydration can't strip the tag, so the next save writes
`<l-n>3</l-n>` into the file. An unclosed `<!--` in a block quote is similar:
the later paragraphs, with their tags, end up as comment text.

<strong>Suggest:</strong> Look at `Html` events as well as `InlineHtml` events.
At `End(HtmlBlock)`, if the block's lines opened a listed element (or a comment)
without closing it, start suppression. Add the `<iframe>` example above, and a
type-1 block closed by its container, to the required Exceptions tests.

<del>\### 2. Continuation lines are tagged, but the spec says they aren't
(important)</del>

<del>The trigger rules (spec.md:90–94) insert a tag before the first trigger
whose offset is on a new line. If a multi-line `Code`,`InlineMath`,
`InlineHtml`, or `Image` is followed on its last line by more content, that
content is a trigger on a new line. For example, `` `a\nb` c `` produces a
`Text(" c")` on line 2, so line 2 gets a tag in the middle of the line: `<code>a
b</code><l-n>2</l-n> c`. The same happens after `<span\nclass=x>` and after an
image whose alt text spans lines.</del>

Three passages say the opposite:

* spec.md:51–53: lines "continuing an inline event from an earlier line ... get
  no tag".
* spec.md:158–161: "continuation lines of multi-line code spans/inline
  math/inline HTML" are untagged.
* spec.md:317: the Placement test expects "code span and inline math spanning
  lines (second line untagged)".

An implementer can follow the algorithm or the description, but not both, and
the required test result depends on which one they pick.

<strong>Suggest:</strong> Keep the algorithm, since a tag in the middle of a
line still satisfies the lookup rule. Reword the other three passages: a
continuation line gets no tag *at its start*. It is tagged at its first trigger
after the multi-line event, if there is one. Split the test into two cases: a
span that ends its line (second line untagged), and a span followed by text (tag
before that text).

<del>\### 3. Order of the closing event's check is unspecified (moderate)</del>

For opaque depth, spec.md:128–130 says exactly when the counter changes:
"*after* the trigger check". Raw-text suppression (spec.md:139–142) has no such
rule. The closing event is often a trigger itself, for example an
`InlineHtml("</textarea>")` at the start of a line. If suppression ends *before*
that event's trigger check, the tag goes in front of `</textarea>`. That is
still inside the raw text, which is exactly what suppression is meant to
prevent. The opening event has the same question in reverse. A tag before
`<textarea>` is harmless, but only if suppression starts after the check.

<strong>Suggest:</strong> State that suppression starts and ends *after* the
trigger check of the event that opens or closes it, just as opaque depth does.
Add a test where the closing tag starts a line: `<textarea>a\n</textarea> b`
must not produce `<l-n>2</l-n>` inside the textarea.

<del>\### 4. `plaintext` rule misdescribes `parse_doc_block` (minor)</del>

spec.md:145–147 says `plaintext` suppresses "until EOF (`parse_doc_block`
already warns on hidden text)". `parse_doc_block` doesn't warn. It returns an
error (`processing.rs:1639`), which `source_to_codechat_for_web` turns into
`SourceToCodeChatForWebError::ParseFailed`, and the file fails to translate
(`processing/tests.rs:2249`). The same error covers every element that is still
open at the end of a doc block or document: raw-text elements, `template` (the
sentinel is hidden in its contents), and `select` (the sentinel start tag is
dropped). In source files, each doc block is parsed on its own
(`doc_blocks_to_dom`, `processing.rs:1597–1599`). So suppression only matters
for an element that is closed later in the same doc block. "Until EOF" never
produces any output. spec.md:72 uses the same "warning" wording for the
hand-written-tag warning.

<strong>Suggest:</strong> Replace the parenthetical with: "An element still open
at the end of a doc block or document makes `parse_doc_block` reject the
translation, so suppression only needs to handle elements closed later in the
same container." Fix spec.md:72 to match. Either call the hidden-text check an
error, or don't use it as the model for a "non-blocking warning".

<del>\### 5. Which byte offset is mapped isn't stated (minor)</del> spec.md:93
says "the byte offset maps to a valid source line". It doesn't say which offset.
The reasoning for `SoftBreak` and `End` (spec.md:102–106) implies the start of
the event's range, but an implementer who maps the event's whole range could
choose the end. For a multi-line `InlineHtml` or `Code` trigger, the start and
the end are on different lines.

<strong>Suggest:</strong> "...the event's `range.start` maps to a valid source
line `N`".

<del>\### 6. Warning scanner and injected tags (minor)</del>

The adapter injects `Event::InlineHtml("<l-n>N</l-n>")` (spec.md:78–80).
spec.md:70–74 warns on "any `l-n` start tag ... in an `InlineHtml` or `Html`
event". If the warning check sits downstream of the adapter, or checks the
rendered HTML, every generated tag triggers a warning.

<strong>Suggest:</strong> Say that the hand-written-tag check examines parser
events *before* injection (or skips injected events), and that it uses the same
pass that tracks raw-text suppression.

<del>\### 7. Smaller wording gaps (nit)</del>

* The untagged-lines list (spec.md:158–161) leaves out continuation lines of
  multi-line display math (`$$\nx\n$$` within a paragraph). These follow the
  same rule as inline math.
* `HtmlBlock`'s reason (spec.md:136) says tags "could corrupt" its contents.
  However, an HTML block contains only `Html` events, which are never triggers,
  so opaque depth adds nothing there. Listing it does no harm. The real risk is
  the one in finding 1, after the block ends.

What's sound
------------

* The trigger list (spec.md:84–88) and the excluded list (spec.md:96–107)
  together cover every `Event` variant and every `Tag` variant in pulldown-cmark
  0.13.6 (`lib.rs:156–266, 524–600`). This includes `Superscript`, `Subscript`,
  `DefinitionList*`, and the GFM `BlockQuote` kind, all enabled by
  `Options::all()` in `markdown_to_html` (`processing.rs:1446–1455`).
* Making `Image` opaque is correct. `HtmlWriter::raw_text` (`html.rs:480–520`)
  writes the image's inner `InlineHtml`, `Text`, and `Code` events, escaped,
  into `alt`, so an injected tag there would become literal alt text.
* The `SoftBreak` and `HardBreak` rule matches the writer. `SoftBreak` writes
  `\n` and `HardBreak` writes `<br />\n` (`html.rs:125–130`), so a tag injected
  before the next line's first trigger follows that whitespace, as
  spec.md:102–104 says.
* The `MetadataBlock` row is accurate. The writer only suppresses `Text` inside
  it (`in_non_writing_block`, `html.rs:102, 386`), and opaque depth covers the
  rest.
* The example (spec.md:111–124) follows the rules: line 3's tag goes before
  `Some`, the `Emphasis` start on the same line gets none, line 4's tag follows
  the soft break, and line 6's tag goes before `TaskListMarker` inside `<li>`.
* Strict monotonicity holds. `push_html` writes footnote definitions and all
  other events in source order, and the separator lines in source files map to
  no line.
* The doc block separator's terminator line
  (`</pre></script></style></textarea>-->?>]]>`, `processing.rs:305`) is an
  `Html` event. So a `script`, `style`, or `textarea` opened by inline HTML ends
  its suppression at the separator, as spec.md:140–142 allows.

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
