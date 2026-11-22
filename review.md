Review of "Tag format & lookup" (spec.md, lines 39–75)
=====================================================

Scope
-----

This review covers only the `<h2 id="cc-T4yjQxfetF">Tag format &amp;
lookup</h2>` section (spec.md:39–75). Other sections were read only for
context, or are cited where they conflict with this one. Claims about existing
code were checked against the `line-number-support-2` branch:
`server/src/processing.rs` (`markdown_to_html`, `parse_doc_block`,
`doc_blocks_to_dom`, `hoist_fragment_content`, `render_fragment_content`),
`server/src/translation.rs`, `server/src/processing/cache.rs`, and
`client/src/tinymce-config.mts`. Nothing was built or run.

Summary
-------

Since round 1, the section has gained a per-container lookup rule, a
Client-resilience bullet, and a hand-written-tag policy. The syntax rules are
still solid. The new material has four real problems:

* An unclosed hand-written `<l-n>` hides the rest of its container and then
  deletes it on save. A closed one is hidden too, so "renders normally" is
  wrong.
* The lookup rule says "the last tag", but the next bullet says Client
  consumers can't assume ordering. In the Client, "last" has no defined
  meaning.
* The per-container lookup doesn't say how a consumer picks the container.
  Applied as written, every earlier doc block resolves a line that belongs to
  a later block.
* The "Semantics" bullet is false for the mid-line tags that the round 2
  Placement fix introduced.

The remaining findings are about how warnings are reported and detected, a
broken cross-reference, and wording.

Findings
--------

~~### 1. Hand-written `<l-n>`: hidden content and data loss (important)~~

spec.md:72–75 says a hand-written `l-n` start tag "renders normally, and is
stripped (along with its content) during dehydration on the next save".

~~* **It doesn't render.** The Client stylesheet rule `l-n { display: none; }` (Sanitization table, spec.md:258) applies to every `l-n`. A hand-written `<l-n>important</l-n>` disappears from the Client view. Then the next save deletes it from the file. The user gets a warning about text they can't see.~~
~~* **An unclosed tag hides everything after it.** `l-n` is an unknown element to html5ever. A later `<p>` start tag closes an open `p` but not an open  `l-n`. Given `Intro <l-n> oops` followed by more paragraphs, every later  paragraph (and every generated tag in them) nests inside the hand-written  element. `parse_doc_block` accepts this: the sentinel ends up inside the  open element, so `contains_sentinel` is true and the HTML is reparsed  without error (`processing.rs:1625–1640`). The rest of the doc block or  document is invisible in the Client, and dehydration deletes all of it on  the next save.~~
~~* **It breaks this section's own guarantees.** The bullets at spec.md:41–57 apply to "`markdown_to_html` output". A hand-written `<l-n>999</l-n>` (or `<l-n class=x>`, or `<l-n>abc</l-n>`) appears in that output and violates Format and Monotonicity.~~

**Suggest:** Don't let hand-written tags reach the DOM. The adapter already
scans every `Html` and `InlineHtml` event for them, so it can escape the
`<l-n` (emit `&lt;l-n`) or drop the tag in the same pass. The user then sees
the literal text, nothing is hidden, and a save writes it back unchanged.
Keep the warning. If the current policy stays, at least (a) make an unclosed
hand-written `l-n` a translation error, as `parse_doc_block` already does for
other text-hiding constructs, (b) replace "renders normally" with "is hidden
in the Client and deleted with its content on the next save", and (c) limit
the Format and Monotonicity bullets to *generated* tags.

~~### 2. "Last tag" is undefined once ordering isn't guaranteed (important)~~

The lookup rule (spec.md:58–61) resolves line `N` to "the last tag whose value
is $\le N$". That relies on the Monotonicity bullet: in Server output, the
last such tag in document order is also the one with the largest value. The
next bullet (spec.md:62–66) says Client consumers "must not assume ordering or
uniqueness". The scroll-sync consumer (spec.md:299) runs in the Client, so the
only consumer that uses the lookup rule is the one for which the rule's
premise doesn't hold. For example, take the tags `2, 9, 5` after a paste, with
`N = 6`. "Last in document order with value $\le 6$" gives `5`. "Largest value
$\le 6$" also gives `5`. With `N = 9`, the first reading gives `5` and the
second gives `9`. A duplicate `5, 5` leaves it unclear which element to scroll to.

**Suggest:** Define the lookup on values, not positions: "the valid tag with
the greatest value $\le N$; if several share that value, the first in document
order". State that in Server output this is the same as the last such tag.

~~### 3. The lookup doesn't say how to pick the container (important)~~

"Within a single container (Markdown body or one doc block), line `N` resolves
to the last tag whose value is $\le N$" (spec.md:58–61). In a source file,
tag values are global source lines (spec.md:214), so every doc block before
line `N` has a tag $\le N$. A consumer that searches block 0 for a line in
block 3, or for a line in a code block, gets a match. Only a consumer that
already knows which container holds `N` gets the right answer. The parenthesis
"CodeMirror locates code block lines" hints at this, but doesn't say how.

**Suggest:** Make the first step explicit: "First find the block containing
`N` from the block line ranges ($1 + \sum_{j<i} h_j$ up to
$\sum_{j \le i} h_j$, see <xref ref="cc-dl60mkSR4A"></xref>, or the doc block
positions in CodeMirror). Apply the tag lookup only if that block is a doc
block, and only to that block's tags." For a Markdown document, the body is
the only container.

~~### 4. "Semantics" is false for mid-line tags (moderate)~~

Round 2 (Placement finding 2) kept tags in the middle of a line after a
multi-line inline event, and the Omitted-tags parenthesis (spec.md:53–55)
describes them. The Semantics bullet wasn't updated:

* "Marks where line `N`'s inline content starts" (spec.md:48). For
  `` `a\nb` c ``, the output is `<l-n>1</l-n><code>a b</code><l-n>2</l-n> c`.
  Line 2's content starts at `b`, inside the code span, not at tag 2.
* "Content between two tags belongs to the first tag's line or a subsequent
  untagged line" (spec.md:48–49). The `b` lies between tags 1 and 2 and comes
  from line 2. Line 2 is neither the first tag's line nor untagged.

**Suggest:** "Marks the first trigger which starts on line `N`. Content
between two tags belongs to the first tag's line, to a later untagged line,
or (for the continuation of a multi-line inline event) to the second tag's
line." A tag for line `N` then gives a position *on* line `N`, not its start,
which is all scroll sync needs.

~~### 5. The warning has no channel and no detection rule (moderate)~~

spec.md:72–75 requires "a non-blocking warning". It doesn't say where the
warning goes. `markdown_to_html` returns a bare `String`
(`processing.rs:1446`), `source_to_codechat_for_web` returns
`Result<CodeChatForWeb, SourceToCodeChatForWebError>` with no warning field,
and the only existing mechanism is `log::warn!`. A log line is easy to
implement but the user never sees it, which matters because their text is
deleted on save (finding 1). Implementers will choose differently, and the
required test ("produces a warning", spec.md:358) can't be written until the
channel is known.

"Any `l-n` start tag ... in an incoming `InlineHtml` or `Html` event" also
needs a matching rule. An `Html` event is raw text, not tokens. A substring
search for `<l-n` also fires on `<!-- <l-n> -->`, on
`<span title="<l-n>">`, and on `<l-n>` inside a `<script>` or `<textarea>`.
html5ever parses none of these as an element, so dehydration does *not* strip
them, which contradicts "is stripped ... on the next save".

**Suggest:** Name the channel (for example, `log::warn!` with the file path
and source line, plus whatever user-facing route the project prefers). Define
the match, for example the regex `(?i)<l-n[\s/>]`. Say that false positives
inside comments, attribute values, and raw-text elements are accepted, and
that only an actual `l-n` element is stripped on save.

~~### 6. Broken cross-reference `cc-qSbTxGyL2n` (minor)~~

spec.md:63 cites stale edits via `<xref ref="cc-qSbTxGyL2n"></xref>`. No
element in the project has that id. `## Re-translation & diffs`
(spec.md:283) has no id. Hydration will render an error in place of the link.

**Suggest:** Give that heading the id: `<h2 id="cc-qSbTxGyL2n">Re-translation
&amp; diffs</h2>`. This matches the other `<h2 id=...>` headings in the spec.

~~### 7. Client-resilience bullet: wrong sources, undefined "invalid" (minor)~~

spec.md:62–66 lists what can corrupt tags in the Client DOM. Two items don't
belong in the list:

* **Cursor markers** are inserted on the Server, during save
  (`translation.rs:1465–1482`, then `html_to_dom`). They never appear in the
  Client DOM. `client/src` doesn't reference `UNICODE_CURSOR_MARKER`. They
  matter for dehydration, which spec.md:281 already covers.
* **`<xref>` hydration** runs on the Server and only *removes* tags
  (spec.md:262), and stored xref text is cleaned by `CLEAN_LINK_CONTENT_HTML`.
  It can leave lines without a tag, but it can't duplicate or reorder tags.

"Must ignore invalid `N` values" also needs a definition, since every consumer
has to apply the same one.

**Suggest:** Remove the cursor marker from the list. Change the xref item to
"can remove tags". Define validity: "a tag is valid if it has no element
children and its `textContent` matches `^[1-9][0-9]*$`; consumers skip every
other tag."

~~### 8. Smaller wording gaps (nit)~~

* **`N` ≥ 1.** "1-based ... no leading zeros" (spec.md:44–46) still allows
  `0`, which has no leading zero. `cache.rs` uses `0` for an unknown line
  (`cache.rs:2011, 2029`; spec.md:27). Say "`N` ≥ 1" so that a tag can't be
  confused with "unknown".
* **Omitted tags** (spec.md:51–53) lists "opaque content", which means
  opaque depth. It doesn't list lines inside a raw-text suppression region,
  which also get no tag. Add "or content inside a raw-text suppression
  region".
* **Line length.** spec.md:74–75 run past 80 columns, against the wrapping
  rule in `AGENTS.md`.

What's sound
------------

* The Format rule is clear and testable. It matches `render_fragment_content`,
  which counts from 1 and sums `code_doc_block_lines`
  (`processing.rs:2152–2241`).
* `l-n` is a valid custom element name: it starts with a lowercase ASCII
  letter, contains a hyphen, and isn't one of the reserved names. So it can't
  collide with a standard element. The ban on `customElements.define` (added
  after round 1) closes the upgrade risk. No Client code defines `l-n`
  (`tinymce-config.mts:153` registers only `graphviz-graph`, `wc-mermaid`,
  `xref`, and `fragment`).
* Strict monotonicity holds for generated tags in Server output, including
  after hydration. Hydration only removes tags, and `hoist_fragment_content`
  (`processing.rs:1811–1827`) moves a fragment's children to just after the
  fragment without reordering them.
* The new "leading content maps to no line" sentence and the "no position"
  fallback in the lookup rule close round 1 findings 2 and 6 for Markdown
  documents.
* Checking hand-written tags "before tag injection" (spec.md:74) correctly
  keeps generated tags from setting off the warning (round 2 Placement
  finding 6).

Out of scope, noted in passing
------------------------------

The introduction (spec.md:35–37) relies on `source_to_codechat_for_web`
seeing `is_toc`. That parameter is currently `_is_toc` and unused
(`processing.rs:972`), so the implementation checklist should include passing
it to `markdown_to_html`.
