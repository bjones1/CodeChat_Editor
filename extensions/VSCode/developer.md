**NOTE**: This file was copied from a previous project and needs significant
editing.

# Developer documentation

## From source

To install from source:

- Install [npm](https://nodejs.org/en/).
- Install this extension's manifest
  ([package.json](https://code.visualstudio.com/api/references/extension-manifest)):
  from this directory, open a command prompt/terminal then execute::

  ```
  npm install
  ```

## Debugging the extension

- From VSCode, select File | Add Folder to Workspace... then choose the folder
  containing this file.
- Press ctrl+shift+B to compile the extension.
- Press F5 or click start debugging under the Debug menu.
- A new instance of VSCode will start in a special mode (Extension Development
  Host) which contains the CodeChat extension.
- Open any source code, then press Ctrl+Shift+P and type "CodeChat" to run the
  CodeChat extension. You will be able to see the rendered version of your
  active window.

## Release procedure

- In the Client:
  - Update the version of the plugin in `package.json`.
  - Clean out `client/static/bundled`.
  - Run `npm update`.
  - Run `npm outdated` and check that everything is current.
  - Run `npm run dist`.
- In the Server:
  - Run `cargo update`.
  - Run `cargo outdated` and check that everything is current.
  - Run `cargo test`.
  - Run `dist build`, then copy files to this extension
- Here:
  - Update the version of the plugin in `package.json`.
  - Run `npm update`.
  - Run `npm outdated` and check that everything is current.
  - Verify that the extension still works after upgrading these packages.
  - Run `npx vsce publish --target win32-x64` (on Windows)
  - Repeat this for each target.
  ([docs](https://code.visualstudio.com/api/working-with-extensions/publishing-extension))
  - Uncomment the ignore for `server/` in `.vscodeignore`.
  - Run `npx vsce publish`.

## Tests

TODO: tests are missing.
