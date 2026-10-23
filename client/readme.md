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

`readme.md` - overview of the Client
====================================

Inside the client:

* The Framework exchanges messages with the Server and loads the appropriate
  Client (simple view, PDF view, editor, document-only editor).
* The editor provides basic Client services and handles document-only mode.
* The CodeMirror integration module embeds TinyMCE into CodeMirror, providing
  the primary editing environment.
