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

TODO: Prompting TODO model, TODO effort, with `Review the section named
"Inline inclusion of fragments" in server/src/processing/cache-spec.md, writing
findings to review.md` produced the following output; strikethrough indicates
LLM output that I deemed incorrect or irrelevant.

TODO: paste the output.
