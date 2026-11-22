Review of "Placement" (spec.md, lines 76–170)
=============================================

Scope
-----

This review covers only "Placement" (spec.md:76–170), including its "Trigger
rules", "Example", and "Exceptions" subsections. Other sections were read only
for context. Claims about existing code were checked against the
`line-number-support-2` branch, mainly `server/src/processing.rs`
(`markdown_to_html`, the doc block separator, `doc_blocks_to_dom`,
`parse_doc_block`), and against the locked pulldown-cmark 0.13.6 source
(`lib.rs`, `html.rs`, `scanners.rs`, `firstpass.rs`). Nothing was built or
run.

Summary
-------

The trigger model is sound. The trigger and non-trigger lists together cover
every `Event` and `Tag` variant in pulldown-cmark 0.13.6, so no event is left
unclassified. The worked example matches the rules. There are four real
problems:

* Raw-text suppression is only triggered by `InlineHtml`. An HTML *block* can  leave the same elements open past its end.
* The trigger rules tag continuation lines of multi-line inline events, while three other places in the spec say those lines get no tag.
* The spec doesn't say whether suppression ends before or after the closing event's own trigger check.
* The `plaintext` rule is based on an inaccurate description of `parse_doc_block`.

The remaining findings are small gaps in the wording.

Findings
--------

~~### 1. Raw-text elements opened by an HTML block aren't suppressed (important)~~

~~spec.md:139–142 starts raw-text suppression only "when an `InlineHtml` event opens one of the tags below". The spec assumes that an HTML block is covered by opaque depth. It isn't, because opaque depth ends when the `HtmlBlock` ends, and html5ever doesn't stop there. CommonMark ends many HTML blocks at a blank line or at the end of their container, even if an element they opened is still open:~~

* `iframe`, `noframes`, and `title` start type-6 blocks, which end at a blank line (pulldown-cmark `scanners.rs:70, 78, 93`).
* `xmp`, `noembed`, `noscript`, `plaintext`, `template`, and `select` alone on a line start type-7 blocks, which also end at a blank line.
* Type-1 blocks (`script`, `style`, `textarea`, `pre`; `firstpass.rs:2182`) and type-2 comment blocks end at their container's end, for example inside a list item or block quote.

For example:

```markdown
<iframe>

Some text

</iframe>
```

This renders as `<iframe>\n<p><l-n>3</l-n>Some text</p>\n<p></iframe></p>`.`parse_doc_block` accepts it because the element is closed before the sentinel. html5ever then keeps `<l-n>3</l-n>` as literal iframe text. This is the same corruption spec.md:143–147 describes for the `InlineHtml` case: dehydration can't strip the tag, so the next save writes `<l-n>3</l-n>` into the file. An unclosed `<!--` in a block quote is similar: the later paragraphs, with their tags, end up as comment text.

**Suggest:** Look at `Html` events as well as `InlineHtml` events. At
`End(HtmlBlock)`, if the block's lines opened a listed element (or a comment)
without closing it, start suppression. Add the `<iframe>` example above, and a
type-1 block closed by its container, to the required Exceptions tests.

~~### 2. Continuation lines are tagged, but the spec says they aren't (important)~~

~~The trigger rules (spec.md:90–94) insert a tag before the first trigger whose offset is on a new line. If a multi-line `Code`,`InlineMath`, `InlineHtml`, or `Image` is followed on its last line by more content, that content is a trigger on a new line. For example, `` `a\nb` c `` produces a `Text(" c")` on line 2, so line 2 gets a tag in the middle of the line: `<code>a b</code><l-n>2</l-n> c`. The same happens after `<span\nclass=x>` and after an image whose alt text spans lines.~~

Three passages say the opposite:

* spec.md:51–53: lines "continuing an inline event from an earlier line ... get no tag".
* spec.md:158–161: "continuation lines of multi-line code spans/inline math/inline HTML" are untagged.
* spec.md:317: the Placement test expects "code span and inline math spanning lines (second line untagged)".

An implementer can follow the algorithm or the description, but not both, and
the required test result depends on which one they pick.

**Suggest:** Keep the algorithm, since a tag in the middle of a line still
satisfies the lookup rule. Reword the other three passages: a continuation
line gets no tag *at its start*. It is tagged at its first trigger after the
multi-line event, if there is one. Split the test into two cases: a span that
ends its line (second line untagged), and a span followed by text (tag before
that text).

~~### 3. Order of the closing event's check is unspecified (moderate)~~

For opaque depth, spec.md:128–130 says exactly when the counter changes:
"*after* the trigger check". Raw-text suppression (spec.md:139–142) has no
such rule. The closing event is often a trigger itself, for example an
`InlineHtml("</textarea>")` at the start of a line. If suppression ends
*before* that event's trigger check, the tag goes in front of `</textarea>`.
That is still inside the raw text, which is exactly what suppression is meant
to prevent. The opening event has the same question in reverse. A tag before
`<textarea>` is harmless, but only if suppression starts after the check.

**Suggest:** State that suppression starts and ends *after* the trigger check
of the event that opens or closes it, just as opaque depth does. Add a test
where the closing tag starts a line: `<textarea>a\n</textarea> b` must not
produce `<l-n>2</l-n>` inside the textarea.

~~### 4. `plaintext` rule misdescribes `parse_doc_block` (minor)~~

spec.md:145–147 says `plaintext` suppresses "until EOF (`parse_doc_block`
already warns on hidden text)". `parse_doc_block` doesn't warn. It returns an
error (`processing.rs:1639`), which `source_to_codechat_for_web` turns into
`SourceToCodeChatForWebError::ParseFailed`, and the file fails to translate
(`processing/tests.rs:2249`). The same error covers every element that is
still open at the end of a doc block or document: raw-text elements,
`template` (the sentinel is hidden in its contents), and `select` (the
sentinel start tag is dropped). In source files, each doc block is parsed on
its own (`doc_blocks_to_dom`, `processing.rs:1597–1599`). So suppression only
matters for an element that is closed later in the same doc block. "Until
EOF" never produces any output. spec.md:72 uses the same "warning" wording
for the hand-written-tag warning.

**Suggest:** Replace the parenthetical with: "An element still open at the end
of a doc block or document makes `parse_doc_block` reject the translation, so
suppression only needs to handle elements closed later in the same container."
Fix spec.md:72 to match. Either call the hidden-text check an error, or don't
use it as the model for a "non-blocking warning".

~~### 5. Which byte offset is mapped isn't stated (minor)~~
spec.md:93 says "the byte offset maps to a valid source line". It doesn't say
which offset. The reasoning for `SoftBreak` and `End` (spec.md:102–106)
implies the start of the event's range, but an implementer who maps the
event's whole range could choose the end. For a multi-line `InlineHtml` or
`Code` trigger, the start and the end are on different lines.

**Suggest:** "...the event's `range.start` maps to a valid source line `N`".

~~### 6. Warning scanner and injected tags (minor)~~

The adapter injects `Event::InlineHtml("<l-n>N</l-n>")` (spec.md:78–80).
spec.md:70–74 warns on "any `l-n` start tag ... in an `InlineHtml` or `Html`
event". If the warning check sits downstream of the adapter, or checks the
rendered HTML, every generated tag triggers a warning.

**Suggest:** Say that the hand-written-tag check examines parser events
*before* injection (or skips injected events), and that it uses the same
pass that tracks raw-text suppression.

~~### 7. Smaller wording gaps (nit)~~

* The untagged-lines list (spec.md:158–161) leaves out continuation lines of
  multi-line display math (`$$\nx\n$$` within a paragraph). These follow the
  same rule as inline math.
* `HtmlBlock`'s reason (spec.md:136) says tags "could corrupt" its contents.
  However, an HTML block contains only `Html` events, which are never
  triggers, so opaque depth adds nothing there. Listing it does no harm. The
  real risk is the one in finding 1, after the block ends.

What's sound
------------

* The trigger list (spec.md:84–88) and the excluded list (spec.md:96–107)
  together cover every `Event` variant and every `Tag` variant in
  pulldown-cmark 0.13.6 (`lib.rs:156–266, 524–600`). This includes
  `Superscript`, `Subscript`, `DefinitionList*`, and the GFM `BlockQuote`
  kind, all enabled by `Options::all()` in `markdown_to_html`
  (`processing.rs:1446–1455`).
* Making `Image` opaque is correct. `HtmlWriter::raw_text`
  (`html.rs:480–520`) writes the image's inner `InlineHtml`, `Text`, and
  `Code` events, escaped, into `alt`, so an injected tag there would become
  literal alt text.
* The `SoftBreak` and `HardBreak` rule matches the writer. `SoftBreak` writes
  `\n` and `HardBreak` writes `<br />\n` (`html.rs:125–130`), so a tag
  injected before the next line's first trigger follows that whitespace, as
  spec.md:102–104 says.
* The `MetadataBlock` row is accurate. The writer only suppresses `Text`
  inside it (`in_non_writing_block`, `html.rs:102, 386`), and opaque depth
  covers the rest.
* The example (spec.md:111–124) follows the rules: line 3's tag goes before
  `Some `, the `Emphasis` start on the same line gets none, line 4's tag
  follows the soft break, and line 6's tag goes before `TaskListMarker`
  inside `<li>`.
* Strict monotonicity holds. `push_html` writes footnote definitions and all
  other events in source order, and the separator lines in source files map
  to no line.
* The doc block separator's terminator line
  (`</pre></script></style></textarea>-->?>]]>`, `processing.rs:305`) is an
  `Html` event. So a `script`, `style`, or `textarea` opened by inline HTML
  ends its suppression at the separator, as spec.md:140–142 allows.
