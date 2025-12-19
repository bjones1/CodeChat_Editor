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
