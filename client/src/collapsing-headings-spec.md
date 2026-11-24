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
* **Heading key:** the tuple of a heading's level, exact DOM `textContent`
  excluding collapse controls, and its 1-based occurrence among headings with
  that same level and text in document order. This key is used only to restore
  collapsed state after an IDE content update (rule 9); it is not a saved id.

Rules
-----

1. **Icon:** each heading always shows a button just left of its text: ▾
   when its section is shown, ▸ when collapsed. A heading hidden by a collapsed
   ancestor and its button are both hidden. Tab and Shift+Tab reach and leave
   visible buttons in document order; Enter or Space activates a focused
   button. The button has an accessible name identifying its heading and
   reports its state with `aria-expanded`. Activating it keeps keyboard focus
   on the button. The heading text stays editable as before. An empty section
   can be toggled, but no document content is hidden.
2. **Collapse:** clicking ▾, or activating it with the keyboard, hides the
   heading's section and shows ▸ instead. The heading itself stays visible.
   Hidden content cannot receive keyboard focus or be edited while hidden;
   an edit affecting it first reveals it (rule 8).
3. **Expand:** clicking ▸, or activating it with the keyboard, shows the
   section again. A subheading that was collapsed before stays collapsed, so
   expanding a section restores it the way the reader left it. Automatically
   expanding a section also preserves collapsed descendants unless they must
   open to reveal an edit or cursor target (rules 7 and 8).
4. **Markdown documents:** the section consists of the elements after the
   heading in the document's contents, ending before the next heading of the
   same or a higher level, or at the end of the document.
5. **Source files:** the same section boundary applies across code and doc
   blocks. A section may start and end in the same doc block. Otherwise it
   contains the rest of the heading's doc block, all intervening code and doc
   blocks, and the content before the ending heading in its doc block. If
   there is no ending heading, it contains everything through the end of the
   file. Hiding part of a doc block leaves its other content visible. Code in
   a collapsed section is hidden, not deleted.
6. **Never saved:** collapsing and expanding change only the Client's view.
   They do not mark the document as edited or send a document-content update.
   Icons and all collapse-only elements, attributes, classes and styles must
   be absent from document HTML sent to the Server, from the Markdown the
   Server produces from that HTML, and from the saved file. Hidden content
   remains present and unchanged in that HTML. This applies both to TinyMCE's
   `save({ format: "raw" })` paths and to direct `innerHTML` reads; relying on
   TinyMCE's normal serializer alone is insufficient. Existing heading ids and
   other document attributes must be preserved; collapsing adds no saved ids.
   Cursor positions sent to the IDE and restored in the Client must refer to
   the same document text as with every section shown. Paths and offsets used
   for synchronization must describe the document without collapse controls,
   even if the displayed DOM contains them.
   Verify this for Markdown documents and source-file doc blocks by comparing
   outgoing document HTML and the resulting saved file after the same visible
   edit with sections collapsed and shown. Also check that collapsing or
   expanding alone sends no document content, and that a cursor after a
   collapse control maps to the same text in both the Client and the IDE.
7. **Shown when needed:** when the IDE's cursor moves into hidden content,
   expand every collapsed section hiding that target before placing the
   Client's cursor there. Receiving an unchanged cursor position along with
   an IDE update does not itself reopen a section. An IDE scroll position
   alone never expands a section: if its target is hidden, scroll to the
   heading of the outermost collapsed section hiding it. If an update includes
   both a moved cursor and a scroll position, first reveal the cursor target,
   then resolve the scroll target using the resulting collapsed state.
8. **Edits:** before a Client edit that would change hidden content, expand
   every section needed to reveal the affected content, then apply the edit
   with the same result as if that content had been shown. This includes
   Backspace or Delete merging across a collapsed section and replacing or
   deleting a selection that crosses one, in Markdown and source files.
   For example 1 with A collapsed, Backspace at the start of C must reveal A's
   section before merging C into p2; selecting from A to C and typing must
   reveal the affected hidden content before replacing the selection.
   A section follows its heading through Client edits. Editing the heading's
   text preserves its collapsed state; changing its level recomputes its end.
   Deleting a heading or turning it into ordinary text removes its collapsed
   state and recomputes visibility. Other collapsed headings retain their own
   state, so this does not necessarily reveal every former descendant.
9. **Lifetime:** collapsed state lasts only while the file is open in the
   Client. On each IDE content update, retain the keys of collapsed headings
   before applying the update and restore collapsed state to the headings with
   matching keys afterwards. This applies to both incremental updates and full
   content replacements that rebuild the view, before resolving cursor and
   scroll targets (rule 7). Discard keys with no match; unmatched new headings
   start expanded, though a collapsed ancestor may still hide them. Thus an
   IDE rename or level change normally loses that heading's collapsed state.
   Occurrence matching is deterministic, but inserting or deleting an
   identical heading may transfer the state to another occurrence; tracking
   identity across such changes is out of scope. Opening another file, closing
   and reopening this file, or reloading the Client starts with all sections
   expanded. An automatic full content update of the open file is not a reload.

Out of scope
------------

* Remembering collapsed sections after the file is closed or the Client reloads.
* Commands to collapse or expand every section at once.
* Headings inside lists, block quotes and tables, which aren't headings by the
  definition above.
* Tracking heading identity across IDE updates beyond the heading-key rule.
* Folding code by its syntax, which CodeMirror already offers and which stays
  as it is.

Examples
--------

`H1 A` is a level-1 heading with text `A`; `p1` is a paragraph; `code1` is a
code block. Each example starts with every section shown unless its Document
cell specifies otherwise. Results describe visible content and any observable
save or synchronization behavior. IDE content-update examples leave the cursor
outside hidden content unless cursor movement is the action being tested.

| # | Document | Action | Expected result |
| -- | -- | -- | -- |
| 1 | `H1 A`, `p1`, `H2 B`, `p2`, `H1 C`, `p3` | Collapse A | `H1 A`, `H1 C`, `p3` |
| 2 | Same as 1 | Collapse B | `H1 A`, `p1`, `H2 B`, `H1 C`, `p3` |
| 3 | Same as 1 | Collapse B, collapse A, expand A | `H1 A`, `p1`, `H2 B`, `H1 C`, `p3` (B stays collapsed) |
| 4 | `H2 A`, `p1`, `H3 B`, `p2` | Collapse A | `H2 A` (the section runs to the end) |
| 5 | A list holding `H2 X` | (none) | No icon beside `X` |
| 6 | Source file: doc block [`H1 A`, `p1`], `code1`, doc block [`H2 B`], `code2`, doc block [`H1 C`] | Collapse A | Doc block [`H1 A`], doc block [`H1 C`] |
| 7 | Source file: doc block [`H2 A`, `p1`], `code1`, doc block [`p2`, `H2 B`, `p3`] | Collapse A | Doc block [`H2 A`], doc block [`H2 B`, `p3`] |
| 8 | Same as 1 | Collapse A, edit p3, wait for autosave | Outgoing document HTML and the saved file equal those from the same edit with A shown; p1, B and p2 remain in the document |
| 9 | Same as 6, A collapsed | The IDE moves the cursor into `code1` | Every section shown; the cursor in `code1` |
| 10 | Same as 1, A collapsed | Delete heading A in the Client | `p1`, `H2 B`, `p2`, `H1 C`, `p3` |
| 11 | Source file: doc block [`H2 A`, `p1`, `H2 B`, `p2`] | Collapse A | Doc block [`H2 A`, `H2 B`, `p2`] |
| 12 | Same as 1, initially clean | Collapse A, then expand A without editing | No document content sent; no dirty state |
| 13 | Same as 1, A collapsed | The IDE inserts a paragraph above A | New paragraph, `H1 A`, `H1 C`, `p3`; A remains collapsed |
| 14 | Same as 1, A collapsed | The IDE renames A to D | `H1 D`, `p1`, `H2 B`, `p2`, `H1 C`, `p3`; D starts expanded |
| 15 | Same as 1, A collapsed | The IDE sends a full content replacement with the same headings and an edit to p3 | `H1 A`, `H1 C`, edited p3; A remains collapsed |
| 16 | Same as 6, A collapsed; IDE cursor outside A's section | The IDE scrolls to `code1`, repeating its unchanged cursor position | A stays collapsed; the Client scrolls to A |
| 17 | Same as 1, A and B collapsed | The IDE moves the cursor into p2 | A and B open before the cursor is placed in p2 |
| 18 | Same as 1, A collapsed | Backspace at the start of C in the Client | A opens before C merges into p2; result equals the same edit with A shown |
| 19 | Same as 1, A collapsed | Select from the start of A to the start of C and type `X` | A opens before replacement; document content equals the same replacement with A shown |
| 20 | Same as 1, A collapsed | Place the Client cursor in C | The position sent to the IDE maps to C, as with A shown |
| 21 | Same as 1, A and B collapsed | Delete heading A in the Client | `p1`, `H2 B`, `H1 C`, `p3`; B stays collapsed |
| 22 | `H2 A`, `p1`, `H2 A`, `p2`, both A headings collapsed | The IDE updates p1 without changing headings | Both A headings remain collapsed, matched by occurrences 1 and 2 |
| 23 | Same as 1, A collapsed | Close and reopen the file | Every section shown |
| 24 | Same as 1 | Tab to A's button, press Space, then Enter | A collapses and expands; focus stays on the button and `aria-expanded` follows its state |
| 25 | `H2 A` at the end of the document | Collapse A | A stays visible, with ▸; no content changes |
| 26 | Source file: doc block rendered from `/// # Examples` and `/// p1`, then `code1`, then doc block rendered from `/// # Next` | Collapse Examples | Doc block [`H1 Examples`], doc block [`H1 Next`] |
| 27 | `H1 A` with an existing id, followed by p1 | Collapse A, edit A's text, wait for autosave | A stays collapsed; the saved heading keeps its id and contains no collapse markup |
| 28 | `H2 A`, `p1`, `H2 B`, `p2`; A collapsed | Change A to level 1 in the Client | A remains collapsed and now also hides B and p2 |

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
  account for sections spanning code and doc blocks and for boundaries inside
  one doc block. A `StateField` can map code positions through Client edits,
  but heading boundaries inside doc-block HTML also need tracking. Doc-block
  widgets must reflect the fold state when created or updated, including when
  an inactive block is moved into or out of TinyMCE. Hidden code must remain
  in the underlying document, and fold decorations must coexist with doc-block
  replacements and syntax folding without overlapping replacement ranges.
* **IDE updates:** Markdown content is reset with `setContent`; a full source
  update destroys and recreates the CodeMirror view, while a source diff uses
  transactions. Preserve and restore heading keys across each path (rule 9),
  without inserting fold state into the content used as the next diff's base.
  Existing heading ids serve cross-references and must remain unchanged.
* **Cursor, scroll and edits:** distinguish cursor movement from a repeated
  cursor position in an IDE update, since the IDE also sends its scroll
  position. Resolve targets using rule 7 after restoring fold state. Reveal
  hidden content before an edit affects it (rule 8), including edits crossing
  doc-block boundaries.
* **Tests:** cover every example and rule in Client tests in
  [CodeChatEditor-test.mts](CodeChatEditor-test.mts), including raw saves,
  inactive doc-block `innerHTML`, nested folds, keyboard focus, and both full
  and incremental IDE updates. Repeat save and cursor-mapping checks for both
  Markdown and source files. Use the browser integration tests in
  [overall.rs](../../server/tests/overall.rs) for end-to-end saving and IDE
  synchronization; prefer Client tests for behavior that does not need that
  round trip.
