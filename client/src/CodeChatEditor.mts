// Copyright (C) 2022 Bryan A. Jones.
//
// This file is part of the CodeChat Editor.
//
// The CodeChat Editor is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
//
// The CodeChat Editor is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with the CodeChat Editor. If not, see [http://www.gnu.org/licenses/](http://www.gnu.org/licenses/).
//
// `CodeChat-editor.mts` — TypeScript which implements part of the client-side portion of the CodeChat Editor
// ==========================================================================================================
//
// The overall process of load a file is:
//
// 1.  The user browses to a file on the local machine, using the very simple file browser webpage provided by the CodeChat Server. Clicking on this file starts the process of loading a file into the CodeChat editor.
// 2.  The server sees a request for a file supported by the CodeChat Editor. It lexes the files into code and doc blocks, then wraps these in a webpage which contains this program (the CodeChat Editor).
// 3.  On load, this program (the CodeChat Editor) transforms these code and doc blocks into HTML. Specifically, code blocks are placed in [ACE editor](https://ace.c9.io/) instances, while doc blocks are placed in [TinyMCE](https://www.tiny.cloud/) instances.
//
// The user then uses the editing capabilities of ACE/TinyMCE to edit their program. When the user saves a file:
//
// 1.  This program looks through the HTML, converting ACE editor/TinyMCE instances back into code blocks and doc blocks.
// 2.  It sends these code/doc blocks back to the server.
// 3.  The server then transforms these code/doc blocks into source code, then writes this code to the disk.
//
// Imports
// -------
//
// ### JavaScript/TypeScript
import { ace } from "./ace-webpack.mjs";
import "./EditorComponents.mjs";
import "./graphviz-webcomponent-setup.mts";
import "./graphviz-webcomponent/index.min.mjs";
import { html_beautify } from "js-beautify";
import { tinymce, tinymce_init } from "./tinymce-webpack.mjs";
import TurndownService from "turndown";

// Not exactly an import, but this seems like the place to instantiate this
const turndownService = new TurndownService();

// ### CSS
import "./../static/css/CodeChatEditor.css";

// Initialization
// --------------
//
// Load code when the DOM is ready.
export const page_init = (all_source: any) => {
    // Use [URLSearchParams](https://developer.mozilla.org/en-US/docs/Web/API/URLSearchParams) to parse out the search parameters of this window's URL.
    const urlParams = new URLSearchParams(window.location.search);
    // Get the mode from the page's query parameters. Default to edit using the [nullish coalescing operator](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Operators/Nullish_coalescing_operator). This works, but TypeScript marks it as an error. Ignore this error by including the [@ts-ignore directive](https://www.typescriptlang.org/docs/handbook/intro-to-js-ts.html#ts-check).
    /// @ts-ignore
    const editorMode = EditorMode[urlParams.get("mode") ?? "edit"];
    on_dom_content_loaded(() => open_lp(all_source, editorMode));
};

// This is copied from [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Document/DOMContentLoaded_event#checking_whether_loading_is_already_complete).
const on_dom_content_loaded = (on_load_func: () => void) => {
    if (document.readyState === "loading") {
        // Loading hasn't finished yet.
        document.addEventListener("DOMContentLoaded", on_load_func);
    } else {
        // `DOMContentLoaded` has already fired.
        on_load_func();
    }
};

// Define all possible editor modes; these are passed as a [query string](https://en.wikipedia.org/wiki/Query_string) (`http://path/to/foo.py?mode=toc`, for example) to the page's URL.
enum EditorMode {
    // Display the source code using CodeChat, but disallow editing.
    view,
    // For this source, the same a view; the server uses this to avoid recursive iframes of the table of contents.
    toc,
    // The full CodeChat editor.
    edit,
    // Show only raw source code; ignore doc blocks, treating them also as code.
    raw,
}

// This function is called on page load to "load" a file. Before this point, the server has already lexed the source file into code and doc blocks; this function transforms the code and doc blocks into HTML and updates the current web page with the results.
const open_lp = (
    // A data structure provided by the server, containing the source and associated metadata. See [`AllSource`](#AllSource).
    all_source: AllSource,
    // See `[EditorMode](#EditorMode)`.
    editorMode: EditorMode
) => {
    // Get the `[current_metadata](#current_metadata)` from the provided `all_source` struct and store it as a global variable.
    current_metadata = all_source["metadata"];
    const code_doc_block_arr = all_source["code_doc_block_arr"];
    let html;
    if (is_doc_only()) {
        // Special case: a CodeChat Editor document's HTML doesn't need lexing; it only contains HTML. Instead, its structure is always: `[["", "", HTML]]`. Therefore, the HTML is at item \[0\]\[2\].
        html = `<div class="CodeChat-TinyMCE">${code_doc_block_arr[0][2]}</div>`;
    } else {
        html = classified_source_to_html(code_doc_block_arr);
    }

    document.getElementById("CodeChat-body")!.innerHTML = html;
    // Initialize editors for this new content. Return a promise which is accepted when the new content is ready.
    return make_editors(editorMode);
};

// This defines a single code or doc block entry:
export type code_or_doc_block = [
    // The indent for a doc bloc; empty for a code block.
    string,
    // The opening comment delimiter for a doc block;
    string | null,
    string
];

// The server passes this to the client to load a file. See [LexedSourceFile](../../server/src/webserver.rs#LexedSourceFile).
type AllSource = {
    metadata: { mode: string };
    code_doc_block_arr: code_or_doc_block[];
};

// Store the lexer info for the currently-loaded language.
//
// This mirrors the data provided by the server -- see [SourceFileMetadata](../../server/src/webserver.rs#SourceFileMetadata).
let current_metadata: {
    mode: string;
};

// Tell TypeScript about the global namespace this program defines.
declare global {
    interface Window {
        CodeChatEditor_test: any;
    }
}

// Editors
// -------
//
// This code instantiates editors/viewers for code and doc blocks.
const make_editors = async (
    // A instance of the `EditorMode` enum.
    editorMode: EditorMode
) => {
    return new Promise((accept) => {
        setTimeout(async () => {
            // In view mode, don't use TinyMCE, since we already have HTML. Raw mode doesn't use TinyMCE at all, or even render doc blocks as HTML.
            if (editorMode === EditorMode.edit) {
                // Instantiate the TinyMCE editor for doc blocks. Wait until this finishes before calling anything else, to help keep the UI responsive. TODO: break this up to apply to each doc block, instead of doing them all at once.
                await make_doc_block_editor(".CodeChat-TinyMCE");
            }

            // Instantiate the Ace editor for code blocks.
            for (const ace_tag of document.querySelectorAll(".CodeChat-ACE")) {
                // Perform each init, then allow UI updates to try and keep the UI responsive.
                await new Promise((accept) =>
                    setTimeout(() => {
                        make_code_block_editor(ace_tag, editorMode);
                        accept("");
                    })
                );
            }

            // Set up for editing the indent of doc blocks.
            for (const td of document.querySelectorAll(
                ".CodeChat-doc-indent"
            )) {
                // While this follows the [MDN docs](https://developer.mozilla.org/en-US/docs/Web/API/HTMLElement/beforeinput_event) and also works, TypeScript still reports an error. Suppress it.
                /// @ts-ignore
                td.addEventListener(
                    "beforeinput",
                    doc_block_indent_on_before_input
                );
            }

            // If tests should be run, then the [following global variable](CodeChatEditor-test.mts#CodeChatEditor_test) is function that runs them.
            if (typeof window.CodeChatEditor_test === "function") {
                window.CodeChatEditor_test();
            }

            accept("");
        });
    });
};

// Instantiate a doc block editor (TinyMCE).
const make_doc_block_editor = (
    // CSS selector to specify which HTML elements should be editable using TinyMCE.
    selector: string
) => {
    return tinymce_init({
        // Enable the [browser-supplied spellchecker](https://www.tiny.cloud/docs/tinymce/6/spelling/#browser_spellcheck), since TinyMCE's spellchecker is a premium feature.
        browser_spellcheck: true,
        // Put more buttons on the [quick toolbar](https://www.tiny.cloud/docs/tinymce/6/quickbars/) that appears when text is selected. TODO: add a button for code format (can't find this one -- it's only on the [list of menu items](https://www.tiny.cloud/docs/tinymce/6/available-menu-items/#the-core-menu-items) as `codeformat`).
        quickbars_selection_toolbar:
            "align | bold italic underline | quicklink h2 h3 blockquote",
        // Place the Tiny MCE menu bar at the top of the screen; otherwise, it floats in front of text, sometimes obscuring what the user wants to edit. See the [docs](https://www.tiny.cloud/docs/configure/editor-appearance/#fixed_toolbar_container).
        fixed_toolbar_container: "#CodeChat-menu",
        inline: true,
        // When true, this still prevents hyperlinks to anchors on the current page from working correctly. There's an onClick handler that prevents links in the current page from working -- need to look into this. See also [a related GitHub issue](https://github.com/tinymce/tinymce/issues/3836).
        //readonly: true  // Per the comment above, this is commented out.
        // TODO: Notes on this setting.
        relative_urls: true,
        selector: selector,
        // This combines the [default TinyMCE toolbar buttons](https://www.tiny.cloud/blog/tinymce-toolbar/) with a few more from plugins. I like the default, so this is currently disabled.
        //toolbar: 'undo redo | styleselect | bold italic | alignleft aligncenter alignright alignjustify | outdent indent | numlist bullist | ltr rtl | help',

        // Settings for plugins
        //
        // [Image](https://www.tiny.cloud/docs/plugins/opensource/image/)
        image_caption: true,
        image_advtab: true,
        image_title: true,
        // Needed to allow custom elements.
        extended_valid_elements:
            "graphviz-graph[graph|scale],graphviz-script-editor[value|tab],graphviz-combined[graph|scale]",
        custom_elements:
            "graphviz-graph,graphviz-script-editor,graphviz-combined",
    });
};

// Instantiate the code block editor (the Ace editor).
const make_code_block_editor = (
    // The HTML element which contains text to be edited by the Ace editor.
    element: Element,
    // The editor mode; this determines if the editor is in read-only mode (view/toc EditorModes).
    editorMode: EditorMode
) => {
    ace.edit(element, {
        // The leading `+` converts the line number from a string (since all HTML attributes are strings) to a number.
        firstLineNumber: +(
            element.getAttribute("data-CodeChat-firstLineNumber") ?? 0
        ),
        // This is distracting, since it highlights one line for each ACE editor instance on the screen. Better: only show this if the editor has focus.
        highlightActiveLine: false,
        highlightGutterLine: false,
        maxLines: 1e10,
        mode: `ace/mode/${current_metadata["mode"]}`,
        // TODO: this still allows cursor movement. Need something that doesn't show an edit cursor / can't be selected; arrow keys should scroll the display, not move the cursor around in the editor.
        readOnly:
            editorMode === EditorMode.view || editorMode == EditorMode.toc,
        showPrintMargin: false,
        theme: "ace/theme/textmate",
        wrap: true,
    });
};

// UI
// --
//
// Allow only spaces and delete/backspaces when editing the indent of a doc block.
const doc_block_indent_on_before_input = (event: InputEvent) => {
    // Only modify the behavior of inserts.
    if (event.data) {
        // Block any insert that's not an insert of spaces. TODO: need to support tabs.
        if (event.data !== " ".repeat(event.data.length)) {
            event.preventDefault();
        }
    }
};

// Provide a shortcut of ctrl-s (or command-s) to save the current file.
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

// Save CodeChat Editor contents.
export const on_save = async () => {
    // This is the data to write — the source code. First, transform the HTML back into code and doc blocks.
    console.log(turndownService);
    const source_code = editor_to_code_doc_blocks();
    // Then, wrap these in a [struct the server expects](../server/src/webserver.rs#ClientSourceFile) and send it.
    await save({
        metadata: current_metadata,
        code_doc_block_arr: source_code,
    });
};

// Save the provided contents back to the filesystem, by sending a `PUT` request to the server. See the [save\_file endpoint](CodeChatEditorServer.v.html#save_file).
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

// Convert lexed code into HTML
// ----------------------------
//
// This function converts an array of code/doc blocks into editable HTML.
const classified_source_to_html = (
    classified_source: [string, string | null, string][]
) => {
    // An array of strings for the new content of the current HTML page.
    let html = [];

    // Keep track of the current line number.
    let line = 1;

    for (let [indent, delimiter, contents] of classified_source) {
        // In a code or doc block, omit the last newline; otherwise, code blocks would show an extra newline at the end of the block. (Doc blocks ending in a `<pre>` tag or something similar would also have this problem).
        const m = contents.match(/\n$/);
        if (m) {
            contents = contents.substring(0, m.index);
        }

        if (delimiter === "") {
            // Code state: emit an ACE editor block.
            // prettier-ignore
            html.push(
                '<div class="CodeChat-code">',
                    // TODO: Add the correct number of spaces here so that line numbers stay aligned through the whole file.
                    '<div class="CodeChat-ACE-gutter ace_editor"></div>',
                    `<div class="CodeChat-ACE" data-CodeChat-firstLineNumber="${line}">`,
                        escapeHTML(contents),
                    "</div>",
                "</div>"
            );
        } else {
            // Comment state: insert a TinyMCE editor.
            // prettier-ignore
            html.push(
                '<div class="CodeChat-doc">',
                    // TODO: Add spaces matching the number of digits in the ACE gutter's line number. Currently, this is three spaces, assuming a file length of 100-999 lines.
                    '<div class="CodeChat-ACE-gutter-padding ace_editor">   </div>',
                    // This is a thin margin which matches what ACE does.
                    '<div class="CodeChat-ACE-padding"></div>',
                    // This doc block's indent. TODO: allow paste, but must only allow pasting whitespace.
                    `<div class="ace_editor CodeChat-doc-indent" contenteditable onpaste="return false">${indent}</div>`,
                    // The contents of this doc block.
                    `<div class="CodeChat-TinyMCE" data-CodeChat-comment="${delimiter}" id="mce-${line}">`,
                        contents,
                    '</div>',
                '</div>'
            );
        }

        // There are an unknown number of newlines in this source string. One was removed [here](#newline-movement), so include that in the count.
        line += 1 + (contents.match(/\n/g) || []).length;
    }

    return html.join("");
};

// Convert HTML to lexed code
// --------------------------
//
// This transforms the current editor contents (which are in HTML) into code and doc blocks.
const editor_to_code_doc_blocks = () => {
    // Walk through each code and doc block, extracting its contents then placing it in `classified_lines`.
    let classified_lines: code_or_doc_block[] = [];
    for (const code_or_doc_tag of document.querySelectorAll(
        ".CodeChat-ACE, .CodeChat-TinyMCE"
    )) {
        // The type of this block: `null` for code, or >= 0 for doc (the value of n specifies the indent in spaces).
        let indent = "";
        // The delimiter for a comment block, or an empty string for a code block.
        let delimiter: string | null = "";
        // A string containing all the code/docs in this block.
        let full_string;

        // Get the type of this block and its contents.
        if (code_or_doc_tag.classList.contains("CodeChat-ACE")) {
            // See if the Ace editor was applied to this element.
            full_string =
                // TypeScript knows that an element doesn't have a `env` attribute; ignore this, since Ace elements do.
                /// @ts-ignore
                code_or_doc_tag.env === undefined
                    ? unescapeHTML(code_or_doc_tag.innerHTML)
                    : ace.edit(code_or_doc_tag).getValue();
        } else if (code_or_doc_tag.classList.contains("CodeChat-TinyMCE")) {
            // Get the indent from the previous table cell. For a CodeChat Editor document, there's no indent (it's just a doc block). Likewise, get the delimiter; leaving it blank for a CodeChat Editor document causes the next block of code to leave off the comment delimiter, which is what we want.
            if (!is_doc_only()) {
                indent =
                    code_or_doc_tag.previousElementSibling!.textContent ?? "";
                // Use the pre-existing delimiter for this block if it exists; otherwise, use the default delimiter.
                delimiter =
                    code_or_doc_tag.getAttribute("data-CodeChat-comment") ??
                    null;
            }
            // See [`get`](https://www.tiny.cloud/docs/tinymce/6/apis/tinymce.root/#get) and [`getContent()`](https://www.tiny.cloud/docs/tinymce/6/apis/tinymce.editor/#getContent). If this element wasn't managed by TinyMCE, it returns `null`, in which case we can directly get the `innerHTML`.
            //
            // Ignore the missing `get` type definition.
            /// @ts-ignore
            const tinymce_inst = tinymce.get(code_or_doc_tag.id);
            const html =
                tinymce_inst === null
                    ? code_or_doc_tag.innerHTML
                    : tinymce_inst.getContent();
            // The HTML from TinyMCE is a mess! Wrap at 80 characters, including the length of the indent and comment string.
            full_string = html_beautify(html, {
                wrap_line_length:
                    80 - indent.length - (delimiter?.length ?? 1) - 1,
            });
            full_string = turndownService.turndown(full_string);
            console.log(full_string);
        } else {
            throw `Unexpected class for code or doc block ${code_or_doc_tag}.`;
        }

        // There's an implicit newline at the end of each block; restore it.
        full_string += "\n";

        // Merge this with previous classified line if indent and delimiter are the same; otherwise, add a new entry.
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

// Helper functions
// ----------------
//
// Given text, escape it so it formats correctly as HTML. Because the solution at [SO](https://stackoverflow.com/a/48054293) transforms newlines in odd ways (see [innerText](https://developer.mozilla.org/en-US/docs/Web/API/HTMLElement/innerText)), it's not usable with code. Instead, this is a translation of Python's `html.escape` function.
const escapeHTML = (unsafeText: string): string => {
    // Must be done first!
    unsafeText = unsafeText.replaceAll("&", "&amp;");
    unsafeText = unsafeText.replaceAll("<", "&lt;");
    unsafeText = unsafeText.replaceAll(">", "&gt;");
    return unsafeText;
};

// This handles only three HTML entities, but no others!
const unescapeHTML = (html: string): string => {
    let text = html.replaceAll("&gt;", ">");
    text = text.replaceAll("&lt;", "<");
    text = text.replaceAll("&amp;", "&");
    return text;
};

// True if this is a CodeChat Editor document (not a source file).
const is_doc_only = () => {
    return current_metadata["mode"] === "codechat-html";
};

// Per [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Navigator/platform#examples), here's the least bad way to choose between the control key and the command key.
const os_is_osx =
    navigator.platform.indexOf("Mac") === 0 || navigator.platform === "iPhone"
        ? true
        : false;

// A great and simple idea taken from [SO](https://stackoverflow.com/a/54116079): wrap all testing exports in a single variable. This avoids namespace pollution, since only one name is exported, and it's clearly marked for testing only. Test code still gets access to everything it needs.
export const exportedForTesting = {
    editor_to_code_doc_blocks,
    EditorMode,
    open_lp,
};
