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

Reviewed the `Tag format & lookup` (`spec.md`, lines 39–75) and `Placement`
(`spec.md`, lines 76–170) sections against `server/src/processing.rs`,
`server/src/translation.rs`, `server/src/processing/cache.rs`,
`client/src/tinymce-config.mts`, `client/src/CodeMirror-integration.mts`, and
`pulldown-cmark` 0.13.6 across four review rounds, resolving 31 total findings
(including escaping hand-written `<l-n>` tags, defining value-based lookup and
container selection, skipping `<l-n>` subtrees during Client caret placement,
resetting raw-text suppression at boundaries with formal two-state scanner
rules, and specifying the `markdown_to_html` signature and `is_toc` wiring).

#### Specification review rounds

| Round | Scope                                          | Findings                                     | Resolved                                                                                                                                                                                                                                 |
| ----- | ---------------------------------------------- | -------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1     | `Tag format` (`spec.md`, lines 50–77)          | 10 (5 important, 1 moderate, 2 minor, 2 nit) | 10 (9 applied directly; 1 resolved via warning + save-time strip)                                                                                                                                                                        |
| 2     | `Placement` (`spec.md`, lines 76–170)          | 7 (2 important, 1 moderate, 3 minor, 1 nit)  | 7 (unified `Html`/`InlineHtml` raw-text \& warning scanner, clarified continuation-line tags, specified post-check suppression timing and `range.start` mapping, fixed `parse_doc_block` error wording)                                  |
| 3     | `Tag format & lookup` (`spec.md`, lines 39–75) | 8 (3 important, 2 moderate, 2 minor, 1 nit)  | 8 (escaped hand-written `<l-n` tags with `log::warn!`, defined value-based lookup and container selection, updated mid-line semantics, fixed `cc-qSbTxGyL2n` cross-reference, defined valid tag syntax, and tightened wording/wrapping) |
| 4     | `Placement` (`spec.md`, lines 88–183)          | 6 (2 important, 3 moderate, 1 nit)           | 6 (added rule and test to skip `<l-n>` subtrees on Client caret entry, reset raw-text suppression at boundaries with `log::warn!`, defined two-state scanner rules, fixed start-of-line `<textarea>` test example, specified `markdown_to_html` signature and `is_toc` wiring, and tightened wording/wrapping) |

#### Last specification review

**LLM used:** Claude Opus 5.5 Medium (Cursor)
**Prompt used:** `Review the section named "Placement" in @spec.md, writing
findings to @review.md`

All 6 findings in this round were resolved in `spec.md`.

Review of "Placement" (spec.md, lines 88–183)
=============================================

Scope
-----

This review covers only the `<h2 id="cc-gpOIpo2gjZ">Placement</h2>` section
(spec.md:88–183), including its subsections "Trigger rules", "Example", and
"Exceptions". Other sections were read only for context, or are cited where
they conflict with this one. Claims about existing code were checked against
the `line-number-support-2` branch: `server/src/processing.rs`
(`markdown_to_html`, `source_to_codechat_for_web`, the
`DOC_BLOCK_SEPARATOR_*` constants, `doc_blocks_to_dom`, `parse_doc_block`,
`html_to_dom`) and `client/src/CodeMirror-integration.mts`. CommonMark
behavior was checked against the 0.31.2 spec that `processing.rs` cites.
Nothing was built or run.

Summary
-------

All seven round 2 findings are fixed. The trigger rules, the opaque-depth
counter, and the check-then-toggle order for raw-text suppression are sound.
This round found four real problems:

* Placement makes `<l-n>` the first child of nearly every block, and the
  Client's existing caret code walks `firstChild` down to the first text
  node. Entering a doc block from above puts the caret inside the hidden tag,
  so typed text is invisible and deleted on save.
* Raw-text suppression is never reset. One false-positive opener (an
  attribute value, or markup inside a comment) silently drops every later tag
  in the document, or in every later doc block of a source file. The
  parenthesis that is meant to rule this out is inaccurate.
* The scanner's matching rules are undefined. A naive scan gets the empty
  comments `<!-->` and `<!--->` wrong, and those trigger the drop above.
* The section's key example of a closing tag at the start of a line,
  `<textarea>a\n</textarea> b`, is a CommonMark HTML block, so it never goes
  through the rule it is meant to test.

The remaining findings are about the `markdown_to_html` signature and wording.

Findings
--------

~~### 1. Every block's first text node is now hidden, and the Client puts the caret there (important)~~

The trigger rules put the tag *inside* `<p>`, `<h1>`, `<li>`, `<td>`, and so
on, before the first inline content (spec.md:108–113). Doc block HTML
therefore almost always starts `<p><l-n>N</l-n>…`.

`CodeMirror-integration.mts:1238–1270` handles the caret moving into a doc
block from the line above. It descends `contents.firstChild` until it reaches
a text node, then calls `range.setStart(edgeNode, 0)`. With tags present, that
text node is the digits inside `<l-n>`. The comment there explains that
TinyMCE keeps this selection after it initializes. So the user's first
keystrokes go into the `l-n` element: they are hidden by
`l-n { display: none; }`, and the dehydration pre-pass deletes them along
with the tag on the next save. The Sanitization table says the CSS rule
"block[s] caret entry" (spec.md:269). That holds for caret movement by the
browser, but not for a range set by script.

The end-of-block branch (`lastChild`) is safe, because a tag is never the last
child of its line. Any other Client code that picks "the first text node" has
the same problem.

**Suggest:** Add a rule to the spec, and an entry to the implementation
checklist: Client code that chooses a caret position must skip `l-n`
subtrees. In the code above, that means a walk that ignores `l-n` elements
when it looks for the first text node. Extend the Client test (spec.md:393)
so that it enters a tagged doc block from above, types a character, and
checks that the character is visible and survives a save.

~~### 2. Suppression is never reset, so a false positive silently drops all later tags (important)~~

The scanner looks at raw `Html`/`InlineHtml` text. Suppression ends only when
"a later `Html` or `InlineHtml` event contains its closing sequence"
(spec.md:153). Nothing ends it at a doc block separator or at the end of the
input. These cases open suppression and never close it:

* **Attribute values.** `<a title="<select> menu">x</a>` is valid inline
  HTML. html5ever sees an `a` element, but the scanner sees `<select>`.
* **Markup inside comments.** `<!-- TODO: add <template> -->`. Whether the
  scanner ignores openers while a comment is open is unspecified (finding 3).
* **Empty comments.** See finding 3.

The doc block separator happens to contain
`</pre></script></style></textarea>-->` (`processing.rs:300`). So false
positives for `script`, `style`, `textarea`, and comments end at the next
doc block. False positives for `title`, `xmp`, `iframe`, `noembed`,
`noframes`, `noscript`, `plaintext`, `template`, and `select` never end. They
remove every tag from the rest of the Markdown document, or from every later
doc block in a source file. Nothing warns, and scroll sync and cache line
facts are lost without any visible error.

The parenthesis at spec.md:159 is meant to rule this out. It says that
"Unclosed elements at a doc-block or document boundary cause
`parse_doc_block` to reject the translation". That is true only for elements
that are actually open in html5ever's DOM (raw-text elements, `select`, and
`template` all lose the sentinel, so `parse_doc_block` rejects them). It says
nothing about the scanner's false positives. It is also wrong for comments:
`parse_doc_block` accepts a sentinel found inside a comment
(`processing.rs:1635–1637`). Comments are safe only because the separator's
`-->` ends suppression in the Markdown event stream before
`DOC_BLOCK_SEPARATOR_REMOVE_FENCE` deletes that `-->` from the HTML.

**Suggest:** Reset suppression at every doc block separator. This is safe
because, as shown above, a real open target cannot cross a separator without
`parse_doc_block` rejecting the translation. Log a `log::warn!` (with file
path and line) whenever suppression is still active at a separator or at the
end of the input, so that a dropped tail is at least visible. Rewrite the
parenthesis to say why the reset is safe, and mention the comment case.
Optionally, have the scanner skip quoted attribute values, which removes the
most common false positive.

~~### 3. The scanner's matching rules aren't defined (moderate)~~

spec.md:153–157 name the targets but not the matching rules. Implementations
will differ on:

* **Empty comments.** CommonMark 0.31.2 (§6.6) accepts `<!-->` and `<!--->`
  as complete inline HTML comments, and html5ever also closes them right
  away. A scanner that searches for `-->` *after* the `<!--` opener misses
  both closers, because they overlap the opener. Suppression then starts and
  never ends (finding 2).
* **Several transitions in one event.** An `Html` line such as
  `</textarea><!-- x` closes one target and opens another. "Contains its
  closing sequence" doesn't say whether the scan continues past the closer.
* **Openers while suppressed.** Inside a comment or `<textarea>`, a
  `<script>` has no effect in html5ever. The spec doesn't say whether the
  scanner ignores it or treats it as a nested opener.
* **Tag-name boundaries.** html5ever ends a raw-text element at `</name`
  followed by whitespace, `/`, or `>`, case-insensitively. So
  `</TEXTAREA >` and `</textarea\n>` close it. An opener must not match
  `<selection>` or `<titlebar>`. The hand-written-tag rule already gives a
  regex for this (spec.md:81); this rule has none.

**Suggest:** Define the scanner as a two-state machine that runs repeatedly
over each event. When not suppressed, find the earliest opener matching
`(?i)<(script|style|…|select)(?=[\s/>])` or `<!--`. When suppressed, look
*only* for that target's closer: `(?i)</name(?=[\s/>])` for elements, or
`-->` searched from the first byte of `<!--` (so that `<!-->` and `<!--->`
count as closed). After a closer, continue scanning the rest of the event.
Add `<!-->`, `<!--->`, and `</textarea><!--` to the Exceptions tests.

~~### 4. The section's own example of a closing tag at the start of a line isn't inline HTML (moderate)~~

Raw-text suppression starts and ends after the trigger check "so a closing
tag at the start of a line does not get an `<l-n>` tag placed right before
it" (spec.md:153). The Testing section's example of this is
`<textarea>a\n</textarea> b`, where `<l-n>2</l-n>` must appear after
`</textarea>` (spec.md:341–342). However, `<textarea` at the start of a line
begins a CommonMark HTML block of type 1 (§4.6, condition 1), and type 1
blocks can also interrupt a paragraph. The whole input becomes one
`HtmlBlock`, so it is opaque, and ` b` is raw HTML that gets no tag at all.
The required assertion fails for every correct implementation.

The same applies to `script`, `style`, and `pre`. `title`, `iframe`, and
`noframes` begin type 6 blocks, which can also interrupt a paragraph. Only a
target opened *in the middle of* a paragraph line exercises the
closing-event rule.

**Suggest:** Change the example to `x <textarea>a\n</textarea> b`. Its
expected output contains `<l-n>1</l-n>x <textarea>a\n</textarea><l-n>2</l-n>
b`. Add a sentence to "Raw-text suppression" saying that a target opened at
the start of a line is normally an HTML block. For HTML blocks, suppression
matters only when the block ends before its target closes, for example at a
blank line (types 6–7) or at a container boundary.

~~### 5. `markdown_to_html`'s new inputs aren't specified (moderate)~~

`markdown_to_html` currently takes only `markdown: &str` (`processing.rs:1446`)
and has two callers: the Markdown document path (`processing.rs:1024`) and
the source-file path (`processing.rs:1079`). After this change it needs three
new inputs, and the section names none of them:

* the line map, which is the identity-plus-one function for Markdown
  documents but a list of per-block segments for source files
  (spec.md:194–243);
* a way to turn tags off when `is_toc` is true (spec.md:35–37, 181–182). The
  parameter is still the unused `_is_toc` (`processing.rs:972`);
* the file path, which the hand-written-tag warning requires (spec.md:83).

Existing tests call `markdown_to_html(markdown)` directly
(`processing/tests.rs:1709, 1798, 1820, 1845, 1874`), so the signature
affects every one of them.

**Suggest:** Give the signature, for example
`markdown_to_html(markdown: &str, line_map: Option<&LineMap>, file_path: &Path)`
where `None` means "emit no tags" (the TOC case and tests that don't care).
Add the `is_toc` wiring to checklist item 2.

~~### 6. Smaller wording gaps (nit)~~

* **HtmlBlock opacity is redundant.** The table gives the reason "Contains
  only non-trigger `Html` events" (spec.md:148). If so, depth never matters
  inside it. Either say the entry is defensive, or drop it.
* **Untagged lines** (spec.md:170–174) leaves out continuation lines of a
  multi-line link destination or title (`[a](url\n"t")`) and lines inside a
  raw-text suppression region.
* **Sidebar TOC** is stated three times: in the introduction (spec.md:35),
  under Other constructs (spec.md:181), and in the Tag format section. Keep
  only the one in the introduction.
* **Formatting.** spec.md:151, 153–157, and 159 run past 80 columns, against
  `AGENTS.md`. Five blank lines (spec.md:160–164) separate "Raw-text
  suppression" from "Other constructs".

What's sound
------------

* The trigger list is complete for pulldown-cmark 0.13 with `Options::all()`
  minus smart punctuation. It covers every inline leaf event and every inline
  container tag, including `Superscript`/`Subscript` and the wikilinks that
  `Options::all()` turns on, which arrive as `Tag::Link`.
* Excluding `SoftBreak`, `HardBreak`, and `End` is right. Their ranges start
  on the previous line or repeat the `Start` range. Keeping break whitespace
  outside the tag is what makes the round trip byte-identical.
* No trigger can land on a doc block separator line. Every separator line is
  an `Html` event inside an `HtmlBlock`, and the separator's leading blank
  line ends any paragraph, list, or block quote that is still open. So
  `DOC_BLOCK_SEPARATOR_REMOVE_FENCE`'s exact-string replacement and
  `DOC_BLOCK_SEPARATOR_BROKEN_FENCE`'s `.*?` never meet an injected tag.
* `MetadataBlock` must be opaque. pulldown-cmark's HTML writer suppresses
  `Text` inside it but still writes `InlineHtml`, so an injected tag would
  otherwise appear in the output.
* Raw-text suppression only reads `Html`/`InlineHtml`, which is correct.
  pulldown-cmark escapes `>` in `Text`, `Code`, and attributes, so generated
  HTML can never contain a closing sequence that the scanner misses.
* The Example output is what minify-html produces from pulldown-cmark's
  output: it leaves out the optional `</p>` before `<ul>`, and the soft break
  collapses to the space before `<l-n>4</l-n>`.


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
