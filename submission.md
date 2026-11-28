Project 2 submission
====================

Team name: Collapsing headings 1

The specification consists of the file
[client/src/collapsing-headings-spec.md](client/src/collapsing-headings-spec.md).

Draft submission
----------------

### Askar Oralkhan

GitHub userid: askaroralkhan

#### Contributions

Wrote the first draft of the spec, ran the three spec review rounds and split
their findings among the team, simplified rules 7 and 9 after round 2, and
wrote this file and the pull request.

#### Build and run test

Claude Opus 5.5 (Claude Code), prompted with "Verify that you can build and
run the project in the current git repo.", reported that the project builds
and runs, but not through `./bt build`:

* The Rust workspace built with `cargo +1.98.0 build --workspace`, the
  `export_bindings` tests passed (13 of 13), the Client's esbuild bundles
  built, and `tsc -noEmit` reported no errors.
* The standalone server started with
  `cargo run -p codechat-editor-standalone -- --port 8765 serve` and served
  `/` and `toc.md` with status 200.
* `./bt build` stopped because my default Rust toolchain was broken (since
  fixed with `rustup toolchain install stable --force`) and `cargo-nextest`
  wasn't installed. It didn't run `./bt install`, because that installs tools
  globally and pipes a script from the internet into bash.

#### Specification review rounds

Each round was a fresh, headless Claude Code session in a throwaway worktree
of the reviewed commit, so the reviewer could run `cargo`, `node` and
`pnpm exec` to check its findings.

| Round | Commit  | Findings | Main changes made |
| ----- | ------- | -------- | ----------------- |
| 1     | cc05f2f | 13       | Collapse state kept out of raw saves, `innerHTML` reads and cursor paths; collapsed headings found again after updates by a heading key; an edit touching hidden content opens it first; examples added for these (applied by Daniil) |
| 2     | d1e4650 | 14       | Simplified instead of adding rules: the IDE's cursor and scroll never open a section; rule 9 covers the Server's own re-translations; the cursor moves to the heading when its section collapses; the exact Tab order dropped; examples trimmed from 28 to 24 with a test layer each |
| 3     | 6248335 | 11       | Not applied yet; next: take heading keys from the Client's current content, keep the cursor out of content hidden by a Client edit or a Server update, limit link-following to Markdown until source-file links work, and cover TinyMCE's search and a keyboard route to doc-block buttons |

#### Last specification review

Prompting Claude Opus 5.5, extra high effort (Claude Code 2.1.330), with
`Review the spec in client/src/collapsing-headings-spec.md.` produced the
following output. Strikethrough would mark output that we deemed incorrect or
irrelevant; we struck none.

I found three problems that would cause wrong behaviour if the spec is implemented as written, and some areas the spec doesn't cover. The rules and examples agree with each other: examples 1–3, 6–8, 17 and 18 come out right under the section definition.

##### Would cause wrong behaviour

**1. The heading key breaks for headings edited in the Client.**
- **The problem:** the key uses the heading text "in the HTML the Server sends". In example 11 (collapse A, edit its text, autosave), the Server's last HTML still has A's old text. If a re-translation follows, as it does in example 24, the old key won't match the new content and A expands. In source files it's worse: the active doc block's current text is held in TinyMCE, not in `DocBlockWidget.contents`, until autosave.
- **"Text" is ambiguous:** it could mean `textContent` or the inner HTML. The Server writes attributes unquoted (`ref=foo`, `server/src/ide/vscode/tests.rs:2171`); TinyMCE doesn't, so inner HTML won't match between them.
- **Cross-references:** an empty `<xref>` inside a heading takes its text from its target. A cross-reference re-translation, one of the cases rule 9 lists, can then change the heading's key.
- **Fix:** take the "before" keys from the Client's current content as it would be sent (no controls, math not rendered). Compare `textContent` with whitespace and `&nbsp;` normalised. List the xref case beside the duplicate-heading limitation.

**2. The cursor can end up in hidden content without anyone collapsing.** Rule 2 only moves the cursor when a section is collapsed. Two other things can hide it:
- **Editing a different heading:** take `H1 A`, p1, `H1 C`, p3 with A collapsed and the cursor in C. Making C an `H2` or a paragraph pulls C and p3 into A's hidden section, cursor included. Example 17 only covers the reverse.
- **Server updates:** Markdown restores the selection as a list of child indexes (`client/src/CodeChatEditor.mts:400-412`, and `client/src/CodeMirror-integration.mts:539-545`). In example 19, a cursor in C (index 4) is put back at index 4, which is now p2, and p2 is hidden.
- **The examples avoid it:** the intro text says Server updates leave the cursor outside hidden content, so this case is never specified.
- **Fix:** a Client edit should never hide what it's editing; expand the section that would swallow it, as rule 8 does. After a Server update, move a hidden cursor to the end of the outermost collapsed heading without opening anything, as rule 7 does.

**3. Following a link into a collapsed section of a source file depends on an existing bug being fixed.**
- **Why it fails:** `followLink` finds its target with `document.querySelector` (`client/src/CodeChatEditor.mts:734`). CodeMirror only renders content near the viewport, and doesn't render replaced (folded) ranges at all. A collapsed target is never in the page, so the Client can't tell which sections to open.
- **It's already broken:** the same-file link tests are marked `ignore` for this reason (`server/tests/overall/overall_5.rs:1290-1333`). Example 14 is Markdown only, so nothing tests the source-file case.
- **Fix:** either limit the link part of rule 7 to Markdown, with a `TODO:` saying when that limit can go, or require looking up ids in the doc-block contents held in CodeMirror's state.

##### Not covered by the spec

**4. TinyMCE's search.** The `searchreplace` plugin is enabled (`client/src/tinymce-config.mts:92`), and it's the only in-editor search in Markdown documents. The Implementation section only names CodeMirror's search and go-to-line. TinyMCE's search needs to open sections, and its Replace All edits hidden content. The spec should also say whether the browser's own find counts.

**5. Keyboard access.**
- **Tab:** rule 1 deliberately doesn't promise a Tab order, but example 5 says "Tab to A's button". In source files, Tab indents code (`indentWithTab`, `client/src/CodeMirror-integration.mts:1846`) and doc-block contents can't be tabbed to. So there's no stated keyboard route to a doc-block heading's button.
- **Space and Enter:** pressed on a button inside a TinyMCE editor, they also reach TinyMCE's own key handling. Example 5 should check that the content is unchanged and the document isn't marked dirty.
- **Screen readers:** the button's only accessible name is the ▾/▸ symbol. Give it an `aria-label`, and add a collapsed heading to the fixtures used by `overall_a11y.rs`.

**6. Cases no example covers:**
- **Rule 2's cursor move:** not covered in either document type.
- **Source-file saves and dirty state:** no example covers either, yet the riskiest content-leak paths are source-file only: reading inactive blocks' `innerHTML` (`:826`), TinyMCE→div via `save()` (`:1527`), and div→TinyMCE via `resetContent(contentsDiv.innerHTML)` (`:1547`). The Tests bullet asks for save checks in both document types, but there's no example to give the source-file checks an expected result.
- **New headings:** a heading created in the Client, by changing a paragraph or pasting, getting its button.
- **Undo and redo:** TinyMCE undo replaces the editor's content, so the display-only buttons must be put back afterwards, as after `setContent`.

**7. The Tests bullet doesn't describe how Client tests run.**
- **Not just `test.py`:** the suite runs on any page loaded with `?test` (`client/src/CodeChatEditor-test.mts:21-24`), once per page load, and then deletes itself.
- **Markdown fixture:** the `test_client` test in `server/tests/overall/overall_1.rs:681-746` loads `test.py` and runs the suite. A Markdown fixture placed beside it won't run until it gets its own test like that.
- **Cheaper layer:** example 12 and the outgoing-HTML part of example 10 check Client-side behaviour (the cursor path and the raw save). They could be Client tests, which AGENTS.md prefers to the slow browser tests.

##### Nits
- **Paragraph breaks:** the second paragraph in rules 6, 8 and 9 has no blank line before it, so each renders merged into the first.
- **Rule 6:** "Check both raw TinyMCE saves…" is test guidance that repeats the Implementation section. It belongs under Tests.
- **"Every update":** "the IDE sends a line number with every update" isn't accurate. `cursor_position` is optional.
- **Review notes:** `collapsing-headings-daniil-reviews.md` still says rule 7 reflects the older cursor-expansion behaviour, but rule 7 has been rewritten since.

##### Confirmed correct
- **Heading definition:** `.CodeChat-doc-contents` is the right container in both document types.
- **Rust headings:** `///` is a doc-block delimiter for Rust (`server/src/lexer/supported_languages.rs:372`).
- **Update paths:** the three update paths are as described (`setContent`; destroying and recreating the view; diff transactions).
- **Markdown cursor:** `scrollToLine` really is a `TODO` for Markdown.
- **Gather lists:** they're inserted after their heading, not inside it (`server/src/processing.rs:2329`), so they don't affect heading keys.
- **Line width:** no prose line is longer than 80 characters.

### Daniil Chegodayev

GitHub userid: Daniil-Chegodayev

#### Contributions

Applied the round 1 findings across the whole spec with OpenAI Codex, then
simplified his rules (1–4 and 6) after round 2: where the cursor goes when a
section collapses, keyboard access without a fixed Tab order, and a shorter
"never saved" rule.

#### Build and run test

(Daniil: add a short summary of your LLM's answer to "Verify that you can build
and run the project in the current git repo.", and which LLM you used.)

#### Specification review rounds

OpenAI Codex reviews of Daniil's changes, run before and after Askar's round 2:

| Round | Reviewed version | Findings | Main changes made |
| ----- | ---------------- | -------- | ----------------- |
| 1 | fc02540 | 8 | Document order, Rust doc headings, empty sections and sections ending inside one doc block; raw saves, `innerHTML` reads and cursor mapping; heading matching across updates; preserved heading ids; new acceptance examples (applied in d1e4650) |
| Follow-up 1 | d1e4650 | 0 blocking | Text checks only, not a build or feasibility check; Askar's round 2 then found 14 issues |
| 2 (rules 1–4 and 6) | Changes after round 2 | 3, plus 2 consistency issues | Cursor and selection moved to the heading on collapse; exact keyboard navigation promises removed; "left" replaced by "before"; rule 3's cursor-driven expansion removed; rule 6 allows a normal cursor update (applied in bb34d61) |
| Follow-up 2 | bb34d61 | 0 | Wording and cross-references checked |

#### Last specification review

(Daniil: add the Codex model and the prompt you used.)

Rules 1–4 and 6: no further findings after the current corrections. Keyboard
access is required without promising an exact Tab order. Collapsing relocates
a cursor or selection endpoint from hidden content to the heading. Rule 3 no
longer promises cursor-driven expansion. Rule 6 allows normal cursor updates
while forbidding content updates caused solely by toggling. No findings were
rejected.

### Zhan Dzholdybayev

GitHub userid: zhanapan

#### Contributions

Trimmed the examples from 28 to 24, added a test layer for each example and
rewrote the test plan to match, and added the example for the Server's update
after an autosave.

#### Build and run test

(Zhan: add a short summary of your LLM's answer to "Verify that you can build
and run the project in the current git repo.", and which LLM you used.)

#### Specification review rounds

(Zhan: add your review table, or a sentence saying that you didn't run your
own review.)

#### Last specification review

(Zhan: add your last review, the prompt and the LLM, if you ran one.)

Final submission
----------------

Not required for this project, which has only a draft submission.
