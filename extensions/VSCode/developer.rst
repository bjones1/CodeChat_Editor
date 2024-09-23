***********************
Developer documentation
***********************

From source
===========
To install from source:

*   Install `npm <https://nodejs.org/en/>`_.
*   Install this extension's manifest (`package.json <https://code.visualstudio.com/api/references/extension-manifest>`_): from this directory, open a command prompt/terminal then execute::

        npm install


Debugging the extension
=======================
*   From VSCode, select File | Add Folder to Workspace... then choose the folder containing this file.
*   Press ctrl+shift+B to compile the extension.
*   Press F5 or click start debugging under the Debug menu.
*   A new instance of VSCode will start in a special mode (Extension Development Host) which contains the CodeChat extension.
*   Open any source code, then press Ctrl+Shift+P and type "CodeChat" to run the CodeChat extension. You will be able to see the rendered version of your active window.


Release procedure
=================
-   Update the version of the plugin in `package.json`.
-   Run ``npm update``.
-   Verify that the extension still works after upgrading these packages.
-   Run ``npx vsce publish`` (`docs <https://code.visualstudio.com/api/working-with-extensions/publishing-extension>`__) and ``npx ovsx publish -p <token>`` (`docs <https://github.com/eclipse/openvsx/wiki/Publishing-Extensions#5-package-and-upload>`__).


Notes
=====
*   The NPM Thrift 0.13.0 release is `broken <https://github.com/apache/thrift/pull/1947>`_. Don't install it.


Tests
=====
TODO: tests are missing.


Contents
========
.. toctree::
    :maxdepth: 2

    README
    src/extension.ts
    package.json
    tsconfig.json
    .eslintrc.yml
