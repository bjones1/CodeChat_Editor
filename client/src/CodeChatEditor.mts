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
// ==================================================
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
// -------
//
// ### JavaScript/TypeScript
//
// #### Third-party
import "./third-party/wc-mermaid/wc-mermaid.js";

// #### Local
import { assert } from "./assert.mjs";
import { consoleLog, DEBUG_ENABLED } from "./debug_enabled.mjs";
import {
    applyDiffStr,
    codeMirrorLoad,
    codeMirrorSave,
    mathJaxTypeset,
    mathJaxUnTypeset,
    scrollToLine as codemirrorScrollToLine,
    setCodeMirrorPositions,
} from "./CodeMirror-integration.mjs";
import "./graphviz-webcomponent-setup.mjs";
// This must be imported *after* the previous setup import, so it's placed here,
// instead of in the third-party category above.
import "./third-party/graphviz-webcomponent/graph.js";
import type {
    Editor,
    EditorEvent,
    Events,
    RawEditorOptions,
    TinyMCE,
} from "tinymce";
import {
    CodeChatForWeb,
    CodeMirrorDiffable,
    UpdateMessageContents,
    CodeMirror,
    autoUpdateTimeoutMs,
    rand,
} from "./shared.mjs";
import { showToast } from "./show_toast.mjs";

// ### CSS
import "./css/CodeChatEditor.css";
import { CursorPosition } from "./rust-types/CursorPosition.js";

// Data structures
// ---------------
//
// <a id="EditorMode"></a>Define all possible editor modes; these are passed as
// a [query string](https://en.wikipedia.org/wiki/Query_string)
// (`http://path/to/foo.py?mode=toc`, for example) to the page's URL.
//
// The member names below are looked up by the string value of the `mode`
// query parameter (see the numeric-enum reverse-mapping trick used where this
// is read), so they must stay exactly these lowercase strings rather than
// following the usual PascalCase member naming convention.
/* eslint-disable @typescript-eslint/naming-convention */
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
/* eslint-enable @typescript-eslint/naming-convention */

// Tell TypeScript about the global namespace this program defines.
declare global {
    interface Window {
        CodeChatEditor: {
            // Called by the Client Framework.
            openLp: (
                codechatForWeb: CodeChatForWeb,
                isReTranslation: boolean,
                cursorPosition?: CursorPosition,
                scrollLine?: number,
            ) => Promise<void>;
            doDebug: () => void;
            sendUpdate: (_only_if_dirty: boolean) => Promise<void>;
            scrollToLine: (
                cursorPosition?: CursorPosition,
                scrollLine?: number,
            ) => void;
            showToast: (text: string) => void;
            allow_navigation: boolean;
        };
        CodeChatEditor_test: unknown;
    }
}

// Globals
// -------
//
// The ID of the auto update timer; when this timer expires, the document will
// be updated.
let autoUpdateTimeoutId: null | number = null;

// Store the lexer info for the currently-loaded language.
//
// <a id="currentMetadata"></a>This mirrors the data provided by the server --
// see [SourceFileMetadata](../../server/src/webserver.rs#SourceFileMetadata).
let currentMetadata: {
    mode: string;
};

const webSocketComm = () => parent.window.CodeChatEditorFramework.webSocketComm;

// This set when a TinyMCE `input` event occurs, which usually produces a
// duplicate `Dirty` event which should be ignored.
let ignoreTinyMceDirty = false;

// True if the document is dirty (needs saving).
let isDirty = false;

export const setIsDirty = (value: boolean = true) => {
    isDirty = value;
};

export const getIsDirty = () => isDirty;

// ### TinyMCE dynamic import
//
// TinyMCE is dynamically imported when `init` is called.
export const init = async (options: RawEditorOptions) => {
    const tinymceConfig = await import("./tinymce-config.mjs");
    tinymce = tinymceConfig.tinymce;
    return await tinymceConfig.init(options);
};
// The imported module is stored in this variable.
export let tinymce: undefined | TinyMCE = undefined;
// A single TinyMCE instance is used for all doc blocks. Avoid accessing this
// through `tinymce.activeEditor`, which fails if the editor isn't active.
export const tinymceInstance = () => tinymce?.get(0);

// Page initialization
// -------------------

// This is copied from
// [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Document/DOMContentLoaded_event#checking_whether_loading_is_already_complete).
export const onDomContentLoaded = (onLoadFunc: () => void) => {
    if (document.readyState === "loading") {
        // Loading hasn't finished yet.
        document.addEventListener("DOMContentLoaded", onLoadFunc);
    } else {
        // `DOMContentLoaded` has already fired.
        onLoadFunc();
    }
};

// File handling
// -------------
//
// True if this is a CodeChat Editor document (not a source file).
const isDocOnly = () => {
    // This might be called by the framework before a document is loaded. So,
    // make sure `currentMetadata` exists first.
    return currentMetadata?.["mode"] === "markdown";
};

const openLp = async (
    codechatForWeb: CodeChatForWeb,
    isReTranslation: boolean,
    cursorPosition?: CursorPosition,
    scrollLine?: number,
) =>
    // Wait for the DOM to load before opening the file.
    await new Promise<void>((resolve) =>
        onDomContentLoaded(async () => {
            await _openLp(
                codechatForWeb,
                isReTranslation,
                cursorPosition,
                scrollLine,
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
let docContent = "";

// For debugging, allow the extension or server to run this routine by sending
// the appropriate message.
const doDebug = () => {
    if (DEBUG_ENABLED) {
        tinymceInstance()?.save({ format: "raw" });
    }
};

// This function is called on page load to "load" a file. Before this point, the
// server has already lexed the source file into code and doc blocks; this
// function transforms the code and doc blocks into HTML and updates the current
// web page with the results.
const _openLp = async (
    // A data structure provided by the server, containing the source and
    // associated metadata. See [`AllSource`](#AllSource).
    codechatForWeb: CodeChatForWeb,
    isReTranslation: boolean,
    cursorPosition?: CursorPosition,
    scrollLine?: number,
) => {
    // Note that globals, such as `isDirty` and document contents, may change
    // between `await` calls. The only call to `await` is based on TinyMCE init,
    // which should only cause an async delay on its first execution. So, we
    // should be OK for the rest of this function.
    //
    // Now, make all decisions about `isDirty`: if the text is dirty, do some
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
    if (getIsDirty() && isReTranslation) {
        consoleLog(`Ignoring re-translation because Client is dirty.`);
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
        const mode = urlParams.get("mode") ?? EditorMode[EditorMode.edit];
        // `EditorMode` is a numeric enum, so indexing it by the mode name
        // yields the matching enum value, or `undefined` for an unknown name.
        // Fall back to `edit` in that case.
        const _editorMode: EditorMode =
            EditorMode[mode as keyof typeof EditorMode] ?? EditorMode.edit;

        // Get the <code>[currentMetadata](#currentMetadata)</code> from the
        // provided `code_chat_for_web` struct and store it as a global
        // variable.
        currentMetadata = codechatForWeb["metadata"];
        const source = codechatForWeb["source"];
        const codechatBody = document.getElementById("CodeChat-body");
        assert(codechatBody instanceof HTMLDivElement);
        if (isDocOnly()) {
            // Per the
            // [docs](https://docs.mathjax.org/en/latest/web/typeset.html#updating-previously-typeset-content),
            // "If you modify the page to remove content that contains typeset
            // mathematics, you will need to tell MathJax about that so that it
            // knows the typeset math that you are removing is no longer on the
            // page."
            window.MathJax?.typesetClear?.(codechatBody);
            // Note that `==` is intentional: `null` (no editor instance) or
            // `undefined` (TinyMCE not loaded).
            if (tinymceInstance() == null) {
                // We shouldn't have a diff if the editor hasn't been
                // initialized.
                assert("Plain" in source);
                // Special case: a CodeChat Editor document's HTML is stored
                // in`source.doc`. We don't need the CodeMirror editor at all;
                // instead, treat it like a single doc block contents div.
                docContent = source.Plain.doc;
                codechatBody.innerHTML = `<div class="CodeChat-doc-contents" spellcheck="true">${docContent}</div>`;
                await init({
                    selector: ".CodeChat-doc-contents",
                    // In the doc-only mode, add auto update functionality.
                    // While there is an
                    // [autosave plugin](https://www.tiny.cloud/docs/tinymce/6/autosave/),
                    // this autosave functionality is completely different from
                    // the auto update provided here. Per
                    // [handling editor events](https://www.tiny.cloud/docs/tinymce/6/events/#handling-editor-events),
                    // this is how to create a TinyMCE event handler.
                    setup: (editor: Editor) => {
                        editor.on("Dirty", () => {
                            if (!ignoreTinyMceDirty) {
                                setIsDirty(true);
                                startAutoUpdateTimer();
                            }
                        });

                        editor.on("input", () => {
                            ignoreTinyMceDirty = true;
                            setIsDirty(true);
                            startAutoUpdateTimer();
                        });

                        // Send updates on cursor movement.
                        editor.on(
                            "SelectionChange",
                            (
                                _event: EditorEvent<
                                    Events.EditorEventMap["SelectionChange"]
                                >,
                            ) => {
                                startAutoUpdateTimer();
                            },
                        );
                    },
                });
                tinymceInstance()!.focus();
            } else {
                // Save the cursor location before the update, then restore it
                // afterwards, if TinyMCE has focus.
                const sel = tinymceInstance()!.hasFocus()
                    ? saveSelection()
                    : undefined;
                docContent =
                    "Plain" in source
                        ? source.Plain.doc
                        : applyDiffStr(docContent, source.Diff.doc);
                tinymceInstance()!.setContent(docContent);
                if (sel !== undefined) {
                    restoreSelection(sel);
                }
            }
            await mathJaxTypeset(codechatBody);
            scrollToLine(cursorPosition, scrollLine);
        } else {
            if (getIsDirty() && "Diff" in source) {
                // Send an `OutOfSync` response, so that the IDE will send the
                // full text to overwrite these changes with.
                webSocketComm().sendResult(
                    // Pick a rarely-used ID, since we're not responding to a
                    // specific message.
                    0,
                    // There's not a version that matters. TODO: replace this
                    // with a more suitable error.
                    { OutOfSync: [0, 0] },
                );
            } else {
                await codeMirrorLoad(
                    codechatBody,
                    codechatForWeb,
                    [],
                    cursorPosition,
                    scrollLine,
                );
            }
        }
    } finally {
        // Use a `finally` block to ensure the cleanup code always runs.
        //
        // Per the discussion at the beginning of this function, the dirty
        // contents have been overwritten by contents from the IDE. By the same
        // reasoning, restart the auto update timer.
        clearAutoUpdateTimer();
        setIsDirty(false);

        // <a id="CodeChatEditor_test"></a>If tests should be run, then the
        // [following global variable](CodeChatEditor-test.mts#CodeChatEditor_test)
        // is function that runs them.
        if (typeof window.CodeChatEditor_test === "function") {
            window.CodeChatEditor_test();
        }
    }
};

const saveLp = async (
    // Avoid relying on the global `isDirty`, which may change during an
    // `await`.
    isDirtyNow: boolean,
) => {
    const update: UpdateMessageContents = {
        // The Framework will fill in this value.
        file_path: "",
        is_re_translation: false,
    };
    if (isDocOnly()) {
        const location = saveSelection();
        // If there's a selection (cursor location), send it to the server,
        // which will locate the corresponding line.
        if (location.selectionOffset !== undefined) {
            update.cursor_position = {
                DomLocation: {
                    dom_path: location.selectionPath,
                    dom_offset: location.selectionOffset,
                    // Use this since it's a Markdown-only file; the server will
                    // ignore this value.
                    from: 0,
                },
            };
        }
    } else {
        setCodeMirrorPositions(update);
    }

    // Add the contents only if the document is dirty.
    if (isDirtyNow) {
        /// @ts-expect-error("Declare here; it will be completed later.")
        let codeMirrorDiffable: CodeMirrorDiffable = {};
        if (isDocOnly()) {
            // Untypeset all math before saving the document.
            const codechatBody = document.getElementById("CodeChat-body");
            assert(codechatBody instanceof HTMLDivElement);
            mathJaxUnTypeset(codechatBody);
            // Use a try/finally to ensure that the document is retypeset even
            // if errors occur.
            try {
                // To save a document only, simply get the HTML from the only
                // Tiny MCE div. Update the `doc_contents` to stay in sync with
                // the Server.
                docContent = tinymceInstance()!.save({ format: "raw" });
                // The `save()` flushes any duplicate `Dirty` events. After
                // this, following `Dirty` events are genuine.
                ignoreTinyMceDirty = false;
                (
                    codeMirrorDiffable as {
                        Plain: CodeMirror;
                    }
                ).Plain = {
                    doc: docContent,
                    doc_blocks: [],
                };
            } finally {
                // Retypeset all math after saving the document.
                await mathJaxTypeset(codechatBody);
            }
        } else {
            codeMirrorDiffable = codeMirrorSave();
            assert("Plain" in codeMirrorDiffable);
        }
        update.contents = {
            metadata: currentMetadata,
            version: rand(),
            source: codeMirrorDiffable,
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
    // in this node is placed in `selectionOffset`.)
    const sel = window.getSelection();
    const selectionPath = [];
    const selectionOffset = sel?.anchorOffset;
    if (sel?.anchorNode) {
        // Find a path from the selection back to the containing div.
        for (
            let currentNode = sel.anchorNode;
            // Continue until we find the div which contains the doc block
            // contents: either it's not an element (such as a div), ...
            !(currentNode instanceof Element) ||
            // or it's not the doc block contents div.
            (!currentNode.classList.contains("CodeChat-doc-contents") &&
                // Sometimes, the parent of a custom node (`wc-mermaid`) skips
                // the TinyMCE div and returns the overall div. I don't know
                // why.
                !currentNode.classList.contains("CodeChat-doc"));
            currentNode = currentNode.parentNode!
        ) {
            // Store the index of this node in its' parent list of child
            // nodes/children.
            const p = currentNode.parentNode;
            // In case we go off the rails, give up if there are no more
            // parents.
            if (p === null) {
                return {
                    selectionPath: [],
                    selectionOffset: undefined,
                };
            }
            selectionPath.unshift(
                Array.prototype.indexOf.call(p.childNodes, currentNode),
            );
        }
    }
    return { selectionPath, selectionOffset };
};

// Restore the selection produced by `saveSelection` to the active TinyMCE
// instance.
export const restoreSelection = ({
    selectionPath,
    selectionOffset,
}: {
    selectionPath: number[];
    selectionOffset?: number;
}) => {
    // Copy the selection over to TinyMCE by indexing the selection path to find
    // the selected node.
    if (selectionPath.length && typeof selectionOffset === "number") {
        let selectionNode: Node = tinymceInstance()!.getContentAreaContainer();
        // Avoid mutating `selectionPath` by making a copy of it.
        const selectionPathCopy = [...selectionPath];
        while (selectionPathCopy.length) {
            const newSelectionNode =
                selectionNode.childNodes[selectionPathCopy.shift()!];
            // If we get lost during the descent, then stop just before that.
            if (!(newSelectionNode instanceof Node)) {
                break;
            }
            selectionNode = newSelectionNode;
        }
        // In case of edits, avoid an offset past the end of the node. Note that
        // the maximum value is `length`, not `length - 1`, which represents a
        // selection at the very end of the text node.
        const finalSelectionOffset = Math.min(
            selectionOffset,
            selectionNode.nodeValue?.length ?? 0,
        );
        // Use that to set the selection.
        tinymceInstance()!.selection.setCursorLocation(
            selectionNode,
            finalSelectionOffset,
        );
    }
};

// Save CodeChat Editor contents if dirty; send the current selection and scroll
// position.
const sendUpdate = async (onlyIfDirty: boolean = false) => {
    if (onlyIfDirty && !isDirty) {
        return;
    }
    clearAutoUpdateTimer();

    // <a id="save"></a>Save the provided contents back to the filesystem, by
    // sending an update message over the websocket.
    consoleLog(
        "CodeChat Editor Client: sent Update - saving document/updating cursor location.",
    );
    // Don't wait for a response to change `isDirty`; this boogers up logic.
    webSocketComm().sendMessage({ Update: await saveLp(isDirty) });
    isDirty = false;
};

// ### Auto update feature
//
// Schedule an autosave and/or a selection/scroll update; call this whenever the
// document is modified or the selection/scroll offset changes.
export const startAutoUpdateTimer = () => {
    // When the document/selection/scroll position is changed, perform an auto
    // update after no changes have occurred for a little while. To do this,
    // first cancel any current timeout...
    clearAutoUpdateTimer();
    // ...then start another timeout which updates the document when it expires.
    autoUpdateTimeoutId = window.setTimeout(() => {
        consoleLog("CodeChat Editor Client: auto updating.");
        sendUpdate();
    }, autoUpdateTimeoutMs);
};

const clearAutoUpdateTimer = () => {
    if (autoUpdateTimeoutId !== null) {
        clearTimeout(autoUpdateTimeoutId);
        autoUpdateTimeoutId = null;
    }
};

// Navigation
// ----------
//
// The TOC and this page calls this when a hyperlink is clicked. This saves the
// current document before navigating.
const onNavigate = (navigateEvent: NavigateEvent) => {
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
    consoleLog("CodeChat Editor Client: saving document before navigation.");
    saveThenNavigate(new URL(navigateEvent.destination.url));
};

// This is able to intercept clicks on links that the Navigation API doesn't,
// specifically those that TinyMCE generates (since they're always set to open
// in a new tab).
const onClick = (event: MouseEvent) => {
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
            saveThenNavigate(new URL(url));
        } else {
            // This is navigation to some external link. Let that proceed
            // without interruption in a pure browser environment. However,
            // VSCode will block navigation, since it's cross-origin (the root
            // iframe has no URL, in contrast with the localhost URL of the
            // CodeChat Editor Server). In this case, ask the Server to open the
            // requested link.
            if (window.location.pathname.startsWith("/vsc")) {
                event.preventDefault();
                parent.window.CodeChatEditorFramework.webSocketComm.sendMessage(
                    { OpenUrl: url },
                );
            }
        }
    }
};
// Save the current document, then navigate to the provided URL, which must be a
// reference to another CodeChat Editor document.
const saveThenNavigate = (codeChatEditorUrl: URL) => {
    const navigate = () => {
        // Avoid recursion!
        window.navigation.removeEventListener("navigate", onNavigate);
        parent.window.CodeChatEditorFramework.webSocketComm.currentFile(
            codeChatEditorUrl,
        );
    };
    // Navigate after the save completes. If the save fails, still navigate --
    // otherwise the user is stranded on the current page with only a generic
    // error toast -- but report the failure so the lost save isn't silent.
    sendUpdate(true).then(navigate, (reason) => {
        showToast(`Error saving before navigation: ${reason}`);
        navigate();
    });
};

// This can be called by the framework. Therefore, make no assumptions about
// variables being valid; it be called before a file is loaded, etc.
const scrollToLine = (cursorPosition?: CursorPosition, scrollLine?: number) => {
    if (isDocOnly()) {
        // TODO.
    } else {
        codemirrorScrollToLine(cursorPosition, scrollLine);
    }
};

// A global error handler: this is called on any uncaught exception.
export const onError = (event: Event) => {
    let errStr: string;
    if (event instanceof ErrorEvent) {
        errStr = `${event.filename}:${event.lineno}: ${event.message}`;
        if (event.error?.stack) {
            errStr += `\n${event.error.stack}`;
        }
    } else if (event instanceof PromiseRejectionEvent) {
        const reason = event.reason;
        let userMessage = "An unexpected error occurred. Please try again.";
        console.log(reason, reason instanceof Error, typeof reason);
        // A simple `reason instanceof Error` fails here. Better would be
        // [Error.isError()](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Error/isError),
        // but this requires es2027.
        if (typeof reason?.message === "string") {
            // Extracts the text from `reject(new Error('Your text'))`.
            userMessage = reason.message;
        } else if (typeof reason === "string") {
            // Extracts the text from `reject('Your text')`.
            userMessage = reason;
        }
        errStr = `Promise rejected: ${userMessage}`;
        if (reason instanceof Error && reason.stack) {
            errStr += `\n${reason.stack}`;
        }
    } else {
        errStr = `Unexpected error ${typeof event}: ${event}`;
    }
    showToast(errStr);
    console.error(event);
};

// Load the dynamic content into the static page. Place this last, since we need
// functions above defined before assigning them to the `CodeChatEditor`
// namespace.
onDomContentLoaded(async () => {
    // Intercept links in this document to save before following the link.
    window.navigation.addEventListener("navigate", onNavigate);
    const ccb = document.getElementById("CodeChat-sidebar");
    if (ccb instanceof HTMLIFrameElement) {
        ccb.contentWindow?.navigation.addEventListener("navigate", onNavigate);
    }
    document.addEventListener("click", onClick);
    // Provide basic error reporting for uncaught errors.
    window.addEventListener("unhandledrejection", onError);
    window.addEventListener("error", onError);

    window.CodeChatEditor = {
        doDebug,
        openLp,
        sendUpdate,
        scrollToLine,
        showToast,
        allow_navigation: false,
    };
});

// Testing
// -------
//
// A great and simple idea taken from
// [SO](https://stackoverflow.com/a/54116079): wrap all testing exports in a
// single variable. This avoids namespace pollution, since only one name is
// exported, and it's clearly marked for testing only. Test code still gets
// access to everything it needs.
export const exportedForTesting = {};
