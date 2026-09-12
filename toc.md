The CodeChat Editor
===================

User documentation
==================

* [The CodeChat Editor manual](README.md)
* [The CodeChat Editor extension for Visual Studio Code manual](extensions/VSCode/README.md)
* [Literate programming using the CodeChat Editor](docs/style_guide.cpp)
* [Capture token setup guide](docs/capture-token-setup-guide.html)

Design
======

* [CodeChat Editor Design](docs/design.md)
* [Implementation](docs/implementation.md)
* [Accessibility review](docs/accessibility_review.md)

Implementation
==============

* [Server](server/readme.md)
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
  * [capture.rs](server/src/capture.rs)
    * [Capture events schema](server/scripts/capture_events_schema.sql)
  * [ide.rs](server/src/ide.rs)
    * [vscode.rs](server/src/ide/vscode.rs)
  * [translation.rs](server/src/translation.rs)
  * [processing.rs](server/src/processing.rs)
    * [cache.rs](server/src/processing/cache.rs)
  * Tests
    * [Test utilities](test_utils/readme.md)
      * [Cargo.toml](test_utils/Cargo.toml)
      * [lib.rs](test_utils/src/lib.rs)
      * [test\_utils.rs](test_utils/src/test_utils.rs)
      * [testing\_logger.rs](test_utils/src/testing_logger.rs)
      * [test\_macros.rs](test_utils/src/test_macros.rs)
    * Lexer [tests.rs](server/src/lexer/tests.rs)
    * Webserver [tests.rs](server/src/webserver/tests.rs)
    * ide/vscode [tests.rs](server/src/ide/vscode/tests.rs)
    * Processing [tests.rs](server/src/processing/tests.rs)
    * Webdriver-based
      * [overall.rs](server/tests/overall.rs)
      * [overall_common/mod.rs](server/tests/overall/common/mod.rs)
      * [overall_1.rs](server/tests/overall/overall_1.rs)
      * [overall_2.rs](server/tests/overall/overall_2.rs)
      * [overall_3.rs](server/tests/overall/overall_3.rs)
      * [overall_4.rs](server/tests/overall/overall_4.rs)
      * [overall_5.rs](server/tests/overall/overall_5.rs)
      * [overall_a11y.rs](server/tests/overall/overall_a11y.rs)
  * [Cargo.toml](server/Cargo.toml)
* [Client](client/readme.md)
  * Editor
    * [CodeChatEditorFramework.mts](client/src/CodeChatEditorFramework.mts)
    * [CodeChatEditor.mts](client/src/CodeChatEditor.mts)
      * [CodeMirror-integration.mts](client/src/CodeMirror-integration.mts)
      * [tinymce-config.mts](client/src/tinymce-config.mts)
      * [graphviz-webcomponent-setup.mjs](client/src/graphviz-webcomponent-setup.mjs)
      * [Mermaid](client/src/third-party/wc-mermaid/developer.md)
      * [shared.mts](client/src/shared.mts)
      * [assert.mts](client/src/assert.mts)
      * [show\_toast.mts](client/src/show_toast.mts)
      * [debug\_enabled.mts](client/src/debug_enabled.mts)
    * [global.d.ts](client/src/global.d.ts)
  * Styles
    * [CodeChatEditorBase.css](client/src/css/CodeChatEditorBase.css)
    * [CodeChatEditor.css](client/src/css/CodeChatEditor.css)
    * [CodeChatEditorProject.css](client/src/css/CodeChatEditorProject.css)
    * Themes
      * [light.css](client/src/css/themes/light.css)
  * Tests
    * [CodeChatEditor-test.mts](client/src/CodeChatEditor-test.mts)
      * [Run tests](README.md?test)
    * [HTML to Markdown conversion test document](docs/Markdown_HTML.js)
    * [PDF test](docs/helloworld.pdf)
* [Extensions](extensions/readme.md)
  * [Developer documentation](extensions/developer.md)
  * Visual Studio Code
    * [Developer documentation](extensions/VSCode/developer.md)
    * [extension.ts](extensions/VSCode/src/extension.ts)
    * [capture-policy.ts](extensions/VSCode/src/capture-policy.ts)
      * [capture-policy.test.mjs](extensions/VSCode/src/capture-policy.test.mjs)
    * [lib.rs](extensions/VSCode/src/lib.rs)
      * [build.rs](extensions/VSCode/build.rs)
    * [Cargo.toml](extensions/VSCode/Cargo.toml)
    * [LICENSE.md](extensions/VSCode/LICENSE.md)
  * Standalone
    * [main.rs](extensions/standalone/src/main.rs)
    * [filewatcher.rs](extensions/standalone/src/filewatcher.rs)
    * [cli.rs](extensions/standalone/tests/cli.rs)
    * [Cargo.toml](extensions/standalone/Cargo.toml)
* Development tools
  * [CLAUDE.md](CLAUDE.md)
  * Builder
    * [builder/Cargo.toml](builder/Cargo.toml)
    * [builder/src/main.rs](builder/src/main.rs)
    * [server/bt](server/bt) - shortcut to run the build tool
    * [server/bt.ps1](server/bt.ps1) - PowerShell shortcut to run the build tool
  * Continuous integration
    * [check.yml](.github/workflows/check.yml)
    * [release.yml](.github/workflows/release.yml)
  * Development environment
    * [devcontainer.json](.devcontainer/devcontainer.json)
      * [postCreateCommand.sh](.devcontainer/postCreateCommand.sh)
      * [postStartCommand.sh](.devcontainer/postStartCommand.sh)
  * Git
    * [.gitignore](.gitignore)
    * [server/.gitignore](server/.gitignore)
    * [client/static/.gitignore](client/static/.gitignore)
    * [client/.gitignore](client/.gitignore)
    * [client/src/.gitignore](client/src/.gitignore)
    * [extensions/VSCode/.gitignore](extensions/VSCode/.gitignore)
    * [builder/.gitignore](builder/.gitignore)
  * NPM/esbuild
    * [HashReader.mts](client/src/HashReader.mts)
    * [client/package.json5](client/package.json5)
    * [client/tsconfig.json](client/tsconfig.json)
    * [client/eslint.config.js](client/eslint.config.js)
    * [client/.prettierrc.json5](client/.prettierrc.json5)
    * [client/.prettierignore](client/.prettierignore)
    * [extensions/VSCode/eslint.config.js](extensions/VSCode/eslint.config.js)
    * [extensions/VSCode/tsconfig.json](extensions/VSCode/tsconfig.json)
    * [extensions/VSCode/jsconfig.json](extensions/VSCode/jsconfig.json)
    * [extensions/VSCode/.vscodeignore](extensions/VSCode/.vscodeignore)
    * [.prettierignore](.prettierignore)
  * Misc
    * [Cargo.toml](Cargo.toml) - workspace manifest
    * [config.toml](.cargo/config.toml) - for Rust code coverage
    * [dist-workspace.toml](dist-workspace.toml) - cargo-dist configuration

Misc
====

* [New project template](examples/new-project-template/README.md)
  * [Template table of contents](examples/new-project-template/toc.md)
* [Table of contents](toc.md)
* [Changelog](CHANGELOG.md)
* [Index](docs/index.md)

[License](LICENSE.md)
