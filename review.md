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
