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
