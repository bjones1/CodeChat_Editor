Copyright (C) 2023 Bryan A. Jones.

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
[http://www.gnu.org/licenses/](http://www.gnu.org/licenses/).

Changelog
=========

*   [Github master](https://github.com/bjones1/CodeChat_Editor):
    *   Better support for opening a page in a web browser.
*   v0.1.21, 2025-Jul-18:
    *   Allow specifying the host address the server binds to.
    *   Send server logs to the console by default.
*   v0.1.20, 2025-Jul-18:
    *   Correct data corruption in Client on delete/insert diff operations.
*   v0.1.19, 2025-Jul-17: 
    *   Correctly apply diffs to Client document.
    *   Avoid deleting adjacent doc blocks.
    *   Correct error where edits in the IDE were ignored.
    *   Provide in-browser feedback on Mermaid errors.
    *   Fix word wrapping in Mermaid diagrams in lists.
*   v0.1.18, 2025-Jul-14:
    *   Send diffs to the Client, which prevents visual jitter.
*   v0.1.17, 2025-Apr-14:
    *   Fix heading level 3 and below word wrapping.
    *   Correctly handle unclosed fenced code blocks.
*   v0.1.16, 2025-Apr-11:
    *   Fix to allow running inside a GitHub Codespace.
    *   Add: new command-line option to open a file/directory --
        `codechat-editor-server start [filename/diretory]`.
*   v0.1.15, 2025-Mar-31:
    *   Correctly view binary files (images, PDFs, etc.) within a project.
    *   Include support for viewing PDF files in VSCode.
*   v0.1.14, 2025-Mar-13:
    *   Correct translation of leading slash in Linux/OS X paths to/from a URL.
        This fixes rewrites of URL in Markdown to long relative paths.
*   v0.1.13, 2025-Mar-10:
    *   Show PDFs in the browser, instead of downloading them.
    *   Update to new release of Actix framework.
*   v0.1.12, 2025-Mar-08:
    *   Fixed error when creating a new document in VSCode.
    *   Fixed error when updating a non-CodeChat Editor document in VSCode.
*   v0.1.11, 2025-Feb-27:
    *   Fixed data corruption while editing math: typeset math, instead of LaTeX
        source, was saved to the source file. Now, math is untypeset during
        edits, then retypeset afterwards.
    *   Correctly handle webview shutdown in VSCode extension.
*   v0.1.10, 2025-Feb-20:
    *   Update to the 2024 edition of Rust.
    *   Update dependencies.
    *   Update source formatting using current CodeChat Editor.
*   v0.1.9, 2025-Jan-20:
    *   Correct word wrapping inside Mermaid diagrams.
    *   Correct translation after adding newlines to code blocks in the Editor.
    *   Use setext headings, different symbols for bullets.
    *   Drop prettier for word wrap.
*   v0.1.8, 2025-Jan-12:
    *   Correctly handle file not found in VSCode.
    *   Correct filename handling on Windows.
*   v0.1.7, 2025-Jan-08:
    *   Fixed hyperlink navigation.
    *   Fixed case-insensitive filename handling bugs.
    *   Improve filename handling in tests.
*   v0.1.6, 2024-Dec-29:
    *   Improvements to the build tool.
    *   Corrections to the C parser.
*   v0.1.5, 2024-Dec-21:
    *   Improvements to the build tool and tests.
    *   Fixed filewatcher bugs.
*   v0.1.4, 2024-Dec-19:
    *   Added support for [Mermaid](https://mermaid.js.org/).
    *   Fixed MathJax packaging.
    *   Resize large images to fit in browser.
    *   Switch to new parser for Python and C/C++.
    *   Correct styles so that the selection and current line are visible.
    *   Created a build tool to automate the build process and added CI checks.
    *   Fixed OS-specific warnings and bugs.
    *   Fixed filewatcher bugs.
*   v0.1.3, 2024-Nov-18:
    *   Switch to using MathJax 4 beta; load MathJax in the frame it's used, per
        [MathJax issue #3309](https://github.com/mathjax/MathJax/issues/3309).
    *   Modernize graphviz-webcomponent build.
    *   Move CSS to `client/src`.
*   v0.1.2, 2024-Nov-12:
    *   Fix [issue #28](https://github.com/bjones1/CodeChat_Editor/issues/28),
        autosave failures.
    *   Fix filewatcher -- incorrect file path comparison.
    *   Fix errors saving mathematics in Markdown-only files.
    *   Improve spellchecking coverage.
    *   Fix data loss when the CodeChat Editor Client is not visible.
*   v0.1.1, 2024-Nov-04:
    *   Added basic theme support; used a theme similar to Sphinx's Alabaster.
    *   Added support for printing.
    *   Added a user manual, improved documentation.
    *   Fixed a bug in the filewatcher that prevented saving changes made in the
        Client.
    *   Added math support.
    *   Removed save button.
    *   Added support for Kotlin.
    *   Updates to the build system.
    *   Cross-platform fixes.
*   v0.1.0, 2024-Oct-16:
    *   Initial release, with binaries for Windows only. Built with
        manually-patched CodeMirror per [this
        issue](https://github.com/bjones1/CodeChat_Editor/issues/27).