Review of "Placement" (spec.md, lines 88–183)
=============================================

Scope
-----

This review covers only the `<h2 id="cc-gpOIpo2gjZ">Placement</h2>` section
(spec.md:88–183), including its subsections "Trigger rules", "Example", and
"Exceptions". Other sections were read only for context, or are cited where
they conflict with this one. Claims about existing code were checked against
the `line-number-support-2` branch: `server/src/processing.rs`
(`markdown_to_html`, `source_to_codechat_for_web`, the
`DOC_BLOCK_SEPARATOR_*` constants, `doc_blocks_to_dom`, `parse_doc_block`,
`html_to_dom`) and `client/src/CodeMirror-integration.mts`. CommonMark
behavior was checked against the 0.31.2 spec that `processing.rs` cites.
Nothing was built or run.

Summary
-------

All seven round 2 findings are fixed. The trigger rules, the opaque-depth
counter, and the check-then-toggle order for raw-text suppression are sound.
This round found four real problems:

* Placement makes `<l-n>` the first child of nearly every block, and the
  Client's existing caret code walks `firstChild` down to the first text
  node. Entering a doc block from above puts the caret inside the hidden tag,
  so typed text is invisible and deleted on save.
* Raw-text suppression is never reset. One false-positive opener (an
  attribute value, or markup inside a comment) silently drops every later tag
  in the document, or in every later doc block of a source file. The
  parenthesis that is meant to rule this out is inaccurate.
* The scanner's matching rules are undefined. A naive scan gets the empty
  comments `<!-->` and `<!--->` wrong, and those trigger the drop above.
* The section's key example of a closing tag at the start of a line,
  `<textarea>a\n</textarea> b`, is a CommonMark HTML block, so it never goes
  through the rule it is meant to test.

The remaining findings are about the `markdown_to_html` signature and wording.

Findings
--------

~~### 1. Every block's first text node is now hidden, and the Client puts the caret there (important)~~

The trigger rules put the tag *inside* `<p>`, `<h1>`, `<li>`, `<td>`, and so
on, before the first inline content (spec.md:108–113). Doc block HTML
therefore almost always starts `<p><l-n>N</l-n>…`.

`CodeMirror-integration.mts:1238–1270` handles the caret moving into a doc
block from the line above. It descends `contents.firstChild` until it reaches
a text node, then calls `range.setStart(edgeNode, 0)`. With tags present, that
text node is the digits inside `<l-n>`. The comment there explains that
TinyMCE keeps this selection after it initializes. So the user's first
keystrokes go into the `l-n` element: they are hidden by
`l-n { display: none; }`, and the dehydration pre-pass deletes them along
with the tag on the next save. The Sanitization table says the CSS rule
"block[s] caret entry" (spec.md:269). That holds for caret movement by the
browser, but not for a range set by script.

The end-of-block branch (`lastChild`) is safe, because a tag is never the last
child of its line. Any other Client code that picks "the first text node" has
the same problem.

**Suggest:** Add a rule to the spec, and an entry to the implementation
checklist: Client code that chooses a caret position must skip `l-n`
subtrees. In the code above, that means a walk that ignores `l-n` elements
when it looks for the first text node. Extend the Client test (spec.md:393)
so that it enters a tagged doc block from above, types a character, and
checks that the character is visible and survives a save.

~~### 2. Suppression is never reset, so a false positive silently drops all later tags (important)~~

The scanner looks at raw `Html`/`InlineHtml` text. Suppression ends only when
"a later `Html` or `InlineHtml` event contains its closing sequence"
(spec.md:153). Nothing ends it at a doc block separator or at the end of the
input. These cases open suppression and never close it:

* **Attribute values.** `<a title="<select> menu">x</a>` is valid inline
  HTML. html5ever sees an `a` element, but the scanner sees `<select>`.
* **Markup inside comments.** `<!-- TODO: add <template> -->`. Whether the
  scanner ignores openers while a comment is open is unspecified (finding 3).
* **Empty comments.** See finding 3.

The doc block separator happens to contain
`</pre></script></style></textarea>-->` (`processing.rs:300`). So false
positives for `script`, `style`, `textarea`, and comments end at the next
doc block. False positives for `title`, `xmp`, `iframe`, `noembed`,
`noframes`, `noscript`, `plaintext`, `template`, and `select` never end. They
remove every tag from the rest of the Markdown document, or from every later
doc block in a source file. Nothing warns, and scroll sync and cache line
facts are lost without any visible error.

The parenthesis at spec.md:159 is meant to rule this out. It says that
"Unclosed elements at a doc-block or document boundary cause
`parse_doc_block` to reject the translation". That is true only for elements
that are actually open in html5ever's DOM (raw-text elements, `select`, and
`template` all lose the sentinel, so `parse_doc_block` rejects them). It says
nothing about the scanner's false positives. It is also wrong for comments:
`parse_doc_block` accepts a sentinel found inside a comment
(`processing.rs:1635–1637`). Comments are safe only because the separator's
`-->` ends suppression in the Markdown event stream before
`DOC_BLOCK_SEPARATOR_REMOVE_FENCE` deletes that `-->` from the HTML.

**Suggest:** Reset suppression at every doc block separator. This is safe
because, as shown above, a real open target cannot cross a separator without
`parse_doc_block` rejecting the translation. Log a `log::warn!` (with file
path and line) whenever suppression is still active at a separator or at the
end of the input, so that a dropped tail is at least visible. Rewrite the
parenthesis to say why the reset is safe, and mention the comment case.
Optionally, have the scanner skip quoted attribute values, which removes the
most common false positive.

~~### 3. The scanner's matching rules aren't defined (moderate)~~

spec.md:153–157 name the targets but not the matching rules. Implementations
will differ on:

* **Empty comments.** CommonMark 0.31.2 (§6.6) accepts `<!-->` and `<!--->`
  as complete inline HTML comments, and html5ever also closes them right
  away. A scanner that searches for `-->` *after* the `<!--` opener misses
  both closers, because they overlap the opener. Suppression then starts and
  never ends (finding 2).
* **Several transitions in one event.** An `Html` line such as
  `</textarea><!-- x` closes one target and opens another. "Contains its
  closing sequence" doesn't say whether the scan continues past the closer.
* **Openers while suppressed.** Inside a comment or `<textarea>`, a
  `<script>` has no effect in html5ever. The spec doesn't say whether the
  scanner ignores it or treats it as a nested opener.
* **Tag-name boundaries.** html5ever ends a raw-text element at `</name`
  followed by whitespace, `/`, or `>`, case-insensitively. So
  `</TEXTAREA >` and `</textarea\n>` close it. An opener must not match
  `<selection>` or `<titlebar>`. The hand-written-tag rule already gives a
  regex for this (spec.md:81); this rule has none.

**Suggest:** Define the scanner as a two-state machine that runs repeatedly
over each event. When not suppressed, find the earliest opener matching
`(?i)<(script|style|…|select)(?=[\s/>])` or `<!--`. When suppressed, look
*only* for that target's closer: `(?i)</name(?=[\s/>])` for elements, or
`-->` searched from the first byte of `<!--` (so that `<!-->` and `<!--->`
count as closed). After a closer, continue scanning the rest of the event.
Add `<!-->`, `<!--->`, and `</textarea><!--` to the Exceptions tests.

~~### 4. The section's own example of a closing tag at the start of a line isn't inline HTML (moderate)~~

Raw-text suppression starts and ends after the trigger check "so a closing
tag at the start of a line does not get an `<l-n>` tag placed right before
it" (spec.md:153). The Testing section's example of this is
`<textarea>a\n</textarea> b`, where `<l-n>2</l-n>` must appear after
`</textarea>` (spec.md:341–342). However, `<textarea` at the start of a line
begins a CommonMark HTML block of type 1 (§4.6, condition 1), and type 1
blocks can also interrupt a paragraph. The whole input becomes one
`HtmlBlock`, so it is opaque, and ` b` is raw HTML that gets no tag at all.
The required assertion fails for every correct implementation.

The same applies to `script`, `style`, and `pre`. `title`, `iframe`, and
`noframes` begin type 6 blocks, which can also interrupt a paragraph. Only a
target opened *in the middle of* a paragraph line exercises the
closing-event rule.

**Suggest:** Change the example to `x <textarea>a\n</textarea> b`. Its
expected output contains `<l-n>1</l-n>x <textarea>a\n</textarea><l-n>2</l-n>
b`. Add a sentence to "Raw-text suppression" saying that a target opened at
the start of a line is normally an HTML block. For HTML blocks, suppression
matters only when the block ends before its target closes, for example at a
blank line (types 6–7) or at a container boundary.

~~### 5. `markdown_to_html`'s new inputs aren't specified (moderate)~~

`markdown_to_html` currently takes only `markdown: &str` (`processing.rs:1446`)
and has two callers: the Markdown document path (`processing.rs:1024`) and
the source-file path (`processing.rs:1079`). After this change it needs three
new inputs, and the section names none of them:

* the line map, which is the identity-plus-one function for Markdown
  documents but a list of per-block segments for source files
  (spec.md:194–243);
* a way to turn tags off when `is_toc` is true (spec.md:35–37, 181–182). The
  parameter is still the unused `_is_toc` (`processing.rs:972`);
* the file path, which the hand-written-tag warning requires (spec.md:83).

Existing tests call `markdown_to_html(markdown)` directly
(`processing/tests.rs:1709, 1798, 1820, 1845, 1874`), so the signature
affects every one of them.

**Suggest:** Give the signature, for example
`markdown_to_html(markdown: &str, line_map: Option<&LineMap>, file_path: &Path)`
where `None` means "emit no tags" (the TOC case and tests that don't care).
Add the `is_toc` wiring to checklist item 2.

~~### 6. Smaller wording gaps (nit)~~

* **HtmlBlock opacity is redundant.** The table gives the reason "Contains
  only non-trigger `Html` events" (spec.md:148). If so, depth never matters
  inside it. Either say the entry is defensive, or drop it.
* **Untagged lines** (spec.md:170–174) leaves out continuation lines of a
  multi-line link destination or title (`[a](url\n"t")`) and lines inside a
  raw-text suppression region.
* **Sidebar TOC** is stated three times: in the introduction (spec.md:35),
  under Other constructs (spec.md:181), and in the Tag format section. Keep
  only the one in the introduction.
* **Formatting.** spec.md:151, 153–157, and 159 run past 80 columns, against
  `AGENTS.md`. Five blank lines (spec.md:160–164) separate "Raw-text
  suppression" from "Other constructs".

What's sound
------------

* The trigger list is complete for pulldown-cmark 0.13 with `Options::all()`
  minus smart punctuation. It covers every inline leaf event and every inline
  container tag, including `Superscript`/`Subscript` and the wikilinks that
  `Options::all()` turns on, which arrive as `Tag::Link`.
* Excluding `SoftBreak`, `HardBreak`, and `End` is right. Their ranges start
  on the previous line or repeat the `Start` range. Keeping break whitespace
  outside the tag is what makes the round trip byte-identical.
* No trigger can land on a doc block separator line. Every separator line is
  an `Html` event inside an `HtmlBlock`, and the separator's leading blank
  line ends any paragraph, list, or block quote that is still open. So
  `DOC_BLOCK_SEPARATOR_REMOVE_FENCE`'s exact-string replacement and
  `DOC_BLOCK_SEPARATOR_BROKEN_FENCE`'s `.*?` never meet an injected tag.
* `MetadataBlock` must be opaque. pulldown-cmark's HTML writer suppresses
  `Text` inside it but still writes `InlineHtml`, so an injected tag would
  otherwise appear in the output.
* Raw-text suppression only reads `Html`/`InlineHtml`, which is correct.
  pulldown-cmark escapes `>` in `Text`, `Code`, and attributes, so generated
  HTML can never contain a closing sequence that the scanner misses.
* The Example output is what minify-html produces from pulldown-cmark's
  output: it leaves out the optional `</p>` before `<ul>`, and the soft break
  collapses to the space before `<l-n>4</l-n>`.
