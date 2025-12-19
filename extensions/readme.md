`readme.py` - Overview of extensions
================================================================================

The goal of the CodeChat Editor is to provide extensions for a number of IDEs
and environments. To support this, an explicit design goal of the Editor is to
implement most of the program's functionality in the Server and Client, and to
provide generic interfaces as a part of the server to IDEs (see ide.rs).

Currently, the system supports two IDEs:

* Visual Studio Code. This is the primary platform for the CodeChat Editor.
* Universal -- the file watcher extension allows use with any IDE by looking for
  changes made to the current file and automatically reloading it when changes
  are made.
