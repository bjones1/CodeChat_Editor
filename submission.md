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

Rewrote the old test, which expected an error for a fragment in a Markdown
document, to check the new behavior; added 15 edge-case tests for Markdown
fragments; and wrote the "Testing" subsection of the specification.

#### Build and run test

In a new context, Claude Opus 5.5 (Claude Code) was given the prompt `Verify
that you can build and run the project in the current git repo.` It found that
Rust, `cargo-nextest`, Node.js and the client and extension packages were
installed, but that `pnpm` failed with a corepack signing-key error and that
`chromedriver` was missing. It did not run `./bt install`, which pipes remote
install scripts into the shell. `./bt build` then finished with no errors: the
Rust workspace, the 13 binding-export tests, the client bundles and type check,
and the VS Code extension all built. All 197 non-browser tests passed; it did
not run the browser tests, since `chromedriver` is missing. It started the
standalone server with `codechat-editor-standalone --port 8123 serve`, which
answered `/ping` and served a rendered `README.md` (HTTP 200) on
127.0.0.1:8123, then stopped it with `/stop`. It noted that the build leaves an
untracked `extensions/VSCode/src/codechat-editor-client.darwin-arm64.node`,
since `.gitignore` lists only the Windows version of this file.

#### Specification review rounds

| Round | Findings | Main changes made |
| ----- | -------- | ----------------- |
| 1 | 11 | Found that `test_markdown_fragment_gathered` translated a document twice, with a wrong comment, and fixed the test; removed a sentence about history and one which repeated the Examples intro; listed the third kind of result the tests check; gave the real reason for no browser test |
| 2 | 10 | No factual errors; four wording fixes: the short-name rule placed before the first short name, the two tests with several documents named, "complete strings" corrected, and the reason the gather list drops a space stated |

#### Last specification review

Prompting Claude Opus 5.5 (a Claude Code agent started with no prior context)
with `Review only the section named "Testing" under "Fragments in Markdown
documents" in server/src/processing/cache-spec.md, writing findings to
review.md.` produced the following output. The agent also received a note that
it was reviewing a student's clone for a course assignment, and should write
only `review.md`. Strikethrough indicates output that I deemed irrelevant.
Findings 1 and 2 ask for tests of, or a test map for, the "Rendering a
fragment's content" and "Examples" subsections, which other team members own;
findings 3, 4, 9 and 10 repeat round 1 findings that I rejected, since they ask
for new tests rather than a change to the specification. Findings 5-8 were
applied.

> Review of "Testing" (cache-spec.md, "Fragments in Markdown documents")
> =====================================================================
>
> Scope: server/src/processing/cache-spec.md, lines 350-398 (the
> `<h4 id="cc-MvsDHHHWVW">Testing</h4>` section). I compared the section with
> the tests in server/src/processing/tests.rs (lines 3243-3840) and with the
> other subsections of "Fragments in Markdown documents".
>
> Checks that passed:
>
> * `cd server && cargo test --lib -- markdown` runs 24 tests. All 24 pass,
>   which includes the edited `test_markdown_fragment_gathered`.
> * Each test name in the table exists in tests.rs.
> * The table has one row for each of the nine bullets in "Edge cases and
>   errors".
> * The id `cc-MvsDHHHWVW` is unique in the project.
> * The paragraph about the shared cache agrees with the order of calls in
>   `_gathered` and `_section_grows`.
>
> Findings
> --------
>
> 1. ~~(Major) The section does not cover "Rendering a fragment's content". The~~
>    ~~table lists tests for the edge cases only, and the last sentence asks for~~
>    ~~test changes only when "the examples or the edge cases" change. Several~~
>    ~~rendering rules have no test:~~
>    * ~~removal of `id` and `data-gather` from an enclosed block (for example, a~~
>      ~~heading with an `id` inside the fragment);~~
>    * ~~re-hydration of a cross-reference inside the fragment;~~
>    * ~~a `contenteditable` of `false` that stays only on `<span>` and `<xref>`;~~
>    * ~~a comment after the last enclosed block that comes along, and a comment~~
>      ~~before the first enclosed block that does not;~~
>    * ~~a relative URL that is copied unchanged.~~
>    ~~Only `_nested_sections` (removal of another fragment's tag) and `_gathered`~~
>    ~~(a heading keeps its level) touch these rules. Add a row or a table for~~
>    ~~the rendering rules. If a rule stays without a test, say so in the section.~~
>
> 2. ~~(Major) The section does not name the tests for the examples. The~~
>    ~~"Examples" subsection says that every row has a matching test, but no~~
>    ~~text names it. Nine tests appear nowhere in the Testing section's table:~~
>    ~~`_section`, `_mid_section`, `_before_first_heading`, `_following`,~~
>    ~~`_section_end`, `_nested_block`, `_nested_sections`,~~
>    ~~`_following_counts_only_elements`, and `_gathered`. Thus "these tests" in~~
>    ~~the last sentence is not clear. Add a table that maps each example row to~~
>    ~~its test, or add a "Tests" column to the examples table.~~
>
> 3. ~~(Medium) The row "Blocks are counted in the rendered HTML" claims too much~~
>    ~~for `test_hydrate_fragment_in_markdown`. That test translates a document~~
>    ~~with only one block (tests.rs:3278). Thus it cannot show that the fragment~~
>    ~~begins with the empty paragraph, or that `following="0"` encloses nothing~~
>    ~~visible. Also, the comment on the test (tests.rs:3272-3274) gives a~~
>    ~~different purpose. Add a test with a tag alone in its paragraph,~~
>    ~~`following="0"`, and a paragraph after it. Change the test comment to~~
>    ~~agree with the spec.~~
>
> 4. ~~(Medium) The reason for "There is no browser test" is incomplete. The~~
>    ~~Client treats the `<fragment>` tag the same in both modes~~
>    ~~(client/src/tinymce-config.mts:151 and 153). But a Markdown document opens~~
>    ~~in doc-only mode, in TinyMCE without CodeMirror. Its gathered content also~~
>    ~~has no `cc-fragment-*` wrappers. The browser suite "Gathered fragment~~
>    ~~layout" (client/src/CodeChatEditor-test.mts:169) checks only a fragment~~
>    ~~from a source file. Thus no test checks how the Client shows a gathered~~
>    ~~Markdown fragment. Write this as a known gap, or change the reason to~~
>    ~~this: the Server makes all fragment HTML, and the unit tests compare it.~~
>
> 5. (Minor) The text uses the short names `_gathered` and `_section_grows` at
>    line 376. The rule that explains the short names comes later, at line 382.
>    Move the sentence "Each name below omits the `test_markdown_fragment`
>    prefix..." before the first short name.
>
> 6. (Minor) "such as `_gathered` and `_section_grows`" tells the reader that
>    other tests use several documents. These two tests are the only ones.
>    Write "The tests `_gathered` and `_section_grows` ...".
>
> 7. (Minor) "Tests compare complete strings, not substrings" is not true for
>    all tests. `_in_code` (tests.rs:3544) and `_invalid_id`
>    (tests.rs:3638-3642) make sure that the fragment map is empty. Write
>    "Tests compare complete strings, or make sure that no fragment is stored,
>    ...".
>
> 8. (Minor) In "a heading which held the tag keeps the space before it", the
>    word "it" can refer to the heading or to the tag. The space is the one
>    that came before the tag, which cleaning removes. Also, the text does not
>    say why the gather list differs: the translated document goes through
>    `minify` (processing.rs:1372), but the stored content does not. Write: "a
>    heading which held the tag keeps the space that preceded the tag, as in
>    `<h2>A </h2>`; the translated document is minified, so its gather list
>    shows `<h2>A</h2>`."
>
> 9. ~~(Minor) The row "`following` values" hides a gap. `_following_error_in_tag`~~
>    ~~checks the tag error only for `"x"` (tests.rs:3799). The values `"-1"`,~~
>    ~~`"1.5"` and `""` are checked only in the stored content. The edge case~~
>    ~~also says that every gather element shows the error, but no test renders a~~
>    ~~gather list for this case. Add the other values to the tag test, or say~~
>    ~~that the stored content stands for the gather element.~~
>
> 10. ~~(Minor) The row "A tag in code is text" names `_in_code`, but that test~~
>     ~~only makes sure that the cache stores no fragment (tests.rs:3544). It~~
>     ~~does not make sure that the tag shows as text in the translated document.~~
>     ~~Add a check of the translated document, or change the row to "no fragment~~
>     ~~is stored".~~
>
> Total: 10 findings (2 major, 2 medium, 6 minor).

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
