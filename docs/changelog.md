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

# Changelog

- [Github master](https://github.com/bjones1/CodeChat_Editor):
  - Correctly handle file not found in VSCode.
  - Correct filename handling on Windows.
- v0.1.7, 2025-Jan-08:
  - Fixed hyperlink navigation.
  - Fixed case-insensitive filename handling bugs.
  - Improve filename handling in tests.
- v0.1.6, 2024-Dec-29:
  - Improvements to the build tool.
  - Corrections to the C parser.
- v0.1.5, 2024-Dec-21:
  - Improvements to the build tool and tests.
  - Fixed filewatcher bugs.
- v0.1.4, 2024-Dec-19:
  - Added support for [Mermaid](https://mermaid.js.org/).
  - Fixed MathJax packaging.
  - Resize large images to fit in browser.
  - Switch to new parser for Python and C/C++.
  - Correct styles so that the selection and current line are visible.
  - Created a build tool to automate the build process and added CI checks.
  - Fixed OS-specific warnings and bugs.
  - Fixed filewatcher bugs.
- v0.1.3, 2024-Nov-18:
  - Switch to using MathJax 4 beta; load MathJax in the frame it's used, per
    [MathJax issue #3309](https://github.com/mathjax/MathJax/issues/3309).
  - Modernize graphviz-webcomponent build.
  - Move CSS to `client/src`.
- v0.1.2, 2024-Nov-12:
  - Fix [issue #28](https://github.com/bjones1/CodeChat_Editor/issues/28),
    autosave failures.
  - Fix filewatcher -- incorrect file path comparison.
  - Fix errors saving mathematics in Markdown-only files.
  - Improve spellchecking coverage.
  - Fix data loss when the CodeChat Editor Client is not visible.
- v0.1.1, 2024-Nov-04:
  - Added basic theme support; used a theme similar to Sphinx's Alabaster.
  - Added support for printing.
  - Added a user manual, improved documentation.
  - Fixed a bug in the filewatcher that prevented saving changes made in the
    Client.
  - Added math support.
  - Removed save button.
  - Added support for Kotlin.
  - Updates to the build system.
  - Cross-platform fixes.
- v0.1.0, 2024-Oct-16:
  - Initial release, with binaries for Windows only. Built with manually-patched
    CodeMirror per
    [this issue](https://github.com/bjones1/CodeChat_Editor/issues/27).
