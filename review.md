Review: "Inline inclusion of fragments" (round 1)
================================================

Section reviewed: "Inline inclusion of fragments" in
`server/src/processing/cache-spec.md`, checked against the rest of the spec and
the current implementation (`server/src/processing.rs`,
`client/src/tinymce-config.mts`).

Findings
--------

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

Changes made
------------

* Kept the `<include ref>` element in every hydrated form. Inline inclusion
  becomes `<include ref contenteditable="false">…</include>`. Block inclusion
  becomes `<div class="cc-include">` holding an empty `<include>` and then the
  content. Dehydration rules are now defined for both (1).
* Redefined block inclusion: the include is the only content of a top-level
  `<p>`, which is replaced by the `div` (2).
* Required re-hydrating `<xref>`s as a gather list does (3).
* Allowed `<include>` in Markdown documents, with the rationale and the
  HTML-to-Markdown round trip, and removed the open question (4).
* Added the TinyMCE and sanitizer requirements (5).
* Completed the flattening rules: keep the permitted content of `<a>`, omit
  other elements, unwrap links under an `<a>` ancestor, remove includes from
  `<xref>` optional content, and drop empty blocks and trim (6).
* Rewrote the example as a C doc block with `following="0"`, included from a
  Markdown heading (7).
* Added layer 6 to the cache-design list, added `<include>` to the definition
  of `IdEntry::dependents`, linked to the section from the fragments section,
  and gave the heading the id `cc-JzLRfBr83U` (8).
* Stated the phase ordering and the self-include case (9), and made an empty
  `ref` an error (10).
