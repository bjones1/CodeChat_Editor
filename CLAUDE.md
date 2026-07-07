Instructions for Claude Code
============================

Code blocks and doc blocks
--------------------------

The CodeChat Editor divides source code into code blocks and documentation (doc)
blocks. These blocks are separated by newlines. A code block consists of all
lines in a source file which aren't classified as a doc block. Note that code
blocks may consist entirely of a comment, as illustrated below.

A doc block consists of a comment (inline or block) optionally preceded by
whitespace and optionally succeeded by whitespace. At least one whitespace
character must separate the opening comment delimiter from the doc block text.
Doc blocks are differentiated by their indent: the whitespace characters
preceding the opening comment delimiter. Adjacent doc blocks with identical
indents are combined into a single, larger doc block.

```c
// This is all one doc block, since only the preceding
//   whitespace (there is none) matters, not the amount of
// whitespace following the opening comment delimiters.
  // This is the beginning of a different doc
  // block, since the indent is different.
    // Here's a third doc block; inline and block comments
    /* combine as long as the whitespace preceding the comment
delimiters is identical. Whitespace inside the comment doesn't affect
       the classification. */
// These are two separate doc blocks,
void foo();
// since they are separated by a code block.
```

Architecture
------------

A Visual Studio Code extension in `extensions/VSCode` exchanges messages with the CodeChat Editor Server, located in `server/` (also terms the Server), which also exchanges message with the CodeChat Editor Client (also termed the Client) located in `client/`.

Project build
-------------

All build commands must be executed from the `server/` directory.

* To build the entire project, execute `./bt build`.
* To build (bundle) only the Client, execute `./bt client-build`.
* To run tests, execute `cargo test`.
