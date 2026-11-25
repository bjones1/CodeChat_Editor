Project 2 submission
====================

Team name: line number support

The specification consists of the file [line-number-spec.md](line-number-spec.md).

Draft submission
----------------

### Dias Jakupov

GitHub userid: diasjakupovkbtu

#### Contributions

Ran spec review rounds 3-8 and rewrote the spec to name every function it
changes, then implemented it in the Server and Client.

#### Build and run test

Prompt: "Verify that you can build and run the project in the current git
repo." Summarized output, on commit `8284648` (code identical to `main`
`061a0a1`):

- Environment: macOS 26.6.2 (arm64); rustc 1.98.0 and cargo 1.98.0; Node.js
  25.8.2, npm 11.11.1, pnpm 12.23.0; cargo-nextest 0.9.152; Chrome 158 for the
  browser tests.
- `./bt build`: finished, exit code 0.
- `cargo nextest run --workspace --no-fail-fast`: 221 run, 211 passed, 10
  failed, 6 skipped. All 10 failures are browser tests (`overall_1::test_client`,
  `overall_2::test_5` and 8 in `overall_5`) which time out waiting for MathJax
  to render; the Server log shows `/static/mathjax/tex-chtml.js` is missing
  from the local build. Every unit and websocket test passed.

#### Specification review rounds

| Round | Findings | Main changes made |
| ----- | -------- | ----------------- |
| 3     | 9        | Own check against the code: `l-n` must be allowed by Ammonia and TinyMCE and hidden by CSS; markers removed when saving; link text and gathered fragments cleaned; footnote and provenance rules dropped; one placement rule |
| 4     | 11       | Fresh LLM review: a marker in an `<xref>`'s optional content would be saved; `compare_html` must ignore markers; no lexer change; table/test contradiction on raw HTML and display math; existing test expectations gain markers; prototype named |
| 5     | 1        | While planning the code: the caret walk into a doc block must skip a hidden marker |
| 6     | 1        | Found by a browser test: a re-translation must keep the Client's doc block when only markers differ, or the caret moves and TinyMCE turns dirty |
| 7     | 1        | Found by a browser test: a paragraph holding only a marker has no height, so the Client pads it with TinyMCE's `<br>` |
| 8     | 6        | Last fresh LLM review: an `</iframe>` alone on a line left every later line unmarked (fixed in code); wording fixes on `<xref>` lines, which lexer has the known deviation, the vscode test expectations, the overstated "never at the top level" and the gather heading level |

#### Last specification review

LLM: Claude Opus 5.5, run as a fresh reviewer with no part in writing the spec
or the code. It had the repo with the implementation in the working tree, the
instructor's prototype and `LLM-use.md`, and could run `cargo test --lib`.
Paths in the prompt are shortened: `{repo}` is the CodeChat Editor clone,
`{book}` the course book repo, `{scratch}` a scratch folder.

<details>
<summary>Prompt</summary>

> Review the specification `line-number-spec.md` at the root of the CodeChat Editor repo "{repo}" (quote the path: it has spaces, an apostrophe and Cyrillic). Write your findings to {scratch}/spec-review-round8.md . Do not edit any file in the repo.
>
> The assignment the spec answers (from the instructor): "Enhance the CodeChat Editor by writing a new spec that causes the Editor to embed line number information in the HTML it sends to the Client: each line of Markdown in a source file or Markdown document should emit a `<l-n>24</l-n>` tag (with exceptions where this isn't possible -- inside code blocks, etc.)". The instructor's early prototype is "{book}/project_notes/md-ast-line-numbers/src/main.rs" (it emits `<!--line N-->` comments instead of `<l-n>`). The instructor's spec-writing guidelines: "{book}/LLM-use.md". Project rules for the code: `AGENTS.md` in the repo.
>
> The spec has been implemented in the repo's working tree (uncommitted; `git diff` against HEAD shows it). This is the last review before the draft submission: find any remaining issues.
>
> Rules for the review:
> - Facts only: real problems in the spec's logic, in what it requires of the code, and in its tests. Check each finding against the actual code (server/src/processing.rs, server/src/translation.rs, client/src/\*, the pulldown-cmark 0.13 source under ~/.cargo/registry/src/\*/pulldown-cmark-0.13\*) and cite file:line evidence. A finding you could not confirm in code must say so.
> - Flag places where the spec and the implemented code disagree, where the spec misses a place in the code the change touches, or describes behavior that doesn't exist.
> - Flag text the instructor would have to ask "why?" about, and machinery that adds complexity without preventing a real bug. The instructor wants a concise spec that is the smallest change to the existing code and names the existing functions it extends.
> - Do not propose large rewrites or extra features. Small remarks may be listed at the end, briefly.
> - Number findings; for each give: the spec text concerned, the problem, the evidence, a one-line suggested fix. If you find no real problem, say so plainly.
> - You may run `cargo test --lib` from server/ after: export PATH="$HOME/.cargo/bin:$HOME/.rustup/toolchains/1.98.0-aarch64-apple-darwin/bin:$PATH"; export CARGO_TARGET_DIR={scratch}/review-spec8/target . Do NOT run the browser tests (server/tests/overall\*.rs) or ./bt. Put any scratch code under {scratch}/review-spec8/ .
> Keep the review under ~120 lines. Reply with just the file path and the count of findings.

</details>

Output. All six findings were checked against the code and accepted. Finding
1(b) was fixed in code, not only in the spec. Implementation review 2 later
replaced the fixes for findings 1(a) and 2 with code: elements opened by an
HTML block and `<xref>` content are now tracked, so neither is a deviation.

<details>
<summary>Full review</summary>

Reviewed against the working tree (uncommitted diff on `8284648`),
pulldown-cmark 0.13.6 (the version in `Cargo.lock`) and TinyMCE in
`client/node_modules`. `cargo test --lib`: 164 passed, 1 ignored. The 16
`ide::vscode` tests need a copy of `server/hashLocations.json` next to the
custom target directory; that is setup, not code. Probes ran in a scratch copy
of the repo. I did not run the browser tests.

Overall: the spec matches the code. Every function it names exists and does
what the spec says. All 12 test-table rows have matching tests, and the
expected HTML matches. I found no machinery without a reason: a probe
reproduced the reason for each piece, except the two browser-only claims
(see the small remarks). Six findings follow, all of low or medium severity.

##### Findings

1. **Spec text:** "Insert nothing either from an inline HTML tag which opens
   an element … up to the inline tag which closes it." **Problem:** the
   tracking only looks at `InlineHtml` events (`processing.rs:1549`). A tag
   in an HTML block (`Html` events) is never seen. Two cases come from this:
   (a) The element is opened by an HTML block that a blank line ends
   (CommonMark types 6 and 7: `iframe`, `title`, `noframes`, `noscript`,
   `noembed`, `xmp`, `wc-mermaid`, `graphviz-graph`). The lines after the
   blank line then get markers inside the element. These markers become text
   and are saved.
   (b) The element is opened inline but closed by a `</iframe>` that stands
   alone on a line, which is a type-7 HTML block. `text_element` then stays
   set, and no later line in the file gets a marker. **Evidence:**
   * Probe (a): `<iframe src="x">⏎⏎hello⏎</iframe>` gives
     `<iframe src=x><p><l-n>3</l-n>hello</p></iframe>`, and saving writes
     `<l-n>3</l-n>` into the source.
   * Probe (a): `<noscript>⏎⏎hello⏎</noscript>` shows
     `&lt;l-n>3&lt;/l-n>hello` in the Client and saves
     `\<p>\<l-n>3\</l-n>hello \<l-n>4\</l-n>`.
   * Probe (b): `Text <iframe src="x">⏎⏎</iframe>⏎⏎More⏎⏎And more` gives
     `<p>More<p>And more` with no markers. In a JS file, a later doc block
     loses its marker in the same way.

   These inputs are rare, and even before this change a save rewrites them
   (without markers, the iframe input is saved as
   `<iframe src="x"><p>hello</p></iframe>`). The marker text is the new part.
   **Fix:** add one sentence under "Known deviation": "an element which an
   HTML block opens or closes isn't tracked". Checking `Html` events instead
   would mean scanning every tag on the line, because a line such as
   `<iframe …></iframe>` must not leave the element open.

2. **Spec text:** "A line with no inline event of its own gets no marker: a
   blank line, a code fence, …" **Problem:** the list leaves out one case. A
   line whose first inline event is inside an `<xref>`'s optional content
   gets a marker, and server change 6 then removes it. That line ends up with
   no marker, even when text follows `</xref>` on the same line. **Evidence:**
   * Probe: `See <xref ref="t">two⏎lines</xref> after` gives
     `<p><l-n>1</l-n>See <xref …>…</xref> after`, with no `<l-n>2</l-n>`.
   * The expected HTML of `test_line_markers_copies` (`tests.rs:3433`) also
     has no marker for line 4.
   * Cause: `remove_line_markers` runs in `hydrate_source_xref`
     (`processing.rs:2004`).

   **Fix:** add "or a line which starts inside an `<xref>`'s content" to that
   list.

3. **Spec text:** "For a block comment whose text starts on the line after
   the opening delimiter (`/*⏎`), the lexer drops that newline …"
   **Problem:** only the generic lexer drops it. C and C++ (`.c`, `.h`,
   `.cpp`, …) use the Pest parser, which keeps it, so their markers are
   exact. **Evidence:**
   * `lexer.rs:701` returns the Pest parser's result before the generic
     lexer runs. `supported_languages.rs:163,178` maps C/C++ to
     `pest_parser::c`.
   * Probe, `/*⏎ Block text⏎ second⏎*/⏎code();⏎// After`: `.c` and `.cpp`
     give markers 2, 3, 6 (correct). `.js`, `.ts`, `.rs` and `.java` give 1,
     2, 5 (one line early).

   **Fix:** "the generic lexer (all languages except C/C++ and Python) drops
   that newline".

4. **Spec text:** "Existing expected HTML in `processing/tests.rs` and
   `server/tests/overall*.rs` gains markers; removing them must give the old
   expectations exactly." **Problem:** the diff also changes 13 expectations
   in `server/src/ide/vscode/tests.rs` and one fixture in the `translation.rs`
   tests, and the spec doesn't name either. One vscode change is a diff
   length, not a marker (`to: Some(110)` → `Some(122)`, `tests.rs:1929`), so
   "removing them must give the old expectations exactly" doesn't hold there.
   **Fix:** name `ide/vscode/tests.rs`, and add "(diff lengths grow by the
   markers' length)".

5. **Spec text:** "These always sit inside a paragraph, heading, list item,
   table cell or definition, so a marker never lands … at the top level" (the
   same claim is in `markdown_to_html`'s doc comment, `processing.rs:1470`).
   **Problem:** overstated. Inline HTML that opens a block element ends the
   `<p>` in html5ever, so markers for later lines of that paragraph land at
   the top level. **Evidence:** probe `Text <div>a⏎b</div> c⏎d` gives
   `<p><l-n>1</l-n>Text<div>a <l-n>2</l-n>b</div> c <l-n>3</l-n>d<p>`.
   Nothing breaks, because the text the marker labels is at the top level
   too. **Fix:** "never lands between `<tr>` and `<td>`", and drop "or at the
   top level", or add "unless inline HTML opens a block element".

6. **Spec text:** the closing `<h4 id="cc-FekMIBZOuZ" data-gather=…>`.
   **Problem:** it comes straight after the level-2 heading "Tests", so it
   skips level 3. AGENTS.md ("Don't skip heading levels") and `LLM-use.md`
   item 5 both forbid that. `cache-spec.md` puts its gathers one level below
   the section heading (`<h3>` after `<h2>`, lines 131 and 220).
   **Fix:** make it `<h3>`.

##### Small remarks

* Server change 9, "(a word wrap changes the code text)": the reader may ask
  why a word wrap changes code. Say "a doc block's line count changes, so
  CodeMirror's `doc` differs".
* Two browser-only claims are not verified here (browser tests not run):
  "browser tests `overall_1::test_client_updates` and `overall_3::test_8`
  fail without this", and the `NewBlock`/parser-filter padding.
  `overall_3.rs` is unchanged from `main`, and `test_8` does exercise Enter
  at the start of a paragraph and then Enter in the empty paragraph it
  leaves.
* Two Client changes have no row in the test table: removing markers from
  copied or cut HTML, and the caret walk stepping over a marker. The review
  log says the clipboard change was checked by hand in a standalone page.
  Either add rows or say they are checked by hand.
* Server change 7 ("one written by hand is removed too") has no unit test.
* Checked and fine: the inline-event list is complete for pulldown-cmark
  0.13.6, and `HtmlBlock` is rightly opaque, because `append_html_line`
  (`firstpass.rs:1480`) can emit `Text` inside a container.

</details>

### Amy Smith

GitHub userid: Kenzpie

#### Contributions

Wrote first draft of spec and review rounds 1 & 2.

#### Build and run test

Prompt: "Show that you can build and run the project in the current git repo."

- Environment: Windows; VS Code PowerShell terminal; Node.js 26.13.1 and npm 11.20.0.
- `cargo build`: completed successfully.
- `cargo test`: 223 workspace tests and 1 builder test passed. Six tests were skipped, with no failures.
- Execution was verified through the baseline test suite, including browser integration tests. The line-number feature was still at the specification stage.

#### Specification review rounds

| Round | Findings | Main changes made |
| ----- | -------- | ----------------- |
| 1 | 3 | Clarify how to distinguish generated markers and add a test to verify beharvior. Clarify line placement, and assert exact markers for selected lines. |
| 2 | 1 | Skip continuation lines represented as opaque and clarify generated numbers stay hidden when the document is in edit mode. |

#### Last specification review

LLM: GPT-6.1 Sol Light.

Prompt: "Run another review to find any remaining issues."

Output: "The line-number spec is ready for submission. It clearly defines which Markdown lines receive markers, how those markers preserve the original file’s line numbers, and when a marker should be skipped."

### Alisher Arginbekov

GitHub userid: aarginbekov

#### Contributions

TBA

#### Build and run test

TBA

#### Specification review rounds

TBA

#### Last specification review

TBA

### Aslan Tasmagambetov

GitHub userid: tasma-07

#### Contributions

TBA

#### Build and run test

TBA

#### Specification review rounds

TBA

#### Last specification review

TBA

Final submission
----------------

### Dias Jakupov

GitHub userid: diasjakupovkbtu

#### Contributions

Ran spec review rounds 3-8, implemented the spec in the Server and Client with
its tests, and ran implementation review rounds 1-6, fixing each accepted
finding, with a test where one applies.

#### Specification review rounds

Rounds 3-8, as in the draft section above. Since round 8 the spec changed only
through the implementation reviews below.

#### Last specification review

Round 8 (Claude Opus 5.5); prompt and output are in the draft section above.

#### Implementation review rounds

| Round | Findings | Main changes made |
| ----- | -------- | ----------------- |
| 1     | 8 (7 accepted) | Claude Opus 5.5 as a fresh reviewer: no markers inside `iframe`, `textarea` and other raw-text or diagram elements opened by inline HTML; markers left out of copied HTML; fragments, stale comments and 80-column wraps fixed |
| 2     | 8 + overengineering check (5 accepted) | Gemini 4 Neon (High): a tag in an image's `alt` text, an element left open at the end of a doc block, and an element opened by an HTML block handled; `<xref>` content tracked like the other elements, which removed a server change and a known deviation; `compare_html` strips markers first. Three Client editing findings and the caret walk rejected or not reproduced |
| 3     | 3 minor (3 accepted) | Gemini 4 Neon (High), verdict "yes": a nested two-line `<xref>` and an HTML block line which closes one `<iframe>` and opens another no longer save a marker; copied plain text keeps a `<l-n>` typed as text |
| 4     | 2 minor (2 accepted) | Gemini 4 Neon (High), verdict "yes": one function, `track_unmarked_element`, reads tags from left to right for inline HTML and HTML blocks alike, fixing a nested `<xref>` in an HTML block and a `<script>` whose text names `<iframe` |
| 5     | 3 minor + 1 trivial (1 accepted) | Gemini 4 Neon (High), verdict "yes", remaining findings "effectively" trivial: a tag name ends only at whitespace, `/` or `>`. Two findings did not reproduce |
| 6     | 2 minor + 2 trivial (1 + 1 accepted) | Claude Opus 5.5 as a fresh reviewer, verdict "yes": when Enter splits a paragraph just after its marker, the marker moves to the second half, so the empty half saves as an empty paragraph and Enter in an empty list item leaves the list; a TinyMCE parser filter is deleted. Not acted on: an unclosed `<!--`, which doesn't round-trip without markers either |

#### Last implementation review

LLM: Claude Opus 5.5, run as a fresh reviewer with no part in writing the code,
on this branch with the round 3 to round 5 fixes, against `main` `061a0a1`.
Rounds 2-5 used Gemini 4 Neon. It could read everything, run `cargo test --lib`
and probes in a copy of the repo, and test the Client's TinyMCE handler in a
standalone TinyMCE page. Paths in the prompt are shortened: `{repo}` is the
CodeChat Editor clone, `{book}` the course book repo, `{scratch}` a scratch
folder.

<details>
<summary>Prompt</summary>

```text
You are reviewing a student team's change to the CodeChat Editor, a clone at "{repo}" (quote the path: it has spaces, an apostrophe and Cyrillic). Do not edit any file in the repo. Report facts only.

## The task (instructor's assignment, verbatim)

> Enhance the CodeChat Editor by writing a new spec that causes the Editor to
> embed line number information in the HTML it sends to the Client: each line
> of Markdown in a source file or Markdown document should emit a
> `<l-n>24</l-n>` tag (with exceptions where this isn't possible -- inside code
> blocks, etc.) See <xref ref="cc-TB0Jxbo_gu"></xref> for an early prototype.

The prototype is `{book}/project_notes/md-ast-line-numbers/src/main.rs` (pulldown-cmark, inserts `<!--line N-->`).

Grading rules for this final submission (instructor, verbatim excerpts):

> An implementation of this section of the specification using an LLM, followed
> by repeated reviews of the implementation until the LLM produces trivial or
> contradictory findings.

> Avoid letting the LLM make significant changes, since these produce an
> overspecified result and limit your understanding of the problem.

On an earlier project the instructor rejected a spec as "far too verbose" and asked for the smallest change to the existing code, naming the existing functions it extends and adding no new structures.

## The change

- Spec: `line-number-spec.md` at the repo root.
- Implementation: committed on the current branch, plus the round 3 to round 5 fixes, uncommitted in the working tree. The full diff against upstream `main` (`061a0a1`), without `submission.md`, is in `{scratch}/implementation.diff` (same content as `git diff 061a0a1`).
- Main code: `server/src/processing.rs` (`markdown_to_html`, `UNMARKED_ELEMENTS`, the clean/dehydrate code, `compare_html`), `server/src/translation.rs` (`keep_client_doc_block_contents`), `client/src/tinymce-config.mts` (`handleLineMarkers`), `client/src/CodeMirror-integration.mts`, `client/src/css/CodeChatEditor.css`, and tests in `server/src/processing/tests.rs`, `server/src/ide/vscode/tests.rs`, `server/tests/overall/`.
- House rules for code and comments: `AGENTS.md`.
- Known and not to be reported: 10 browser tests time out on `main` too, because `/static/mathjax/tex-chtml.js` is missing from the local build.

## Earlier review rounds (already decided)

This is the sixth implementation review; rounds 2-5 used a different LLM. Round 5 changed `track_unmarked_element` so that a tag name ends only at whitespace, `/` or `>` (before, `<title_x>` on a line of an HTML block was read as `<title>` and hid every later marker). Check that this is right, and look with fresh eyes at the whole change, not only at the tag tracking.

These points were checked and decided; report them again only with a new, concrete repro that the earlier decision did not cover:

- Rejected: after bolding a whole paragraph and pressing Enter just past the marker, a later re-parse can leave a zero-height empty paragraph (several steps; a save drops an empty paragraph anyway).
- Rejected: `GetContent` with a selection in `tree` format (no caller uses it).
- Not reproduced in Chromium: typed text landing inside a hidden marker (caret at its start, at the end of a trailing one, or between two adjacent ones). The caret-walk skip stays as a guard.
- Rejected: `contains("</iframe")`-style checks have no tag-name boundary (`</iframe-x>`, a tag name inside a comment). Not worth a tag parser.
- Not added: a multi-line `<xref>` or `<iframe>` in the round-trip test (the save rewrites them the same way with or without markers); a test for a hand-written `<l-n>`.
- Known deviation, in the spec: a generic-lexer `/*⏎` comment puts the markers one line early.
- Not acted on (round 3, not reproduced): the `PreInit` node filter doesn't ignore whitespace-only text nodes; the `NewBlock` handler checks only the first half of a split block.
- Fixed in round 3, checked in round 4: a nested `<xref>` spanning two lines; an HTML block line which closes one `<iframe>` and opens another; `GetContent` strips markers from HTML only (the plain text comes from `innerText`).
- Not acted on (round 4): an end tag such as `</xref>` or `</wc-mermaid>` inside a Markdown paragraph, after an HTML block opened the element. html5ever ignores it (`p` is a special element), so the element runs to the end of the doc block while the tracking stops at the end tag. Such input doesn't round-trip without markers either.
- Not acted on (round 4): `markdown_to_html` builds the line table for an empty range list (only tests pass one).
- Fixed in round 4, checked in round 5: one function, `track_unmarked_element`, reads the tags of inline HTML and of HTML block lines from left to right.
- Not reproduced (round 5): the caret walk stopping in a trailing marker when a doc block is entered from below (`// Hello⏎// <script>x</script>` gives no trailing marker; the spec covers only entering from above). A marker inside `<select>` (it stays a hidden `<l-n>` element) or `<plaintext>` (the existing unclosed-tag check refuses the doc block).
- Not acted on (round 5): the spec's test table names test inputs and browser tests; the spec has no GPL header (the house rules ask for it in source files); an unclosed inline `<xref>` is closed by html5ever at the end of its paragraph while the tracking runs to the end of the doc block (malformed input; a marker is only missing).
- Kept: `script` and `style` in `UNMARKED_ELEMENTS`, though Ammonia drops them with their content anyway (keeps `markdown_to_html`'s own output right).

## What to assess

1. Correctness against the task. Does every Markdown line get a marker where one is possible? Are the exceptions right: is anything skipped that could be marked, or marked where the marker breaks something? Do markers stay out of what gets saved back to the source file, and does editing in the Client still work? For each problem give a concrete input and the wrong result versus the expected one. If your tools let you run commands (git, grep, `cargo test` in `server/`), use them to reproduce; do not modify files.
2. Overengineering. For each piece of the change, say whether the task or a real, reproducible bug requires it. Name what could be deleted or made simpler, and what would break if it were removed. Judge the spec too: is it longer or more detailed than the task needs?
3. Mismatches between the spec and the code, and tests that don't check what the spec claims.

Rate each finding: major (wrong output a user would hit), minor (rare input or cosmetic), or trivial (wording, style, taste).

## Tool use

You may read any file and run commands, but do not edit any file in the repo. To probe the code, copy the repo without `target/` and `node_modules/` to `{scratch}/repo` and edit only that copy. Before running cargo: `export PATH="$HOME/.cargo/bin:$HOME/.rustup/toolchains/1.98.0-aarch64-apple-darwin/bin:$PATH"; export CARGO_TARGET_DIR={scratch}/target`. Run `cargo test --lib` from `server/`; do not run the browser tests (`server/tests/overall*.rs`) or `./bt`. TinyMCE's source is in `client/node_modules`, pulldown-cmark's and html5ever's under `~/.cargo/registry/src/`.

## Output format

Numbered findings. For each: severity (major / minor / trivial), category (correctness / overengineering / spec-code mismatch / tests), location (file:line), what is wrong, evidence (repro or exact code reference), smallest fix. Then a short "Small remarks" list for minor points. End with a verdict: does the change solve the task (yes / partly / no), whether the remaining findings are trivial, and the three most important changes, if any. Keep it under 900 words. Don't pad: if something is fine, say so in one line.

Write the review to `{scratch}/output.md` and reply with just that path and the count of findings by severity.
```

</details>

Output. Finding 1 reproduced in the reviewer's TinyMCE page and is fixed: the
`NewBlock` handler moves the markers to the second half, and the `PreInit`
parser filter is deleted. Browser test `overall_3::test_8` fails without the
move. Finding 2 is malformed input which doesn't round-trip without markers
either, so it is not acted on. Of the trivial findings, the stale test comment
is fixed; the spec keeps its event list (the placement rule) and test table,
and the helper test stays, since the spec names the two browser tests which
fail without its call.

<details>
<summary>Full review</summary>

Checked: diff against `061a0a1`; `cargo test --lib` in a copy (164 pass, plus
probes: 32 inputs translated and saved with and without markers); a standalone
TinyMCE 8.10.1 page with the Client's config, CSS and `handleLineMarkers`
(browser tests not run).

##### Findings

###### 1. Minor -- correctness (Client) -- `client/src/tinymce-config.mts:216-259`

The `NewBlock` handler pads the first half of the split with a `<br>` but leaves
the marker in it, so TinyMCE still sees that block as holding content.

* Repro A: `<p><l-n>1</l-n>First para</p><p><l-n>3</l-n>Second</p>`, caret at the
  start of "Second" (after the marker, where `test_8`'s click lands), Enter, then
  `editor.save()`. That is what `CodeMirror-integration.mts:1537` writes into the
  static div when TinyMCE moves to another doc block. Result:
  `<p><l-n>3</l-n></p>`, height 0. The next Client update sends it, and the save
  drops it (`# First para\n#\n# Second`). Without markers, `save()` gives
  `<p>&nbsp;</p>`, height 18.5. So the spec's claim (line 146) that "the empty
  paragraph keeps its height" holds only while TinyMCE edits the block.
* Repro B: `<ul><li><l-n>3</l-n>a</li>…`, caret at the start of "a", Enter, Up,
  Enter. A third `<li>` is added. Without markers, TinyMCE turns the empty item
  into a `<p>` before the list.
* Smallest fix, checked in the same page: move the markers into the second half
  (`newBlock.prepend(...block.querySelectorAll("l-n"))`) before adding the
  `<br>`. Both repros then match the no-marker behaviour, Enter in the empty
  paragraph still works, and a `getContent`/`setContent` round trip keeps the
  `<br>`. The `PreInit` node filter (lines 231-259) then has nothing left to do
  and can be deleted. Spec bullet at line 102 shrinks to one sentence.

###### ~~2. Minor -- correctness (Server) -- `server/src/processing.rs:1476` (`track_unmarked_element`)~~ *(not acted on)*

An unclosed `<!--` on an HTML block line swallows the rest of the doc block, and
markers inside it are saved. Repro (`foo.md`): `<div>\n<!-- x\n</div>\n\nText\n\nMore\n`.

* With markers, the save writes `<p><l-n>5</l-n>Text</p>\n<p><l-n>7</l-n>More</p>`
  inside the comment.
* Without markers, the save writes `<p>Text</p>…` there. So the input doesn't
  round-trip in either case, but only this change puts `<l-n>` into the file.

Fix: treat `<!--` with no later `-->` like an opened unmarked element, closed by
`-->`. Or record it as malformed input, as round 4 did for a stray `</xref>`.

###### ~~3. Trivial -- overengineering (spec) -- `line-number-spec.md`~~ *(not acted on)*

155 lines is reasonable for the edge cases, but two parts could be shorter:

* The 13-row test table repeats the test code.
* The full list of inline `Event` variants is an implementation detail; "the
  first inline event of the line" plus the table-cell reason is enough.

Everything else names existing functions and is needed.

###### 4. Trivial -- tests *(first point: comment fixed)*

* `test_line_markers_inline_html_text` (`processing/tests.rs:3394`) also covers
  HTML-block cases (`<iframe src="x">\nfallback…`, `<div>\n<title_x>`), so its
  name and comment ("which inline HTML opens") undersell it.
* `test_keep_client_doc_block_contents` (`translation.rs:2131`) tests the helper,
  not its call in `send_re_translation`. Only the browser tests catch a removed
  call.

##### Overengineering, piece by piece

Everything is required except where noted:

* Marker loop, `file_ranges`, `opaque_depth`: the task and the prototype.
* `xref` tracking: the 80-column wrap makes two-line `<xref>`s common, and the
  dehydrating walk skips `<xref>` children, so markers would be saved.
* Diagram and raw-text tracking: without it, `a <textarea>\nb</textarea>` shows
  `&lt;l-n>2…` as text.
* Ammonia and clean lists, dehydration, `compare_html`: required.
* `keep_client_doc_block_contents`: required, per the spec's browser-test claim.
* CSS and valid elements: required.
* Clipboard filter: a real but small case (pasting into another app).
* Can go: the `PreInit` filter, via finding 1.
* Still a guard with no reproduced bug: the caret skip (kept by an earlier
  decision).

##### Small remarks

* Round 5 rule checked. The tag name ends where the HTML tokenizer ends it
  (whitespace, `/`, `>`), and the name is lowercased. So `</iframex>` doesn't
  close and `<iframe/>` opens, as in html5ever. Correct.
* `<a title="x <iframe y">link</a>\nmore\n\nPara\n` loses the markers for lines
  2 and 4. This is the same class as the rejected "tag name inside a comment";
  markers are only missing.
* The known deviation is confirmed: `/*\n a\n*/\nx;\n// b\n` (`a.js`) gives
  markers 1 and 4 instead of 2 and 5.
* Client markers stay stale after Client edits until an IDE-side translation.
  This is documented in spec item 8 and acceptable for hidden metadata.
* Fine with no change:
  * Multi-line inline tags and comments get their marker after the tag
    (`<span\nclass=x><l-n>2</l-n>y`).
  * Footnotes, definition lists, setext headings, lazy quotes, tables, task
    lists, nested lists with code, C block comments and indented Python doc
    blocks are all numbered right.
  * For every probe, the save matches the save of the marker-stripped HTML
    (except finding 2).
  * Saving Client-shaped HTML (marker inside `<strong>`, marker plus bogus `<br>`)
    matches the save without markers.

##### Verdict

* Does the change solve the task? **Yes.** Every Markdown line with inline
  content gets a correct marker. The exceptions match the prototype plus the
  necessary raw-text, diagram and `xref` cases. Markers stay out of saves except
  for malformed input (finding 2).
* Remaining findings: two minor, two trivial. None is major.
* The three most important changes:
  1. In `NewBlock`, move the markers into `newBlock`, then delete the `PreInit`
     filter (finding 1). This is a fix and a simplification at once.
  2. Decide on the unclosed-comment case: track `<!--`…`-->`, or record it as
     malformed (finding 2).
  3. Optionally trim the spec's test table and the inline-event list
     (finding 3).

</details>

### Amy Smith

GitHub userid: Kenzpie

TBA

### Alisher Arginbekov

GitHub userid: aarginbekov

TBA

### Aslan Tasmagambetov

GitHub userid: tasma-07

TBA
