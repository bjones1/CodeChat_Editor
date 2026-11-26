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
its tests, and ran implementation review rounds 1-8, fixing each accepted
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
| 7     | 1 major + 2 minor (3 accepted) | Claude Opus 5.5 as a fresh reviewer, verdict "yes", remaining findings "not trivial": Enter before a later line of a paragraph left that line's marker behind, so the Server sent a re-translation and the caret moved; the `NewBlock` handler moves every marker with nothing after it, `compare_html` removes markers before `minify`, and `restoreSelection` steps over a marker. Also fixed: a non-breaking space typed before a marker was saved into the source |
| 8     | 1 major + 1 minor (2 accepted) | Claude Opus 5.5 as a fresh reviewer, verdict "partly", remaining findings "not trivial": removing a marker left the text of its line as a separate text node, and htmd escaped block syntax such as `2.` at its start, so a save added backslashes; `dehydrating_walk_node` now joins the two text nodes, and the round-trip test covers it. TinyMCE Find and Replace searching the hidden markers is now a known deviation in the spec |

#### Last implementation review

LLM: Claude Opus 5.5, run as a fresh reviewer with no part in writing the code,
on this branch at `8dab56d` (rounds 1-7), against `main` `061a0a1`. Same setup
and prompt as round 7, with the round 7 changes described and its decisions
added; it was sent with a short note on the scratch setup. It could read
everything, run `cargo test --lib` and probes in a copy of the repo, and test
the Client's TinyMCE handler in a standalone TinyMCE page. Paths in the prompt
are shortened: `{repo}` is the CodeChat Editor clone, `{book}` the course
book repo, `{scratch}` a scratch folder.

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
- Implementation: committed on the current branch (`8dab56d`); the working tree is clean. The full diff against upstream `main` (`061a0a1`), without `submission.md`, is in `{scratch}/implementation.diff` (same content as `git diff 061a0a1 -- . ':!submission.md'`).
- Main code: `server/src/processing.rs` (`markdown_to_html`, `UNMARKED_ELEMENTS`, the clean/dehydrate code, `compare_html`), `server/src/translation.rs` (`keep_client_doc_block_contents`), `client/src/tinymce-config.mts` (`handleLineMarkers`), `client/src/CodeChatEditor.mts` (`restoreSelection`), `client/src/CodeMirror-integration.mts`, `client/src/css/CodeChatEditor.css`, and tests in `server/src/processing/tests.rs`, `server/src/ide/vscode/tests.rs`, `server/tests/overall/`.
- House rules for code and comments: `AGENTS.md`.
- Known and not to be reported: 10 browser tests time out on `main` too, because `/static/mathjax/tex-chtml.js` is missing from the local build.

## Earlier review rounds (already decided)

This is the eighth implementation review. Rounds 2-5 used a different LLM; rounds 6 and 7 used other fresh reviewers. Round 7 changed four things. (1) When Enter splits a block (`NewBlock` event), `handleLineMarkers` moves every marker with nothing after it in the first half (an empty inline element, whitespace or a bogus `<br>` count as nothing) to the start of the second half, and gives the first half the `<br data-mce-bogus="1">` of an empty block only if it is then empty. (2) `compare_html` removes markers before `transform_html` and `minify`, not after. (3) `restoreSelection` (`CodeChatEditor.mts`) steps from an `<l-n>` to its next sibling at each level of the saved path. (4) The `GetContent` handler turns a non-breaking space just before a marker into a space in the HTML TinyMCE returns. Check that these are right, and look with fresh eyes at the whole change, not only at these.

These points were checked and decided; report them again only with a new, concrete repro that the earlier decision did not cover:

- Rejected: after bolding a whole paragraph and pressing Enter just past the marker, a later re-parse can leave a zero-height empty paragraph (several steps; a save drops an empty paragraph anyway).
- Rejected: `GetContent` with a selection in `tree` format (no caller uses it).
- Not reproduced in Chromium: typed text landing inside a hidden marker (caret at its start, at the end of a trailing one, or between two adjacent ones). The caret-walk skip stays as a guard.
- Rejected: `contains("</iframe")`-style checks have no tag-name boundary (`</iframe-x>`, a tag name inside a comment). Not worth a tag parser.
- Not added: a multi-line `<xref>` or `<iframe>` in the round-trip test (the save rewrites them the same way with or without markers); a test for a hand-written `<l-n>`.
- Known deviation, in the spec: a generic-lexer `/*⏎` comment puts the markers one line early.
- Not acted on (round 3, not reproduced): the `NewBlock` handler checks only the first half of a split block.
- Fixed in round 3, checked in round 4: a nested `<xref>` spanning two lines; an HTML block line which closes one `<iframe>` and opens another; `GetContent` strips markers from HTML only (the plain text comes from `innerText`).
- Not acted on (round 4): an end tag such as `</xref>` or `</wc-mermaid>` inside a Markdown paragraph, after an HTML block opened the element. html5ever ignores it (`p` is a special element), so the element runs to the end of the doc block while the tracking stops at the end tag. Such input doesn't round-trip without markers either.
- Not acted on (round 4): `markdown_to_html` builds the line table for an empty range list (only tests pass one).
- Fixed in round 4, checked in round 5: one function, `track_unmarked_element`, reads the tags of inline HTML and of HTML block lines from left to right.
- Not reproduced (round 5): the caret walk stopping in a trailing marker when a doc block is entered from below (`// Hello⏎// <script>x</script>` gives no trailing marker; the spec covers only entering from above). A marker inside `<select>` (it stays a hidden `<l-n>` element) or `<plaintext>` (the existing unclosed-tag check refuses the doc block).
- Not acted on (round 5): the spec's test table names test inputs and browser tests; the spec has no GPL header (the house rules ask for it in source files); an unclosed inline `<xref>` is closed by html5ever at the end of its paragraph while the tracking runs to the end of the doc block (malformed input; a marker is only missing).
- Kept: `script` and `style` in `UNMARKED_ELEMENTS`, though Ammonia drops them with their content anyway (keeps `markdown_to_html`'s own output right).
- Fixed in round 6: Enter just after a marker left the marker in the empty first half (it saved with no height; Enter in an empty-looking list item added an item instead of leaving the list). See above.
- Not acted on (round 6): an unclosed `<!--` on a line of an HTML block runs to the end of the doc block and the markers inside it are saved (malformed input; it doesn't round-trip without markers either). An attribute value holding a tag name, such as `<a title="x <iframe y">`, hides later markers (same class as a tag name inside a comment; markers are only missing).
- Not acted on (round 6): shortening the spec's test table and its list of inline events (the list is the placement rule); a test of the `keep_client_doc_block_contents` call in `send_re_translation` (two browser tests fail without it).
- Fixed in round 7: Enter just before a later line of a paragraph left that line's marker at the end of the first half, so `compare_html` found a difference (the space before the marker survived `minify`), the Server sent a re-translation, and `restoreSelection` put the caret on the marker. Also fixed: a non-breaking space typed before a marker was saved into the source. See above.
- Not acted on (round 7): `line_starts` counts only `\n`, so a Markdown document with lone `\r` line endings gets wrong markers (callers replace `\r\n` only; rare); an inline `<pre>` inside a paragraph gets a hidden marker, which a save drops.
- Left as is (round 7): spec item 8 describes the doc blocks the Server keeps; the block being edited is covered by the Client caret bullet.

## What to assess

1. Correctness against the task. Does every Markdown line get a marker where one is possible? Are the exceptions right: is anything skipped that could be marked, or marked where the marker breaks something? Do markers stay out of what gets saved back to the source file, and does editing in the Client still work? For each problem give a concrete input and the wrong result versus the expected one. If your tools let you run commands (git, grep, `cargo test` in `server/`), use them to reproduce; do not modify files.
2. Overengineering. For each piece of the change, say whether the task or a real, reproducible bug requires it. Name what could be deleted or made simpler, and what would break if it were removed. Judge the spec too: is it longer or more detailed than the task needs?
3. Mismatches between the spec and the code, and tests that don't check what the spec claims.

Rate each finding: major (wrong output a user would hit), minor (rare input or cosmetic), or trivial (wording, style, taste).

## Tool use

You may read any file and run commands, but do not edit any file in the repo. To probe the code, copy the repo without `target/` and `node_modules/` to `{scratch}/repo` and edit only that copy. Before running cargo: `export PATH="$HOME/.cargo/bin:$HOME/.rustup/toolchains/1.98.0-aarch64-apple-darwin/bin:$PATH"; export CARGO_TARGET_DIR={scratch}/target`. Run `cargo test --lib` from `server/`; do not run the browser tests (`server/tests/overall*.rs`) or `./bt`. TinyMCE's source is in `client/node_modules`, pulldown-cmark's and html5ever's under `~/.cargo/registry/src/`. To test the Client's TinyMCE handler, you may build a standalone page in `{scratch}` from `client/node_modules/tinymce` with the Client's `handleLineMarkers`, valid-element settings and CSS, and drive it with any browser tool you have. TinyMCE 8.10.1 is already copied to `{scratch}/tiny/tinymce`; put your page in `{scratch}/tiny`. Browser tools won't run scripts on a `file://` page: the preview config `tinymce-probe-8` (preview_start with that name) serves `{scratch}/tiny` at http://127.0.0.1:8767.

## Output format

Numbered findings. For each: severity (major / minor / trivial), category (correctness / overengineering / spec-code mismatch / tests), location (file:line), what is wrong, evidence (repro or exact code reference), smallest fix. Then a short "Small remarks" list for minor points. End with a verdict: does the change solve the task (yes / partly / no), whether the remaining findings are trivial, and the three most important changes, if any. Keep it under 900 words. Don't pad: if something is fine, say so in one line.

Write the review to `{scratch}/output.md` and reply with just that path and the count of findings by severity.
```

</details>

Output. Both findings were checked and accepted. Finding 1 reproduced with a
new case in `test_line_markers_round_trip`: with markers, a line starting with
`2.` saved as `2\.`. `dehydrating_walk_node` now joins the text on either side
of a removed marker, and the test fails without the join. htmd moves `2.` off
the start of a line when it rewraps, without markers too, so the case expects
the rewrapped save. Finding 2 was checked in TinyMCE's `searchreplace` source,
which walks the text of a hidden marker; changing the plugin is outside this
change, so the spec names it as a known deviation. Not acted on: a
`<template>`'s content, which Ammonia drops with or without markers (small
remark).

<details>
<summary>Full review</summary>

How I tested:

* `cargo test --lib` in a copy of the repo: 164 passed, 1 ignored.
* A probe test, in the copy only, that saves each translation twice, once as sent and once with the markers stripped, and compares the two sources.
* A TinyMCE 8.10.1 page with the Client's handler, settings, CSS and plugins. I drove it with real key presses (Backspace, Home, Return, Shift+Home) and the `searchreplace` plugin API.

##### Findings

###### 1. Major (correctness, tests): a save adds backslashes that the same save without markers doesn't add

**Location:** `server/src/processing.rs:3248` (`dehydrating_walk_node`) and `server/src/processing/tests.rs:3521` (`test_line_markers_round_trip`).

**What is wrong:** removing an `<l-n>` leaves the text before and after it as two adjacent text nodes. htmd's `escape_if_needed` (`dom_walker.rs`) escapes block-start syntax at the start of each text node. As a result, every source line start is escaped, even when the save joins or rewraps that line into the middle of a paragraph.

**Evidence:** probe results for a Markdown document. A JavaScript doc block behaves the same.

| Source | Saved with markers | Saved with markers stripped |
| --- | --- | --- |
| `Some text⏎2. not a list` | `Some text 2\. not a list` | `Some text 2. not a list` |
| `Some text⏎2) not a list` | `2\)` | `2)` |
| A 75-character line, then `⏎2. not a list.` | `… flag 2\.⏎not a list.` | `… flag 2.⏎not a list.` |
| `…⏎~5 ms`, `…⏎= y holds` | `\~5`, `\= y` | `~5`, `= y` |
| `Some text⏎\# not a heading` (same for `\-`, `\>`, `\=`) | `Some text \# not a heading` | `Some text # not a heading` |

* An edit anywhere in such a doc block writes these backslashes into lines the user never touched.
* Line starts that are not block syntax are unaffected: `--release`, `#12`, `3.12`, `+1`.
* No file in the repo (61 `.md`) or the book (26 `.md`) triggers it. It needs hand-written prose with one of these line starts.
* None of the inputs to `test_line_markers_round_trip` start a continuation line this way. The test therefore doesn't back the spec's claim "Source identical to the input".

**Smallest fix:** after removing a marker, merge its two neighbours into one when both are text nodes. I checked this in the copy: all 9 probe cases then match the stripped save, and the 164 library tests pass.

```rust
if index > 0 && index < node.children.borrow().len() {
    let (prev, next) = { let c = node.children.borrow(); (c[index - 1].clone(), c[index].clone()) };
    if let (NodeData::Text { contents: a }, NodeData::Text { contents: b }) = (&prev.data, &next.data) {
        let b = b.borrow().clone();
        a.borrow_mut().push_tendril(&b);
        node.children.borrow_mut().remove(index);
    }
}
```

Then add `Some text⏎2. not a list` to the round-trip test.

###### 2. Minor (correctness): TinyMCE Find/Replace sees the hidden markers

**Location:** `client/src/tinymce-config.mts:92`, where the `searchreplace` plugin is enabled. It has no handling in `handleLineMarkers`.

**Evidence:** with `<p><l-n>1</l-n>First line <l-n>2</l-n>Second line</p>` in the editor:

* Find `line Second` returns 0 matches. Without markers it returns 1.
* Find `2` returns 1 match: the hidden marker.
* Find `line 2Second` returns 1 match.
* Replace all `2` with `X` gives `<l-n>X</l-n>`. The save still drops it, but `LINE_MARKER` (`\d+`) no longer matches it, so `compare_html` reports a difference.

**Smallest fix:** add one line to the spec's Client section that names this as a known limitation.

##### Checked and correct

The four round 7 changes:

* **`NewBlock`:**
  * Home then Enter at the start of a paragraph moves the marker to the second half and gives the first half `<p><br data-mce-bogus="1"></p>`.
  * Enter in an emptied list item leaves the list.
  * Deleting text also deletes its marker, so no trailing markers are left.
* **`compare_html`:** strips markers before `transform_html` and `minify`. `test_compare_line_markers` covers a space before a marker.
* **`restoreSelection`:** steps over an L-N correctly.
* **`GetContent`:** the regex covers both `&nbsp;` and U+00A0. The handler fires on `save({format:"raw"})`, which is the path the Client uses.

Also correct:

* **Placement:** right for every construct I probed: headings, lists, lazy quotes, footnotes, tables, task lists, code, math, raw HTML, images and hard breaks, plus Python, C, JavaScript and Rust doc blocks.
* **Cache:** no markers reach it.
* **Cursor:** its character is inserted before dehydration.

##### Overengineering

* Every piece is required by the task or by a bug reproduced in an earlier round. This includes the clipboard strip: without it, text pasted outside the Client shows the numbers.
* The CodeMirror caret-walk skip is still a guard for a case nobody has reproduced. Earlier rounds decided to keep it.
* The spec is long: 162 lines for a 4-line task. That was settled in earlier rounds. Finding 2 adds one line.

##### Small remarks

* The spec's test-table row "Source identical" holds only once finding 1 is fixed.
* Ammonia drops the content of a `<template>` element, with or without markers.

##### Verdict

**Solves the task: partly.** Markers are placed correctly and Client editing works. One save regression remains: finding 1.

**Remaining findings:** not trivial. There is 1 major and 1 minor.

**Top 3 changes:**

1. Merge the text nodes around a removed marker in `dehydrating_walk_node`.
2. Add a round-trip test whose continuation line starts with `2.`.
3. Note the Find/Replace limitation in the spec.

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
