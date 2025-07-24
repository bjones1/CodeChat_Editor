// `style_guide.cpp` - Literate programming using the CodeChat Editor
// ==================================================================
//
// This document, written as a C++ source file, primarily demonstrates the use
// of the CodeChat Editor in literate programming. It should be viewed using the
// CodeChat Editor.
//
// Copyright (C) 2025 Bryan A. Jones.
//
// This file is part of the CodeChat Editor. The CodeChat Editor is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, either
// version 3 of the License, or (at your option) any later version.
//
// The CodeChat Editor is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along with
// the CodeChat Editor. If not, see
// [http://www.gnu.org/licenses](http://www.gnu.org/licenses).
//
// Introduction
// ------------
//
// This document provides a style guide for literate programming using the
// CodeChat Editor. For basic use, see the [user manual](../README.md).
//
// The CodeChat Editor divides code into code blocks and documentation (doc)
// blocks.​ These blocks are separated by newlines; comments on the same line as
// code are not interpreted doc blocks.​ Doc blocks must have 1 space after the
// comment delimiter.​ For example, this paragraph is a doc block;

const char* CODE_BLOCK =
    "this is a code block."; // Comments here are NOT part of a doc block.
//Likewise, comments without a space following the comment delimiter are
//not part of a doc block.

    // Each doc block has an associated indent;
  // doc blocks with differing indents cannot be combined.
/* Doc blocks may use either inline comments (`//` in C++) or block comments
   (like this comment). Doc blocks with differing delimiters cannot be combined. */
// Doc blocks are interpreted using Markdown (specifically, 
// [CommonMark](https://commonmark.org/)), enabling the use of headings,
// *emphasis*, **strong emphasis**, `monospaced fonts`, and much more; see a
// [brief overview of Markdown](https://commonmark.org/help/).
//
// Approach
// --------
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
// snippets, documentation, and ideas. ​Update the approach you sketched out as
// your learn what works (and doesn't) through the development process. Explain
// any pieces of code that took significant development or debug time, or which
// contain difficult to understand code.
//
// **Phase 3 - post-writing.** Re-read what you wrote. Does this still make
// sense?​ Update your overall approach based on what you discover. Get another
// person to review what you wrote, then implement their ideas and suggestions.
//
// <a id="organization"></a>Organization
// -------------------------------------
//
// The program should use headings to appropriately organize the contents. Near
// the top of the file, include a single level-1 heading, providing the title of
// the file; per the HTML spec, there should be [only one level-1
// heading](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/Heading_Elements#avoid_using_multiple_h1_elements_on_one_page).
// For source files, include the file name at the beginning of the title, in a
// monospaced font.
//
// Following the title, include additional heading levels; [don't skip
// levels](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/Heading_Elements#navigation),
// e.g. by placing a level-3 heading immediately following a level-1 heading.
// Use headings to provide a natural outline of your program. The [end of this
// document](#org-style) provides the recommended organizational style.
//
// Location
// --------
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
};

// Use of mathematics
// ------------------
//
// Formulas should be placed near code that implements them, along with good
// explanations of the equations used. For example:
//
// This function computes an accurate value for $g$, the acceleration due to
// Earth's gravity.
//
// Return value: $g$, in $m/s^2$.
double accurate_g(
    // Latitude, in degrees.
    double degrees_latitude,
    // Height above sea level, in meters.
    double height_meters
) {
    // This text comes from the [SensorsOne Local Gravity
    // Calculator](https://www.sensorsone.com/local-gravity-calculator/). For
    // more detail, see [Theoretical
    // Gravity](https://en.wikipedia.org/wiki/Theoretical_gravity).
    //
    // The formulas used by this function are based on the [International
    // Gravity Formula IGF)
    // 1980](https://en.wikipedia.org/wiki/Normal_gravity_formula#International_gravity_formula_1980) 
    // from the parameters of the [Geodetic Reference System 1980
    // (GRS80)](https://en.wikipedia.org/wiki/GRS_80), which determines the
    // gravity from the position of latitude, and the [Free Air Correction
    // (FAC)](https://en.wikipedia.org/wiki/Gravity_of_Earth#Free_air_correction)
    // which corrects for height above and below mean sea level in free air.
    //
    // Compute the International Gravity Formula (IGF):\
    // $IGF = 9.780327 (1 + 0.0053024 \\sin^2 \\phi – 0.0000058 \\sin^2 2\\phi)$
    double IGF = 9.780327 * (
        1 + 0.0053024 * pow(sin(degrees_latitude), 2)
        - 0.0000058 * pow(sin(2 * degrees_latitude), 2)
    );
    // Compute the Free Air Correction (FAC):\
    // $FAC = -3.086 \\cdot 10^{-6} h$
    double FAC = -3.086E-6 * height_meters;
    // $g = IGF + FAC$
    return IGF + FAC;
    // Symbols:
    //
    // *   $g$ = Theoretical local gravity, in $m/s^2$.
    // *   $\\phi$ = Latitude, in decimal degrees.
    // *   $h$ = Height relative to sea level, in $m$.
}

// Excellence in code
// ------------------
//
// Literate programming should be accompanied by excellence in authoring code.
// Specifically:
//
// *   Use meaningful, descriptive names for variables, classes, functions, etc.
//     Doc blocks should only supply what [self-documenting
//     code](https://en.wikipedia.org/wiki/Self-documenting_code) cannot --
//     design choices, purpose, etc.
// *   Be consistent; preferably, use a [code
//     formatter](https://en.wikipedia.org/wiki/Prettyprint#Programming_code_formatting)
//     to ensure this consistency.
// *   Employ [DRY](https://en.wikipedia.org/wiki/Don%27t_repeat_yourself)
//     principles.
// *   Address warnings, not only errors; preferably, use a 
//     [linter](https://en.wikipedia.org/wiki/Lint_\(software\)).
// *   Write automated tests; employ [test-driven
//     development](https://en.wikipedia.org/wiki/Test-driven_development).
//
// Editor configuration
// --------------------
//
// Properly configuring the text editor used with the CodeChat Editor
// significantly improves the authoring process. Recommended settings:
//
// *   Enable word wrap:
//     [vscode](https://learn.microsoft.com/en-us/visualstudio/ide/reference/how-to-manage-word-wrap-in-the-editor?view=vs-2022)
// *   Use spaces, not tabs​, for indentation:
//     [vscode](https://code.visualstudio.com/docs/editor/codebasics#_indentation)
// *   Enable auto-save:
//     [vscode](https://code.visualstudio.com/docs/editor/codebasics#_save-auto-save)
// *   Auto-reload enabled​: default in vscode
// *   On save, remove trailing whitespace​:
//     [vscode](https://stackoverflow.com/a/53663494/16038919)
// *   Use a spell checker:
//     [vscode](https://marketplace.visualstudio.com/items?itemName=streetsidesoftware.code-spell-checker)
// *   On a big monitor, place your IDE side by side with the CodeChat Editor.
//
// Common problems
// ---------------
//
// *   Don't drag and drop an image into the Editor – this creates a mess.
//     Instead, save all images to a file, then use an SVG or PNG image for
//     text/line art​ or a JPEG image for photos​. The Markdown syntax to insert
//     an image is `![Alt text](https://url.to/image.svg)`.
// *   Indent your comments to match the indentation of nearby code; don't
//     purposelessly vary the comment indentation.
// *   Avoid inserting a one-line empty code block (a blank line) between
//     paragraphs in a doc block; instead, use a single doc block to store
//     multiple paragraphs.
// *   Use minimal formatting. Markdown is a simple, rather limited syntax;
//     however, it is very easy to use and read. While the CodeChat Editor will
//     happily replace simple Markdown constructs with verbose HTML to
//     accomplish the formatting you specify, avoid the resulting <span
//     style="color: #e03e2d;">messy syntax</span> produced by this process.
//     Pasting from an HTML source (such as Word or a web page) directly to the
//     CodeChat Editor likewise produces a lot of messy syntax; consider pasting
//     text only, then reformatting as necessary.
//
// ### Commenting out code
//
// Many developers comment out code while testing, or to save a snippet of code
// for later use. When using the CodeChat Editor, **ensure these comments aren't
// interpreted as a doc block**. Otherwise, this commented out code will be
// interpreted as Markdown then rewritten, which almost certainly corrupts the
// code. To avoid this, append extra characters immediately after the opening
// comment delimiter: for example, use `///` or `/**` in C or C++, `##` in
// Python, etc. See also the example at the end of this file, which includes an
// improved alternative to commenting out code using preprocessor directives for
// C/C++.
//
// Example structure
// -----------------
//
// As discussed in [organization](#organization), the remainder of this document
// presents the preferred use of headings to organize source code.
//
// <a id="org-style"></a>Includes
// ------------------------------
//
// Include files (in Python, imports; Rust, use statements; JavaScript,
// require/import, etc.) should be organized by category; for example, [PEP
// 8](https://peps.python.org/pep-0008/#imports) recommends the following
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

// Global variables/constants
// --------------------------
//
// Use units when describing physical quantities. For example, this gives the
// acceleration due to gravity in $m/s^2$.
const double accel_m_s2 = 9.8067;

// Macros
// ------
#define LED1 (LATB16)

// Structures/classes
// ------------------
class BlinkLed {
};

// Code
// ----
int main(int argc, char* argv[]) {
    // Here's an example of commenting code out when using the CodeChat Editor:
    /**
     *  foo();
     */
    // However, when using C/C++, macros provide a nestable way to comment out
    // code that may contain block comments (which aren't nestable in
    // standardized C/C++):
    #if 0
    /* This block comment doesn't end the commented-out code. */
    foo();
    #endif

    return 0;

}
