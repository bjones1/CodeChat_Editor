// <details>
//     <summary>Copyright (C) 2022 Bryan A. Jones.</summary>
//     <p>This file is part of the CodeChat Editor.</p>
//     <p>The CodeChat Editor is free software: you can redistribute it and/or
//         modify it under the terms of the GNU General Public License as
//         published by the Free Software Foundation, either version 3 of the
//         License, or (at your option) any later version.</p>
//     <p>The CodeChat Editor is distributed in the hope that it will be useful,
//         but WITHOUT ANY WARRANTY; without even the implied warranty of
//         MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
//         General Public License for more details.</p>
//     <p>You should have received a copy of the GNU General Public License
//         along with the CodeChat Editor. If not, see <a
//             href="http://www.gnu.org/licenses/">http://www.gnu.org/licenses/</a>.
//     </p>
// </details>
// <h1><code>CodeChat-editor.mts</code> &mdash; TypeScript which implements part
//     of the client-side portion of the CodeChat Editor</h1>
// <p>The overall process of load a file is:</p>
// <ol>
//     <li>The user browses to a file on the local machine, using the very
//         simple file browser webpage provided by the CodeChat Server. Clicking
//         on this file starts the process of loading a file into the CodeChat
//         editor.</li>
//     <li>The server sees a request for a file supported by the CodeChat
//         Editor. It lexes the files into code and doc blocks, then wraps these
//         in a webpage which contains this program (the CodeChat Editor).</li>
//     <li>On load, this program (the CodeChat Editor) transforms these code and
//         doc blocks into HTML. Specifically, code blocks are placed in <a
//             href="https://ace.c9.io/">ACE editor</a> instances, while doc
//         blocks are placed in <a href="https://www.tiny.cloud/">TinyMCE</a>
//         instances.</li>
// </ol>
// <p>The user then uses the editing capabilities of ACE/TinyMCE to edit their
//     program. When the user saves a file:</p>
// <ol>
//     <li>This program looks through the HTML, converting ACE editor/TinyMCE
//         instances back into code blocks and doc blocks.</li>
//     <li>It sends these code/doc blocks back to the server.</li>
//     <li>The server then transforms these code/doc blocks into source code,
//         then writes this code to the disk.</li>
// </ol>
// <h2>Imports</h2>
// <h3>JavaScript/TypeScript</h3>
import "./EditorComponents.mjs";
import "./graphviz-webcomponent-setup.mts";
import "graphviz-webcomponent";
import { Editor, init, tinymce } from "./tinymce-config.mjs";

// <h3>CSS</h3>
import "./../static/css/CodeChatEditor.css";
import { CodeMirror_load, CodeMirror_save } from "./CodeMirror-integration.mjs";

// <h2>Initialization</h2>

// <p>The server passes this to the client to load a file. See <a
//         href="../../server/src/webserver.rs#LexedSourceFile">LexedSourceFile</a>.
// </p>
type LexedSourceFile = {
    metadata: { mode: string };
    source: {
        doc: string;
        doc_blocks: [DocBlockJSON];
        selection: any;
    };
};

// <p>Store the lexer info for the currently-loaded language.</p>
// <p><a id="current_metadata"></a>This mirrors the data provided by the server
//     -- see <a
//         href="../../server/src/webserver.rs#SourceFileMetadata">SourceFileMetadata</a>.
// </p>
let current_metadata: {
    mode: string;
};

// <p>Load code when the DOM is ready.</p>
export const page_init = (all_source: any) => {
    // <p>Use <a
    //         href="https://developer.mozilla.org/en-US/docs/Web/API/URLSearchParams">URLSearchParams</a>
    //     to parse out the search parameters of this window's URL.</p>
    const urlParams = new URLSearchParams(window.location.search);
    // <p>Get the mode from the page's query parameters. Default to edit using
    //     the <a
    //         href="https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Operators/Nullish_coalescing_operator">nullish
    //         coalescing operator</a>. This works, but TypeScript marks it as
    //     an error. Ignore this error by including the <a
    //         href="https://www.typescriptlang.org/docs/handbook/intro-to-js-ts.html#ts-check">@ts-ignore
    //         directive</a>.</p>
    /// @ts-ignore
    const editorMode = EditorMode[urlParams.get("mode") ?? "edit"];
    on_dom_content_loaded(async () => {
        open_lp(all_source, editorMode);
    });
};

// <p>This is copied from <a
//         href="https://developer.mozilla.org/en-US/docs/Web/API/Document/DOMContentLoaded_event#checking_whether_loading_is_already_complete">MDN</a>.
// </p>
const on_dom_content_loaded = (on_load_func: () => void) => {
    if (document.readyState === "loading") {
        // <p>Loading hasn't finished yet.</p>
        document.addEventListener("DOMContentLoaded", on_load_func);
    } else {
        // <p><code>DOMContentLoaded</code> has already fired.</p>
        on_load_func();
    }
};

// <p><a id="EditorMode"></a>Define all possible editor modes; these are passed
//     as a <a href="https://en.wikipedia.org/wiki/Query_string">query
//         string</a> (<code>http://path/to/foo.py?mode=toc</code>, for example)
//     to the page's URL.</p>
enum EditorMode {
    // <p>Display the source code using CodeChat, but disallow editing.</p>
    view,
    // <p>For this source, the same a view; the server uses this to avoid
    //     recursive iframes of the table of contents.</p>
    toc,
    // <p>The full CodeChat editor.</p>
    edit,
    // <p>Show only raw source code; ignore doc blocks, treating them also as
    //     code.</p>
    raw,
}

// <p>Tell TypeScript about the global namespace this program defines.</p>
declare global {
    interface Window {
        CodeChatEditor_test: any;
    }
}

// <p>This function is called on page load to "load" a file. Before this point,
//     the server has already lexed the source file into code and doc blocks;
//     this function transforms the code and doc blocks into HTML and updates
//     the current web page with the results.</p>
const open_lp = (
    // <p>A data structure provided by the server, containing the source and
    //     associated metadata. See <a
    //         href="#AllSource"><code>AllSource</code></a>.</p>
    all_source: LexedSourceFile,
    // <p>See <code><a href="#EditorMode">EditorMode</a></code>.</p>
    editorMode: EditorMode
) => {
    // <p>Get the <code><a href="#current_metadata">current_metadata</a></code>
    //     from the provided <code>all_source</code> struct and store it as a
    //     global variable.</p>
    current_metadata = all_source["metadata"];
    const source = all_source["source"];
    const codechat_body = document.getElementById(
        "CodeChat-body"
    ) as HTMLDivElement;
    if (is_doc_only()) {
        // <p>Special case: a CodeChat Editor document's HTML is stored in `source.doc`. We don't need the CodeMirror editor at all; instead, treat it like a single doc block contents div./p>
        codechat_body.innerHTML = `<div class="CodeChat-doc-contents">${source.doc}</div>`;
        init({ selector: ".CodeChat-doc-contents" }).then((editors) =>
            editors[0].focus()
        );
    } else {
        CodeMirror_load(codechat_body, source);
    }

    // <p><a id="CodeChatEditor_test"></a>If tests should be run, then
    //     the <a
    //         href="CodeChatEditor-test.mts#CodeChatEditor_test">following
    //         global variable</a> is function that runs them.</p>
    if (typeof window.CodeChatEditor_test === "function") {
        window.CodeChatEditor_test();
    }
};

// <p>Provide a shortcut of ctrl-s (or command-s) to save the current file.</p>
export const on_keydown = (event: KeyboardEvent) => {
    if (
        event.key === "s" &&
        ((event.ctrlKey && !os_is_osx) || (event.metaKey && os_is_osx)) &&
        !event.altKey
    ) {
        on_save();
        event.preventDefault();
    }
};

// <p>Save CodeChat Editor contents.</p>
export const on_save = async () => {
    /// @ts-expect-error
    let source: LexedSourceFile["source"] = {};
    if (is_doc_only()) {
        // To save a document only, simply get the HTML from the only Tiny MCE div.
        source.doc = tinymce.get(0)!.getContent();
    } else {
        source = CodeMirror_save();
    }
    await save({
        metadata: current_metadata,
        source,
    });
};

// <p><a id="save"></a>Save the provided contents back to the filesystem, by
//     sending a <code>PUT</code> request to the server. See the <a
//         href="CodeChatEditorServer.v.html#save_file">save_file endpoint</a>.
// </p>
const save = async (contents: LexedSourceFile) => {
    let response;
    try {
        response = await window.fetch(window.location.href, {
            method: "PUT",
            headers: {
                "Content-Type": "application/json",
            },
            body: JSON.stringify(contents),
        });
    } catch (error) {
        window.alert(`Save failed -- ${error}.`);
        return;
    }
    if (response.ok) {
        const response_body = await response.json();
        if (response_body.success !== true) {
            window.alert("Save failed.");
        }
        return;
    }
    window.alert(
        `Save failed -- server returned ${response.status}, ${response.statusText}.`
    );
};

// <h2>Helper functions</h2>
// <p>Given text, escape it so it formats correctly as HTML. Because the
//     solution at <a href="https://stackoverflow.com/a/48054293">SO</a>
//     transforms newlines in odd ways (see <a
//         href="https://developer.mozilla.org/en-US/docs/Web/API/HTMLElement/innerText">innerText</a>),
//     it's not usable with code. Instead, this is a translation of Python's
//     <code>html.escape</code> function.</p>
const escapeHTML = (unsafeText: string): string => {
    // <p>Must be done first!</p>
    unsafeText = unsafeText.replaceAll("&", "&amp;");
    unsafeText = unsafeText.replaceAll("<", "&lt;");
    unsafeText = unsafeText.replaceAll(">", "&gt;");
    return unsafeText;
};

// <p>This handles only three HTML entities, but no others!</p>
const unescapeHTML = (html: string): string => {
    let text = html.replaceAll("&gt;", ">");
    text = text.replaceAll("&lt;", "<");
    text = text.replaceAll("&amp;", "&");
    return text;
};

// <p>True if this is a CodeChat Editor document (not a source file).</p>
const is_doc_only = () => {
    return current_metadata["mode"] === "codechat-html";
};

// <p>Per <a
//         href="https://developer.mozilla.org/en-US/docs/Web/API/Navigator/platform#examples">MDN</a>,
//     here's the least bad way to choose between the control key and the
//     command key.</p>
const os_is_osx =
    navigator.platform.indexOf("Mac") === 0 || navigator.platform === "iPhone"
        ? true
        : false;

// <p>A great and simple idea taken from <a
//         href="https://stackoverflow.com/a/54116079">SO</a>: wrap all testing
//     exports in a single variable. This avoids namespace pollution, since only
//     one name is exported, and it's clearly marked for testing only. Test code
//     still gets access to everything it needs.</p>
export const exportedForTesting = {
    EditorMode,
    open_lp,
};
