Project 2 submission
====================

Repeat the following block for each team member.

Team member: Arsen Zhexenbiyev (GitHub: Arsen-kbtu)
---------------------------------------------------

### Contribution (5 points)

Reviewed the "Inline inclusion of fragments" section of
`server/src/processing/cache-spec.md` against the spec and the implementation,
and revised it to fix the findings, including support for `<include>` in
Markdown documents.

### Build and run check (5 points)

Prompt: "Verify that you can build and run the project in the current git repo."

Summary of the LLM's output (Claude Opus 5.5, Windows 10, Rust/cargo 1.100.0,
Node.js 20.11.1):

* `./bt install` installed pnpm and the Client and VSCode extension npm
  packages, and fetched all crates. It then failed at installing cargo-binstall,
  since that step requires `pwsh` (PowerShell 7), which isn't installed.
  `cargo-nextest`, which the build requires, was installed instead with `cargo
  install cargo-nextest --locked`; the other tools (cargo-dist, cargo-audit, and
  so on) aren't needed to build.
* `./bt build` built the Rust workspace (server and standalone binary; one
  `unreachable_code` warning and two unused-dependency warnings), passed the 13
  `export_bindings` tests, and bundled the Client with esbuild. It then failed
  at `node HashReader.js`: Node 20 can't read `package.json5`, so it didn't
  treat the ES module as one. Rerunning with
  `NODE_OPTIONS=--experimental-detect-module` built the Client and passed `tsc
  -noEmit`.
* The VSCode extension build (`npx napi build`) still failed: `@inquirer/core`
  imports `util.styleText`, which Node 20.11 lacks. CI uses Node 26; upgrading
  Node is required to build the extension.
* Running `target/debug/codechat-editor-standalone --port 8080 serve` started
  the server: `/ping` returned `pong`, `/` redirected to the file browser
  `/fw/fsb/`, and opening `README.md` through it returned the Client page (HTTP
  200). The standalone server and Client build and run; the VSCode extension
  does not build with Node 20.

### Review rounds (10 points)

The section reviewed is "Inline inclusion of fragments" in
`server/src/processing/cache-spec.md`.

| Round | Findings | Main changes made |
| ----- | -------- | ----------------- |
| 1     | 10 (4 major, 2 medium, 3 minor, 1 nit); see `review.md`. Major: the hydrated form lost `ref`; block inclusion was unreachable and produced a `div` inside a `p`; `<xref>`s in included content linked from the wrong file; Markdown support was left open. | Kept `<include ref>` in every hydrated form and defined dehydration for each; redefined block inclusion as a top-level `<p>` replaced by a `div`; required `<xref>` re-hydration as in gather lists; allowed `<include>` in Markdown documents; added TinyMCE and sanitizer requirements; completed the flattening rules; fixed the example; added layer 6 and `<include>` to the cache-design section. |

### Last review (10 points)

Prompting Claude Opus 5.5 (Claude Code), high effort, with `Review the spec
and make the changes` (the original prompt was in Russian: "сделай ревью
спеки и внеси изменения") produced the following findings, also saved in
`review.md`, for the section "Inline inclusion of fragments" in
`server/src/processing/cache-spec.md`; strikethrough indicates LLM output
that I deemed incorrect or irrelevant.

1. **Major: the hydrated form loses `ref`.** The example hydrates `<include
   ref="id1"></include>` to a bare `<span class="cc-include">`, and dehydration
   is said to leave `<include ref="id"></include>`. With the `<include>`
   element gone, dehydration has no way to recover the `ref`; the first save
   would erase the include. `<xref>` avoids this by keeping its element in the
   hydrated form.

2. **Major: block inclusion (case 2) can never occur, and its output is invalid
   HTML.** Doc blocks and Markdown documents are Markdown, so an `<include>`
   alone on a line parses as `<p><include ...></include></p>` (see
   `test_hydrate_fragment_in_markdown` for `<fragment>`). The include is never
   "the only content of its doc block"; its host is always a `<p>`. Inserting
   the proposed `<div>` inside that `<p>` is invalid, and the HTML parser would
   split the paragraph.

3. **Major: cross-references in included content link from the wrong file.**
   `Fragment::content` hydrates each `<xref>` relative to the fragment's file.
   A gather list re-hydrates them from `data-content` (layer 5); the include
   section doesn't, so relative links break whenever the including file is in
   a different directory.

4. **Major: Markdown is left as an open question.** The branch is "<fragment>
   for Markdown", yet whether `<include>` works in Markdown documents is left
   open. Markdown documents can't define fragments, so including fragments
   from source files is the main way a Markdown document can use them. The
   spec should allow it and describe the round trip through the
   HTML-to-Markdown conversion.

5. **Medium: the Client and sanitizer requirements are missing.** `include` is
   in neither the TinyMCE `custom_elements` and `extended_valid_elements` lists
   nor the Server's Ammonia allowlist (`AMMONIA_OPTIONS`), so the element would
   be stripped before hydration or dropped by the Client.

6. **Medium: the flattening rules (case 1) are incomplete.**
   * They say nothing about elements that are neither block wrappers nor the
     listed inline formatting (diagrams, `iframe`, images, display math).
   * They don't cover an `<include>` inside a link, which would produce nested
     `<a>` elements.
   * They don't cover an `<include>` inside `<xref>` optional content.
   * Empty blocks aren't dropped. In the spec's own example, the fragment's
     doc block also holds the `<fragment>` tag, whose paragraph is empty after
     cleaning. Without trimming, this adds a stray space.

7. **Minor: the example is ambiguous.** A fragment includes the *following*
   code/doc block by default. The example doesn't say what follows the `<h4>`,
   so if a doc block follows, its text would be included too. Use
   `following="0"` or say that a code block follows.

8. **Minor: the rest of the spec isn't updated.**
   * The cache-design layer list stops at 5, even though this section
     introduces "layer 6".
   * The definition of `IdEntry::dependents` lists only `<xref>` and
     `data-gather` as ways to depend on an id.
   * The fragments section still says only that fragments aren't allowed in
     Markdown.
   * The new `h2` has no `id`, unlike the other `h2`s, so it can't be
     cross-referenced.

9. **Minor: same-file ordering is asserted, not explained.** "Including a
   fragment from the same file needs no special case" holds only if include
   hydration runs after the file's own fragments are committed, as gather-list
   hydration does. The case of a fragment including itself isn't mentioned.

10. **Nit: the error rules don't match the fragment rules.** A `ref` that is
    empty should be treated like a missing one, matching the rule for a
    fragment's `id`.


Team member: Aruzhan Matzhan (GitHub: TODO username)
----------------------------------------------------

### Contribution (5 points)

Wrote the first draft of the "Inline inclusion of fragments" section of
`server/src/processing/cache-spec.md` (commit `6be1387`, branch
`feature-markdown-2`): the `<include ref="id"></include>` syntax for inserting
the text of a fragment into the surrounding content; the rules for choosing the
included content (flattening to inline content when the include shares its host
with other text, and inserting the doc content unchanged when it is the only
content of its doc block); the error cases; the dependency and cache-layer
rules (layer 6, removal of includes from `Fragment::content` and
`Target::inner_html`); and the open questions.

### Build and run check (5 points)

TODO: paste a summary of the LLM's output.

### Review rounds (10 points)

TODO: describe the review rounds.

### Last review (10 points)

TODO: paste the model, effort, prompt, and output of the last review.
