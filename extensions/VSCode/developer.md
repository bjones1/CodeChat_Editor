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

Developer documentation
=======================

From source
-----------

To install from source:

* Install [npm](https://nodejs.org/en/).

* Install this extension's manifest
  ([package.json](https://code.visualstudio.com/api/references/extension-manifest)):
  from this directory, open a command prompt/terminal then execute::

  ```
  npm install
  ```

Debugging the extension
-----------------------

* From VSCode, select File | Add Folder to Workspace... then choose the folder
  containing this file.
* Press ctrl+shift+B to compile the extension.
* Press F5 or click start debugging under the Debug menu.
* A new instance of VSCode will start in a special mode (Extension Development
  Host) which contains the CodeChat extension.
* Open any source code, then press Ctrl+Shift+P and type "CodeChat" to run the
  CodeChat extension. You will be able to see the rendered version of your
  active window.

Release procedure
-----------------

* In the Client:
  * Update the version of the plugin in `package.json`.
* In the Server:
  * Update the version in `cargo.toml`.
* Here:
  * Update the version of the plugin in `package.json`.
  * Run `cargo run -- release` on each platform, which produces a `.vsix` file
    for that platform
  * Run `npx vsce publish --packagePath blah`.
    ([docs](https://code.visualstudio.com/api/working-with-extensions/publishing-extension#platformspecific-extensions))

Tests
-----

TODO: tests are missing.
