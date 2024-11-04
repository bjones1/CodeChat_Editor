# The CodeChat Editor extension for Visual Studio Code

This extension provides the CodeChat Editor's capabilities within the Visual
Studio Code IDE.

![Screenshot of the CodeChat Editor extension](https://github.com/bjones1/CodeChat_Editor/blob/main/extensions/VSCode/screenshot.png?raw=true)

## Installation

First, install [Visual Studio Code](https://code.visualstudio.com/). Next:

1.  [Install the CodeChat Editor extension](https://marketplace.visualstudio.com/items?itemName=CodeChat.codechat-editor-client).
2.  (Recommended)
    [switch to a light theme](https://code.visualstudio.com/docs/getstarted/themes),
    since the CodeChat Editor only provides a light theme.

## Running

1.  Open a file that the CodeChat Editor
    [supports](https://github.com/bjones1/CodeChat_Editor/blob/main/README.md#supported-languages)
    (many source files, along with Markdown files).
2.  Open the
    [Visual Studio Code command palette](https://code.visualstudio.com/docs/getstarted/userinterface#_command-palette)
    by pressing `Ctrl+Shift+P`. Type `CodeChat`, select "Enable the CodeChat
    Editor", then press enter to run the extension. After a moment, the rendered
    file should load. If it doesn't:

    1.  Open the Visual Studio Code settings for the CodeChat Editor by
        navigating to `File` > `Preferences` > `Settings` then typing `CodeChat`
        in the search box. Change the port from its default of 8080 to some
        other value.
    2.  Run the extension again (close the existing window, type `Ctrl+Shift+P`
        then select Enable the CodeChat Editor).

## Additional documentation

See the
[user manual](https://github.com/bjones1/CodeChat_Editor/blob/main/README.md).
