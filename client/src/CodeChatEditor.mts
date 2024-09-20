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
// # `CodeChatEditor.mts` -- the CodeChat Editor Client
//
// The overall process of load a file is:
//
// 1.  The user browses to a file on the local machine, using the very simple
//     file browser webpage provided by the CodeChat Server. Clicking on this
//     file starts the process of loading a file into the CodeChat editor.
// 2.  The server sees a request for a file supported by the CodeChat Editor. It
//     lexes the files into code and doc blocks, then wraps these in a webpage
//     which contains this program (the CodeChat Editor).
// 3.  On load, this program (the CodeChat Editor) loads these code and doc
//     blocks into the CodeMirror text editor, using TinyMCE to provide a GUI
//     editor within CodeMirror for doc blocks.
//
// The user then uses the editing capabilities of CodeMirror/TinyMCE to edit
// their program. When the user saves a file:
//
// 1.  This program serializes the CodeMirror text plus doc blocks, and
//     transforms HTML back to markdown.
// 2.  It sends these code/doc blocks back to the server.
// 3.  The server then transforms these code/doc blocks into source code, then
//     writes this code to the disk.
//
// ## Imports

// ### JavaScript/TypeScript
//
// #### Third-party
import { EditorView, ViewUpdate } from "@codemirror/view";
import prettier from "prettier/esm/standalone.mjs";
import parserMarkdown from "prettier/esm/parser-markdown.mjs";
import TurndownService from "./turndown/turndown.browser.es.js";
import { gfm } from "./turndown/turndown-plugin-gfm.browser.es.js";

// #### Local
import {
    CodeMirror_load,
    CodeMirror_save,
    addDocBlock,
    updateDocBlock,
} from "./CodeMirror-integration.mjs";
import "./EditorComponents.mjs";
import "./graphviz-webcomponent-setup.mts";
// This must be imported _after_ the previous setup import, so it's placed here,
// instead of in the third-party category above.
import "graphviz-webcomponent";
import { tinymce, init, Editor } from "./tinymce-config.mjs";

// ### CSS
import "./../static/css/CodeChatEditor.css";

// ## Markdown to HTML conversion
//
// Instantiate [turndown](https://github.com/mixmark-io/turndown) for HTML to
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
// ## Page initialization
//
// Load the dynamic content into the static page.
export const page_init = () => {
    on_dom_content_loaded(async () => {
        document
            .getElementById("CodeChat-save-button")!
            .addEventListener("click", on_save);
        document.body.addEventListener("keydown", on_keydown);

        // Intercept links in this document to save before following the link.
        /// @ts-ignore
        navigation.addEventListener("navigate", on_navigate);
        const ccb = document.getElementById("CodeChat-sidebar") as
            | HTMLIFrameElement
            | undefined;
        /// @ts-ignore
        ccb?.contentWindow?.navigation.addEventListener(
            "navigate",
            on_navigate,
        );

        window.CodeChatEditor = {
            open_lp,
        };
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

// ## File handling
//
// Store the lexer info for the currently-loaded language.
//
// <a id="current_metadata"></a>This mirrors the data provided by the server --
// see [SourceFileMetadata](../../server/src/webserver.rs#SourceFileMetadata).
let current_metadata: {
    mode: string;
};

// True if this is a CodeChat Editor document (not a source file).
const is_doc_only = () => {
    return current_metadata["mode"] === "markdown";
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

// This function is called on page load to "load" a file. Before this point, the
// server has already lexed the source file into code and doc blocks; this
// function transforms the code and doc blocks into HTML and updates the current
// web page with the results.
export const open_lp = async (
    // A data structure provided by the server, containing the source and
    // associated metadata. See [`AllSource`](#AllSource).
    all_source: CodeChatForWeb,
) => {
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

    // Get the <code><a href="#current_metadata">current_metadata</a></code>
    // from the provided `all_source` struct and store it as a global variable.
    current_metadata = all_source["metadata"];
    const source = all_source["source"];
    const codechat_body = document.getElementById(
        "CodeChat-body",
    ) as HTMLDivElement;
    if (is_doc_only()) {
        if (tinymce.activeEditor === null) {
            // Special case: a CodeChat Editor document's HTML is stored in
            // `source.doc`. We don't need the CodeMirror editor at all;
            // instead, treat it like a single doc block contents div.
            codechat_body.innerHTML = `<div class="CodeChat-doc-contents">${source.doc}</div>`;
            await init({
                selector: ".CodeChat-doc-contents",
                // In the doc-only mode, add autosave functionality. While there
                // is an
                // [autosave plugin](https://www.tiny.cloud/docs/tinymce/6/autosave/),
                // this autosave functionality is completely different from the
                // autosave provided here. Per
                // [handling editor events](https://www.tiny.cloud/docs/tinymce/6/events/#handling-editor-events),
                // this is how to create a TinyMCE event handler.
                setup: (editor: Editor) => {
                    // The
                    // [supported browser-native events list](https://www.tiny.cloud/docs/tinymce/6/events/#supported-browser-native-events)
                    // includes the `input` event.
                    editor.on("input", (_event: Event) => {
                        startAutosaveTimer();
                    });
                },
            });
            tinymce.activeEditor!.focus();
        } else {
            // Save and restore cursor/scroll location after an update per the
            // [docs](https://www.tiny.cloud/docs/tinymce/6/apis/tinymce.dom.bookmarkmanager).
            // However, this doesn't seem to work for the cursor location.
            // Perhaps when TinyMCE normalizes the document, this gets lost?
            const bm = tinymce.activeEditor!.selection.getBookmark();
            tinymce.activeEditor!.setContent(source.doc);
            tinymce.activeEditor!.selection.moveToBookmark(bm);
        }
    } else {
        await CodeMirror_load(codechat_body, source, current_metadata.mode, [
            autosaveExtension,
        ]);
    }

    // <a id="CodeChatEditor_test"></a>If tests should be run, then the
    // [following global variable](CodeChatEditor-test.mts#CodeChatEditor_test)
    // is function that runs them.
    if (typeof window.CodeChatEditor_test === "function") {
        window.CodeChatEditor_test();
    }
};

const save_lp = async () => {
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
        await codechat_html_to_markdown(source);
    }

    let update: UpdateMessageContents = {
        contents: {
            metadata: current_metadata,
            source,
        },
        scroll_position: undefined,
        cursor_position: undefined,
    };
    return update;
};

// Per
// [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Navigator/platform#examples),
// here's the least bad way to choose between the control key and the command
// key.
const os_is_osx =
    navigator.platform.indexOf("Mac") === 0 || navigator.platform === "iPhone"
        ? true
        : false;

// Provide a shortcut of ctrl-s (or command-s) to save the current file.
const on_keydown = (event: KeyboardEvent) => {
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
const on_save = async () => {
    // <a id="save"></a>Save the provided contents back to the filesystem, by
    // sending an update message over the websocket.
    const webSocketComm = parent.window.CodeChatEditorFramework.webSocketComm;
    webSocketComm.send_message({ Update: await save_lp() });
};

const codechat_html_to_markdown = async (source: any) => {
    // Join all the doc blocks, then convert them to Markdown, then split them
    // back.
    //
    // Turndown currently removes HTML blocks with no content; add placeholder
    // content to avoid this.
    const separator = "<codechateditor-separator>a</codechateditor-separator>";
    const placeholder_html = "<p><empty-para>a</empty-para></p>";
    const placeholder_markdown = "<empty-para>a</empty-para>";
    // Replace empty doc blocks (which Turndown will remove) with a placeholder
    // to prevent their removal; pass non-empty content for standard Turndown
    // processing.
    const combined_doc_blocks_html = source.doc_blocks
        .map((doc_block_JSON: DocBlockJSON) =>
            doc_block_JSON[4].trim() ? doc_block_JSON[4] : placeholder_html,
        )
        .join(separator);
    const combined_doc_blocks_markdown = turndownService.turndown(
        combined_doc_blocks_html,
    );
    const doc_blocks_markdown = combined_doc_blocks_markdown
        .split(separator)
        .map((s: string) => s.trim());
    // Wrap each doc block based on the available width on this line: 80 -
    // indent - delimiter length - 1 space that always follows the delimiter.
    // Use a minimum width of 40 characters.
    for (const [index, doc_block] of source.doc_blocks.entries()) {
        const dbm = doc_blocks_markdown[index];
        doc_block[4] =
            (await prettier_markdown(
                // Replace the placeholder here, so it won't be wrapped by Prettier.
                dbm == placeholder_markdown ? "" : dbm,
                Math.max(
                    40,
                    80 - doc_block[3].length - doc_block[2].length - 1,
                ),
                // Prettier trims whitespace; we can't include the newline in the
                // replacement above. So, put it here.
            )) || "\n";
    }
};

// ### Autosave feature
//
// The ID of the autosave timer; when this timer expires, the document will be
// autosaved.
let autosaveTimeoutId: null | number = null;

// True to enable autosave.
let autosaveEnabled = true;

// Schedule an autosave; call this whenever the document is modified.
const startAutosaveTimer = () => {
    if (!autosaveEnabled) {
        return;
    }
    // When the document is changed, perform an autosave after no changes have
    // occurred for a little while. To do this, first cancel any current
    // timeout...
    if (autosaveTimeoutId !== null) {
        clearTimeout(autosaveTimeoutId);
    }
    // ...then start another timeout which saves the document when it expires.
    autosaveTimeoutId = window.setTimeout(on_save, 1000);
};

// There doesn't seem to be any tracking of a dirty/clean flag built into
// CodeMirror v6 (although
// [v5 does](https://codemirror.net/5/doc/manual.html#isClean)). The best I've
// found is a
// [forum post](https://discuss.codemirror.net/t/codemirror-6-proper-way-to-listen-for-changes/2395/11)
// showing code to do this, which I use below.
//
// How this works: the
// [EditorView.updateListener](https://codemirror.net/docs/ref/#codemirror) is a
// [Facet](https://codemirror.net/docs/ref/#state.Facet) with an
// [of function](https://codemirror.net/docs/ref/#state.Facet.of) that creates a
// CodeMirror extension.
const autosaveExtension = EditorView.updateListener.of(
    // CodeMirror passes this function a
    // [ViewUpdate](https://codemirror.net/docs/ref/#view.ViewUpdate) which
    // describes a change being made to the document.
    (v: ViewUpdate) => {
        // The
        // [docChanged](https://codemirror.net/docs/ref/#view.ViewUpdate.docChanged)
        // flag is the relevant part of this change description. However, this
        // only describes changes to the code blocks (the document, from
        // CodeMirror's perspective).
        let isChanged = v.docChanged;
        // Look for changes to doc blocks as well; skip if a change was already
        // detected for efficiency.
        if (!v.docChanged && v.transactions?.length) {
            // Check each effect of each transaction.
            outer: for (let tr of v.transactions) {
                for (let effect of tr.effects) {
                    // Look for a change to a doc block.
                    if (effect.is(addDocBlock) || effect.is(updateDocBlock)) {
                        isChanged = true;
                        break outer;
                    }
                }
            }
        }
        if (isChanged) {
            startAutosaveTimer();
        }
    },
);

// User `prettier` to word-wrap Markdown before saving it.
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

// ## Navigation
//
// Since this is experimental, TypeScript doesn't define it. See the
// [docs](https://developer.mozilla.org/en-US/docs/Web/API/NavigateEvent).
interface NavigateEvent extends Event {
    canIntercept: boolean;
    destination: any;
    downloadRequest: String | null;
    formData: any;
    hashChange: boolean;
    info: any;
    navigationType: String;
    signal: AbortSignal;
    userInitiated: boolean;
    intercept: any;
    scroll: any;
}

// The TOC and this page calls this when a hyperlink is clicked. This saves the
// current document before navigating.
const on_navigate = (navigateEvent: NavigateEvent) => {
    if (
        // Some of this was copied from
        // [Modern client-side routing: the Navigation API](https://developer.chrome.com/docs/web-platform/navigation-api/#deciding_how_to_handle_a_navigation).
        // If we're navigating within the document, ignore this.
        navigateEvent.hashChange ||
        // If this is a download, let the browser perform the download.
        navigateEvent.downloadRequest ||
        // If this is a form submission, let that go to the server.
        navigateEvent.formData
    ) {
        return;
    }

    // If we can't intercept this, we can't save the current content. TODO --
    // this is a problem is data wasn't saved! Need a sync way to do this. Store
    // it in local data or something.
    if (!navigateEvent.canIntercept) {
        return;
    }

    // Intercept this navigation so we can save the document first.
    navigateEvent.intercept();
    on_save().then((_value) => {
        // Avoid recursion!
        /// @ts-ignore
        navigation.removeEventListener("navigate", on_navigate);
        parent.window.CodeChatEditorFramework.webSocketComm.current_file(
            new URL(navigateEvent.destination.url),
        );
    });
};

// ## Testing
//
// Tell TypeScript about the global namespace this program defines.
declare global {
    interface Window {
        CodeChatEditor: {
            open_lp: (all_source: CodeChatForWeb) => Promise<void>;
        };
        CodeChatEditor_test: any;
    }
}

// A great and simple idea taken from
// [SO](https://stackoverflow.com/a/54116079): wrap all testing exports in a
// single variable. This avoids namespace pollution, since only one name is
// exported, and it's clearly marked for testing only. Test code still gets
// access to everything it needs.
export const exportedForTesting = {
    open_lp,
    codechat_html_to_markdown,
};
