Copyright (C) 2026 Bryan A. Jones.

This file is part of the CodeChat Editor.

The CodeChat Editor is free software: you can redistribute it and/or modify it
under the terms of the GNU General Public License as published by the Free
Software Foundation, either version 3 of the License, or (at your option) any
later version.

The CodeChat Editor is distributed in the hope that it will be useful, but
WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more
details.

You should have received a [copy](LICENSE.html) of the GNU General Public
License along with the CodeChat Editor. If not, see
[https://www.gnu.org/licenses/](https://www.gnu.org/licenses/).

CodeChat Editor accessibility review
====================================

This review measures the accessibility of the pages the Client actually renders.
Every finding below comes from
[`overall_a11y.rs`](../server/tests/overall/overall_a11y.rs), which drives the
same headless Chrome the other `overall_*` tests use: it loads a file, lets the
Client render it, then runs the
[axe-core](https://github.com/dequelabs/axe-core) audit engine and a structural
probe against each frame of the result, and walks the tab order by pressing keys
at the browser.

Keeping this document current
-----------------------------

This file is written by hand; only its evidence is generated. After changing
anything which affects the rendered page, refresh the evidence and then edit the
prose against it:

1. Rebuild the Client if any of its TypeScript or CSS changed, by running `./bt
   client-build` from `server/`. The tests serve the *bundle*, so a source-only
   change stays invisible to them until it's rebuilt. This is the step which is
   easy to forget and which silently yields stale findings.

2. Run `cargo test --test overall a11y` from `server/`. Each scenario rewrites
   `server/target/a11y/<scenario>.json` -- the audit findings, the heading and
   landmark outline, every focusable control with its accessible name, the
   measured tab order, and the list of rules which passed -- and then rebuilds
   `server/target/a11y/summary.md` from all four.

3. Read `summary.md` and update the findings below to match it. It collects what
   the four reports say into one page: which reports are current, every
   violation with its WCAG tags, every focusable control and the name it
   exposes, and the keyboard measurements. Run-specific noise is substituted out
   of it, so `diff`ing it against a copy from before your change shows what
   actually moved. The JSON reports remain the place to look for a violation's
   failing elements.

   The tests deliberately don't fail on violations -- see \[`assert_audit_ran`\]
   in [`overall_a11y.rs`](../server/tests/overall/overall_a11y.rs) -- so nothing
   forces this file back into agreement with the measurements. That comparison
   is yours to make.

Everything in step 2 is written under `server/target/`, which version control
ignores: these files describe one browser version on one platform, and so aren't
reproducible enough to check in. `summary.md` is generated on every run, so
don't edit it -- this file is where the prose belongs.

Scope
-----

The Server emits materially different markup for each combination of file kind
and project membership, so all four are measured:

| Scenario              | File      | In a project |
| --------------------- | --------- | ------------ |
| `markdown_standalone` | `test.md` | no           |
| `code_standalone`     | `test.py` | no           |
| `markdown_project`    | `test.md` | yes          |
| `code_project`        | `test.py` | yes          |

These findings describe the working tree as measured, not the last commit.
Several passes and fixes recorded below live in uncommitted changes to
[`webserver.rs`](../server/src/webserver.rs),
[`tinymce-config.mts`](../client/src/tinymce-config.mts),
[`CodeMirror-integration.mts`](../client/src/CodeMirror-integration.mts),
[`CodeChatEditor.mts`](../client/src/CodeChatEditor.mts), and the
[user manual](../README.md).

Each scenario audits every frame of the page. The Client nests two frames deep,
and a project adds a third:

* `framework` -- the outer page the Server serves, which holds
  `#CodeChat-iframe` and nothing else.
* `client` -- the Client's own page, at rest, immediately after rendering.
* `client-editing` -- the same page after clicking a doc block, which hands the
  block to TinyMCE and builds TinyMCE's menu bar into `#CodeChat-menu`. This is
  where the Client's only chrome lives, so a review which skipped it would miss
  most of the interactive surface.
* `toc` -- the table of contents in `#CodeChat-sidebar`; projects only.

What was measured, and what wasn't
----------------------------------

axe-core applied every rule which its content made applicable: 17 rules on the
near-empty framework page, 16 on the table of contents, and 36 to 44 on the
Client itself. The exact list per frame is in each scenario's JSON under
`passed_rules`. Together these cover the mechanical half of WCAG: accessible
names, ARIA validity, color contrast, landmark and heading structure, list
markup, frame titles, and language declaration.

Every frame which contains an iframe also reports `frame-tested` as *incomplete*
rather than passed. That is an artifact of how this harness drives axe: it runs
with `iframes: false` and visits each frame itself, so axe sees a frame it was
told not to descend into and declines to conclude anything about it. The nested
frame is measured -- as its own row in these reports -- not skipped.

It does not cover the half that needs a person. Nothing here establishes that
the editor is *usable* with a screen reader -- only that its markup exposes the
names and roles a screen reader needs. In particular, these remain unverified:

* Whether editing a doc block is comprehensible with a screen reader running,
  given that a doc block is a `contenteditable` region nested inside a
  CodeMirror editor which is itself a `role="textbox"`.
* Whether the arrow-key navigation between doc and code blocks (see
  `docBlockNavKeymap` in
  [`CodeMirror-integration.mts`](../client/src/CodeMirror-integration.mts))
  announces the transition.
* Anything about the VSCode extension's own UI, which this harness doesn't load.
* Zoom and reflow behavior at 200% and 400%, which the fixed 1920x768 test
  window can't exercise.

Findings
--------

### 1\. The CodeMirror editor has no accessible name -- FIXED

*Serious. WCAG 2.1 4.1.2 (Name, Role, Value), level A. Both code scenarios;
absent from Markdown files, which contain no code block.*

CodeMirror rendered its editing surface as `<div class="cm-content"
contenteditable="true" role="textbox">` with no `aria-label`, `aria-labelledby`,
or `title`. A control which declares `role="textbox"` and offers no name is
announced by a screen reader as an unlabeled text field -- and this was the
control holding the user's source code, the single most important element on the
page. axe reported it as `aria-input-field-name` in the `client` and
`client-editing` frames of both `code_standalone` and `code_project`.

**Fixed** in
[`CodeMirror-integration.mts`](../client/src/CodeMirror-integration.mts) by
adding an `EditorView.contentAttributes` facet carrying
`aria-labelledby="CodeChat-filename"`, which points the editor at the filename
the Server already renders into the page header. Referencing that element rather
than composing a literal `aria-label` means the label names whichever file is
open without this code having to learn the path, and it stays identical to what
a sighted user reads at the top of the page. The structural probe now reports
the editor's name as `test.py - <directory>`, and the violation is gone from all
four scenarios.

### 2\. Doc blocks are out of the tab order, and the active one has no role -- BY DESIGN

*Moderate, and partly a design decision. Bears on WCAG 2.1 4.1.2 (Name, Role,
Value), level A. Both code scenarios.*

Doc blocks in a source file used to be `<div tabindex="0">` with no `role` and
no accessible name: focusable, but announcing themselves only by reading out
their text, with nothing to say they were editable. They now carry `tabIndex =
-1` instead, which takes them out of the tab order entirely. The measured tab
order confirms it -- in both code scenarios, `Tab` cycles between the CodeMirror
editor and the page outside it, and never lands on a doc block.

That trades one question for another. Doc blocks are no longer unlabeled stops
in the tab order, but the only remaining route into one is the arrow-key
navigation in `docBlockNavKeymap`, which moves the caret from the code editor
into an adjacent block. That's a reasonable model -- doc blocks are content
*inside* the code editor, not separate controls -- and it matches how the code
reads. What it needs is confirmation that a screen reader announces the
transition, which is listed above as unverified and can't be settled by a DOM
audit.

The remaining markup gap is on the block being edited. In a source file it
becomes `<div id="TinyMCE-inst" class="CodeChat-doc-contents mce-content-body">`
with `contenteditable` but no explicit `role`, so it's exposed by the
`contenteditable` alone and named by its own text -- the probe reports its name
as "Accessibility sampleA paragraph containing a link and inline code." axe
doesn't flag this -- with no ARIA role declared, the `aria-input-field-name`
rule never applies -- so it came out of the structural probe.

That block is also the one doc block the probe lists as focusable in the code
scenarios: TinyMCE's own container doesn't carry the `tabIndex = -1` which
`DocBlockWidget.toDOM` sets on the blocks around it. The walk never records it
as a tab stop even so -- the presses it makes inside the editor are swallowed by
CodeMirror's indent keymap (see the bug recorded at the end of this file), and
the focus it does capture alternates between the editor and `body`.

The fix this would call for is `role="textbox"`, `aria-multiline="true"`, and an
`aria-label` such as "Documentation block" on the active block, so that entering
one is announced as entering an editable region rather than as plain text.

**Resolution:** This is by design; together, code blocks and doc blocks form a
single editing surface; keyboard navigation should move from/to the editing
surface as a whole, not to fragments of it. The fix suggested above therefore
does not apply.

### 3\. TinyMCE's upgrade promotion is hidden yet focusable -- FIXED

*Serious. WCAG 2.1 4.1.2 (Name, Role, Value), level A, and 1.4.3 (Contrast
Minimum), level AA. All four scenarios.*

TinyMCE injects a "Get all features" link advertising its cloud product:

```html
<a href="https://www.tiny.cloud/tinymce-upgrade-to-cloud/?..."
   aria-hidden="true" class="tox-promotion-link">...</a>
```

It carried `aria-hidden="true"` yet stayed in the tab order, which is the
`aria-hidden-focus` failure: a keyboard user lands on a control their screen
reader has been told does not exist. Separately, its text failed contrast at
4.31:1 (`#086be6` on `#e8f1f8`) against the 4.5:1 requirement. It was also the
*only* contrast failure on any of the four pages -- the project's own light
theme passes throughout.

**Fixed** by `promotion: false` in the TinyMCE options in
[`tinymce-config.mts`](../client/src/tinymce-config.mts), which removes the
element entirely and clears both failures at once. The structural probe no
longer finds the link among the focusable elements of any scenario.

### 4\. The table of contents has no `main` landmark -- ACCEPTED

*Moderate. Best practice (axe `landmark-one-main`); no WCAG success criterion.
Both project scenarios.*

The TOC page the Server generates wrapped its content in a bare `<div
class="CodeChat-TOC">`, so it had no landmark of any kind: screen reader users
navigating a project by landmark found nothing to jump to in the frame they use
to move between files.

A `<nav>` now wraps that div in the TOC branch of `file_to_response` in
[`webserver.rs`](../server/src/webserver.rs), which puts the list inside a
landmark and clears the `region` violation; the probe confirms the `nav` in the
`toc` frame of both project scenarios. `landmark-one-main` remains: axe also
wants one `main` per document, and a `<nav>` isn't one.

**Resolution**: accept the finding. The entire frame contains only navigation
content; labeling it as main content is inaccurate.

The rule is still enabled, so the audit continues to report it -- it is the only
violation left in any scenario. Excluding it for the `toc` frame in
[`overall_a11y.rs`](../server/tests/overall/overall_a11y.rs) would record that
decision where the measurement is taken, and is what stands between this review
and a clean run; see the closing section.

### 5\. The menu bar is outside the tab order -- RESOLVED

*Moderate. No WCAG failure -- the menu bar is reachable, but only by a shortcut,
which the UI itself still doesn't mention. All four scenarios.*

Every item in TinyMCE's menu bar (Edit, Insert, View, Format, Table, Help)
carries `tabindex="-1"`, and the measured tab order confirms that no amount of
tabbing reaches it. The tab order cycles only among the code editor, the page
body, and -- in a project -- the TOC iframe.

TinyMCE's `Alt+F9` shortcut *does* reach it: with the modifier held across the
keypress, focus lands on the "Edit" menu item, in all four scenarios. So this is
a discoverability problem rather than a keyboard-access failure -- and at the
time it was found, nothing told a keyboard user that the shortcut existed.

Note that establishing this took care. Sending `Alt+F9` as `send_keys(Key::Alt +
Key::F9)` delivers two independent presses; the page sees a bare `F9`, focus
doesn't move, and the menu bar looks unreachable. The test now holds the
modifier down explicitly with `key_down`/`key_up`, and records the `keydown`
events the page received alongside the resulting focus, so a future reader can
tell "the shortcut was ignored" from "the shortcut never arrived".

**Resolution**: The [user manual](../README.md) now includes an accessibility
section describing how to navigate each kind of block: `Esc` then `Tab` from
within a code block, `Tab` alone from within a doc block, and `Alt+0` (`⌥0` on
MacOS) for TinyMCE's own list of doc block shortcuts, which is where `Alt+F9` is
named. The measurement above is unchanged -- `Alt+F9` still lands on "Edit" in
all four scenarios -- but it is now discoverable.

### 6\. Links inside doc blocks can't be reached by Tab

*Moderate. Bears on WCAG 2.1 2.1.1 (Keyboard), level A, if the page is read as a
document; arguably conformant if it's read as an editor. See the caveat below.
All four scenarios.*

The structural probe finds the `<a>` elements rendered inside doc blocks, and
they are nominally focusable -- each appears in the `focusable` list of every
scenario, named "link" -- but the measured tab order never lands on one. In a
source file, doc blocks are outside the tab order altogether (finding 2), so
their links are too. A Markdown file is the same case with one block: the whole
document is rendered into a single `.CodeChat-doc-contents` div which also
carries `tabindex="-1"` (see
[`CodeChatEditor.mts`](../client/src/CodeChatEditor.mts)), so the measured walk
never leaves `body` -- apart from the TOC iframe in a project -- and steps over
every link on the page. Either way, a keyboard user reading a CodeChat Editor
document cannot reach its hyperlinks by tabbing.

The caveat is that this is normal for a rich-text editor: inside an editing
surface, a link is content to be edited rather than a control to be activated,
and every major editor behaves this way. It matters more here than it would
elsewhere, because a CodeChat Editor project is also a *document* -- the TOC and
cross-references between files are how a reader navigates -- and the reading
experience and the editing experience are the same page.

**Fix:** worth a design decision rather than a code change. One option is a
read-only presentation mode in which doc blocks aren't editable and their links
join the tab order.

**Resolution pending**: add a reading mode to enable this. Future work.

### 7\. The error overlay is silent and doesn't manage focus -- FIXED

*Moderate. WCAG 2.1 4.1.3 (Status Messages), level AA.*

`haltOnError` reveals `#error-overlay`, a fixed, full-viewport gray panel
reading "Fatal error", by switching it from `display: none` to `display: block`.
The element had no `role`, no `aria-live`, and no focus management, and the
content it covers stayed focusable -- so a keyboard user could tab into a UI
they could no longer see, and a screen reader was told nothing by the overlay
itself. It was only partly mitigated by the toast: `haltOnError` throws, the
error handler turns the message into a toast, and Toastify defaults to
`aria-live="polite"`, so the error text was announced even though the overlay's
appearance wasn't.

**Fixed** across [`webserver.rs`](../server/src/webserver.rs) and
[`CodeMirror-integration.mts`](../client/src/CodeMirror-integration.mts). The
overlay is now `role="alertdialog" aria-modal="true"`, labelled by its "Fatal
error" heading and described by a message paragraph; `haltOnError` writes the
error text into that paragraph, moves focus to the overlay, and marks every
other child of `body` `inert` so the dead UI leaves both the tab order and the
accessibility tree.

Two things about that shape are worth recording. It uses `alertdialog` rather
than the `role="alert"` first proposed here because `aria-modal` is supported
only on dialog roles -- pairing it with `alert` would trade this finding for an
`aria-allowed-attr` violation. And because a screen reader honoring `aria-modal`
ignores everything outside the overlay, including the toast which had been
carrying the error text, the overlay now states the failure itself instead of
depending on that toast; the toast remains for sighted users, and is exempted
from the `inert` sweep so it stays readable and dismissable.

The audit can't confirm any of this: the overlay is `display: none` in every
scenario measured, so axe skips it. It appears only after a fatal Client error,
which none of the four scenarios provoke.

It does leave two marks on the evidence, both harmless. The structural probe
collects headings without filtering by visibility, so the overlay's `<h1>Fatal
error</h1>` heads the outline of every `client` frame ahead of the document's
own `h1`; axe, which ignores hidden content, reports no heading-order problem.
And the tab-order walk prints the accessible name of `body` when focus rests
there, which begins with that same "Fatal error" text.

### 8\. The framework page has no level-one heading -- FIXED

*Minor. Best practice (axe `page-has-heading-one`); no WCAG success criterion.
All four scenarios.*

The outer framework page contained a `<main>` holding a single iframe and no
heading at all. It is a shell, and its `<title>` and the iframe's `title` both
describe it, so the practical impact was small.

**Fixed** in `get_client_framework` in
[`webserver.rs`](../server/src/webserver.rs) by adding `<h1>The CodeChat
Editor</h1>` to the `<main>`, clipped to a single pixel rather than hidden with
`display: none`, which would remove it from the accessibility tree along with
the display. The structural probe now reports the heading in the `framework`
frame of all four scenarios, and the violation is gone.

### 9\. Focus indication on doc blocks is suppressed -- BY DESIGN

*Needs manual verification. WCAG 2.1 2.4.7 (Focus Visible), level AA.*

[`CodeChatEditorBase.css`](../client/src/css/CodeChatEditorBase.css) sets
`outline: 0px` on `.CodeChat-doc-contents:focus-visible` and
`.CodeChat-doc-contents.mce-edit-focus`, with the rationale that the outline
hides the caret and that the whole screen is an editor.

Tab no longer reaches a doc block at all (finding 2); focus arrives by clicking
one or by arrowing into it from the code editor. The probe shows what happens
when it does: the block the `client-editing` frame was clicked into carries
`mce-edit-focus`, so TinyMCE is active and a caret is drawn -- which is normally
an adequate focus indicator for a text region. This is listed as a check rather
than a failure because a caret's adequacy depends on how visible it is in
practice, which a DOM audit can't judge. Someone should confirm on a real
display, particularly at high zoom.

**Resolution**: This is by design. Making the focus visible hides the caret.
Together, the code and doc blocks form one editing surface, in which a caret
shows the currently focused item.

What passes
-----------

Worth recording, so that a future change doesn't quietly undo it:

* Every iframe carries a `title` -- `#CodeChat-iframe` is "The CodeChat Editor
  main window", `#CodeChat-sidebar` is "CodeChat Editor table of contents".
* Every page declares `lang="en"`.
* The Client's page uses real landmarks: `header` and `main`, plus a `nav`
  around the sidebar in a project. The TOC frame is a `nav` (finding 4).
* The heading outline the translator produces is correct: an `h1` followed by an
  `h2`, with no skipped levels, from both Markdown and source comments. The
  hidden `h1` the probe lists ahead of it belongs to the error overlay (finding
  7).
* The light theme passes contrast everywhere, with no exceptions now that
  TinyMCE's promotion link is suppressed (finding 3).
* Toast notifications are announced, via Toastify's default
  `aria-live="polite"`.
* The framework page carries a level-one heading, clipped out of sight but left
  in the accessibility tree (finding 8).
* The code editor names itself after the file it holds (finding 1), and in a
  source file it is the one control of the Client's own which the tab order
  reaches -- in a project, the TOC iframe is the other stop. Doc blocks are
  deliberately not tab stops (finding 2); the arrow keys move between them and
  the code.
* TinyMCE's menu bar is reachable by `Alt+F9` in all four scenarios, and the
  manual says so (finding 5).

Status and remaining work
-------------------------

Findings 1, 3, 7, and 8 are fixed; findings 2 and 9 are resolved as design
decisions, finding 5 by documentation, and finding 4 by accepting the rule as
inapplicable. Every finding is therefore settled except finding 6, which is
deferred. One axe violation remains, and it is the accepted one:

| Scenario                           | Frame | Violation           | Finding |
| ---------------------------------- | ----- | ------------------- | ------- |
| `code_project`, `markdown_project` | `toc` | `landmark-one-main` | 4       |

The two standalone scenarios have no violations in any frame.

What remains, in suggested order:

1. Exclude `landmark-one-main` for the `toc` frame in
   [`overall_a11y.rs`](../server/tests/overall/overall_a11y.rs), recording
   finding 4's resolution where the measurement happens. That leaves the audit
   reporting nothing, at which point
   [`assert_audit_ran`](../server/tests/overall/overall_a11y.rs) can be
   tightened from "the audit ran" to "the audit found nothing", turning this
   review into a regression gate.
2. A reading mode which puts doc block links into the tab order (finding 6).
3. The manual checks this harness can't make: screen reader behavior when
   arrowing between code and doc blocks, caret visibility as a focus indicator
   (finding 9), and reflow at 200% and 400% zoom.

A bug found along the way
-------------------------

The tab-order walk uncovered a defect which isn't an accessibility problem, but
which is worth recording. `Tab` pressed inside the editor reaches CodeMirror's
`indentWithTab` keymap, which inserts an indent into the document rather than
moving focus. The Client then sends the Server an `Update` whose CodeMirror
`doc` begins `" \n\n\n\ndef sample():..."` while its first doc block still
claims the range `from: 0, to: 6`. Translating that state panics the Server's
processing task at the assertion in
[`processing.rs:543`](../server/src/processing.rs#L543) requiring the text
spanned by a doc block to be newlines.

The panic is in a spawned task, so it kills the file's editing session rather
than the process, and the test which triggers it still passes. Reproduce it by
running `cargo test --test overall a11y::test_code_standalone -- --nocapture`
and looking for `panicked at src\processing.rs` in the output. This is why the
tab-order walk runs strictly last in each scenario: nothing may depend on
document state after it.

The walk's own output shows the aftermath, which is why it must be read with
that in mind rather than as a finding. In `code_project` the session dies
partway through, and the last two steps reach a toast reading "Promise rejected:
INVALID\_STATE\_ERR : Pausing to reconnect websocket" and its close button --
controls which exist only because the walk broke the document. Note also that
the walk presses a bare `Tab`, not the `Esc` then `Tab` which the
[manual](../README.md) documents as the way out of a code block; it measures
what an uninstructed keyboard user meets, which is exactly the sequence that
triggers this bug.
