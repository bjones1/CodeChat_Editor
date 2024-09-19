// # `style_guide.cpp` - Literate programming using the CodeChat Editor
//
// This document, written as a C++ source file, primarily demonstrates the use
// of the CodeChat Editor in literate programming.
//
// ## Introduction
//
// The CodeChat Editor divides code into code blocks and documentation (doc)
// blocks.​ These blocks are separated by newlines; comments on the same line as
// code are not interpreted doc blocks.​ Doc blocks must have 1 space after the
// comment delimiter.​ For example, this is a doc block;

const char* CODE_BLOCK =
    "this is a code block."; // Comments here are NOT part of a doc block.
//Likewise, comments without a space following the comment delimiter are
//not part of a doc block.

    // Each doc block has an associated indent;
  // doc blocks with differing indents cannot be combined.
/* Doc blocks may use either inline comments (`//` in C++) or block comments
   (like this comment). Doc blocks with differing delimiters cannot be combined. */
// Doc blocks are interpreted using Markdown
// (specifically, [CommonMark](https://commonmark.org/)), enabling the use of
// headings, _emphasis_, **strong emphasis**, `monospaced fonts`, and much more;
// see a [brief overview of Markdown](https://commonmark.org/help/).
//
// ## Approach
//
// Viewing a program as a document defines the heart of the literate programming
// paradigm. A program/document -- constructed as a series of code blocks and
// doc blocks -- provides unique opportunities to write better programs, by
// interleaving code with explanation. Specifically, think of the process of
// writing a program/document in three phases:
//
// **Phase 1 - pre-writing.** Before writing code, record your ideas in doc
// blocks. What is the purpose of this code? How can it best be expressed or
// explained? Use pseudocode, block diagrams, flowcharts, truth tables, etc. to
// visually capture your idea. Write down the expected inputs, expected outputs,
// and sketch out an approach to produce the desired outputs from the provided
// inputs. Anticipate any corner cases or problems your approach must correctly
// handle.
//
// **Phase 2 - writing.** As you write code, save links to helpful code
// snippets, documentation, and ideas.​Update the approach you sketched out as
// your learn what works (and doesn't) through the development process. Explain
// any pieces of code that took significant development or debug time, or which
// contain difficult to understand code.
//
// **Phase 3 - post-writing.** Re-read what you wrote. Does this still make
// sense?​ Update your overall approach based on what you discover. Get another
// person to review what you wrote, then implement their ideas and suggestions.
//
// ## <a id="organization"></a>Organization
//
// The program should use headings to appropriately organize the contents. Near
// the top of the file, include a single level-1 heading, providing the title of
// the file; per the HTML spec, there should be
// [only one level-1 heading](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/Heading_Elements#avoid_using_multiple_h1_elements_on_one_page).
// For source files, include the file name at the beginning of the title, in a
// monospaced font.
//
// Following the title, include additional heading levels;
// [don't skip levels](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/Heading_Elements#navigation),
// e.g. by placing a level-3 heading immediately following a level-1 heading.
// Use headings to provide a natural outline of your program. The
// [end of this document](#org-style) provides the recommended organizational
// style.
//
// ## Location
//
// In general, place documentation before the corresponding code. For example:
//
// This class blinks an LED based on the number of pushbutton presses recorded.
class LedBlinker {
    // Store the number of pushbutton presses.
    unsigned int pb_presses;

    // Blink the LED based on the number of pushbutton presses; stop blinking if
    // the pushbutton is pressed or released while the LED is blinking.
    //
    // Returns the number of blinks performed; this value is <= `pb_presses`.
    unsigned int blink_led(
        // The time, in ms, between blinks.
        unsigned int blink_time_ms
    );
}

// ## Editor configuration
//
// Properly configuring the text editor used with the CodeChat Editor
// significantly improves the authoring process. Recommended settings:
//
// - Enable word wrap:
//   [vscode](https://learn.microsoft.com/en-us/visualstudio/ide/reference/how-to-manage-word-wrap-in-the-editor?view=vs-2022)
// - Use spaces, not tabs​, for indentation:
//   [vscode](https://code.visualstudio.com/docs/editor/codebasics#_indentation)
// - Enable auto-save:
//   [vscode](https://code.visualstudio.com/docs/editor/codebasics#_save-auto-save)
// - Auto-reload enabled​: default in vscode
// - On save, remove trailing whitespace​:
// - On a big monitor, place your IDE side by side with the CodeChat Editor.
//
// ## Common problems
//
// - Don't drag and drop an image into the Editor – this creates a mess.
//   Instead, save all images to a file, then use an SVG or PNG image for
//   text/line art​ or a JPEG image for photos​. The Markdown syntax to insert
//   an image is `![Alt text](https://url.to/image.svg)`.
// - Indent your comments to match the indentation of nearby code; don't
//   purposelessly vary the comment indentation.
// - Avoid inserting a one-line empty code block (a blank line) between
//   paragraphs in a doc block; instead, use a single doc block to store
//   multiple paragraphs.
// - Use minimal formatting. Markdown is a simple, rather limited syntax;
//   however, it is very easy to use and read. While the CodeChat Editor will
//   happily replace simple Markdown constructs with verbose HTML to accomplish
//   the formatting you specify, avoid the resulting
//   <span style="color: #e03e2d;">messy syntax</span> produced by this process.
//   Pasting from an HTML source (such as Word or a web page) directly to the
//   CodeChat Editor likewise produces a lot of messy syntax; consider pasting
//   text only, then reformatting as necessary.
//
// ## Example structure
//
// As discussed in [organization](#organization), the remainder of this document
// presents the preferred use of headings to organize source code.
//
// ## <a id="org-style"></a>Includes
//
// Include files (in Python, imports; Rust, use statements; JavaScript,
// require/import, etc.) should be organized by category; for example,
// [PEP 8](https://peps.python.org/pep-0008/#imports) recommends the following
// categories:
//
// ### Standard library
#include <stdio>
#include <stdlib>

// ### Third-party
#include <boost/circular_buffer.hpp>

// ### Local
//
// Note: This is a fictitious file, here for example only.
#include <style_guide.hpp>

// ## Global variables/constants
//
// Use units when describing physical quantities. For example, this gives the
// acceleration due to gravity in m/s^2.
const double accel_m_s2 = 9.8067;

// ## Macros
#define LED1 (LATB16)

// ## Structures/classes
class BlinkLed {
};

// ## Code
int main(int argc, char* argv[]) {
    return 0;
}