Project 2 submission
====================

Team name: Enhanced cross-references 2

Team: Pavel Shegay, Viacheslav Sokolov, Arsen Zhanserkenov. Arsen Sailau did
not take part in this submission.

The specification is the new and changed sections of
[cache-spec.md](server/src/processing/cache-spec.md) which let an `<xref>`
carry an optional `id` that produces a backlink at its target.

Draft submission
----------------

### Viacheslav Sokolov

GitHub userid: SokolovVD-IT

#### Contributions

The core approach (the "Bidirectional cross-references" subsection of
Cross-references), the attribute rule allowing `id` on an `<xref>`, `id="*"`
for an `<xref>`, the error rules for an invalid, duplicate, or nested `id` and
for `ref` errors, what the Server's sanitizer and TinyMCE must allow, and the
dehydration of the backlink markup and of the `<xref>`.

#### Build and run test

Prompting Claude Code (Claude Opus 5.5) in VS Code with `Verify that you can
compile and run the project in the current git repo.` produced:

> Rust 1.99.1 was installed, but Node.js was not, so at first only the server
> could be checked: `cargo build` in `server/` passed, and `cargo test --lib`
> passed 142 tests with 16 failures, all in `ide::vscode::tests`, because
> `server/hashLocations.json`, which the client build produces, was missing.
>
> After installing Node.js LTS 26.13.1 and PowerShell 7.7.0 (which `./bt
> install` uses to fetch `cargo-binstall`):
>
> - `./bt install` -- passed (one retry of `pnpm install` in
>   `extensions/VSCode` after a Windows file lock, os error 32).
> - `./bt build` -- passed.
> - `./bt test` -- passed: 223 tests run, 223 passed, 6 skipped, including
>   the browser-based integration tests in `server/tests/overall.rs`; the
>   builder's own test also passed.
>
> No source files were changed.

#### Specification review rounds

| Round | Findings | Main changes made |
| ----- | -------- | ----------------- |
| 1 | 10 (3 accepted, 4 deferred to later sections, 3 rejected) | Core approach: the `<xref>`'s `id` counts as a definition for `IdState`, a reference to it renders an error, the term "backlink text" is introduced with the constraint that it never includes the target's inner HTML (its full definition is left to the backlink markup section), and a `ref` change outdates both target files. |
| 2 | 5 (2 deferred to later sections, 3 rejected) | No changes; the findings concerned implementation details or sections not yet written. |
| 3 | 5 (2 accepted into the error rules, 3 rejected) | Syntax: allowed `id` on `<xref>`, kept by hydration and dehydration; `id="*"` works on an `<xref>`. The accepted findings became the rules for an empty `id` and for `id` versus `ref` errors. |
| 4 | 5 (2 accepted as one change, 3 rejected) | Error rules: an `<xref>` with an `id` nested in another `<xref>`'s optional content defines no id and produces no backlink. |
| 5 | 3 (1 accepted, 2 rejected) | TinyMCE and dehydration: the Server's sanitizer of hydrated HTML (`minify`) must also allow the `cc-backlinks` class and `contenteditable` on the backlink markup. |
| 6 | 2 (1 deferred to a later section, 1 rejected) | No changes; which elements carry `cc-backlinks` is decided with the backlink markup. |

#### Last specification review

Prompting Copilot Fusion, Standard, in VS Code with `Review round 6: only the
staged changes to server/src/processing/cache-spec.md, the paragraph "Two
sanitizers must preserve both new pieces of markup" at the end of the
"Bidirectional cross-references" subsection. Check it against AMMONIA_OPTIONS
and minify in server/src/processing.rs and against extended_valid_elements in
client/src/tinymce-config.mts. Do not edit files. List numbered findings, or
say that there are none.` produced the following output; strikethrough
indicates LLM output that I deemed incorrect or irrelevant.

> 1. ~~**The TinyMCE rule omits `data-content`.** The existing `<xref>` rule
>    requires `data-content` to survive editing, but `extended_valid_elements`
>    must allow `data-content` in addition to `id`, `contenteditable`, and
>    `ref`. As written, the paragraph only adds `id` and could leave optional
>    content stripped.~~ (TinyMCE already keeps `data-content`: the browser
>    test `test_xref_optional_content_round_trip` passes with the current
>    schema.)
> 2. **The Server allowlist does not yet preserve `cc-backlinks`.**
>    `AMMONIA_OPTIONS` has no allowed `cc-backlinks` class, and `minify` uses
>    that configuration. The paragraph correctly says this class must be added,
>    but it does not specify on which elements; this leaves the required
>    implementation incomplete until the markup's element/class pairing is
>    defined. (Deferred: the elements are chosen in the backlink markup
>    section.)

### Pavel Shegay

GitHub userid: pashegay

#### Contributions

TODO (Pavel).

#### Build and run test

TODO (Pavel).

#### Specification review rounds

TODO (Pavel).

#### Last specification review

TODO (Pavel).

### Arsen Zhanserkenov

GitHub userid: Zhanserkenov

#### Contributions

TODO (Arsen Zh.).

#### Build and run test

TODO (Arsen Zh.).

#### Specification review rounds

TODO (Arsen Zh.).

#### Last specification review

TODO (Arsen Zh.).
