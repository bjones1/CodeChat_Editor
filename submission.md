Project 2 submission
====================

Team name: Enhanced cross-references 2

Team: Pavel Shegay, Viacheslav Sokolov, Arsen Zhanserkenov, Arsen Sailau.

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
for an `<xref>`, and the error rules for an invalid, duplicate, or nested
`id` and for `ref` errors. TODO (Viacheslav): TinyMCE and dehydration.

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
| 1 | 10 (3 accepted, 4 deferred to later sections, 3 rejected) | Core approach: the `<xref>`'s `id` counts as a definition for `IdState`, a reference to it renders an error, "backlink text" is defined and never includes the target's inner HTML, and a `ref` change outdates both target files. |
| 2 | 5 (2 deferred to later sections, 3 rejected) | No changes; the findings concerned implementation details or sections not yet written. |
| 3 | 5 (2 accepted into the error rules, 3 rejected) | Syntax: allowed `id` on `<xref>`, kept by hydration and dehydration; `id="*"` works on an `<xref>`. The accepted findings became the rules for an empty `id` and for `id` versus `ref` errors. |
| 4 | 5 (2 accepted as one change, 3 rejected) | Error rules: an `<xref>` with an `id` nested in another `<xref>`'s optional content defines no id and produces no backlink. |

#### Last specification review

Prompting Copilot Chat in VS Code (Agent mode) with `Review only the staged
changes to server/src/processing/cache-spec.md: the error rules added at the
end of the "Bidirectional cross-references" subsection (invalid id, duplicate
id, ref errors, both). Check them against the Details list under
Cross-references, the CSS-identifier rule under "Fragments and gather
elements", "Missing and duplicate ids", and the rest of the "Bidirectional
cross-references" subsection for contradictions, undefined terms, and missing
cases. Ignore TinyMCE, dehydration, backlink text and markup, and the
reverse-edge diff: later sections cover those. Do not edit files. List
numbered findings.` produced the following output; strikethrough indicates
LLM output that I deemed incorrect or irrelevant.

> 1. ~~**Invalid `id` can be caught by the generic CSS rule, but the spec does
>    not say it also blocks duplicate detection.** "Invalid ids define no id"
>    is consistent with the CSS rule, but repeated invalid strings should be
>    clarified as only one invalid-id error, not a missing/duplicate entry.~~
>    (Already stated: an invalid `id` defines no id, so it can't be a
>    duplicate.)
> 2. **The HTML-cleaning rule can remove a nested `<xref>` definition.**
>    Cleaning says nested `<xref>` elements are unwrapped and their `id`
>    attributes stripped. It is unclear whether a nested bidirectional `<xref>`
>    counts as a definition or is explicitly excluded; the identity rule does
>    not reserve top-level-only syntax.
> 3. ~~**Duplicate and `ref` errors may need independent error precedence.**
>    The spec says both errors must report both, but duplicate `id` already
>    says its `<xref>` renders a duplicate-id error and no backlink. It does
>    not define a required combined format or whether a
>    missing/fragment/duplicate `ref` replaces or supplements that message.~~
>    (The message format is an implementation detail; "reports both" says it
>    supplements.)
> 4. ~~**The Missing-id section does not cover a missing `id` attribute versus
>    `id=""`.** `<xref>` without `id` is one-way, while `id=""` is
>    bidirectional and invalid. The distinction is stated, but "Missing and
>    duplicate ids" should explicitly define that an absent attribute is the
>    one-way marker and an empty present attribute is invalid rather than
>    missing.~~ (As the finding notes, the distinction is already stated;
>    repeating it would break DRY.)
> 5. **The CSS rule for fragments, targets, and gathers uses a blanket "all
>    ids," creating an unclear scope.** It does not distinguish a
>    target/fragment id, a bidirectional xref anchor, and an id inside cleaned
>    optional content. The bidirectional section should state that only its
>    valid anchor participates in `IdEntry`; cleaned nested ids remain
>    excluded.

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
