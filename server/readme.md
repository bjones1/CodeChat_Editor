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

`readme.md` - Overview of the Server
================================================================================

Overall:

* The webserver module runs the overall Server.
* The translation module translates messages sent between the IDE and the
  Client.
  * The ide module provides an API interface between the Server and an IDE that
    uses language-neutral data structures. Its submodules provide IDE-specific
    code (for VSCode and for a file watcher).
* The processing module handles conversion of source code to the Client's format
  and vice versa.
  * The lexer divides source code into code and doc blocks.
* A main module provides a basic CLI interface for running the server outside an
  IDE using the file watcher.
