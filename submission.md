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

TBA