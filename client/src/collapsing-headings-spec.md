The CodeChat Editor, a word processor for software developers.<br>Copyright (C)
2024-2026 Bryan A. Jones.

This file is part of the CodeChat Editor.

The CodeChat Editor is free software: you can redistribute it and/or modify it
under the terms of the GNU General Public License as published by the Free
Software Foundation, either version 3 of the License, or (at your option) any
later version.

The CodeChat Editor is distributed in the hope that it will be useful, but
WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more
details.

You should have received a copy of the GNU General Public License along with the
CodeChat Editor. If not, see
[http://www.gnu.org/licenses](http://www.gnu.org/licenses).

Collapsing headings spec
========================

Each heading in a document gets an icon beside it that collapses its section:
all content after the heading and before the next heading of the same or a
higher level, including subheadings, doc blocks and code blocks. The heading
itself stays visible. The icon then changes to an expand icon, which shows the
section again. This lets a reader fold away the parts of a long document or
source file they aren't working on.

Collapsing changes only what the Client shows. Document content sent to the
Server or IDE, and the saved file, contains no collapse controls or state.
Normal cursor and scroll synchronization may still occur.

Definitions
-----------

* **Heading:** an `<h1>`–`<h6>` element that is a direct child of the contents
  of a Markdown document (`.CodeChat-doc-contents` in
  [CodeChatEditor.mts](CodeChatEditor.mts)) or of a doc block (the
  `.CodeChat-doc-contents` of a `DocBlockWidget` in
  [CodeMirror-integration.mts](CodeMirror-integration.mts)). A heading inside a
  list, block quote or table isn't one. A heading rendered from a Rust comment
  such as `/// # Examples` qualifies on the same basis.
* **Doc block:** documentation that the Server renders as editable HTML in a
  source file. Code between doc blocks stays in CodeMirror; heading-like text
  in that code isn't a heading.
* **Level:** the heading's number: `<h1>` is level 1, the highest.
* **Document order:** in Markdown, the order of the document's contents; in a
  source file, the order of code and doc blocks, including the order of
  elements inside each doc block.
* **Section:** everything after a heading in document order, up to but not
  including the next heading at the same or a higher level (a lower or equal
  number), or to the end of the document. The heading itself isn't part of its
  section. A section may be empty.
* **Heading key:** the tuple of a heading's level, its text, and its 1-based
  occurrence among headings with that same level and text in document order.
  The text is the heading's `textContent` in the Client's current content, as
  the Client would send it to the Server (no collapse controls, math not yet
  rendered), with each run of whitespace, including non-breaking spaces,
  collapsed to one space. This key is used only to restore collapsed state
  after a content update from the Server (rule 9); it is not a saved id.

Rules
-----

1. **Icon:** each heading shows a button before its text: ▾ when its section
   is shown, ▸ when collapsed. The button is always visible unless its heading
   is hidden by a collapsed ancestor. It can be reached and activated with the
   keyboard and reports its state with `aria-expanded`. The heading text stays
   editable as before.
2. **Collapse:** activating ▾ hides the heading's section and shows ▸ instead.
   The heading itself stays visible. If the Client's cursor or either end of
   its selection is in the section being hidden, move the cursor to the end
   of the heading and clear the selection. Hidden content cannot receive
   keyboard focus; an edit affecting it first reveals it (rule 8).
3. **Expand:** activating ▸ shows the section again. A subheading that was
   collapsed before stays collapsed, unless it must open to reveal an edit
   (rule 8).
4. **Markdown documents:** the section consists of the elements after the
   heading in the document's contents, ending before the next heading of the
   same or a higher level, or at the end of the document.
5. **Source files:** the same section boundary applies across code and doc
   blocks. A section may start and end in the same doc block. Otherwise it
   contains the rest of the heading's doc block, all intervening code and doc
   blocks, and the content before the ending heading in its doc block. If
   there is no ending heading, it contains everything through the end of the
   file. Hiding part of a doc block leaves its other content visible. Code in
   a collapsed section is hidden, not deleted. The code and whole doc blocks
   in a section are folded with CodeMirror's own folding, so they show as
   CodeMirror's fold placeholder (…). Clicking the placeholder, or a CodeMirror
   unfold command such as Unfold All, expands the section like ▸.
6. **Never saved:** collapsing and expanding do not mark the document as edited
   or send document content. All content, including hidden content, is kept;
   collapse controls and state are absent from HTML sent to the Server, the
   Markdown the Server produces, and the saved file. Existing document
   attributes, including heading ids, are preserved. Cursor paths and offsets
   describe the document without collapse controls. Moving the cursor under
   rule 2 may still send a normal cursor update.
   Check both raw TinyMCE saves and direct `innerHTML` reads: after the same
   visible edit, outgoing content and the saved file must match those with all
   sections shown. Toggling alone sends no document content; a cursor in visible
   text maps to the same text with sections collapsed or shown.
7. **Cursor in hidden content:** the cursor never stays in hidden content.
   When it moves there, every collapsed section hiding it opens, as
   CodeMirror's folding already does for code. This covers the cursor the IDE
   sends, a search or go-to-line that selects hidden content (including
   TinyMCE's search in Markdown documents), following a link or
   cross-reference, and a Client edit or Server update that leaves the cursor
   in a collapsed section, such as changing a heading's level so that a
   collapsed section now holds the cursor. A line in a doc block that is only
   partly hidden points to that doc block, which is visible, so it opens
   nothing. A scroll position alone never opens a section.
   Two of these cases need fixes in the Client first, and this spec assumes
   they will be made: in Markdown documents the Client doesn't yet follow the
   IDE's cursor (`scrollToLine` is a `TODO`), and same-file links don't yet
   work in source files (their tests in `overall_5.rs` are ignored).
8. **Edits:** before a Client edit that would change hidden content, expand
   every section needed to reveal the affected content, then apply the edit
   with the same result as if that content had been shown. This includes
   Backspace or Delete merging across a collapsed section and replacing or
   deleting a selection that crosses one, in Markdown and source files. Undo
   and redo are edits too. In example 1, with A collapsed, Backspace at the
   start of C must reveal A's section before merging C into p2; selecting from
   A to C and typing must reveal the affected hidden content before replacing
   the selection.
   A section follows its heading through Client edits. Editing the heading's
   text preserves its collapsed state; changing its level recomputes its end.
   Deleting a heading or turning it into ordinary text removes its collapsed
   state and recomputes visibility. Other collapsed headings retain their own
   state, so this does not necessarily reveal every former descendant.
9. **Lifetime:** collapsed state lasts only while the file is open in the
   Client. On each content update from the Server, retain the keys of
   collapsed headings before applying the update and restore collapsed state
   to the headings with matching keys afterwards. This covers edits made in
   the IDE and the re-translations the Server sends on its own (after some
   saves, or when a cross-referenced target changes). It applies to both
   incremental updates and full content replacements that rebuild the view,
   before resolving cursor and scroll targets (rule 7). Discard keys with no
   match; unmatched new headings start expanded, though a collapsed ancestor
   may still hide them. Thus an IDE rename or level change normally loses that
   heading's collapsed state.
   Occurrence matching is deterministic, but inserting or deleting an
   identical heading may transfer the state to another occurrence, and a
   heading whose text comes from a cross-reference gets a new key when its
   target's text changes; tracking identity across such changes is out of
   scope. Opening another file, closing
   and reopening this file, or reloading the Client starts with all sections
   expanded. An automatic full content update of the open file is not a reload.

Out of scope
------------

* Remembering collapsed sections after the file is closed or the Client reloads.
* Commands to collapse or expand every section at once.
* Headings inside lists, block quotes and tables, which aren't headings by the
  definition above.
* Tracking heading identity across updates from the Server beyond the
  heading-key rule.
* Changing CodeMirror's folding by syntax, which keeps working alongside the
  folds that collapsed sections add.
* Finding hidden text with the browser's own find; the Client's search covers
  it (rule 7).

Examples
--------

`H1 A` is a level-1 heading with text `A`; `p1` is a paragraph; `code1` is a
code block. Examples whose Document cell starts with "Source file:" describe a
source file; every other example is a Markdown document. Each example starts
with every section shown unless its Document cell specifies otherwise. Results
describe visible content and any observable save or synchronization behavior.
Examples of updates from the Server leave the cursor outside hidden content
unless cursor movement is the action being tested. The Test column says where
the example is tested (see Tests below).

| # | Document | Action | Expected result | Test |
| -- | -- | -- | -- | -- |
| 1 | `H1 A`, `p1`, `H2 B`, `p2`, `H1 C`, `p3` | Collapse A | `H1 A`, `H1 C`, `p3` | Client |
| 2 | Same as 1 | Collapse B, collapse A, expand A | `H1 A`, `p1`, `H2 B`, `H1 C`, `p3` (B stays collapsed) | Client |
| 3 | `H2 A`, `p1`, `H3 B`, `p2` | Collapse A | `H2 A` (the section runs to the end) | Client |
| 4 | A list holding `H2 X` | (none) | No icon beside `X` | Client |
| 5 | Same as 1 | Tab to A's button, press Space, then Enter | A collapses and expands; focus stays on the button and `aria-expanded` follows its state | Client |
| 6 | Source file: doc block [`H1 A`, `p1`], `code1`, doc block [`H2 B`], `code2`, doc block [`H1 C`] | Collapse A | Doc block [`H1 A`], doc block [`H1 C`] | Client |
| 7 | Source file: doc block [`H2 A`, `p1`], `code1`, doc block [`p2`, `H2 B`, `p3`] | Collapse A | Doc block [`H2 A`], doc block [`H2 B`, `p3`] | Client |
| 8 | Source file: doc block [`H2 A`, `p1`, `H2 B`, `p2`] | Collapse A | Doc block [`H2 A`, `H2 B`, `p2`] | Client |
| 9 | Same as 1, initially clean | Collapse A, then expand A without editing | No document content sent; no dirty state | Client |
| 10 | Same as 1 | Collapse A, edit p3, wait for autosave | Outgoing document HTML and the saved file equal those from the same edit with A shown; p1, B and p2 remain in the document | overall.rs |
| 11 | `H1 A` with an existing id, followed by p1 | Collapse A, edit A's text, wait for autosave | A stays collapsed; the saved heading keeps its id and contains no collapse markup | overall.rs |
| 12 | Same as 1, A collapsed | Place the Client cursor in C | The position sent to the IDE maps to C, as with A shown | overall.rs |
| 13 | Same as 6, A collapsed | The IDE moves the cursor into `code1` | A opens; the cursor is in `code1` | Client |
| 14 | Same as 1, A and B collapsed; p3 links to an id in p2 | Follow the link in the Client | A and B open; the Client scrolls to p2 | Client |
| 15 | Same as 1, A collapsed | Backspace at the start of C in the Client | A opens before C merges into p2; result equals the same edit with A shown | Client |
| 16 | Same as 1, A collapsed | Select from the start of A to the start of C and type `X` | A opens before replacement; document content equals the same replacement with A shown | Client |
| 17 | Same as 1, A and B collapsed | Change A into a paragraph in the Client | Paragraph `A`, `p1`, `H2 B`, `H1 C`, `p3`; B stays collapsed | Client |
| 18 | `H2 A`, `p1`, `H2 B`, `p2`; A collapsed | Change A to level 1 in the Client | A remains collapsed and now also hides B and p2 | Client |
| 19 | Same as 1, A collapsed | The IDE inserts a paragraph above A | New paragraph, `H1 A`, `H1 C`, `p3`; A remains collapsed | Client |
| 20 | Same as 1, A collapsed | The IDE renames A to D | `H1 D`, `p1`, `H2 B`, `p2`, `H1 C`, `p3`; D starts expanded | Client |
| 21 | Same as 6, A collapsed | The IDE edits `code1`, sent once as a full content replacement and once as a diff | A remains collapsed in both cases | Client |
| 22 | `H2 A`, `p1`, `H2 A`, `p2`, both A headings collapsed | The IDE updates p1 without changing headings | Both A headings remain collapsed, matched by occurrences 1 and 2 | Client |
| 23 | Same as 1, A collapsed | Close and reopen the file | Every section shown | overall.rs |
| 24 | Same as 1, A collapsed | Edit p3 so that the Server's reply to the autosave changes the content (for example, a line it re-wraps), and wait for that update | The update replaces the content; A remains collapsed and p3 shows the Server's version | overall.rs |

Implementation
--------------

* **Markdown documents** ([CodeChatEditor.mts](CodeChatEditor.mts)): the whole
  document is one TinyMCE editor. Keep fold state separate from document
  content. Any display-only DOM additions must be excluded from every content
  extraction path, including raw saves and direct `innerHTML` reads (rule 6).
  `data-mce-bogus="all"` alone does not establish that these paths are clean.
  Keep cursor paths consistent with the content sent to the Server. Display
  changes must not create document edits or undo entries; user edits retain
  the editor's normal undo behavior.
* **Source files** ([CodeMirror-integration.mts](CodeMirror-integration.mts)):
  fold the code and whole doc blocks of a section with the folding in
  `@codemirror/language`, which `basicSetup` already includes. `foldEffect`
  and `unfoldEffect` add and remove a fold, and `foldState` maps it through
  edits. `foldState` also clears a fold that a deletion touches or that the
  selection head moves into (`clearTouchedFolds`), which gives rules 7 and 8
  for code. A fold starts and ends at doc-block boundaries, so it never
  partly overlaps a doc block's own `Decoration.replace` widget; the parts of
  the first and last doc blocks that belong to the section are hidden inside
  those blocks (rule 1). `DocBlockWidget.eq` must compare that state, so that
  `toDOM` and `updateDOM` redraw the block, including when TinyMCE moves into
  or out of it. See the CodeMirror
  [folding reference](https://codemirror.net/docs/ref/#language.foldEffect).
* **Updates from the Server:** Markdown content is reset with `setContent`; a
  full source update destroys and recreates the CodeMirror view, while a
  source diff uses transactions. Re-translations (`is_re_translation`) take
  the same paths. A diff maps CodeMirror's folds through its changes; a full
  update recreates the view and its `foldState`. Restore collapsed state by
  heading key after each path (rule 9), without inserting fold state into the
  content used as the next diff's base.
  Existing heading ids serve cross-references and must remain unchanged.
* **Cursor, scroll and edits:** in source files, setting CodeMirror's
  selection to the IDE's cursor line opens the folds around it (rule 7).
  TODO: apply rule 7 to the IDE's cursor in Markdown documents once the Client
  follows it there (`scrollToLine` in [CodeChatEditor.mts](CodeChatEditor.mts)
  is a `TODO`). TODO: once same-file links work in source files (the ignored
  test in [overall_5.rs](../../server/tests/overall/overall_5.rs)), find a
  link's target in the doc-block contents held in CodeMirror's state, since
  `followLink`'s `document.querySelector` can't see folded content, and open
  the sections hiding it. In Markdown documents, `followLink` and TinyMCE's
  `searchreplace` must open the sections hiding their target, and its Replace
  All is an edit (rule 8). Reveal hidden content before an edit affects it
  (rule 8), including edits crossing doc-block boundaries.
* **Tests:** every example has one test, in the layer given by its Test
  column. Client tests in [CodeChatEditor-test.mts](CodeChatEditor-test.mts)
  run only in `test.py?test` and cover the examples that need no Server:
  raw saves, inactive doc-block `innerHTML`, nested folds, keyboard focus, and
  both full and incremental IDE updates, simulated by calling the Client's
  update paths. They have no Markdown fixture yet, so add one next to the
  source-file fixture; repeat save checks for both kinds of document. The
  examples that need the Server, a saved file or a page reload (10, 11, 12, 23
  and 24) are browser integration tests in
  [overall.rs](../../server/tests/overall.rs).
