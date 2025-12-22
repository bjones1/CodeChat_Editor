// Copyright (C) 2025 Bryan A. Jones.
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
// `CodeChatEditor.mts` -- the CodeChat Editor Client
// =============================================================================
//
// The overall process of load a file is:
//
// 1. The user browses to a file on the local machine, using the very simple
//    file browser webpage provided by the CodeChat Server. Clicking on this
//    file starts the process of loading a file into the CodeChat editor.
// 2. The server sees a request for a file supported by the CodeChat Editor. It
//    lexes the files into code and doc blocks, then wraps these in a webpage
//    which contains this program (the CodeChat Editor).
// 3. On load, this program (the CodeChat Editor) loads these code and doc
//    blocks into the CodeMirror text editor, using TinyMCE to provide a GUI
//    editor within CodeMirror for doc blocks.
//
// The user then uses the editing capabilities of CodeMirror/TinyMCE to edit
// their program. When the user saves a file:
//
// 1. This program serializes the CodeMirror text plus doc blocks, and
//    transforms HTML back to markdown.
// 2. It sends these code/doc blocks back to the server.
// 3. The server then transforms these code/doc blocks into source code, then
//    writes this code to the disk.
//
// Imports
// -----------------------------------------------------------------------------
//
// ### JavaScript/TypeScript
//
// #### Third-party
import "./third-party/wc-mermaid/wc-mermaid";

// #### Local
import { assert } from "./assert.mjs";
import { DEBUG_ENABLED } from "./debug_enabled.mjs";
import {
    apply_diff_str,
    CodeMirror_load,
    CodeMirror_save,
    mathJaxTypeset,
    mathJaxUnTypeset,
    scroll_to_line as codemirror_scroll_to_line,
    set_CodeMirror_positions,
} from "./CodeMirror-integration.mjs";
import "./graphviz-webcomponent-setup.mts";
// This must be imported *after* the previous setup import, so it's placed here,
// instead of in the third-party category above.
import "./third-party/graphviz-webcomponent/graph.js";
import { init, tinymce } from "./tinymce-config.mjs";
import { Editor, EditorEvent, Events } from "tinymce";
import {
    CodeChatForWeb,
    CodeMirrorDiffable,
    UpdateMessageContents,
    CodeMirror,
    autosave_timeout_ms,
    rand,
} from "./shared_types.mjs";
import { show_toast } from "./show_toast.mjs";

// ### CSS
import "./css/CodeChatEditor.css";

// Data structures
// -----------------------------------------------------------------------------
//
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
        CodeChatEditor: {
            // Called by the Client Framework.
            open_lp: (
                codechat_for_web: CodeChatForWeb,
                is_re_translation: boolean,
                cursor_line?: number,
                scroll_line?: number,
            ) => Promise<void>;
            on_save: (_only_if_dirty: boolean) => Promise<void>;
            scroll_to_line: (
                cursor_line?: number,
                scroll_line?: number,
            ) => void;
            show_toast: (text: string) => void;
            allow_navigation: boolean;
        };
        CodeChatEditor_test: unknown;
    }
}

// Globals
// -----------------------------------------------------------------------------
//
// The ID of the autosave timer; when this timer expires, the document will be
// autosaved.
let autosaveTimeoutId: null | number = null;

// Store the lexer info for the currently-loaded language.
//
// <a id="current_metadata"></a>This mirrors the data provided by the server --
// see [SourceFileMetadata](../../server/src/webserver.rs#SourceFileMetadata).
let current_metadata: {
    mode: string;
};

const webSocketComm = () => parent.window.CodeChatEditorFramework.webSocketComm;

// True if the document is dirty (needs saving).
let is_dirty = false;

export const set_is_dirty = (value: boolean = true) => {
    is_dirty = value;
};

export const get_is_dirty = () => is_dirty;

// Page initialization
// -----------------------------------------------------------------------------

// This is copied from
// [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Document/DOMContentLoaded_event#checking_whether_loading_is_already_complete).
export const on_dom_content_loaded = (on_load_func: () => void) => {
    if (document.readyState === "loading") {
        // Loading hasn't finished yet.
        document.addEventListener("DOMContentLoaded", on_load_func);
    } else {
        // `DOMContentLoaded` has already fired.
        on_load_func();
    }
};

// File handling
// -----------------------------------------------------------------------------
//
// True if this is a CodeChat Editor document (not a source file).
const is_doc_only = () => {
    // This might be called by the framework before a document is loaded. So,
    // make sure `current_metadata` exists first.
    return current_metadata?.["mode"] === "markdown";
};

// Wait for the DOM to load before opening the file.
const open_lp = async (
    codechat_for_web: CodeChatForWeb,
    is_re_translation: boolean,
    cursor_line?: number,
    scroll_line?: number,
) =>
    await new Promise<void>((resolve) =>
        on_dom_content_loaded(async () => {
            await _open_lp(
                codechat_for_web,
                is_re_translation,
                cursor_line,
                scroll_line,
            );
            resolve();
        }),
    );

// Store the HTML sent for CodeChat Editor documents. We can't simply use
// TinyMCE's
// [getContent](https://www.tiny.cloud/docs/tinymce/latest/apis/tinymce.editor/#getContent),
// since this modifies the content based on cleanup rules before returning it --
// which causes applying diffs to this unexpectedly modified content to produce
// incorrect results. This text is the unmodified content sent from the IDE.
let doc_content = "";

// This function is called on page load to "load" a file. Before this point, the
// server has already lexed the source file into code and doc blocks; this
// function transforms the code and doc blocks into HTML and updates the current
// web page with the results.
const _open_lp = async (
    // A data structure provided by the server, containing the source and
    // associated metadata. See [`AllSource`](#AllSource).
    codechat_for_web: CodeChatForWeb,
    is_re_translation: boolean,
    cursor_line?: number,
    scroll_line?: number,
) => {
    // Note that globals, such as `is_dirty` and document contents, may change
    // between `await` calls. Therefore, try to perform processing which relies
    // on these values between `await` calls. For example, evaluate this first:
    //
    // Before calling any MathJax, make sure it's fully loaded and the initial
    // render is finished.
    await window.MathJax.startup.promise;

    // The only the `await` is based on TinyMCE init, which should only cause an
    // async delay on its first execution. (Even then, I'm not sure it does,
    // since all resources are statically imported). So, we should be OK for the
    // rest of this function.
    //
    // Now, make all decisions about `is_dirty`: if the text is dirty, do some
    // special processing; simply applying the update could cause either data
    // loss (overwriting edits made since the last autosave) or data corruption
    // (applying a diff to updated text, causing the diff to be mis-applied).
    // Specifically:
    //
    // 1. If this is a re-translation, then ignore the update, since it's only
    //    changes due to re-translation, not due to updates to IDE content.
    // 2. If this is the full text, discard changes made in the Client since the
    //    last autosave, overwriting them with the provided update.
    // 3. If this is a diff:
    //    1. In document-only mode, we have a backup copy of the full text
    //       before it was modified by the Client. Apply the diff to this,
    //       overwriting changes made in the Client.
    //    2. In normal mode, we don't have a backup copy of the full text.
    //       Report an `OutOfSync` error, which causes the IDE to send the full
    //       text which will then overwrite changes made in the Client.
    if (is_dirty && is_re_translation) {
        console_log(`Ignoring re-translation because Client is dirty.`);
        return;
    }

    try {
        // Use
        // [URLSearchParams](https://developer.mozilla.org/en-US/docs/Web/API/URLSearchParams)
        // to parse out the search parameters of this window's URL.
        const urlParams = new URLSearchParams(window.location.search);
        // Get the mode from the page's query parameters. Default to edit using
        // the
        // [nullish coalescing operator](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Operators/Nullish_coalescing_operator).
        // TODO: this isn't typesafe, since the `mode` might not be a key of
        // `EditorMode`.
        /// @ts-expect-error("See above.")
        const _editorMode = EditorMode[urlParams.get("mode") ?? "edit"];

        // Get the <code>[current_metadata](#current_metadata)</code> from the
        // provided `code_chat_for_web` struct and store it as a global
        // variable.
        current_metadata = codechat_for_web["metadata"];
        const source = codechat_for_web["source"];
        const codechat_body = document.getElementById(
            "CodeChat-body",
        ) as HTMLDivElement;
        // Per the
        // [docs](https://docs.mathjax.org/en/latest/web/typeset.html#updating-previously-typeset-content),
        // "If you modify the page to remove content that contains typeset
        // mathematics, you will need to tell MathJax about that so that it
        // knows the typeset math that you are removing is no longer on the
        // page."
        window.MathJax.typesetClear(codechat_body);
        if (is_doc_only()) {
            if (tinymce.activeEditor === null) {
                // We shouldn't have a diff if the editor hasn't been
                // initialized.
                assert("Plain" in source);
                // Special case: a CodeChat Editor document's HTML is stored
                // in`source.doc`. We don't need the CodeMirror editor at all;
                // instead, treat it like a single doc block contents div.
                doc_content = source.Plain.doc;
                codechat_body.innerHTML = `<div class="CodeChat-doc-contents" spellcheck="true">${doc_content}</div>`;
                await init({
                    selector: ".CodeChat-doc-contents",
                    // In the doc-only mode, add autosave functionality. While
                    // there is an
                    // [autosave plugin](https://www.tiny.cloud/docs/tinymce/6/autosave/),
                    // this autosave functionality is completely different from
                    // the autosave provided here. Per
                    // [handling editor events](https://www.tiny.cloud/docs/tinymce/6/events/#handling-editor-events),
                    // this is how to create a TinyMCE event handler.
                    setup: (editor: Editor) => {
                        editor.on(
                            "dirty",
                            (
                                event: EditorEvent<
                                    Events.EditorEventMap["dirty"]
                                >,
                            ) => {
                                // Sometimes, `tinymce.activeEditor` is null
                                // (perhaps when it's not focused). Use the
                                // `event` data instead.
                                event.target.setDirty(false);
                                is_dirty = true;
                                startAutosaveTimer();
                            },
                        );
                    },
                });
                tinymce.activeEditor!.focus();
            } else {
                // Save the cursor location before the update, then restore it
                // afterwards, if TinyMCE has focus.
                const sel = tinymce.activeEditor!.hasFocus()
                    ? saveSelection()
                    : undefined;
                doc_content =
                    "Plain" in source
                        ? source.Plain.doc
                        : apply_diff_str(doc_content, source.Diff.doc);
                tinymce.activeEditor!.setContent(doc_content);
                if (sel !== undefined) {
                    restoreSelection(sel);
                }
            }
            mathJaxTypeset(codechat_body);
            scroll_to_line(cursor_line, scroll_line);
        } else {
            if (is_dirty && "Diff" in source) {
                // Send an `OutOfSync` response, so that the IDE will send the
                // full text to overwrite these changes with.
                webSocketComm().send_result(
                    // Pick a rarely-used ID, since we're not responding to a
                    // specific message.
                    0,
                    // There's not a version that matters. TODO: replace this
                    // with a more suitable error.
                    { OutOfSync: [0, 0] },
                );
            } else {
                await CodeMirror_load(
                    codechat_body,
                    codechat_for_web,
                    [],
                    cursor_line,
                    scroll_line,
                );
            }
        }
    } finally {
        // Use a `finally` block to ensure the cleanup code always run.
        //
        // Per the discussion at the beginning of this function, the dirty
        // contents have been overwritten by contents from the IDE. By the same
        // reasoning, restart the autosave timer.
        clearAutosaveTimer();
        is_dirty = false;

        // <a id="CodeChatEditor_test"></a>If tests should be run, then the
        // [following global variable](CodeChatEditor-test.mts#CodeChatEditor_test)
        // is function that runs them.
        if (typeof window.CodeChatEditor_test === "function") {
            window.CodeChatEditor_test();
        }
    }
};

const save_lp = (is_dirty: boolean) => {
    const update: UpdateMessageContents = {
        // The Framework will fill in this value.
        file_path: "",
        is_re_translation: false,
    };
    if (is_doc_only()) {
        // TODO: set cursor/scroll position.
    } else {
        set_CodeMirror_positions(update);
    }

    // Add the contents only if the document is dirty.
    if (is_dirty) {
        /// @ts-expect-error("Declare here; it will be completed later.")
        let code_mirror_diffable: CodeMirrorDiffable = {};
        if (is_doc_only()) {
            // Untypeset all math before saving the document.
            const codechat_body = document.getElementById(
                "CodeChat-body",
            ) as HTMLDivElement;
            mathJaxUnTypeset(codechat_body);
            // To save a document only, simply get the HTML from the only Tiny
            // MCE div. Update the `doc_contents` to stay in sync with the
            // Server.
            doc_content = tinymce.activeEditor!.save();
            (
                code_mirror_diffable as {
                    Plain: CodeMirror;
                }
            ).Plain = {
                doc: doc_content,
                doc_blocks: [],
            };
            // Retypeset all math after saving the document.
            mathJaxTypeset(codechat_body);
        } else {
            code_mirror_diffable = CodeMirror_save();
            assert("Plain" in code_mirror_diffable);
        }
        update.contents = {
            metadata: current_metadata,
            version: rand(),
            source: code_mirror_diffable,
        };
    }

    return update;
};

export const saveSelection = () => {
    // Changing the text inside TinyMCE causes it to loose a selection tied to a
    // specific node. So, instead store the selection as an array of indices in
    // the childNodes array of each element: for example, a given selection is
    // element 10 of the root TinyMCE div's children (selecting an ol tag),
    // element 5 of the ol's children (selecting the last li tag), element 0 of
    // the li's children (a text node where the actual click landed; the offset
    // in this node is placed in `selection_offset`.)
    const sel = window.getSelection();
    const selection_path = [];
    const selection_offset = sel?.anchorOffset;
    if (sel?.anchorNode) {
        // Find a path from the selection back to the containing div.
        for (
            let current_node = sel.anchorNode, is_first = true;
            // Continue until we find the div which contains the doc block
            // contents: either it's not an element (such as a div), ...
            current_node.nodeType !== Node.ELEMENT_NODE ||
            // or it's not the doc block contents div.
            (!(current_node as Element).classList.contains(
                "CodeChat-doc-contents",
            ) &&
                // Sometimes, the parent of a custom node (`wc-mermaid`) skips
                // the TinyMCE div and returns the overall div. I don't know
                // why.
                !(current_node as Element).classList.contains("CodeChat-doc"));
            current_node = current_node.parentNode!, is_first = false
        ) {
            // Store the index of this node in its' parent list of child
            // nodes/children. Use `childNodes` on the first iteration, since
            // the selection is often in a text node, which isn't in the
            // `parents` list. However, using `childNodes` all the time causes
            // trouble when reversing the selection -- sometimes, the
            // `childNodes` change based on whether text nodes (such as a
            // newline) are included are not after tinyMCE parses the content.
            const p = current_node.parentNode;
            // In case we go off the rails, give up if there are no more
            // parents.
            if (p === null) {
                return {
                    selection_path: [],
                    selection_offset: 0,
                };
            }
            selection_path.unshift(
                Array.prototype.indexOf.call(
                    is_first ? p.childNodes : p.children,
                    current_node,
                ),
            );
        }
    }
    return { selection_path, selection_offset };
};

// Restore the selection produced by `saveSelection` to the active TinyMCE
// instance.
export const restoreSelection = ({
    selection_path,
    selection_offset,
}: {
    selection_path: number[];
    selection_offset?: number;
}) => {
    // Copy the selection over to TinyMCE by indexing the selection path to find
    // the selected node.
    if (selection_path.length && typeof selection_offset === "number") {
        let selection_node = tinymce.activeEditor!.getContentAreaContainer();
        for (
            ;
            selection_path.length &&
            // If something goes wrong, bail out instead of producing
            // exceptions.
            selection_node !== undefined;
            selection_node =
                // As before, use the more-consistent `children` except for the
                // last element, where we might be selecting a `text` node.
                (
                    selection_path.length > 1
                        ? selection_node.children
                        : selection_node.childNodes
                )[selection_path.shift()!]! as HTMLElement
        );
        // Exit on failure.
        if (selection_node === undefined) {
            return;
        }
        // Use that to set the selection.
        tinymce.activeEditor!.selection.setCursorLocation(
            selection_node,
            // In case of edits, avoid an offset past the end of the node.
            Math.min(selection_offset, selection_node.nodeValue?.length ?? 0),
        );
    }
};

// Per
// [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Navigator/platform#examples),
// here's the least bad way to choose between the control key and the command
// key.
const _os_is_osx =
    navigator.platform.indexOf("Mac") === 0 || navigator.platform === "iPhone"
        ? true
        : false;

// Save CodeChat Editor contents.
const on_save = async (only_if_dirty: boolean = false) => {
    if (only_if_dirty && !is_dirty) {
        return;
    }
    clearAutosaveTimer();

    // <a id="save"></a>Save the provided contents back to the filesystem, by
    // sending an update message over the websocket.
    console_log(
        "CodeChat Editor Client: sent Update - saving document/updating cursor location.",
    );
    // Don't wait for a response to change `is_dirty`; this boogers up logic.
    webSocketComm().send_message({ Update: save_lp(is_dirty) });
    is_dirty = false;
};

// ### Autosave feature
//
// Schedule an autosave; call this whenever the document is modified.
export const startAutosaveTimer = () => {
    // When the document is changed, perform an autosave after no changes have
    // occurred for a little while. To do this, first cancel any current
    // timeout...
    clearAutosaveTimer();
    // ...then start another timeout which saves the document when it expires.
    autosaveTimeoutId = window.setTimeout(() => {
        console_log("CodeChat Editor Client: autosaving.");
        on_save();
    }, autosave_timeout_ms);
};

const clearAutosaveTimer = () => {
    if (autosaveTimeoutId !== null) {
        clearTimeout(autosaveTimeoutId);
        autosaveTimeoutId = null;
    }
};

// Navigation
// -----------------------------------------------------------------------------
//
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
    // this is a problem if data wasn't saved! Need a sync way to do this. Store
    // it in local data or something.
    if (!navigateEvent.canIntercept) {
        return;
    }

    // If the IDE initiated this navigation via a `CurrentFile` message, then
    // allow it.
    if (window.CodeChatEditor.allow_navigation) {
        // We don't need to reset this flag, since this window will be reloaded.
        return;
    }

    // Intercept this navigation so we can save the document first.
    navigateEvent.intercept();
    console_log("CodeChat Editor Client: saving document before navigation.");
    save_then_navigate(new URL(navigateEvent.destination.url));
};

// This is able to intercept clicks on links that the Navigation API doesn't,
// specifically those that TinyMCE generates (since they're always set to open
// in a new tab).
const on_click = (event: MouseEvent) => {
    // TinyMCE by default tries to open all links in a new tab. Look for and fix
    // these.
    if (
        event.target instanceof HTMLAnchorElement &&
        event.target.target === "_blank"
    ) {
        // Get the URL from the link.
        const url = event.target.href;

        // If it's to a CodeChat Editor file, then load it as such.
        if (event.target.origin === window.location.origin) {
            // Ignore the "new tab" target, which doesn't make sense when there
            // is a 1:1 relationship between the active IDE file and the file
            // being edited in the CodeChat Editor. If two tabs are open, which
            // is the current file for the IDE?
            event.preventDefault();
            save_then_navigate(new URL(url));
        } else {
            // This is navigation to some external link. Let that proceed
            // without interruption in a pure browser environment. However,
            // VSCode will block navigation, since it's cross-origin (the root
            // iframe has no URL, in contrast with the localhost URL of the
            // CodeChat Editor Server). In this case, ask the Server to open the
            // requested link.
            if (window.location.pathname.startsWith("/vsc")) {
                event.preventDefault();
                parent.window.CodeChatEditorFramework.webSocketComm.send_message(
                    { OpenUrl: url },
                );
            }
        }
    }
};
// Save the current document, then navigate to the provided URL, which must be a
// reference to another CodeChat Editor document.
const save_then_navigate = (codeChatEditorUrl: URL) => {
    on_save(true).then((_value) => {
        // Avoid recursion!
        window.navigation.removeEventListener("navigate", on_navigate);
        parent.window.CodeChatEditorFramework.webSocketComm.current_file(
            codeChatEditorUrl,
        );
    });
};

// This can be called by the framework. Therefore, make no assumptions about
// variables being valid; it be called before a file is loaded, etc.
const scroll_to_line = (cursor_line?: number, scroll_line?: number) => {
    if (is_doc_only()) {
        // TODO.
    } else {
        codemirror_scroll_to_line(cursor_line, scroll_line);
    }
};

/*eslint-disable-next-line @typescript-eslint/no-explicit-any */
export const console_log = (...args: any) => {
    if (DEBUG_ENABLED) {
        console.log(...args);
    }
};

// A global error handler: this is called on any uncaught exception.
export const on_error = (event: Event) => {
    let err_str: string;
    if (event instanceof ErrorEvent) {
        err_str = `${event.filename}:${event.lineno}: ${event.message}`;
    } else if (event instanceof PromiseRejectionEvent) {
        err_str = `${event.promise} rejected: ${event.reason}`;
    } else {
        err_str = `Unexpected error ${typeof event}: ${event}`;
    }
    show_toast(`Error: ${err_str}`);
    console.error(event);
};

// Load the dynamic content into the static page. Place this last, since we need
// functions above defined before assigning them to the `CodeChatEditor`
// namespace.
on_dom_content_loaded(async () => {
    // Intercept links in this document to save before following the link.
    window.navigation.addEventListener("navigate", on_navigate);
    const ccb = document.getElementById("CodeChat-sidebar") as
        | HTMLIFrameElement
        | undefined;
    ccb?.contentWindow?.navigation.addEventListener("navigate", on_navigate);
    document.addEventListener("click", on_click);
    // Provide basic error reporting for uncaught errors.
    window.addEventListener("unhandledrejection", on_error);
    window.addEventListener("error", on_error);

    window.CodeChatEditor = {
        open_lp,
        on_save,
        scroll_to_line,
        show_toast,
        allow_navigation: false,
    };
});

// Testing
// -----------------------------------------------------------------------------
//
// A great and simple idea taken from
// [SO](https://stackoverflow.com/a/54116079): wrap all testing exports in a
// single variable. This avoids namespace pollution, since only one name is
// exported, and it's clearly marked for testing only. Test code still gets
// access to everything it needs.
export const exportedForTesting = {};
