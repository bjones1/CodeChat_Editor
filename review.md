Implementation review: line-number tags (`line-number-support-2` vs `main`)
==========================================================================

This review checks commits `7d68342`, `7b86251`, and `e1c88a3` against
`server/src/processing/line-number-tags-spec.md` and `AGENTS.md`.

Checks run
----------

| Check | Result |
| :--- | :--- |
| `cargo fmt --check` | pass |
| `cargo clippy --lib --bins` | pass (no warnings) |
| `./bt flint --check` (I ran `--check` because plain `flint` passes `--fix` and rewrites files) | clippy `--workspace --all-targets -Dwarnings` and fmt pass. The run then stops at `cargo audit`, which reports 6 advisories (actix-http ×3, ammonia, cookie, url). They aren't caused by this branch, since `Cargo.lock` is unchanged, but `cargo sort`, `machete`, and `eslint` never ran. |
| `cargo test --lib` | **1 failure**, 178 passed: `processing::tests::test_hydrate_unclosed_tag_keeps_prose` (see B1) |

I didn't run the browser tests (`server/tests/overall.rs`). The updated
expectations in `overall_1.rs` and `overall_2.rs` haven't been checked by running them.

Bugs
----

**B1. `<l-n>` inside `<xref>` is saved as literal text, which fails an existing test.**
`hydrate_dom` / `hoist_fragment_content` (`server/src/processing.rs:2210-2222`,
`2322-2338`) never strip `<l-n>`. `hydrate_source_xref` (`processing.rs:2382`)
therefore copies the tag into `data-content`, and dehydration restores it
from there. The dehydration pre-pass can't remove it, because it is
attribute-encoded text. Failure at `server/src/processing/tests.rs:2297`:

```
// <xref ref="t"><p><l-n>3</l-n>**Important** prose.</p></xref>
```

A multi-line `See <xref ref="t">a\nb</xref>` hits the same path. Spec:
Sanitization & hydration, first hydration invariant.
*Fix:* In `hoist_fragment_content` (or a pass just before it), remove every
`<l-n>` descendant of each `<xref>` and merge the neighbouring text nodes.
`remove_line_number_tags` (`processing.rs:3593`) already does this; reuse it
on each `<xref>` subtree.

**B2. Math and diagram containers aren't cleaned before hydration.** This
spec item is missing. Placement puts tags inside hand-written multi-line
`<span class="math math-inline">x\ny</span>`, inline `<code>`, `wc-mermaid`, and
`graphviz-graph`. `replace_math_node` (`processing.rs:3405`) and `hydrate_child`
(`processing.rs:3346`) need a single text child. With a tag inside, the math
isn't delimited and the diagram isn't hydrated. Stored target and fragment
content can also differ from untagged translation.
*Fix:* At the top of `hydrate_dom` (`processing.rs:2221`), before
`hoist_fragment_content`, select `pre`, `code`, `wc-mermaid`,
`graphviz-graph`, and `span` elements whose class list has `math` plus
`math-inline` or `math-display`. Run `remove_line_number_tags` on each one.

**B3. Client changes are missing (checklist item 4).** `git diff` shows no
changes under `client/`:
* `client/src/tinymce-config.mts:150-153`: add `l-n` to
  `extended_valid_elements` and `~l-n` to `custom_elements`. Without these,
  TinyMCE may unwrap the tag, and its digits become saved text.
* `client/src/css/CodeChatEditor.css`: add an unlayered `l-n { display: none; }`
  rule. Without it, every line number shows in the Client (for example `1Test`).
* `client/src/CodeMirror-integration.mts:1258-1264`: the `firstChild`
  descent from a doc block's contents reaches the text `3` in
  `<p><l-n>3</l-n>a`. The caret then sits inside the hidden tag, and typed text
  is hidden and later deleted by the dehydration pre-pass. Skip `l-n`
  subtrees (use the next sibling).

Spec deviations / scope
-----------------------

**D1. CodeMirror document height changed without a spec entry.**
`processing.rs:1155-1185` now emits one newline per *source* line
(`doc_block.lines`) instead of per content line, with a special case for empty
contents. Any comment with delimiter-only lines (`/*\n…\n*/`) now takes
more CodeMirror lines, which shifts the `from`/`to` of every later block. That may be
correct, since CodeMirror lines now match IDE lines, but the spec only
redefines `DocBlock.lines` and says nothing about `CodeMirrorDocBlock.to` or
the Client document. It also relies on the comment's claim that "contents are
empty only for a comment ending the file", which nothing asserts.
*Fix:* Document this in the spec (Source line offsets) and add a test that pins
`from`/`to` and `doc` for a JS and a C file with `/*` on its own line. Or
keep the old height and use `lines` only for the line map.
`code_doc_block_lines` (`processing.rs:2756`) also feeds the new heights into
the gutter numbers in `render_fragment_content` (`processing.rs:2663`). This is
probably a fix, but no test covers it.

**D2. Missing implementation-gather coverage (AGENTS.md).** The lexer changes
(`server/src/lexer.rs:705-780`, `1416-1464`; `server/src/lexer/pest_parser.rs:69-226`)
and `compare_html` (`server/src/translation.rs:1799-1804`) implement spec
sections but have no `<fragment>`, so the `Implementation of …` headings
(spec lines 267 and 325) omit them. "Sanitization & hydration" has no
implementation heading at all.

Missing tests (spec "Testing")
------------------------------

* **Hydration cleanup:** No test covers `A <span class="math math-inline">x\ny</span>.`,
  `math-display`, or hand-written Mermaid/GraphViz `<pre><code class="language-…">`
  and custom-element HTML (B2).
* **Exceptions:** No test covers a multi-line `<xref>` whose `data-content` holds no tag (B1),
  or that Mermaid/GraphViz fenced code still hydrates in *tagged* translation.
  Raw-text openers are tested only from inline HTML
  (`processing/tests.rs:3845-3851`). Add HTML-block openers, such as
  `<script>` at line start ended early by a container boundary.
* **Diff amplification:** No test calls `diff_code_mirror_doc_blocks` on a
  source file before and after inserting a code line. The test should check
  that the `contents` diffs appear only for later doc blocks.
* **Hand-written tag warning:** `test_line_number_tags_hand_written`
  (`processing/tests.rs:3885`) checks the escaping but never asserts that
  `log::warn!` fires for inline or block HTML, or that it doesn't fire for a code span.
* **Client:** The `CodeChatEditor-test.mts` case is missing. It should load
  `<p><l-n>3</l-n>a</p>`, check `display: none`, enter the block from the line
  above, type, and save (B3).

Minor
-----

* `processing.rs:1904-1905` compiles a new `Regex` for every raw-text opener.
  Use a static per-name table or a case-insensitive ASCII byte search.
* Signatures added without per-parameter or return comments, contrary to
  AGENTS.md "Where to place comments": `LineNumberTagger::source_line`
  (`processing.rs:1792`), `reset_at_segments` (`1801`), `reset_suppression`
  (`1812`), and `is_line_number_trigger` (`1946`).
* `review.md` and `submission.md` are committed at the repo root. Remove them
  before merging, or move them somewhere the project expects.

Verified as matching the spec
-----------------------------

Tag format and monotonicity; the trigger and exclusion lists
(`processing.rs:1946`); opaque depth, with the check made before the state
change; the raw-text scanner (ASCII regexes, inline openers matched only at
offset 0, `-->`/`--!>`, `<!-->`, and `plaintext`); reset at segment starts
after the separator's `Html` events, with the warning reporting the opener
line; escaping of hand-written tags even when the map is `None`; `LineMap` and
`LineStarts` (LF/CRLF/lone CR); `build_segment_line_map` measuring from the
actual text; `is_toc` → `None`; the Ammonia allowlists; `strip_line_number_tags`
and its call order in `compare_html`; the dehydration pre-pass on both
`codechat_for_web_to_source` paths and in `dehydrate_html`, including
foreign content, `template_contents`, the cursor marker, and text merging;
`contents_line_offsets` in both lexers, with thorough metadata tests
(`server/src/lexer/tests.rs:56`, `709`, `787`).
