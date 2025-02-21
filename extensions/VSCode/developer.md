Developer documentation
=======================

From source
-----------

To install from source:

*   Install [npm](https://nodejs.org/en/).

*   Install this extension's manifest
    ([package.json](https://code.visualstudio.com/api/references/extension-manifest)):
    from this directory, open a command prompt/terminal then execute::

    ```
    npm install
    ```

Debugging the extension
-----------------------

*   From VSCode, select File | Add Folder to Workspace... then choose the folder
    containing this file.
*   Press ctrl+shift+B to compile the extension.
*   Press F5 or click start debugging under the Debug menu.
*   A new instance of VSCode will start in a special mode (Extension Development
    Host) which contains the CodeChat extension.
*   Open any source code, then press Ctrl+Shift+P and type "CodeChat" to run the
    CodeChat extension. You will be able to see the rendered version of your
    active window.

Release procedure
-----------------

*   In the Client:
    *   Update the version of the plugin in `package.json`.
*   In the Server:
    *   Update the version in `cargo.toml`.
*   Here:
    *   Update the version of the plugin in `package.json`.
    *   Run `cargo run -- release` on each platform, which produces a `.vsix`
        file for that platform
    *   Run `npx vsce publish --packagePath blah`.
        ([docs](https://code.visualstudio.com/api/working-with-extensions/publishing-extension#platformspecific-extensions))

Tests
-----

TODO: tests are missing.