The CodeChat Editor
================================================================================

User documentation
================================================================================

* [The CodeChat Editor manual](README.md)
* [The CodeChat Editor extension for Visual Studio Code manual](extensions/VSCode/README.md)
* [Literate programming using the CodeChat Editor](docs/style_guide.cpp)

Design
================================================================================

* [CodeChat Editor Design](docs/design.md)
* [Implementation](docs/implementation.md)

Implementation
================================================================================

* [Server](server/readme.md)
  * [main.rs](server/src/main.rs)
  * [lib.rs](server/src/lib.rs)
  * [lexer.rs](server/src/lexer.rs)
    * [Lexer walkthrough](server/src/lexer/lexer-walkthrough.md)
    * [supported\_languages.rs](server/src/lexer/supported_languages.rs)
    * [pest\_parser.rs](server/src/lexer/pest_parser.rs)
      * [Parser design](server/src/lexer/pest/parser_design.md)
      * [shared.pest](server/src/lexer/pest/shared.pest)
      * [c.pest](server/src/lexer/pest/c.pest)
      * [python.pest](server/src/lexer/pest/python.pest)
  * [webserver.rs](server/src/webserver.rs)
    * [log4rs.yml](server/log4rs.yml)
  * [ide.rs](server/src/ide.rs)
    * [filewatcher.rs](server/src/ide/filewatcher.rs)
    * [vscode.rs](server/src/ide/vscode.rs)
  * [translation.rs](server/src/translation.rs)
  * [processing.rs](server/src/processing.rs)
  * Tests
    * [Test utilities](test_utils/readme.md)
      * [test\_utils.rs](test_utils/src/test_utils.rs)
      * [testing\_logger.rs](test_utils/src/testing_logger.rs)
      * [test\_macros.rs](test_utils/src/test_macros.rs)
    * Lexer [tests.rs](server/src/lexer/tests.rs)
    * Webserver [tests.rs](server/src/webserver/tests.rs)
    * ide/vscode [tests.rs](server/src/ide/vscode/tests.rs)
    * Processing [tests.rs](server/src/processing/tests.rs)
    * [cli.rs](server/tests/cli.rs)
  * [Cargo.toml](server/Cargo.toml)
* [Client](client/readme.md)
  * Editor
    * [CodeChatEditorFramework.mts](client/src/CodeChatEditorFramework.mts)
    * [CodeChatEditor.mts](client/src/CodeChatEditor.mts)
      * [CodeMirror-integration.mts](client/src/CodeMirror-integration.mts)
      * [tinymce-config.mts](client/src/tinymce-config.mts)
      * [graphviz-webcomponent-setup.mts](client/src/graphviz-webcomponent-setup.mts)
      * [Mermaid](client/src/third-party/wc-mermaid/developer.md)
      * [shared\_types.mts](client/src/shared_types.mts)
      * [assert.mts](client/src/assert.mts)
      * [show\_toast.mts](client/src/show_toast.mts)
  * Styles
    * [CodeChatEditor.css](client/src/css/CodeChatEditor.css)
    * [CodeChatEditorProject.css](client/src/css/CodeChatEditorProject.css)
    * Themes
      * [light.css](client/src/css/themes/light.css)
  * Tests
    * [CodeChatEditor-test.mts](client/src/CodeChatEditor-test.mts)
      * [Run tests](README.md?test)
    * [HTML to Markdown conversion test document](docs/Markdown_HTML.js)
* [Extensions](extensions/readme.md)
  * [Visual Studio Code](extensions/VSCode/developer.md)
    * [extension.ts](extensions/VSCode/src/extension.ts)
    * [lib.rs](extensions/VSCode/src/lib.rs)
    * [Cargo.toml](extensions/VSCode/Cargo.toml)
    * [Developer documentation](extensions/VSCode/developer.md)
* Development tools
  * Builder
    * [builder/Cargo.toml](Cargo.toml)
    * [builder/src/main.rs](main.rs)
  * Git
    * [server/.gitignore](server/.gitignore)
    * [client/static/.gitignore](client/static/.gitignore)
    * [client/.gitignore](client/.gitignore)
    * [extensions/VSCode/.gitignore](extensions/VSCode/.gitignore)
    * [builder/.gitignore](builder/.gitignore)
  * NPM/esbuild
    * [HashReader.mts](client/src/HashReader.mts)
    * client/package.json
    * [client/tsconfig.json](client/tsconfig.json)
    * [client/eslint.config.js](client/eslint.config.js)
    * [client/.prettierrc.json5](client/.prettierrc.json5)
    * [extensions/VSCode/eslint.config.js](extensions/VSCode/eslint.config.js)
    * [extensions/VSCode/tsconfig.json](extensions/VSCode/tsconfig.json)
    * [extensions/VSCode/jsconfig.json](extensions/VSCode/jsconfig.json)
    * [extensions/VSCode/.vscodeignore](extensions/VSCode/.vscodeignore)
    * [.prettierignore](.prettierignore)
  * Misc
    * [config.toml](server/.cargo/config.toml) - for Rust code coverage
    * [dist-workspace.toml](dist-workspace.toml) - cargo-dist configuration
    * [dist.toml](server/dist.toml) - additional cargo-dist configuration

Misc
================================================================================

* [New project template](new-project-template/README.md)
* [Table of contents](toc.md)
* [Changelog](CHANGELOG.md)
* [Index](docs/index.md)

[License](LICENSE.md)
