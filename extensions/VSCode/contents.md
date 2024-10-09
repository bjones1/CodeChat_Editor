**NOTE**: This file was copied from a previous project and needs significant
editing.

# The Visual Studio Code CodeChat Editor extension

This extension provides CodeChat's capabilities within the Visual Studio Code
editor.

## Installation

First, install [Visual Studio Code](https://code.visualstudio.com/). Next:

1.  [Install the CodeChat Editor Server](../../CodeChat_Server/contents), which
    performs all the back-end work and is required for the extension to work.
2.  (Recommended)
    [switch to a light theme](https://code.visualstudio.com/docs/getstarted/themes),
    since the CodeChat System only provides a light theme.

## Use

1.  Open a file that CodeChat can render (most source files, along with `.md`
    files).
2.  Open the
    [Visual Studio Code command palette](https://code.visualstudio.com/docs/getstarted/userinterface#_command-palette)
    by pressing `Ctrl+Shift+P`. Type `CodeChat`, then press enter to run the
    extension. After a moment, the rendered file should load. If it doesn't:

    1.  Determine the location of the `CodeChat_Server` by entering
        `which CodeChat_Server` (Linux/OS X) or `where CodeChat_Server`
        (Windows) at the terminal/command line.
    2.  Open the Visual Studio Code settings for CodeChat by navigating to
        `File` > `Preferences` > `Settings` then typing `CodeChat` in the search
        box. Enter this path for the `Code Chat.Code Chat Server: Command`.
        **Important**: in Windows, replace `\` in the location you determined
        with either `\\` or `/`.
    3.  Run the extension again (`Ctrl+Shift+P` then select CodeChat).

At any time, run the CodeChat extension again (`Ctrl+Shift+P`, then `CodeChat`)
to show the CodeChat panel, re-start the CodeChat Server if it's not running,
then reconnect with the server. Close the CodeChat panel then run the extension
for a more complete reset.

See the
[CodeChat tutorial](https://codechat.readthedocs.io/en/master/docs/tutorial.html)
for step-by-step instructions on authoring literate programming documents using
Sphinx.
