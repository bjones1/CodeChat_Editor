`readme.md` - overview of the Client
================================================================================

Inside the client:

* The Framework exchanges messages with the Server and loads the appropriate
  Client (simple view, PDF view, editor, document-only editor).
* The editor provides basic Client services and handles document-only mode.
* The CodeMirror integration module embeds TinyMCE into CodeMirror, providing
  the primary editing environment.
