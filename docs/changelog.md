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
  - No changes.
- v0.1.2, 2024-Nov-12:
  - Fix [issue #28](https://github.com/bjones1/CodeChat_Editor/issues/28),
    autosave failures.
  - Fix filewatcher -- incorrect file path comparison.
  - Fix errors saving mathematics in Markdown-only files.
  - Improve spellchecking coverage.
  - Fix data loss when the CodeChat Editor Client is not visible.
- v0.1.1, 2024-Nov-04
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
- v0.1.0, 2024-Oct-16
  - Initial release, with binaries for Windows only. Built with manually-patched
    CodeMirror per
    [this issue](https://github.com/bjones1/CodeChat_Editor/issues/27).
