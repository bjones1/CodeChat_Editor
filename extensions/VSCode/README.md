The CodeChat Editor extension for Visual Studio Code
====================================================

This extension provides the CodeChat Editor's capabilities within the Visual
Studio Code IDE.

![Screenshot of the CodeChat Editor extension](https://github.com/bjones1/CodeChat_Editor/blob/main/extensions/VSCode/screenshot.png?raw=true)

Installation
------------

First, install [Visual Studio Code](https://code.visualstudio.com/). Next:

1. [Install the CodeChat Editor extension](https://marketplace.visualstudio.com/items?itemName=CodeChat.codechat-editor-client).
2. (Recommended)
   [switch to a light theme](https://code.visualstudio.com/docs/getstarted/themes),
   since the CodeChat Editor only provides a light theme.

Running
-------

1. Open a file that the CodeChat Editor
   [supports](https://github.com/bjones1/CodeChat_Editor/blob/main/README.md#supported-languages)
   (many source files, along with Markdown files).

2. Open the
   [Visual Studio Code command palette](https://code.visualstudio.com/docs/getstarted/userinterface#_command-palette)
   by pressing `Ctrl+Shift+P`. Type `CodeChat`, select "Enable the CodeChat
   Editor", then press enter to run the extension. After a moment, the rendered
   file should load. If it doesn't:

   1. Open the Visual Studio Code settings for the CodeChat Editor by navigating
      to `File` > `Preferences` > `Settings` then typing `CodeChat` in the
      search box. Change the port from its default of 8080 to some other value.
   2. Run the extension again (close the existing window, type `Ctrl+Shift+P`
      then select Enable the CodeChat Editor).

Study capture
-------------

Participants who have registered in the capture portal receive a capture token
by email. To use it, run **Manage CodeChat Editor Capture** or **CodeChat
Editor: Enter Capture Token** from the command palette, paste the token, then
turn on consent and recording. The capture status item shows whether the token
is accepted, rejected, unavailable, or disabled by the portal.

The token is imported through the VS Code UI and persisted only in VS Code
SecretStorage. It is never written to settings, workspace files, or a JSON
configuration file. The participant identity used in capture events comes from
CaptureWebService token status, not from the token text.

CodeChat sends capture events only to CaptureWebService and does not connect
directly to the capture database. The old JSON database-secret configuration
path is not used by the extension. Events are sanitized and written to a local
FIFO spool before upload, so events recorded offline after the token has been
accepted and capture-enabled upload automatically when the matching service is
reachable again. If the service endpoint changes, update the user-level
`CodeChatEditor.Capture.ServiceBaseUrl` setting. Workspace values are ignored
for this token-bearing endpoint. Token-bearing service requests must use HTTPS,
except for localhost development endpoints.

Additional documentation
------------------------

See the
[user manual](https://codechat-editor.onrender.com/fw/fsb/opt/render/project/src/README.md).
