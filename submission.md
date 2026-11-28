Project 2 submission
====================

Team members are Armanzhan Onggarbay, Nurlybek Orynbassar, Miras Shaltayev, and
Aidyn Tazhibay. The specification is the "Fragments in Markdown documents"
section of
[server/src/processing/cache-spec.md](server/src/processing/cache-spec.md).

Draft submission
----------------

### Armanzhan Onggarbay

GitHub userid: aronggarbay

#### Contributions

(Armanzhan Onggarbay's contributions)

#### Build and run test

(Summarized LLM output)

#### Specification review rounds

(Table showing spec review rounds)

#### Last specification review

(LLM and prompt used)

(LLM output, possibly with strikethrough)

### Nurlybek Orynbassar

GitHub userid: noxfurry

#### Contributions

Implemented Markdown fragments in the Server -- accepting a `<fragment>` in a
Markdown document, finding the section it encloses, and rendering that content
for gather elements -- and wrote the "Rendering a fragment's content"
subsection of the specification.

#### Build and run test

In a new context, Claude Opus 5.5 (Claude Code) was given the prompt `Verify
that you can build and run the project in the current git repo.` It found that
Rust, Node.js and pnpm were installed, but the client and extension packages
and `cargo-nextest` were missing. Rather than run `./bt install`, which pipes
remote install scripts into the shell, it ran `pnpm install` in `client/` and
`extensions/VSCode/`, installed `cargo-nextest` with `cargo install
cargo-nextest --locked`, and applied the library fix-ups `./bt install` would
have made. `./bt build` then finished with no errors: the Rust workspace, the
binding-export test, the client bundles and type check, and the VS Code
extension all built. It started the standalone server with `cargo run --
serve` in `extensions/standalone`, which served a rendered `README.md` (HTTP
200) on 127.0.0.1:8080, then stopped it. It noted that `docs/design.md` still
gives an outdated start command (`cargo run -- start` in `server/`, now a
library).

#### Specification review rounds

| Round | Findings | Main changes made |
| ----- | -------- | ----------------- |
| 1 | 8 | No factual errors; `contenteditable` kept only on `<span>` and `<xref>`, comments after the last block come along, the cleaning list marked as not exhaustive, cross-reference re-hydration noted, a repetition of "Details" removed |
| 2 | 6 | No factual errors; pointed to the fragment cleaner itself, since "HTML cleaning" omits most of its rules; said which comments come along; restored the `cc-fragment-*` wrappers; described relative URLs as unsupported, matching "HTML cleaning" |
| 3 | 5 | No factual errors; two wording fixes: only line numbers and indents attributed to "Details", and the cleaner's code, not its comment, named as the complete rules |
| 4 | 3 | No changes: the round 3 fixes were confirmed and no new findings appeared; all 3 findings repeated earlier ones |

#### Last specification review

Prompting Claude Opus 5.5 (a Claude Code agent started with no prior context)
with `Review only the section named "Rendering a fragment's content" under
"Fragments in Markdown documents" in server/src/processing/cache-spec.md,
writing findings to review.md.` produced the following output. The agent also
received a note that it was reviewing a student's clone for a course
assignment, and should write only `review.md`. Strikethrough indicates output
that I deemed irrelevant: each struck finding repeats one rejected in earlier
rounds, because it asks for a new test rather than a change to the
specification, or describes an internal cache detail (a comment or a trailing
space stored but harmless when displayed).

> Review: "Rendering a fragment's content" (round 4)
> ==================================================
>
> Scope: only the subsection `#### Rendering a fragment's content` under
> "Fragments in Markdown documents" in `server/src/processing/cache-spec.md`
> (lines 248-286 at commit 669e855, which applied round 3). It was checked
> against `server/src/processing.rs` (`render_markdown_fragment_content`,
> `markdown_fragment_range`, `node_outer_html`, `CLEAN_FRAGMENT_HTML`,
> `AMMONIA_OPTIONS`, `minify`) and `server/src/processing/tests.rs`.
>
> How it was verified: by reading the diff of 669e855 (spec only, 10 lines) and
> the code it describes, and by running the existing `markdown_fragment` tests
> (17 passed, built into a scratch target directory). No code or test changed
> since round 3, so the claims verified in rounds 2 and 3 still hold and were not
> re-probed. No file in the repository was changed apart from this one.
>
> Summary
> -------
>
> Round 3's two wording findings (1 and 2) are fixed, and the new text is
> accurate. The section is now correct and internally consistent. What remains is
> unchanged from round 3: one optional consequence (finding 3), the undocumented
> trailing space (finding 4), and the missing test (finding 5).
>
> Round 3 follow-up
> -----------------
>
> | Round 3 finding | Status |
> | --------------- | ------ |
> | 1. "As noted under Details" covers the wrappers, which Details doesn't mention | Fixed: "none of the `cc-fragment-*` wrappers of a source file's fragment, nor the line numbers and indents noted under 'Details' above". Details says the content "carries no line numbers or indents", so the reference now covers exactly what Details states |
> | 2. "Holds the complete rules" is true of the code, not its comment | Fixed: "whose code defines the complete rules". The comment on `CLEAN_FRAGMENT_HTML` still begins "like `AMMONIA_OPTIONS`" and omits the `<span>`/`<xref>` restriction and the class allowlist, but that is outside this subsection and the spec no longer points the reader at it |
> | 3. Optional: a comment before the closing heading is carried into the fragment | Not addressed (optional) |
> | 4. The space a removed tag leaves in a heading is stored | Not addressed |
> | 5. No test covers this section's claims | Not addressed |
>
> Claims verified (changed text)
> ------------------------------
>
> | Claim | Where in the code | Result |
> | ----- | ----------------- | ------ |
> | `CLEAN_FRAGMENT_HTML`'s code defines the complete cleaning rules | `render_markdown_fragment_content` returns `CLEAN_FRAGMENT_HTML.clean(&html)` with no other filtering of the stored content | Correct for the stored content; see finding 4 for display |
> | It keeps a `contenteditable` of `false` only on `<span>` and `<xref>` | `add_tag_attributes` lists `contenteditable` only for `span` and `xref`; `attribute_filter` drops any value but `false` | Correct |
> | No `cc-fragment-*` wrappers, line numbers or indents | The Markdown path concatenates `node_outer_html` of `top_level[range]`; only the source-file path in `store_fragment_contents` emits those wrappers | Correct |
>
> Findings
> --------
>
> ### ~~1. Optional: a comment introducing the next section is carried into the fragment (consequence, from round 3)~~
>
> ~~Unchanged. Step 2 says comments "following the last one, up to the next block~~
> ~~outside the fragment" come along, so in~~
>
> ```markdown
> ## A <fragment id="f"></fragment>
>
> One.
>
> <!-- Notes for section B. -->
>
> ## B
> ```
>
> ~~the comment meant for section B is stored in `f` (`markdown_fragment_range`~~
> ~~ends the range at the index of `## B`, and `CLEAN_FRAGMENT_HTML` sets~~
> ~~`strip_comments(false)`). It is harmless when rendered. If worth a line, add~~
> ~~a consequence: "A comment just before the heading which ends the fragment~~
> ~~belongs to the fragment, not to the next section."~~
>
> ### ~~2. Still undocumented: the space before a tag in a heading is stored (minor, from rounds 1-3)~~
>
> ~~`## Section <fragment id="frag"></fragment>` stores `<h2>Section </h2>`, which~~
> ~~the heading tests expect; the minifier removes the space before display. The~~
> ~~same is true of the new wording in step 3: the "complete rules" are those of the~~
> ~~stored content, while a gathering document's output additionally passes through~~
> ~~`minify` (`AMMONIA_OPTIONS`, then `minify_html`). One sentence after step 3~~
> ~~would cover both: "Display passes the content through the usual minification,~~
> ~~which, for example, drops the space a removed tag leaves at the end of a~~
> ~~heading."~~
>
> ### ~~3. Still no test covers this section's claims (test coverage, from rounds 1-3)~~
>
> ~~Commit 669e855 changed only the spec. Of this section's claims, only~~
> ~~"comments between enclosed blocks come along" is pinned by a test~~
> ~~(`test_markdown_fragment_following_counts_only_elements`).~~
> ~~`test_markdown_fragment_gathered` and `test_markdown_fragment_gather_same_file`~~
> ~~gather plain text only. Untested remain: comments after the last block and not~~
> ~~before the first; math and diagram hydration in stored content; removal of `id`~~
> ~~and of an id-less `data-gather`; removal of another fragment's tag and~~
> ~~backlinks; the `contenteditable` filter (dropped on `<div>`, `true` dropped on~~
> ~~`<span>`); cross-references re-hydrated in a gathering file in another~~
> ~~directory; a relative image URL copied unchanged.~~
>
> ~~A single `test_markdown_fragment_rendering` test that gathers a section of~~
> ~~`b.md` into `sub/dir/a.md` and asserts both the stored content and the~~
> ~~gathering file's output would pin all of them.~~
>
> Verdict
> -------
>
> The prose of this subsection is now done: it is accurate, its pointers say
> what they point to, and round 3's fixes introduced no new problems. Findings 1
> and 2 are optional one-sentence additions. Finding 3, the missing test, is the
> only substantive gap left and has been carried through four rounds; further
> spec-only rounds won't close it.

### Miras Shaltayev

GitHub userid: miras-kbtu

#### Contributions

(Miras Shaltayev's contributions)

#### Build and run test

(Summarized LLM output)

#### Specification review rounds

(Table showing spec review rounds)

#### Last specification review

(LLM and prompt used)

(LLM output, possibly with strikethrough)

### Aidyn Tazhibay

GitHub userid: aidyntazhibai

#### Contributions

Wrote the "Examples" and "Edge cases and errors" subsections of "Fragments in
Markdown documents", taking every example from the team's tests, and revised
them over four LLM review rounds.

#### Build and run test

I did not use the assignment's prompt in a new session. Instead, in a Claude
Code session I was already using, I asked Claude Opus 5.5 to run the build
check. It found that Node.js and `cargo-nextest` were missing. I installed
Node.js 26.13.1; `./bt install` then installed pnpm and the client and extension
packages, but stopped at its PowerShell 7 step, so `cargo-nextest` was installed
with `cargo install cargo-nextest --locked`. After that, `./bt build` finished
with no errors (13 of 13 binding-export tests passed, and the client and VS Code
extension built and type-checked), and the standalone server started with
`serve` and answered HTTP requests on 127.0.0.1.

#### Specification review rounds

| Round | Findings | Main changes made |
| ----- | -------- | ----------------- |
| 1 | 10 | `following` cannot exclude a gather element in the tag's own block; the tag notation and a list written as one block clarified; where an invalid id's error shows; a heading inside a block quote or list bounds nothing |
| 2 | 7 | Raw HTML headings bound a section; each error bullet says where the error shows (the tag or the gather element); untested claims marked |
| 3 | 13 | No factual errors; dropped the claim that every bullet has a test; added that a tag alone in its paragraph starts with an empty block; a same-document gather element must lie outside the fragment |
| 4 | 11 | No factual errors; the notation covers tags with `following`; the empty-paragraph reason restated; a paragraph reflowed |

#### Last specification review

Prompting Claude Opus 5.5 (Claude Code, medium effort), in a new session, with
`Review the sections named "Examples" and "Edge cases and errors" under
"Fragments in Markdown documents" in server/src/processing/cache-spec.md,
writing findings to review.md.` produced the following output. The session also
received a short note that it was reviewing a student's clone for a course
assignment and should write only `review.md`. Strikethrough indicates output
that I deemed irrelevant: rows or bullets which would need new tests or repeat
rules already stated. Examples finding 1 and edge-case findings 1, 6 and 7 were
applied; edge-case finding 4 is a follow-up for the code's authors.

> Review: "Examples" and "Edge cases and errors" in `cache-spec.md`
> ================================================================
>
> Scope: the two subsections under "Fragments in Markdown documents" in
> `server/src/processing/cache-spec.md` (lines 248-307 at commit `45137ca`). I
> checked each claim against `markdown_fragment_range`,
> `render_markdown_fragment_content`, `store_fragment_contents`, the fragment walk
> in `server/src/processing.rs`, and the `test_markdown_fragment_*` tests in
> `server/src/processing/tests.rs`.
>
> Summary
> -------
>
> Both sections are accurate. Every row in the Examples table matches a test, and
> every edge-case bullet I traced matches the code. I found no factual errors.
> What remains below is one gap in the explanation, a few coverage and
> completeness points, and some wording and formatting nits.
>
> Examples
> --------
>
> ### Verified
>
> All 14 rows match their tests: `_section`, `_mid_section`, `_section_end` (four
> of its cases), `_before_first_heading`, `_nested_block`, `_following` (three
> values), `_following_counts_only_elements`, and `_nested_sections`. So the claim
> "Every row has a matching `test_markdown_fragment_*` test" holds. The tests use
> the id `frag` where the table uses `f`, and `outer`/`inner` where it uses
> `f`/`g`. That difference doesn't matter.
>
> ### Findings
>
> 1. **The notation explains the tag only in one form.** The intro defines
>    `<fragment id="f">` as shorthand for an empty tag. The `following` rows also
>    use `<fragment id="f" following="0">`, which the reader has to read the same
>    way. Suggest: "`<fragment id="f" …>` stands for an empty tag with those
>    attributes, …".
>
> 2. ~~**The tag leaves a trailing space in the content, and the table doesn't
>    show it.** The table lists the enclosed heading as `## A`. The stored
>    content is actually `<h2>A </h2>` (see `_section` and `_section_end`). The
>    space was written before the tag, and cleaning the tag out leaves it behind.
>    The gathered output drops it (`_gathered` shows `<h2>Section</h2>`), so
>    readers never see it. This is fine as a simplification. A short clause would
>    stop someone comparing the table with the tests from thinking it's a bug.~~
>
> 3. ~~**Missing row: the recommended placement combined with `following`.** The
>    section recommends putting the tag in a heading. The Details tell the reader
>    to add `following` to exclude a gather element. No row shows the two
>    together, for example `## A <fragment id="f" following="1">`, `One.`,
>    `Two.` → `## A`, `One.`. This is the combination a reader is most likely to
>    write. The row needs a matching test. The existing `_contains_gather` test
>    already exercises this shape, so it could serve.~~
>
> 4. ~~**Optional row: a level-1 section holds its level-2 sections.**
>    `_section_end` has this case, but the table doesn't. The "higher heading ends
>    a section" row partly covers it, so this one is optional.~~
>
> Edge cases and errors
> ---------------------
>
> ### Verified against the code
>
> * Counting rendered elements: `markdown_fragment_range` counts only nodes for
>   which `get_node_tag_name` returns a tag, so comments and whitespace are
>   carried along without being counted. A tag alone on its line renders as
>   `<p></p>`, which `test_hydrate_fragment_in_markdown` confirms.
> * A tag in code is text: `_in_code`.
> * An invalid id produces an error and no cache entry: `_invalid_id`, and
>   `cached = false` in the walk.
> * `following` parsing: `following.trim().parse::<usize>()`. When parsing fails,
>   the walk sets `FragmentHydration::error`. The backlink patch then shows that
>   error in the tag, and `store_fragment_contents` caches it as the content. So
>   "both the tag and every gather element" is correct. `_following_values`
>   checks only the cached half.
> * A gather element inside the enclosed blocks: `contains_node` searches every
>   descendant, so a nested gather is caught (`_contains_nested_gather`). This
>   error is not stored in `FragmentHydration::error`, so the tag keeps its
>   backlinks, as the bullet says. The start block is always included, even with
>   `following="0"`, so "cannot be excluded" is correct.
> * Only top-level headings bound a section: `heading_level` is applied only to
>   `top_level` entries. A heading nested in a `<div>`, list, or block quote never
>   matches.
> * Text inside the tag is hoisted after it: `_hoisted_content`.
> * A same-file gather element works: `_gather_same_file`.
> * A growing section outdates the gathering file: `_section_grows`.
>
> ### Findings
>
> 1. **Bullet 1's "therefore" doesn't follow.** "A blank line or a link
>    reference definition is no block" doesn't explain why a tag alone on its
>    line becomes an empty paragraph. The real reason is CommonMark's HTML-block
>    rule 7. That rule starts an HTML block only when a line holds a complete open
>    or closing tag followed only by whitespace. `<fragment id="f"></fragment>`
>    doesn't qualify, so it's inline content in a paragraph. Suggest dropping
>    "therefore", or stating that reason.
>
> 2. ~~**An unclosed tag alone on its line isn't covered (unverified).**
>    `<fragment id="f">` alone on a line *does* satisfy rule 7. It becomes a raw
>    HTML block rather than a paragraph, and html5ever will nest everything after
>    it inside the unclosed element until hoisting moves that content back out.
>    The general rules mention this case ("a user first creates a fragment tag,
>    but hasn't closed it"). For Markdown, though, which top-level block the
>    fragment starts in, and therefore what `following` counts, may differ from
>    the closed-tag case. I didn't run this. Either test it and add a bullet, or
>    say the behaviour is unspecified.~~
>
> 3. ~~**The `following` bullet is not quite exhaustive.** `usize::from_str`
>    accepts a leading `+`, so `"+1"` is valid. A value too large for `usize`,
>    such as `"99999999999999999999"`, is a whole number but gets the "must be a
>    whole number" error. Both are minor. One clause such as "a non-negative
>    integer, optionally signed with `+`, that fits in a `usize`" would make the
>    bullet match the code exactly, or the two cases could be left out on purpose.~~
>
> 4. **No test backs several bullets.** The intro admits that some bullets have
>    no test. The ones I found are:
>    * a raw HTML `<h2>` bounding a section;
>    * a heading inside a `<div>`, list, or block quote *not* bounding one;
>    * a gather element in the tag's own block being impossible to exclude;
>    * the tag keeping its backlinks while its content is a gather error;
>    * the tag (not just the cache) showing the `following` error;
>    * a link reference definition not counting as a block.
>
>    The raw-HTML-heading bullet carries the most risk. Whether
>    `<h2 id="b">B</h2>` ends up top-level depends on how pulldown-cmark and
>    html5ever handle raw HTML blocks, which can only be learned by running the
>    code. It deserves a test. This is a follow-up for the code owner, not a
>    change to this text.
>
> 5. ~~**The gather-in-fragment rule is stated twice.** The bullet beginning "A
>    gather element may list a fragment from its own document" repeats the
>    gather-element bullet two items above it ("inside them, it is the error
>    above"). Consider merging it into that bullet, or putting it right after
>    it.~~
>
> 6. **Wrapping.** Line 296 ("…A raw HTML heading such") breaks at column 66,
>    although "as" from the next line fits within 80. The paragraph wasn't
>    reflowed after an edit. AGENTS.md asks for an 80-column wrap.
>
> 7. **Wording nit.** "not each one has its own test" reads awkwardly. Suggest
>    "not every one has its own test".
>
> Not in scope, noted in passing
> ------------------------------
>
> * A comment in `markdown_fragment_range` (`processing.rs`) says "A heading of
>   this level or lower closes the fragment's section". There, "lower" means a
>   lower *number*, which is a higher level. That contradicts the spec's "same or
>   a higher level". This is code, outside this review's scope, but worth a
>   one-word fix.
