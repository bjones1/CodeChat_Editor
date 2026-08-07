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

A Visual Studio Code extension in `extensions/VSCode` exchanges messages with
the CodeChat Editor Server, located in `server/` (also terms the Server), which
also exchanges message with the CodeChat Editor Client (also termed the Client)
located in `client/`.

Project build
-------------

All build commands must be executed from the `server/` directory.

* To build the entire project, execute `./bt build`.
* To build (bundle) only the Client, execute `./bt client-build`.
* To run tests, execute `cargo test`.

Comments
--------

This program uses a literate programming approach to improve the overall
comprehensibility of the code. Guidelines for comments:

* Functions should be preceded by a comment that summarizes their overall
  purpose. Each parameter should be preceded by a comment briefly explaining its
  purpose; the return value when preset should be preceded by a comment
  explaining the data it carries.
* Data structures should be preceded by a command explaining their purpose; each
  value in the data structure should be preceded by a command explaining its
  role.
* Comments in the code should be limited to those that:
  1. Document a connection which cannot easily be determined by inspection --
     for example, explaining the relationship between a web client Ajax call and
     the backend server which handles it.
  2. Explain behavior which can only be determined by run-time inspection or
     debugging; behavior which can be directly derived from the code should
     produce a comment.
  3. Capture requirements or higher-level behavior which specifies the overall
     purpose of the code at a higher level than the implementation.
