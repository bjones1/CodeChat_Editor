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
import { ace } from "./ace-webpack.mjs";
import "./EditorComponents.mjs";
import "./graphviz-webcomponent-setup.mts";
import "./graphviz-webcomponent/index.min.mjs";
import { html_beautify } from "js-beautify";
import { tinymce, tinymce_init } from "./tinymce-webpack.mjs";

// <h3>CSS</h3>
import "./../static/css/CodeChatEditor.css";

// <h2>Initialization</h2>
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
    on_dom_content_loaded(() => open_lp(all_source, editorMode));
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

// <p>This function is called on page load to "load" a file. Before this point,
//     the server has already lexed the source file into code and doc blocks;
//     this function transforms the code and doc blocks into HTML and updates
//     the current web page with the results.</p>
const open_lp = (
    // <p>A data structure provided by the server, containing the source and
    //     associated metadata. See <a
    //         href="#AllSource"><code>AllSource</code></a>.</p>
    all_source: AllSource,
    // <p>See <code><a href="#EditorMode">EditorMode</a></code>.</p>
    editorMode: EditorMode
) => {
    // <p>Get the <code><a href="#current_metadata">current_metadata</a></code>
    //     from the provided <code>all_source</code> struct and store it as a
    //     global variable.</p>
    current_metadata = all_source["metadata"];
    const code_doc_block_arr = all_source["code_doc_block_arr"];
    let html;
    if (is_doc_only()) {
        // <p>Special case: a CodeChat Editor document's HTML doesn't need
        //     lexing; it only contains HTML. Instead, its structure is always:
        //     <code>[["", "", HTML]]</code>. Therefore, the HTML is at item
        //     [0][2].</p>
        html = `<div class="CodeChat-TinyMCE">${code_doc_block_arr[0][2]}</div>`;
    } else {
        html = classified_source_to_html(code_doc_block_arr);
    }

    document.getElementById("CodeChat-body")!.innerHTML = html;
    // <p>Initialize editors for this new content. Return a promise which is
    //     accepted when the new content is ready.</p>
    return make_editors(editorMode);
};

// <p>This defines a single code or doc block entry:</p>
export type code_or_doc_block = [
    // <p>The indent for a doc bloc; empty for a code block.</p>
    string,
    // <p>The opening comment delimiter for a doc block;</p>
    string | null,
    string
];

// <p>The server passes this to the client to load a file. See <a
//         href="../../server/src/webserver.rs#LexedSourceFile">LexedSourceFile</a>.
// </p>
type AllSource = {
    metadata: { mode: string };
    code_doc_block_arr: code_or_doc_block[];
};

// <p>Store the lexer info for the currently-loaded language.</p>
// <p><a id="current_metadata"></a>This mirrors the data provided by the server
//     -- see <a
//         href="../../server/src/webserver.rs#SourceFileMetadata">SourceFileMetadata</a>.
// </p>
let current_metadata: {
    mode: string;
};

// <p>Tell TypeScript about the global namespace this program defines.</p>
declare global {
    interface Window {
        CodeChatEditor_test: any;
    }
}

// <h2>Editors</h2>
// <p>This code instantiates editors/viewers for code and doc blocks.</p>
const make_editors = async (
    // <p>A instance of the <code>EditorMode</code> enum.</p>
    editorMode: EditorMode
) => {
    return new Promise((accept) => {
        setTimeout(async () => {
            // <p>In view mode, don't use TinyMCE, since we already have HTML.
            //     Raw mode doesn't use TinyMCE at all, or even render doc
            //     blocks as HTML.</p>
            if (editorMode === EditorMode.edit) {
                // <p>Instantiate the TinyMCE editor for doc blocks. Wait until
                //     this finishes before calling anything else, to help keep
                //     the UI responsive. TODO: break this up to apply to each
                //     doc block, instead of doing them all at once.</p>
                await make_doc_block_editor(".CodeChat-TinyMCE");
            }

            // <p>Instantiate the Ace editor for code blocks.</p>
            for (const ace_tag of document.querySelectorAll(".CodeChat-ACE")) {
                // <p>Perform each init, then allow UI updates to try and keep
                //     the UI responsive.</p>
                await new Promise((accept) =>
                    setTimeout(() => {
                        make_code_block_editor(ace_tag, editorMode);
                        accept("");
                    })
                );
            }

            // <p>Set up for editing the indent of doc blocks.</p>
            for (const td of document.querySelectorAll(
                ".CodeChat-doc-indent"
            )) {
                // <p>While this follows the <a
                //         href="https://developer.mozilla.org/en-US/docs/Web/API/HTMLElement/beforeinput_event">MDN
                //         docs</a> and also works, TypeScript still reports an
                //     error. Suppress it.</p>
                /// @ts-ignore
                td.addEventListener(
                    "beforeinput",
                    doc_block_indent_on_before_input
                );
            }

            // <p><a id="CodeChatEditor_test"></a>If tests should be run, then
            //     the <a
            //         href="CodeChatEditor-test.mts#CodeChatEditor_test">following
            //         global variable</a> is function that runs them.</p>
            if (typeof window.CodeChatEditor_test === "function") {
                window.CodeChatEditor_test();
            }

            accept("");
        });
    });
};

// <p>Instantiate a doc block editor (TinyMCE).</p>
const make_doc_block_editor = (
    // <p>CSS selector to specify which HTML elements should be editable using
    //     TinyMCE.</p>
    selector: string
) => {
    return tinymce_init({
        // <p>Enable the <a
        //         href="https://www.tiny.cloud/docs/tinymce/6/spelling/#browser_spellcheck">browser-supplied
        //         spellchecker</a>, since TinyMCE's spellchecker is a premium
        //     feature.</p>
        browser_spellcheck: true,
        // <p>Put more buttons on the <a
        //         href="https://www.tiny.cloud/docs/tinymce/6/quickbars/">quick
        //         toolbar</a> that appears when text is selected. TODO: add a
        //     button for code format (can't find this one -- it's only on the
        //     <a
        //         href="https://www.tiny.cloud/docs/tinymce/6/available-menu-items/#the-core-menu-items">list
        //         of menu items</a> as <code>codeformat</code>).</p>
        quickbars_selection_toolbar:
            "align | bold italic underline | quicklink h2 h3 blockquote",
        // <p>Place the Tiny MCE menu bar at the top of the screen; otherwise,
        //     it floats in front of text, sometimes obscuring what the user
        //     wants to edit. See the <a
        //         href="https://www.tiny.cloud/docs/configure/editor-appearance/#fixed_toolbar_container">docs</a>.
        // </p>
        fixed_toolbar_container: "#CodeChat-menu",
        inline: true,
        // <p>When true, this still prevents hyperlinks to anchors on the
        //     current page from working correctly. There's an onClick handler
        //     that prevents links in the current page from working -- need to
        //     look into this. See also <a
        //         href="https://github.com/tinymce/tinymce/issues/3836">a
        //         related GitHub issue</a>.</p>
        //readonly: true  // Per the comment above, this is commented out.
        // <p>TODO: Notes on this setting.</p>
        relative_urls: true,
        selector: selector,
        // <p>This combines the <a
        //         href="https://www.tiny.cloud/blog/tinymce-toolbar/">default
        //         TinyMCE toolbar buttons</a> with a few more from plugins. I
        //     like the default, so this is currently disabled.</p>
        //toolbar: 'undo redo | styleselect | bold italic | alignleft aligncenter alignright alignjustify | outdent indent | numlist bullist | ltr rtl | help',

        // <p>Settings for plugins</p>
        // <p><a
        //         href="https://www.tiny.cloud/docs/plugins/opensource/image/">Image</a>
        // </p>
        image_caption: true,
        image_advtab: true,
        image_title: true,
        // <p>Needed to allow custom elements.</p>
        extended_valid_elements:
            "graphviz-graph[graph|scale],graphviz-script-editor[value|tab],graphviz-combined[graph|scale]",
        custom_elements:
            "graphviz-graph,graphviz-script-editor,graphviz-combined",
    });
};

// <p>Instantiate the code block editor (the Ace editor).</p>
const make_code_block_editor = (
    // <p>The HTML element which contains text to be edited by the Ace editor.
    // </p>
    element: Element,
    // <p>The editor mode; this determines if the editor is in read-only mode
    //     (view/toc EditorModes).</p>
    editorMode: EditorMode
) => {
    ace.edit(element, {
        // <p>The leading <code>+</code> converts the line number from a string
        //     (since all HTML attributes are strings) to a number.</p>
        firstLineNumber: +(
            element.getAttribute("data-CodeChat-firstLineNumber") ?? 0
        ),
        // <p>This is distracting, since it highlights one line for each ACE
        //     editor instance on the screen. Better: only show this if the
        //     editor has focus.</p>
        highlightActiveLine: false,
        highlightGutterLine: false,
        maxLines: 1e10,
        mode: `ace/mode/${current_metadata["mode"]}`,
        // <p>TODO: this still allows cursor movement. Need something that
        //     doesn't show an edit cursor / can't be selected; arrow keys
        //     should scroll the display, not move the cursor around in the
        //     editor.</p>
        readOnly:
            editorMode === EditorMode.view || editorMode == EditorMode.toc,
        showPrintMargin: false,
        theme: "ace/theme/textmate",
        wrap: true,
    });
};

// <h2>UI</h2>
// <p>Allow only spaces and delete/backspaces when editing the indent of a doc
//     block.</p>
const doc_block_indent_on_before_input = (event: InputEvent) => {
    // <p>Only modify the behavior of inserts.</p>
    if (event.data) {
        // <p>Block any insert that's not an insert of spaces. TODO: need to
        //     support tabs.</p>
        if (event.data !== " ".repeat(event.data.length)) {
            event.preventDefault();
        }
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
    // <p>This is the data to write &mdash; the source code. First, transform
    //     the HTML back into code and doc blocks.</p>
    const source_code = editor_to_code_doc_blocks();
    // <p>Then, wrap these in a <a
    //         href="../server/src/webserver.rs#ClientSourceFile">struct the
    //         server expects</a> and send it.</p>
    await save({
        metadata: current_metadata,
        code_doc_block_arr: source_code,
    });
};

// <p><a id="save"></a>Save the provided contents back to the filesystem, by
//     sending a <code>PUT</code> request to the server. See the <a
//         href="CodeChatEditorServer.v.html#save_file">save_file endpoint</a>.
// </p>
const save = async (contents: AllSource) => {
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

// <h2 id="classified_source_to_html">Convert lexed code into HTML</h2>
// <p>This function converts an array of code/doc blocks into editable HTML.</p>
const classified_source_to_html = (
    classified_source: [string, string | null, string][]
) => {
    // <p>An array of strings for the new content of the current HTML page.</p>
    let html = [];

    // <p>Keep track of the current line number.</p>
    let line = 1;

    for (let [indent, delimiter, contents] of classified_source) {
        // <p><span id="newline-movement">In a code or doc block, omit the last
        //         newline; otherwise, code blocks would show an extra newline
        //         at the end of the block. (Doc blocks ending in a
        //         <code>&lt;pre&gt;</code> tag or something similar would also
        //         have this problem).</span></p>
        const m = contents.match(/\n$/);
        if (m) {
            contents = contents.substring(0, m.index);
        }

        if (delimiter === "") {
            // <p>Code state: emit an ACE editor block.</p>
            // prettier-ignore
            html.push(
                '<div class="CodeChat-code">',
                    // <p>TODO: Add the correct number of spaces here so that
                    //     line numbers stay aligned through the whole file.</p>
                    '<div class="CodeChat-ACE-gutter ace_editor"></div>',
                    `<div class="CodeChat-ACE" data-CodeChat-firstLineNumber="${line}">`,
                        escapeHTML(contents),
                    "</div>",
                "</div>"
            );
        } else {
            // <p>Comment state: insert a TinyMCE editor.</p>
            // prettier-ignore
            html.push(
                '<div class="CodeChat-doc">',
                    // <p>TODO: Add spaces matching the number of digits in the
                    //     ACE gutter's line number. Currently, this is three
                    //     spaces, assuming a file length of 100-999 lines.</p>
                    '<div class="CodeChat-ACE-gutter-padding ace_editor">   </div>',
                    // <p>This is a thin margin which matches what ACE does.</p>
                    '<div class="CodeChat-ACE-padding"></div>',
                    // <p>This doc block's indent. TODO: allow paste, but must
                    //     only allow pasting whitespace.</p>
                    `<div class="ace_editor CodeChat-doc-indent" contenteditable onpaste="return false">${indent}</div>`,
                    // <p>The contents of this doc block.</p>
                    `<div class="CodeChat-TinyMCE" data-CodeChat-comment="${delimiter}" id="mce-${line}">`,
                        contents,
                    '</div>',
                '</div>'
            );
        }

        // <p>There are an unknown number of newlines in this source string. One
        //     was removed <a href="#newline-movement">here</a>, so include that
        //     in the count.</p>
        line += 1 + (contents.match(/\n/g) || []).length;
    }

    return html.join("");
};

// <h2>Convert HTML to lexed code</h2>
// <p>This transforms the current editor contents (which are in HTML) into code
//     and doc blocks.</p>
const editor_to_code_doc_blocks = () => {
    // <p>Walk through each code and doc block, extracting its contents then
    //     placing it in <code>classified_lines</code>.</p>
    let classified_lines: code_or_doc_block[] = [];
    for (const code_or_doc_tag of document.querySelectorAll(
        ".CodeChat-ACE, .CodeChat-TinyMCE"
    )) {
        // <p>The type of this block: <code>null</code> for code, or &gt;= 0 for
        //     doc (the value of n specifies the indent in spaces).</p>
        let indent = "";
        // <p>The delimiter for a comment block, or an empty string for a code
        //     block.</p>
        let delimiter: string | null = "";
        // <p>A string containing all the code/docs in this block.</p>
        let full_string;

        // <p>Get the type of this block and its contents.</p>
        if (code_or_doc_tag.classList.contains("CodeChat-ACE")) {
            // <p>See if the Ace editor was applied to this element.</p>
            full_string =
                // <p>TypeScript knows that an element doesn't have a
                //     <code>env</code> attribute; ignore this, since Ace
                //     elements do.</p>
                /// @ts-ignore
                code_or_doc_tag.env === undefined
                    ? unescapeHTML(code_or_doc_tag.innerHTML)
                    : ace.edit(code_or_doc_tag).getValue();
        } else if (code_or_doc_tag.classList.contains("CodeChat-TinyMCE")) {
            // <p>Get the indent from the previous table cell. For a CodeChat
            //     Editor document, there's no indent (it's just a doc block).
            //     Likewise, get the delimiter; leaving it blank for a CodeChat
            //     Editor document causes the next block of code to leave off
            //     the comment delimiter, which is what we want.</p>
            if (!is_doc_only()) {
                indent =
                    code_or_doc_tag.previousElementSibling!.textContent ?? "";
                // <p>Use the pre-existing delimiter for this block if it
                //     exists; otherwise, use the default delimiter.</p>
                delimiter =
                    code_or_doc_tag.getAttribute("data-CodeChat-comment") ??
                    null;
            }
            // <p>See <a
            //         href="https://www.tiny.cloud/docs/tinymce/6/apis/tinymce.root/#get"><code>get</code></a>
            //     and <a
            //         href="https://www.tiny.cloud/docs/tinymce/6/apis/tinymce.editor/#getContent"><code>getContent()</code></a>.
            //     If this element wasn't managed by TinyMCE, it returns
            //     <code>null</code>, in which case we can directly get the
            //     <code>innerHTML</code>.</p>
            // <p>Ignore the missing <code>get</code> type definition.</p>
            /// @ts-ignore
            const tinymce_inst = tinymce.get(code_or_doc_tag.id);
            const html =
                tinymce_inst === null
                    ? code_or_doc_tag.innerHTML
                    : tinymce_inst.getContent();
            // <p>The HTML from TinyMCE is a mess! Wrap at 80 characters,
            //     including the length of the indent and comment string.</p>
            full_string = html_beautify(html, {
                wrap_line_length:
                    80 - indent.length - (delimiter?.length ?? 1) - 1,
            });
        } else {
            throw `Unexpected class for code or doc block ${code_or_doc_tag}.`;
        }

        // <p>There's an implicit newline at the end of each block; restore it.
        // </p>
        full_string += "\n";

        // <p>Merge this with previous classified line if indent and delimiter
        //     are the same; otherwise, add a new entry.</p>
        if (
            classified_lines.length &&
            classified_lines.at(-1)![0] === indent &&
            classified_lines.at(-1)![1] == delimiter
        ) {
            classified_lines.at(-1)![2] += full_string;
        } else {
            classified_lines.push([indent, delimiter, full_string]);
        }
    }

    return classified_lines;
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
    editor_to_code_doc_blocks,
    EditorMode,
    open_lp,
};
