SYSTEM NOTE: Agents must ignore this section and proceed directly to the
"Instructions for LLMs" section below.

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

Instructions for LLMs
=====================

Code blocks and doc blocks
--------------------------

The CodeChat Editor divides source code into code blocks and documentation (doc)
blocks. These blocks are separated by newlines. A code block consists of all
lines in a source file which aren't classified as a doc block. Note that code
blocks may consist entirely of a comment, as illustrated below.

A doc block consists of a comment (inline or block) optionally preceded by
whitespace and optionally succeeded by whitespace. At least one whitespace
character must separate the opening comment delimiter from the doc block text.
Doc blocks are differentiated by their indent -- the whitespace characters
preceding the opening comment delimiter -- and by the opening comment delimiter
itself. Adjacent doc blocks are combined into a single, larger doc block only
when both their indent and their delimiter match.

```c
// This is all one doc block, since only the preceding
//   whitespace (there is none) matters, not the amount of
// whitespace following the opening comment delimiters.
  // This is the beginning of a different doc
  // block, since the indent is different.
    // Here's a third doc block.
    /* And here's a fourth: a different opening delimiter starts a new doc
       block even at an identical indent. Whitespace inside the comment
       doesn't affect the classification. */
// These are two separate doc blocks,
void foo();
// since they are separated by a code block.
```

<h3 id="cc-sZ0Glsgjhu">Cross-references, gather elements, and fragments</h3>

To refer to text in another section of the current document or any file in this
project:

1. Assign a unique id for that section: `<h3 id="some_unique_id">section
   title</h3>` for a level-3 section; in a code file, this must be placed in a
   doc block. Any HTML element with an id can be the target of either a
   hyperlink or a cross-reference.
2. Refer to it using a hyperlink or cross-reference. For example, the following
   syntax links directly to this section's heading: `[Foo](#cc-sZ0Glsgjhu)`
   (link text is "Foo") / `<xref ref="cc-sZ0Glsgjhu"></xref>` (link text comes
   from the cross-referenced span: "Cross-references, gather elements, and
   fragments"); the same syntax (a bare id with no file) also links to any file
   in the project.

When implementing a specification:

1. Label each doc block preceding non-contiguous section of code of more than
   three lines with a fragment placed at the end of the doc block: `<fragment
   id="some_unique_id1"></fragment>`.
2. Gather these fragments in a subheading following the specification: `<h4
   id="another_unique_id" data-gather="some_unique_id1 some_unique_id2
   ...">Implementation of [title of this section]</h4>`.

Each id must be unique throughout the entire project. To create an id:

1. Randomly generate an id of the form `cc-[10 random ASCII letters or
   numbers]`.
2. grep the project to ensure this id is unique.

Architecture
------------

A Visual Studio Code extension in `extensions/VSCode` or the standalone server
in `extensions/standalone` exchanges messages with the CodeChat Editor Server,
located in `server/` (also termed the Server), which also exchanges messages
with the CodeChat Editor Client (also termed the Client) located in `client/`.

Project build
-------------

Execute these build commands, which run the [`bt`](#cc-1s8bUScadp) script, from
the root of the project.

* To build the entire project, execute `./bt build`.
* To format and lint the entire project, execute `./bt flint`.
* To run all tests, execute `./bt test`.

Commenting guide
----------------

This program uses a literate programming approach: comments are prose meant to
be read from top to bottom alongside the code, not annotations bolted onto
individual statements.

`docs/style_guide.cpp` is the project's canonical, reader-facing style guide;
this section is a digest of it aimed at the Rust and TypeScript in this repo.
When asked to change "the style guide", edit that file, and keep the two
consistent.

### What to comment

* Use meaningful, descriptive names for variables, classes, functions, etc. Code
  should be as self-documenting as possible.

* Avoid comments when possible. Comments must describe current code, not its
  history. Only add comments which supply what self-documenting code cannot.

* Comments must satisfy at least one of these criteria:

  1. Document a connection which cannot easily be determined by inspection --
     for example, the relationship between a web client HTTP request and the
     Server endpoint which handles it.
  2. Record behavior discoverable only by running or debugging the code: a
     third-party library quirk, a browser workaround, an ordering constraint.
     Behavior which can be derived directly from the code should not produce a
     comment.
  3. Capture design choices, requirements, etc. which specify the overall
     purpose of the code at a higher level than the implementation.
  4. Link to an external reference (a manual, specification, etc.) which
     explains a subtle design choice.
* Do not restate the code. Say what the item does, then add what the reader
  cannot derive from the signature:

  ```rust
  // Bad -- the signature already says this.
  /// Set the file's contents to the given string.
  fn set_contents(&mut self, contents: String);

  // Good -- keeps the summary, adds the constraint.
  /// Replace the file's contents. The Client autosaves, so this runs on every
  /// pause in typing; keep it cheap and idempotent.
  fn set_contents(&mut self, contents: String);
  ```

* Scale a comment's length to its altitude instead of aiming for uniform
  brevity: file- and section-level comments carry the design narrative and may
  run for dozens of lines (see `server/src/processing/cache.rs`), while a
  comment on a single statement is a line or two.

* In new work, mark deferred work with `TODO:` followed by both the task and the
  condition which will resolve it, so a later reader knows when it may be
  removed. Many existing `TODO`s predate this rule; leave them alone unless you
  are already editing that code.

* When implementing equations, place a comment giving the formula, along with an
  explanation of the terms used, before the code implementing it. Use
  LaTeX-style syntax: $x^2$.

### Where to place comments

Place documentation before the code it describes. In new and edited code,
comment each function parameter individually, and document the return value with
a comment placed immediately before the return type -- including in Rust, where
rustdoc renders neither. Many existing signatures are not yet annotated this
way; add annotations to signatures you touch rather than sweeping the tree:

```rust
/// Phase 1 of hydration: parse the HTML, walk the DOM, then commit the
/// collected facts to the cache.
fn hydrate_dom(
    // The HTML to hydrate.
    html: &str,
    // The cache for the project containing this file.
    cache: &Arc<Mutex<Cache>>,
    // The parsed, patched DOM plus the walk results needed by later phases.
) -> io::Result<(Rc<Node>, WalkContext)> {
```

### Doc block constraints

Because comments become doc blocks, these mechanical rules apply:

* Hold the indent *and* the delimiter constant across a comment's lines, and
  match the indent to that of the code being described.
* Denote paragraphs using an empty comment (`//` or `///`), not an empty line.
* A comment on the same line as code is never a doc block; it stays part of the
  code block.
* When commenting out code, use `/*** ... */` to prevent the code from being
  treated as a doc block.

### File structure

Source files follow this template (a few predate it; match it in new files):

1. Crate-level attributes (Rust only).
2. The GPL copyright and license block, copied verbatim from a neighboring file
   rather than retyped. In Rust it uses plain `//`, so it stays a doc block
   separate from the title which follows it.
3. A single level-1 heading titling the file: the file name in backticks --
   matching the file's actual name -- then `--`, then a short description.
4. The file-level design narrative, if the file needs one.
5. A `Modules` section, if the file has submodules, then an `Imports` section.
   In Rust its subsections are `### Standard library`, `### Third-party`, and
   `### Local`; the Client and the VSCode extension use their own names, so
   follow the file you are editing.
6. The code, organized under further headings which outline the file. Don't skip
   heading levels.

Employ Markdown syntax throughout. Use setext underlines (`====` and `----`) for
heading levels 1 and 2, and ATX markers (`###`, `####`) for level 3 and below;
underline the full width of the heading text. Use asterisks for bullets,
emphasis, and strong emphasis. Wrap lines at 80 characters; if the indent
exceeds column 40, wrap at 40 columns past the indent instead of at column 80.

```rust
//! `cache.rs` -- Keep a cache used to store all targets in a project
//! =================================================================
//!
//! The cache stores the location and contents of every target in a project...

// Imports
// -------
//
// ### Standard library
use std::collections::HashMap;
```

### Rust specifics

* Write file-level prose -- the title heading and the design narrative -- with
  `//!`, so that it documents the module.
* Use `///` where allowed by rustdoc (functions, structs, enums, traits, and
  macros); use `//` otherwise.

### TypeScript specifics

* Use `//` comments instead of `/**` blocks.
* Document parameters as described above instead of using `@param` tags.

### Editing existing comments

Comments describe the code as it now stands. When changing code inside a
documented region, revise the surrounding prose so that it still reads as a
continuous narrative: do not append a new comment beside text which the change
has made stale, and delete comments describing code which no longer exists.
