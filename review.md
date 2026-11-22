Review of "Tag format & lookup" and "Placement" (spec.md, lines 39–194)
=====================================================================

Scope
-----

This review covers `<h2 id="cc-T4yjQxfetF">Tag format &amp; lookup</h2>`
(spec.md:39–86) and `<h2 id="cc-gpOIpo2gjZ">Placement</h2>` (spec.md:88–194),
read together. Other sections were read only for context. Claims about the
code were checked against the `line-number-support-2` branch:
`server/src/processing.rs` (`markdown_to_html`, `source_to_codechat_for_web`,
the `DOC_BLOCK_SEPARATOR_*` constants, `doc_blocks_to_dom`,
`parse_doc_block`, `render_fragment_content`), `server/Cargo.toml`, and
`Cargo.lock`, which pins pulldown-cmark 0.13.6 (its `html.rs` writer was
read). Nothing was built or run.

Summary
-------

All six findings from the previous round are fixed. Four problems remain:

* The regexes in both sections use lookahead, which the project's only regex
  crate (`regex = "1"`) rejects. They also use Unicode `\s` and Unicode case
  folding, so the scanner can decide a raw-text element has closed when
  html5ever hasn't. Tags then land inside RCDATA, where dehydration can't
  strip them, so they get saved.
* An opener inside an inline HTML attribute value still turns suppression on.
  In a Markdown document, nothing resets it before the end of the input. In
  one ordering it also gets the scanner out of step with html5ever and
  corrupts saved text.
* The suppression reset at doc block separators doesn't say where it happens,
  and the required warning has no source line to report.
* `LineMap` is part of the new signature but is defined nowhere.

Findings
--------

~~### 1. The regexes can't compile, and their Unicode matching disagrees with html5ever (moderate)~~

The hand-written-tag regex `(?i)<(/?l-n(?=[\s/>]))` (spec.md:81) and the
scanner's opener and closer patterns (spec.md:165, 168) all use `(?=…)`.
`server/Cargo.toml:84` depends only on `regex = "1"`. That crate has no
lookaround, so `Regex::new` fails, and the `LazyLock::new(|| …unwrap())`
pattern used for every other regex in `processing.rs` would panic the first
time it runs. `Cargo.lock` has no `fancy-regex`.

Getting rid of the lookahead isn't enough by itself. With the `regex` crate's
default Unicode mode, the patterns match things html5ever doesn't:

* `\s` matches U+000B and U+00A0, among others. html5ever ends a tag name
  only at tab, LF, FF, space, `/`, or `>`.
* `(?i)` uses simple case folding, so `s` also matches U+017F (`ſ`). html5ever
  lowercases only ASCII.

If this happens on an opener, the only effect is missing tags. If it happens
on a closer, the result is corruption. Take `x <textarea>a` followed on the
next line by `</textarea\u{A0}>b c`. The scanner sees a closer and ends
suppression. html5ever is still in RCDATA, so the tag placed before `b` is
textarea text. The dehydration pre-pass can't remove it, and the literal
`<l-n>2</l-n>` is saved.

**Suggest:** Give the patterns in a form the `regex` crate accepts, with ASCII
semantics. For example, `(?i-u)<(/?l-n)([\t\n\x0C\r />])` replaced with
`&lt;$1$2`. Consuming the boundary byte is harmless here: the scanner only
needs match positions, and the escape writes that byte back. Use the same
`(?i-u)` and `[\t\n\x0C\r />]` in the opener and closer patterns. (CR can't
appear after the lexer's normalization, but Markdown documents aren't
normalized, so keep it.)

~~### 2. An opener in an inline attribute value still disables tags, and can lead to corruption (moderate)~~

The reset added in the previous round only works at doc block separators
(spec.md:173–178). A Markdown document has no separators. So
`<a title="<select> menu">x</a>` drops every later tag in the document, with
only a warning at the end of the input. The parenthesis in the spec says the
reset stops such false positives "from dropping tags in later blocks". For
Markdown documents, which include `spec.md` itself, that isn't true.

The same false positive can also get the scanner out of step with html5ever
so that tags land in the wrong place:

```markdown
<a title="<!--">x</a> <textarea>a
b <!-- c --> d
</textarea>
```

1. The scanner treats the attribute's `<!--` as an open comment, so it skips
   the real `<textarea>` opener.
2. It closes the "comment" at `-->` on line 2.
3. Line 3's `</textarea>` is inline HTML (only an *opening* `<textarea` starts
   a type 1 HTML block), and suppression is now off, so it gets
   `<l-n>3</l-n>`. html5ever is still in RCDATA at that point, so the tag
   becomes textarea text and is saved.

**Suggest:** In the Unsuppressed state, accept an opener in an `InlineHtml`
event only at byte offset 0. pulldown-cmark emits each inline HTML construct
(one tag, one comment, one processing instruction, and so on) as its own
`InlineHtml` event. So an opener later in that event is always inside an
attribute value, and html5ever never treats it as an opener. Keep the current
anywhere-in-the-event search for closers in `InlineHtml`, because RCDATA and
raw text end at any `</name`, even one inside a quoted value. Also keep it for
openers in `Html` events, where one line can hold several tags. Correct the
parenthesis so it doesn't claim coverage for Markdown documents.

~~### 3. Where the reset happens and what the warning reports aren't defined (moderate)~~

spec.md:173–175 says to "reset suppression at every doc-block separator" and
to warn "with the file path and line number". Three things are missing:

* **How a separator is recognized.** No pulldown-cmark event marks a
  separator. The adapter has to match the separator text or use the segment
  boundaries in the line map. Neither method is named.
* **Whether the reset happens before or after the separator's own `Html`
  events are scanned.** The order matters. Line 3 of the separator
  (`processing.rs:305`) is `</pre></script></style></textarea>-->?>]]>`. If
  the reset happens when the first `<CodeChatEditor-fence>` line arrives, then
  any open `script`, `style`, or `textarea` target, or a comment from an HTML
  block, is still open at that point, so the reset warns. The spec says
  comments "are closed by the separator itself", and `parse_doc_block`
  accepts such a comment (`processing.rs:1635–1637`), so this warning is
  wrong. If the reset happens after the separator has been scanned, those
  targets close normally and only real false positives warn.
* **Which line the warning reports.** Every separator line maps to `None`
  (spec.md:252–253), and end of input has no line. The only line worth
  reporting is the opener's line.

**Suggest:** Record the opener's source line when suppression starts and
report that line in the warning. Reset when the first event at or after the
next segment's `start_md_line` arrives. At that point all of the separator's
`Html` events have been scanned. Also reset after the last event.

~~### 4. `LineMap` is used in the signature but never defined (moderate)~~

spec.md:90 adds `Option<&LineMap>`, but no section defines `LineMap`. The
section on source line offsets (spec.md:198–254) describes three different
things: a byte-offset-to-Markdown-line table built from "the exact input
bytes", the identity-plus-one mapping for Markdown documents, and per-block
segments for source files. The spec doesn't say:

* whether `LineMap` holds the line-start offsets, or `markdown_to_html`
  computes them from its `markdown` argument. If the caller computes them, it
  must use exactly the string that is passed.
* what the Markdown document caller (`processing.rs:1024`) passes when
  `is_toc` is false. It has no segments, so it needs an identity variant or
  segments built specially.
* who builds the segments. The natural place is the fold at
  `processing.rs:1072–1076`, which is where spec.md:245 says to record them.

**Suggest:** Define the type, for example
`enum LineMap { Document, Segments(Vec<(usize, Vec<usize>)>) }`, with the
line-start table computed inside `markdown_to_html`. State what each caller
passes.

~~### 5. What `None` turns off isn't stated (nit)~~

According to spec.md:91, `None` means "emit no tags". That leaves two
questions open. Does the hand-written-tag escape (spec.md:80–86), which runs
"before tag injection", still run? And what source line does its required
warning report when there's no map? If the escape is skipped, a hand-written
`<l-n>` in the sidebar TOC is hidden by `l-n { display: none; }`. If it runs,
TOC output and the output of every test that passes `None` change. Say which
one applies. If the escape runs, also say that the warning leaves out the
line.

~~### 6. `--!>` closes a comment in html5ever but not in the scanner (nit)~~

The HTML tokenizer also ends a comment at `--!>`. The scanner looks only for
`-->` (spec.md:169), so `<!-- a --!>` in an HTML block leaves the scanner
suppressed until the next `-->` or the next reset. The result is only missing
tags. Either add `--!>` to the closer, or list it with the accepted false
positives.

What's sound
------------

* The tag format matches `render_fragment_content`'s 1-based numbering
  (`processing.rs:2179–2185`). Strict monotonicity holds for the whole
  `markdown_to_html` call: events arrive in source order, and the
  content-line maps are strictly increasing within and across blocks.
* The lookup rule is consistent with the placement rules. Every untagged line
  (fences, HTML blocks, suppressed text, continuation lines) resolves to the
  nearest earlier tag in its container, or to no position.
* Raw-text suppression doesn't need an opaque-depth reset. A doc block's
  unterminated fence ends at the separator's backtick or tilde fence, and an
  `Image` can't cross a blank line.
* The separator has no raw-text opener and no `<!--`, so scanning it can't
  turn suppression on.
* The trigger list and the opaque set match pulldown-cmark 0.13.6's writer:
  * `raw_text` (`html.rs:479`) pulls the alt-text events through the adapter,
    so depth counting sees them.
  * `in_non_writing_block` suppresses `Text` but not `InlineHtml`, which is
    why `MetadataBlock` must be opaque.
* The Example output matches existing minified output. For example,
  `translation.rs:1966` shows `<li><input checked type=checkbox>Task list`,
  with no space after the checkbox.
