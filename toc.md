The CodeChat Editor
================================================================================

User documentation
--------------------------------------------------------------------------------

1. [The CodeChat Editor manual](README.md)
2. [The CodeChat Editor extension for Visual Studio Code manual](extensions/VSCode/README.md)
3. [Literate programming using the CodeChat Editor](docs/style_guide.cpp)

Design
--------------------------------------------------------------------------------

1. [CodeChat Editor Design](docs/design.md)
2. [Implementation](docs/implementation.md)

Implementation
--------------------------------------------------------------------------------

1. [Server](server/readme.md)
   1. [main.rs](server/src/main.rs)
   2. [lib.rs](server/src/lib.rs)
   3. [lexer.rs](server/src/lexer.rs)
      1. [Lexer walkthrough](server/src/lexer/lexer-walkthrough.md)
      2. [supported\_languages.rs](server/src/lexer/supported_languages.rs)
      3. [pest\_parser.rs](server/src/lexer/pest_parser.rs)
         1. [Parser design](server/src/lexer/pest/parser_design.md)
         2. [shared.pest](server/src/lexer/pest/shared.pest)
         3. [c.pest](server/src/lexer/pest/c.pest)
         4. [python.pest](server/src/lexer/pest/python.pest)
   4. [webserver.rs](server/src/webserver.rs)
      1. [log4rs.yml](server/log4rs.yml)
   5. [ide.rs](server/src/ide.rs)
      1. [filewatcher.rs](server/src/ide/filewatcher.rs)
      2. [vscode.rs](server/src/ide/vscode.rs)
   6. [translation.rs](server/src/translation.rs)
   7. [processing.rs](server/src/processing.rs)
   8. Tests
      1. [Test utilities](test_utils/readme.md)
         1. [test\_utils.rs](test_utils/src/test_utils.rs)
         2. [testing\_logger.rs](test_utils/src/testing_logger.rs)
         3. [test\_macros.rs](test_utils/src/test_macros.rs)
      2. Lexer [tests.rs](server/src/lexer/tests.rs)
      3. Webserver [tests.rs](server/src/webserver/tests.rs)
      4. ide/vscode [tests.rs](server/src/ide/vscode/tests.rs)
      5. Processing [tests.rs](server/src/processing/tests.rs)
      6. [cli.rs](server/tests/cli.rs)
   9. [Cargo.toml](server/Cargo.toml)
2. Client
   1. Editor
      1. [CodeChatEditorFramework.mts](client/src/CodeChatEditorFramework.mts)
      2. [CodeChatEditor.mts](client/src/CodeChatEditor.mts)
         1. [CodeMirror-integration.mts](client/src/CodeMirror-integration.mts)
         2. [tinymce-config.mts](client/src/tinymce-config.mts)
         3. [EditorComponents.mts](client/src/EditorComponents.mts)
         4. [graphviz-webcomponent-setup.mts](client/src/graphviz-webcomponent-setup.mts)
         5. [Mermaid](client/src/wc-mermaid/developer.md)
         6. [shared\_types.mts](client/src/shared_types.mts)
         7. [assert.mts](client/src/assert.mts)
         8. [show\_toast.mts](client/src/show_toast.mts)
         9. [typings.d.ts](client/src/typings.d.ts)
   2. Styles
      1. [CodeChatEditor.css](client/src/css/CodeChatEditor.css)
      2. [CodeChatEditorProject.css](client/src/css/CodeChatEditorProject.css)
      3. Themes
         1. [light.css](client/src/css/themes/light.css)
   3. Tests
      1. [CodeChatEditor-test.mts](client/src/CodeChatEditor-test.mts)
         1. [Run tests](README.md?test)
      2. [HTML to Markdown conversion test document](docs/Markdown_HTML.js)
3. Extensions
   1. Visual Studio Code
      1. [extension.ts](extensions/VSCode/src/extension.ts)
      2. [lib.rs](extensions/VSCode/src/lib.rs)
      3. [Cargo.toml](extensions/VSCode/Cargo.toml)
      4. [Developer documentation](extensions/VSCode/developer.md)
4. Development tools
   1. Builder
      1. [builder/Cargo.toml](builder/Cargo.toml)
      2. [builder/src/main.rs](builder/src/main.rs)
   2. Git
      1. [server/.gitignore](server/.gitignore)
      2. [client/static/.gitignore](client/static/.gitignore)
      3. [client/.gitignore](client/.gitignore)
      4. [extensions/VSCode/.gitignore](extensions/VSCode/.gitignore)
      5. [builder/.gitignore](builder/.gitignore)
   3. NPM/esbuild
      1. [HashReader.mts](client/src/HashReader.mts)
      2. client/package.json
      3. [client/tsconfig.json](client/tsconfig.json)
      4. [client/eslint.config.js](client/eslint.config.js)
      5. [client/.prettierrc.json5](client/.prettierrc.json5)
      6. [extensions/VSCode/eslint.config.js](extensions/VSCode/eslint.config.js)
      7. [extensions/VSCode/tsconfig.json](extensions/VSCode/tsconfig.json)
      8. [extensions/VSCode/jsconfig.json](extensions/VSCode/jsconfig.json)
      9. [extensions/VSCode/.vscodeignore](extensions/VSCode/.vscodeignore)
      10. [.prettierignore](.prettierignore)
   4. Misc
      1. [config.toml](server/.cargo/config.toml) - for Rust code coverage
      2. [dist-workspace.toml](dist-workspace.toml) - cargo-dist configuration
      3. [dist.toml](server/dist.toml) - additional cargo-dist configuration

Misc
--------------------------------------------------------------------------------

* <a href="new-project-template/README.md" target="_blank" rel="noopener">New
  project template</a>
* [Table of contents](toc.md)
* [Changelog](CHANGELOG.md)
* [Index](docs/index.md)

Notes
--------------------------------------------------------------------------------

* <a id="auto-title"></a>TODO: all links here should be auto-titled and
  autogenerate the page-local TOC.

[License](LICENSE.md)
--------------------------------------------------------------------------------
