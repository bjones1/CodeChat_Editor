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

Code style
----------

This program uses a literate programming approach to improve the overall
comprehensibility of the code. Guidelines:

* Use meaningful, descriptive names for variables, classes, functions, etc. Code
  should be as self-documenting as possible.
* Use headings in comments to appropriately organize the contents. Near the top
  of the file, include a single level-1 heading, providing the title of the
  file. For source files, include the file name at the beginning of the title,
  in a monospaced font. Following the title, include additional heading levels
  to provide a natural outline of the code.

* Comments in the code should only supply what self-documenting code cannot:
  1. Document a connection which cannot easily be determined by inspection --
     for example, explaining the relationship between a web client HTTP request
     and the backend server endpoint which handles it.
  2. Explain behavior which can only be determined by run-time inspection or
     debugging; behavior which can be directly derived from the code should not
     produce a comment.
  3. Capture design choices, requirements, etc. which specifies the overall
     purpose of the code at a higher level than the implementation.
  4. Provides a link to external references (a manual, specification, etc.) to
     explain a subtle design choice.
* Place comments with formulas near code that implements them, along with good
  explanations of the equations used, using LaTeX-style syntax: $x^2$.
* Use units when describing physical quantities:

  ```C++
  // The acceleration due to gravity in $m/s^2$.
  const double accel_m_s2 = 9.8067;
  ```

* Place documentation before the corresponding code. Precede function parameters
  with descriptive comments. For example:

  ```C++
  // This class blinks an LED based on the number of pushbutton presses recorded.
  class LedBlinker {
      // Store the number of pushbutton presses.
      unsigned int pb_presses;

      // Blink the LED based on the number of pushbutton presses.
      //
      // Returns the number of blinks performed; this value is <= `pb_presses`.
      unsigned int blink_led(
          // The time, in ms, between blinks.
          unsigned int blink_time_ms
      );
  };
  ```
