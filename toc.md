# The CodeChat Editor

## Design

1.  [CodeChat Editor Overview](README.md)
2.  [Literate programming using the CodeChat Editor](docs/style_guide.cpp)
3.  [Implementation](docs/implementation.md)

## Implementation

1.  Server
    1.  [main.rs](server/src/main.rs)
    2.  [lib.rs](server/src/lib.rs)
    3.  [lexer.rs](server/src/lexer.rs)
        1.  [Lexer walkthrough](server/src/lexer/lexer-walkthrough.md)
        2.  [supported_languages.rs](server/src/lexer/supported_languages.rs)
        3.  [tests.rs](server/src/lexer/tests.rs)
    4.  [webserver.rs](server/src/webserver.rs)
        1.  [filewatcher.rs](server/src/webserver/filewatcher.rs)
        2.  [vscode.rs](server/src/webserver/vscode.rs)
    5.  [processing.rs](server/src/processing.rs)
    6.  [Cargo.toml](server/Cargo.toml)
2.  Client
    1.  Editor
        1.  [CodeChatEditorFramework.mts](client/src/CodeChatEditorFramework.mts)
        2.  [CodeChatEditor.mts](client/src/CodeChatEditor.mts)
            1.  [CodeMirror-integration.mts](client/src/CodeMirror-integration.mts)
            2.  [tinymce-config.mts](client/src/tinymce-config.mts)
            3.  [EditorComponents.mts](client/src/EditorComponents.mts)
            4.  [graphviz-webcomponent-setup.mts](client/src/graphviz-webcomponent-setup.mts)
            5.  [typings.d.ts](client/src/typings.d.ts)
    2.  Styles
        1.  [CodeChatEditor.css](client/static/css/CodeChatEditor.css)
        2.  [CodeChatEditorProject.css](client/static/css/CodeChatEditorProject.css)
        3.  [CodeChatEditorSidebar.css](client/static/css/CodeChatEditorSidebar.css)
    3.  Tests
        1.  [CodeChatEditor-test.mts](client/src/CodeChatEditor-test.mts)
            1.  [Run tests](server/src/lib.rs?test)
        2.  [HTML to Markdown conversion test document](docs/Markdown_HTML.js)
3.  Development tools
    1.  Git
        1.  [server/.gitignore](server/.gitignore)
        2.  [client/static/.gitignore](client/static/.gitignore)
        3.  [client/.gitignore](client/.gitignore)
    2.  NPM/esbuild
        1.  [HashReader.mts](client/src/HashReader.mts)
        2.  package.json
        3.  [tsconfig.json](client/tsconfig.json)
        4.  [.eslintrc.yml](client/.eslintrc.yml)
        5.  [.prettierignore](.prettierignore)
    3.  Misc
        1.  [config.toml](server/.cargo/config.toml) - for rust code coverage

## Misc

- <a href="new-project-template/README.md" target="_blank" rel="noopener">New
  project template</a>
- [Table of contents](toc.md)
- [Changelog](CHANGELOG.md)
- [Index](docs/index.md)

## Notes

- <a id="auto-title"></a>TODO: all links here should be auto-titled and
  autogenerate the page-local TOC.

## [License](LICENSE.md)
