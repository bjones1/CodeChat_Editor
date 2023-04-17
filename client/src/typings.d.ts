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
