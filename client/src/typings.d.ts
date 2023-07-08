// <h1><code>typing.d.ts</code> &mdash; Global type definitions</h1>
// <p>How a doc block is stored using CodeMirror.</p>
type DocBlockJSON = [
    // From
    number,
    // To
    number,
    // Indent
    string,
    // Delimiter
    string,
    // Contents
    string
];

// <p>These modules keep TypeScript from complaining about missing type
//     definitions for Turndown and Turndown plugin imports. See <a
//         href="CodeChatEditor.mts">CodeChatEditor.mts</a>.</p>
declare module "@joplin/turndown-plugin-gfm";
declare module "prettier/esm/standalone.mjs";
declare module "prettier/esm/parser-markdown.mjs";
declare module "prettier/esm/parser-html.mjs";