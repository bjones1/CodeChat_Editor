// Copyright (C) 2023 Bryan A. Jones.
//
// This file is part of the CodeChat Editor. The CodeChat Editor is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, either
// version 3 of the License, or (at your option) any later version.
//
// The CodeChat Editor is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along with
// the CodeChat Editor. If not, see
// [http://www.gnu.org/licenses](http://www.gnu.org/licenses).
//
// # `CodeChat-editor.mts` -- TypeScript which implements part of the client-side portion of the CodeChat Editor
//
// The overall process of load a file is:
//
// 1.  The user browses to a file on the local machine, using the very simple
//     file browser webpage provided by the CodeChat Server. Clicking on this
//     file starts the process of loading a file into the CodeChat editor.
// 2.  The server sees a request for a file supported by the CodeChat Editor. It
//     lexes the files into code and doc blocks, then wraps these in a webpage
//     which contains this program (the CodeChat Editor).
// 3.  On load, this program (the CodeChat Editor) transforms these code and doc
//     blocks into HTML. Specifically, code blocks are placed in
//     [ACE editor](https://ace.c9.io/) instances, while doc blocks are placed
//     in [TinyMCE](https://www.tiny.cloud/) instances.
//
// The user then uses the editing capabilities of ACE/TinyMCE to edit their
// program. When the user saves a file:
//
// 1.  This program looks through the HTML, converting ACE editor/TinyMCE
//     instances back into code blocks and doc blocks.
// 2.  It sends these code/doc blocks back to the server.
// 3.  The server then transforms these code/doc blocks into source code, then
//     writes this code to the disk.
//
// ## Imports
//
// ### JavaScript/TypeScript
import "./EditorComponents.mjs";
import prettier from "prettier/esm/standalone.mjs";
import parserMarkdown from "prettier/esm/parser-markdown.mjs";
import "./graphviz-webcomponent-setup.mts";
import "graphviz-webcomponent";
import { tinymce, init } from "./tinymce-config.mjs";
import TurndownService from "./turndown/turndown.browser.es.js";
import { gfm } from "./turndown/turndown-plugin-gfm.browser.es.js";

// ### CSS
import "./../static/css/CodeChatEditor.css";
import { CodeMirror_load, CodeMirror_save } from "./CodeMirror-integration.mjs";

// ## Initialization
//
// Instantiate \[turndown\](https://github.com/mixmark-io/turndown) for HTML to
// Markdown conversion
const turndownService = new TurndownService({
    br: "\\",
    codeBlockStyle: "fenced",
    renderAsPure: false,
});

// Add the plugins from
// [turndown-plugin-gfm](https://github.com/laurent22/joplin/tree/dev/packages/turndown-plugin-gfm)
// to enable conversions for tables, task lists, and strikethroughs.
turndownService.use(gfm);
// The server passes this to the client to load a file. See
// [LexedSourceFile](../../server/src/webserver.rs#LexedSourceFile).
type CodeChatForWeb = {
    metadata: { mode: string };
    source: {
        doc: string;
        doc_blocks: DocBlockJSON[];
        selection: any;
    };
};

// Store the lexer info for the currently-loaded language.
//
// <a id="current_metadata"></a>This mirrors the data provided by the server --
// see [SourceFileMetadata](../../server/src/webserver.rs#SourceFileMetadata).
let current_metadata: {
    mode: string;
};

// Load code when the DOM is ready.
export const page_init = (all_source: any) => {
    // Use
    // [URLSearchParams](https://developer.mozilla.org/en-US/docs/Web/API/URLSearchParams)
    // to parse out the search parameters of this window's URL.
    const urlParams = new URLSearchParams(window.location.search);
    // Get the mode from the page's query parameters. Default to edit using the
    // [nullish coalescing operator](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Operators/Nullish_coalescing_operator).
    // This works, but TypeScript marks it as an error. Ignore this error by
    // including the
    // [@ts-ignore directive](https://www.typescriptlang.org/docs/handbook/intro-to-js-ts.html#ts-check).
    /// @ts-ignore
    const editorMode = EditorMode[urlParams.get("mode") ?? "edit"];
    on_dom_content_loaded(async () => {
        open_lp(all_source, editorMode);
    });
};

// This is copied from
// [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Document/DOMContentLoaded_event#checking_whether_loading_is_already_complete).
const on_dom_content_loaded = (on_load_func: () => void) => {
    if (document.readyState === "loading") {
        // Loading hasn't finished yet.
        document.addEventListener("DOMContentLoaded", on_load_func);
    } else {
        // `DOMContentLoaded` has already fired.
        on_load_func();
    }
};

// <a id="EditorMode"></a>Define all possible editor modes; these are passed as
// a [query string](https://en.wikipedia.org/wiki/Query_string)
// (`http://path/to/foo.py?mode=toc`, for example) to the page's URL.
enum EditorMode {
    // Display the source code using CodeChat, but disallow editing.
    view,
    // For this source, the same a view; the server uses this to avoid recursive
    // iframes of the table of contents.
    toc,
    // The full CodeChat editor.
    edit,
    // Show only raw source code; ignore doc blocks, treating them also as code.
    raw,
}

// Tell TypeScript about the global namespace this program defines.
declare global {
    interface Window {
        CodeChatEditor_test: any;
    }
}

// This function is called on page load to "load" a file. Before this point, the
// server has already lexed the source file into code and doc blocks; this
// function transforms the code and doc blocks into HTML and updates the current
// web page with the results.
const open_lp = (
    // A data structure provided by the server, containing the source and
    // associated metadata. See [`AllSource`](#AllSource).
    all_source: CodeChatForWeb,
    // See <code><a href="#EditorMode">EditorMode</a></code>.
    editorMode: EditorMode,
) => {
    // Get the <code><a href="#current_metadata">current_metadata</a></code>
    // from the provided `all_source` struct and store it as a global variable.
    current_metadata = all_source["metadata"];
    const source = all_source["source"];
    const codechat_body = document.getElementById(
        "CodeChat-body",
    ) as HTMLDivElement;
    if (is_doc_only()) {
        // Special case: a CodeChat Editor document's HTML is stored in
        // \`source.doc\`. We don't need the CodeMirror editor at all; instead,
        // treat it like a single doc block contents div./p>
        codechat_body.innerHTML = `<div class="CodeChat-doc-contents">${source.doc}</div>`;
        init({ selector: ".CodeChat-doc-contents" }).then((editors) =>
            editors[0].focus(),
        );
    } else {
        CodeMirror_load(codechat_body, source);
    }

    // <a id="CodeChatEditor_test"></a>If tests should be run, then the
    // [following global variable](CodeChatEditor-test.mts#CodeChatEditor_test)
    // is function that runs them.
    if (typeof window.CodeChatEditor_test === "function") {
        window.CodeChatEditor_test();
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
    /// @ts-expect-error
    let source: CodeChatForWeb["source"] = {};
    if (is_doc_only()) {
        // To save a document only, simply get the HTML from the only Tiny MCE
        // div.
        const html = tinymce.get(0)!.getContent();
        const markdown = turndownService.turndown(html);
        source.doc = await prettier_markdown(markdown, 80);
        source.doc_blocks = [];
    } else {
        source = CodeMirror_save();
        // Join all the doc blocks, then convert them to Markdown, then split
        // them back.
        //
        // Turndown currently removes HTML blocks with no content; add random
        // content to avoid this.
        const separator =
            "<codechateditor-separator>a</codechateditor-separator>";
        const combined_doc_blocks_html = source.doc_blocks
            .map((doc_block_JSON) => doc_block_JSON[4])
            .join(separator);
        const combined_doc_blocks_markdown = turndownService.turndown(
            combined_doc_blocks_html,
        );
        const doc_blocks_markdown = combined_doc_blocks_markdown.split(
            `\n${separator}\n\n`,
        );
        // Wrap each doc block based on the available width on this line: 80 -
        // indent - delimiter length - 1 space that always follows the
        // delimiter. Use a minimum width of 40 characters.
        for (const [index, doc_block] of source.doc_blocks.entries()) {
            doc_block[4] = await prettier_markdown(
                doc_blocks_markdown[index],
                Math.max(
                    40,
                    80 - doc_block[3].length - doc_block[2].length - 1,
                ),
            );
        }
    }
    await save({
        metadata: current_metadata,
        source,
    });
};

// <a id="save"></a>Save the provided contents back to the filesystem, by
// sending a `PUT` request to the server. See the
// [save_file endpoint](CodeChatEditorServer.v.html#save_file).
const save = async (contents: CodeChatForWeb) => {
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
        `Save failed -- server returned ${response.status}, ${response.statusText}.`,
    );
};

// ## Helper functions
const prettier_markdown = async (markdown: string, print_width: number) => {
    return await prettier.format(markdown, {
        // See
        // [prettier from ES modules](https://prettier.io/docs/en/browser.html#es-modules).
        parser: "markdown",
        // TODO:
        //
        // - Unfortunately, Prettier doesn't know how to format HTML embedded in
        //   Markdown; see
        //   [issue 8480](https://github.com/prettier/prettier/issues/8480).
        // - Prettier formats headings using the ATX style; this isn't
        //   configurable per the
        //   [source](https://github.com/prettier/prettier/blob/main/src/language-markdown/printer-markdown.js#L228).
        plugins: [parserMarkdown],
        // See [prettier options](https://prettier.io/docs/en/options.html).
        printWidth: print_width,
        // Without this option, most lines aren't wrapped.
        proseWrap: "always",
    });
};

// True if this is a CodeChat Editor document (not a source file).
const is_doc_only = () => {
    return current_metadata["mode"] === "markdown";
};

// Per
// [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Navigator/platform#examples),
// here's the least bad way to choose between the control key and the command
// key.
const os_is_osx =
    navigator.platform.indexOf("Mac") === 0 || navigator.platform === "iPhone"
        ? true
        : false;

// A great and simple idea taken from
// [SO](https://stackoverflow.com/a/54116079): wrap all testing exports in a
// single variable. This avoids namespace pollution, since only one name is
// exported, and it's clearly marked for testing only. Test code still gets
// access to everything it needs.
export const exportedForTesting = {
    EditorMode,
    open_lp,
};
