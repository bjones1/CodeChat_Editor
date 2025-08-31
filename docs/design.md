CodeChat Editor design
======================

To build from source
--------------------

1.  Clone or download the repository.
2.  [Install the Rust language](https://www.rust-lang.org/tools/install). I
    recommend the 64-bit toolset for Windows.
3.  [Install
    NPM](https://docs.npmjs.com/downloading-and-installing-node-js-and-npm) (the
    Node.js package manager).
4.  In the `server/` directory:
    1.  Run `./bt install --dev`.
    2.  Run `./bt build`.
    3.  Run `cargo run -- start ../README.md`.

Use `./bt` tool's options update all libraries (`update`), run all tests
(`test`), and more.

<a id="vision"></a>Vision
-------------------------

These form a set of high-level requirements to guide the project.

*   View source code as <a id="vision-code-blocks-and-doc-blocks"></a>[code
    blocks and doc blocks](index.md#code-blocks-and-doc-blocks). Doc blocks are
    lines of source which contain only correctly-formatted comments.
*   Provide support for a <a id="vision-programming-language-support"></a>[wide
    variety of programming languages](index.md#programming-language-support).
*   Provide integration with a <a id="vision-ide-integration"></a>[wide variety
    of IDEs/text editors](index.md#ide-integration).
*   Load a document from source code, allow edits in a GUI, then save it
    back to source code.
    *   Provide word processor GUI tools (insert hyperlink, images, headings,
        change font, etc.) for doc blocks.
    *   Provide text editor/IDE tools (syntax highlighting, line numbers, show
        linter feedback) for code blocks.
*   Zero build: eliminate the traditional project build process -- make it
    almost instantaneous.
*   Doc block markup should be readable and well-known: markdown.
*   Support both a single-file mode and a project mode.
    *   A project is a specific directory tree, identified by the presence of a
        TOC. A TOC is just a plain Markdown file with a specific name. A better
        term: not a TOC, but a navigation pane, since the TOC can contain
        anything (see below).
    *   A page in a project build is a single-file page plus:
        *   A TOC, along with previous/next/up navigation. The TOC is
            synchronized to the current page.
        *   Numbering comes from the current page's location within the TOC.
            Pages not in the TOC aren't numbered.
*   <a id="authoring-support"></a>Provide [authoring
    support](index.md#authoring-support), which allows authors to easily
    create book/project-like features. In particular:
    *   Counters for numbering figures, tables, equations, etc. All counters are
        page-local (no global counters).
    *   Auto-titled links: the link text is automatically derived from the
        link's destination (the heading text at the link's destination; a
        figure/table caption, etc.).
    *   Auto-generated back links: anchors support auto-generated links back to
        all their referents, which can be used for footnotes, endnotes,
        citations, and indices. To enable this, all forward links must include
        an anchor and optionally the text to display at the target.
    *   TOC support which:
        *   Given some file(s), expands to a nested list of headings in the
            file(s). Authors may specify the depth of headings to include.
        *   Show the filesystem, optionally not including files that are linked
            in th TOC.
        *   Show a list of all links.
        *   Since it's a plain Markdown file, this could include pretty much
            anything: a list of index entires; a temporally-sorted list of
            pages; an image with links based on a map/diagram; etc.
        *   Tracking support: auto-scrolls the TOC to the first instance of a
            link to the currently viewed file, and tracks headings within the
            current file.
    *   A gathering element: given an anchor, it shows the context of all
        hyperlinks to this anchor.
        *   If the hyperlink is a heading, the context extends to the next
            same-level heading;
        *   If the hyperlink is a start of context, the context ends at the end
            of context or end of file, whichever comes first.
        *   Otherwise, the context extends to the following code block.
    *   A report view: an extended gathering element that operates more like a
        query, producing nested, hierarchical results from the codebase.
    *   Headings can be collapsed, which code code and doc blocks until the next
        same-level heading.
    *   A sequencing/path element: given a starting hyperlink, it produces
        prev/next icons to show a startup/shutdown sequence, etc.
    *   A graph view: shows the entire document as a directed graph of
        hyperlinks.
    *   An inlined output mode, like Jupyter: includes graphs and console output
        produced by executing the code.
    *   Graphical code views:
        *   Present a case statement as a state machine.
        *   Present if/else statements as a truth table.
        *   Visualize data structures.
        *   More?
    *   Interactive learning support: multiple choice, fill-in-th-blank,
        short/long answer, coding problem, etc. from Runestone or similar.
    *   Autogenerated anchors for all anchors (headings, hyperlinks, etc.)
    *   Hyperlinks to identifiers in code (use
        [ctags](https://github.com/universal-ctags/ctags)); perhaps
        auto-generate headings for these identifiers?
    *   An API view; show only parts of the code that's
        exported/publicly-accessible.
    *   Substitutions.
    *   Files/anchors can be freely moved without breaking links. This requires
        all anchors to be globally unique. HTML allows upper/lowercase ASCII
        plus the hyphen and underscore for IDs, meaning that a 5-character
        string provides >250 million unique anchors.

*   Make picking a file/anchor easy: provide a searchable, expanded TOC listing
    every anchor.
*   Provide edit and view options. (Rely on an IDE to edit raw source.)

### Nice to have features

*   Simple to install locally; provide a template CodeSpaces repo for web-based
    editing.
*   Support a static build: producing a set of view-only HTML files which don't
    need a server for a project, or a single HTML file outside a project.
*   An API-only view (Doxygen/Javadoc like feature).

<a id="specification"></a>Requirements
--------------------------------------

The requirements expand on the vision by providing additional details.

### <a id="specification-code-blocks-and-doc-blocks"></a>Code blocks and doc blocks

Comments in most programming languages are either inline comments (which are
terminated by a newline) or block comments, which may span multiple lines. In
C/C++, the opening delimiter for an inline comment is `//`. Likewise, `/*` and
`*/` define the opening and closing delimiters for block comments.

This design treats source code on a line-by-line basis. It does not classify at
any deeper granularity -- for example, it does not support a mix of code block
and doc block on the same line.

A code block consists of all lines in a source file which aren't classified as a
doc block. Note that code blocks may consist entirely of a comment, as
illustrated below.

A doc block consists of a comment (inline or block) optionally preceded by
whitespace and optionally succeeded by whitespace. At least one whitespace
character must separate the opening comment delimiter from the doc block text.
Some examples in C:

<pre>void foo(); // This is not a doc block, because these comments are preceded<br>void bar(); // by non-whitespace characters. Instead, they're a code block.<br>//This is not a doc block, because these inline comments lack<br>//whitespace after the opening comment delimiter //. They're also a code block.<br>/*This is not a doc block, because this block comment lacks<br>  whitespace after the opening comment delimiter /*. It's also a code block. */<br>/* This is not a doc block, because non-whitespace <br>   characters follow the closing comment delimiter. <br>   It's also a code block. */ void food();<br><br>// This is a doc block. It has no whitespace preceding the inline<br>// comment delimiters and one character of whitespace following it.<br>  // This is also a doc block. It has two characters of whitespace <br>  // preceding the comment delimiters and one character of whitespace following it.<br>/* This is a doc block. Because it's based on<br>   a block comment, a single comment can span multiple lines. */<br>/* This is also a doc block, even without the visual alignment<br>or a whitespace before the closing comment delimiter.*/<br>  /* This is a doc block<br>     as well. */</pre>

Doc blocks are differentiated by their indent: the whitespace characters
preceding the opening comment delimiter. Adjacent doc blocks with identical
indents are combined into a single, larger doc block.

<pre>// This is all one doc block, since only the preceding<br>//   whitespace (there is none) matters, not the amount of <br>// whitespace following the opening comment delimiters.<br>  // This is the beginning of a different doc<br>  // block, since the indent is different.<br>    // Here's a third doc block; inline and block comments<br>    /* combine as long as the whitespace preceding the comment<br>delimiters is identical. Whitespace inside the comment doesn't affect<br>       the classification. */<br>// These are two separate doc blocks,<br>void foo();<br>// since they are separated by a code block.</pre>

### <a id="implementation-programming-language-support"></a>\[Programming language

support\](index.md#programming-language-support)

Initial targets come from the Stack Overflow Developer Survey 2022's section on
[programming, scripting, and markup
languages](https://survey.stackoverflow.co/2022/#section-most-popular-technologies-programming-scripting-and-markup-languages)
and IEEE Spectrum's [Top Programming Languages
2022](https://spectrum.ieee.org/top-programming-languages-2022).

### <a id="specification-ide-integration"></a>IDE/text editor integration

Initial targets come from the Stack Overflow Developer Survey 2022's section on
[integrated development
environments](https://survey.stackoverflow.co/2022/#section-most-popular-technologies-integrated-development-environment).

There are two basic approaches:

*   Sync with current window (simplest): have an additional IDE window open
    that displays the file currently being edited. This requires:
    *   Auto-save: the CodeChat Editor autosaves any changes made, to keep files
        synced. Have the host IDE auto-save, so that updates get pushed quickly.
    *   Auto-reload: if a the currently-opened file changes, then automatically
        reload it. Have the host IDE do the same.
    *   Current file sync: when the current tab changes, update the CodeChat
        Editor with the new file. Ideally, also sync the cursor position.
*   Switchable editor (better, complex): provide a command to switch the
    current editor with the CodeChat Editor and vice versa. This requires:
    *   To switch from the IDE editor to CodeChat, need to send the text of the
        IDE's editor to CodeChat. For the opposite, need to get the CodeChat
        Editor text and send that to the IDE's editor.
    *   Need to preserve the current cursor location across switches. This is
        harder inside a doc block. An approximate find might be a good option.

Additional features:

*   Smart navigation: following links to a locally-editable file will open that
    file in the current editor, saving any edits before navigating away.
    Following non-local links opens the file in an external browser.
*   Memory: the editor remembers the last cursor location for recently-opened
    files, restoring that on the next file open.

### Zero-build support

The "build" should occur immediately (to any open files) or when when saving a
file (to closed files, which will be updated when they're next opened).
Exception: edits to the TOC are applied only after a save.

### Authoring support

This system should support custom tags to simplify the authoring process. The
GUI must indicate that text enclosed by the tags isn't directly editable,
instead providing an option to edit the underlying tag that produced the text.
When a new tag is inserted, any tag-produced content should be immediately
added.

License
-------

Copyright (C) 2025 Bryan A. Jones.

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