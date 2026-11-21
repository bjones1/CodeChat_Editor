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
everything from the heading down to the next heading of the same or a higher
level, including subheadings, doc blocks and code blocks. The icon then changes
to an expand icon, which shows the section again. This lets a reader fold away
the parts of a long document or source file they aren't working on.

Collapsing changes only what the Client shows. It never changes the document's
text, so nothing about it reaches the Server, the IDE or the saved file.

Definitions
-----------

* **Heading:** an `<h1>`–`<h6>` element that is a direct child of the contents
  of a Markdown document (`.CodeChat-doc-contents` in
  [CodeChatEditor.mts](CodeChatEditor.mts)) or of a doc block (the
  `.CodeChat-doc-contents` of a `DocBlockWidget` in
  [CodeMirror-integration.mts](CodeMirror-integration.mts)). A heading inside a
  list, block quote or table isn't one.
* **Level:** the heading's number: `<h1>` is level 1, the highest.
* **Section:** everything after a heading, up to but not including the next
  heading at the same or a higher level (a lower or equal number), or to the end
  of the document. The heading itself isn't part of its section.

Rule
----

1. **Icon:** each heading shows a button just left of its text: ▾ when its
   section is shown, ▸ when collapsed. The button can be reached with the
   keyboard and reports its state (`aria-expanded`). It is the only new
   control; the heading text stays editable as before.
2. **Collapse:** clicking ▾ hides the heading's section and shows ▸ instead.
3. **Expand:** clicking ▸ shows the section again. A subheading that was
   collapsed before stays collapsed, so expanding a section restores it the way
   the reader left it.
4. **Markdown documents:** the section is the elements that follow the heading
   in the document's contents.
5. **Source files:** a section can span code and doc blocks. It holds the rest
   of the heading's doc block, every code and doc block after it, and, in the
   doc block holding the heading that ends the section, the content before that
   heading. Lines of code in a collapsed section are hidden, not deleted.
6. **Never saved:** collapsing doesn't change the document's text. The icons
   and the collapsed state never appear in the HTML the Client turns back into
   Markdown, in the updates it sends to the Server, or in the saved file.
   Saving a document with sections collapsed gives the same file as saving it
   with every section shown.
7. **Shown when needed:** when the cursor or a scroll position from the IDE
   lands inside a collapsed section, that section and any collapsed sections
   holding it are expanded, so the Client never moves to text the reader can't
   see.
8. **Edits:** a section follows its heading as the document changes. If the
   heading is deleted or stops being a heading, its section is shown again.
   Changing a heading's level changes where its section ends.
9. **Lifetime:** collapsed state lasts while the file is open in the Client.
   An update from the IDE keeps the state of the headings that still exist;
   opening a file shows every section.

Out of scope
------------

* Remembering collapsed sections after the file is closed or reloaded.
* Commands to collapse or expand every section at once.
* Headings inside lists, block quotes and tables, which aren't headings by the
  definition above.
* Folding code by its syntax, which CodeMirror already offers and which stays
  as it is.

Examples
--------

`H1 A` is a level-1 heading with text `A`; `p1` is a paragraph; `code1` is a
code block. Each example starts with every section shown.

| #  | Document                                          | Action                          | Shown afterwards                                |
| -- | ------------------------------------------------- | ------------------------------- | ----------------------------------------------- |
| 1  | `H1 A`, `p1`, `H2 B`, `p2`, `H1 C`, `p3`          | Collapse A                      | `H1 A`, `H1 C`, `p3`                            |
| 2  | Same as 1                                         | Collapse B                      | `H1 A`, `p1`, `H2 B`, `H1 C`, `p3`              |
| 3  | Same as 1                                         | Collapse B, collapse A, expand A | `H1 A`, `p1`, `H2 B`, `H1 C`, `p3` (B stays collapsed) |
| 4  | `H2 A`, `p1`, `H3 B`, `p2`                        | Collapse A                      | `H2 A` (the section runs to the end)            |
| 5  | A list holding `H2 X`                              | (none)                          | No icon beside `X`                              |
| 6  | Source file: doc block [`H1 A`, `p1`], `code1`, doc block [`H2 B`], `code2`, doc block [`H1 C`] | Collapse A | Doc block [`H1 A`], doc block [`H1 C`] |
| 7  | Source file: doc block [`H2 A`, `p1`], `code1`, doc block [`p2`, `H2 B`, `p3`] | Collapse A | Doc block [`H2 A`], doc block [`H2 B`, `p3`] |
| 8  | Same as 1, A collapsed                            | Save                            | The saved file equals the file saved with A shown |
| 9  | Same as 6, A collapsed                            | The IDE moves the cursor into `code1` | Every section shown; the cursor in `code1` |
| 10 | Same as 1, A collapsed                            | Delete the heading `A`          | `p1`, `H2 B`, `p2`, `H1 C`, `p3`                |

Implementation
--------------

* **Markdown documents** ([CodeChatEditor.mts](CodeChatEditor.mts)): the whole
  document is one TinyMCE editor. Add each icon as an element TinyMCE leaves
  out of its content (`data-mce-bogus="all"`), and hide a section's elements
  with a marker TinyMCE also drops when it serializes, so `getContent` and the
  saved Markdown don't change (rule 6). Check both against TinyMCE's
  serializer before relying on them.
* **Source files** ([CodeMirror-integration.mts](CodeMirror-integration.mts)):
  keep the collapsed headings in a `StateField`, as `docBlockField` keeps doc
  blocks, so their positions map through edits (rule 8). Hide the lines of a
  section with a block `Decoration.replace`, and hide the elements before and
  after a heading inside a doc block from `DocBlockWidget.toDOM` and
  `updateDOM`, which redraw a doc block.
* **Cursor and scroll** (rule 7): expand the sections around a position before
  the Client moves its cursor or scrolls there for the IDE.
* **Tests:** a Client test for each example in
  [CodeChatEditor-test.mts](CodeChatEditor-test.mts); the browser tests in
  [overall.rs](../../server/tests/overall.rs) are slow, so use them only for
  examples 8 and 9, which need the Server.
