// Copyright (C) 2025 Bryan A. Jones.
//
// This file is part of the CodeChat Editor. The CodeChat Editor is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, either
// version 3 of the License, or (at your option) any later version of the GNU
// General Public License.
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
// `extension.ts` - The CodeChat Editor Visual Studio Code extension
// =============================================================================
//
// This extension creates a webview, then uses a websocket connection to the
// CodeChat Editor Server and Client to render editor text in that webview.
//
// Imports
// -----------------------------------------------------------------------------
//
// ### Node.js packages
import assert from "assert";
import process from "node:process";

// ### Third-party packages
import escape from "escape-html";
import vscode, {
    Range,
    TextDocument,
    TextEditor,
    TextEditorRevealType,
} from "vscode";
import { CodeChatEditorServer, initServer } from "./index.js";

// ### Local packages
import {
    autosave_timeout_ms,
    EditorMessage,
    EditorMessageContents,
    KeysOfRustEnum,
    MessageResult,
    rand,
    UpdateMessageContents,
} from "../../../client/src/shared_types.mjs";
import {
    DEBUG_ENABLED,
    MAX_MESSAGE_LENGTH,
} from "../../../client/src/debug_enabled.mjs";
import { ResultErrTypes } from "../../../client/src/rust-types/ResultErrTypes.js";
import * as os from "os";

import * as crypto from "crypto";

// Globals
// -----------------------------------------------------------------------------
enum CodeChatEditorClientLocation {
    html,
    browser,
}

// Create a unique session ID for logging
const CAPTURE_SESSION_ID = crypto.randomUUID();

// True on Windows, false on OS X / Linux.
const is_windows = process.platform === "win32";

// These globals are truly global: only one is needed for this entire plugin.
//
// Where the webclient resides: `html` for a webview panel embedded in VSCode;
// `browser` to use an external browser.
let codechat_client_location: CodeChatEditorClientLocation =
    CodeChatEditorClientLocation.html;
// True if the subscriptions to IDE change notifications have been registered.
let subscribed = false;

// A unique instance of these variables is required for each CodeChat panel.
// However, this code doesn't have a good UI way to deal with multiple panels,
// so only one is supported at this time.
//
// The webview panel used to display the CodeChat Client
let webview_panel: vscode.WebviewPanel | undefined;
// A timer used to wait for additional events (keystrokes, etc.) before
// performing a render.
let idle_timer: NodeJS.Timeout | undefined;
// The text editor containing the current file.
let current_editor: vscode.TextEditor | undefined;
// True to ignore the next change event, which is produced by applying an
// `Update` from the Client.
let ignore_text_document_change = false;
// True to ignore the next active editor change event, since a `CurrentFile`
// message from the Client caused this change.
let ignore_active_editor_change = false;
// True to ignore the next text selection change, since updates to the cursor or
// scroll position from the Client trigged this change.
let ignore_selection_change = false;
// True to not report the next error.
let quiet_next_error = false;
// True if the editor contents have changed (are dirty) from the perspective of
// the CodeChat Editor (not if the contents are saved to disk).
let is_dirty = false;
// The version of the current file.
let version = 0.0;

// An object to start/stop the CodeChat Editor Server.
let codeChatEditorServer: CodeChatEditorServer | undefined;
// Before using `CodeChatEditorServer`, we must initialize it.
{
    const ext = vscode.extensions.getExtension(
        "CodeChat.codechat-editor-client",
    );
    assert(ext !== undefined);
    initServer(ext.extensionPath);
}

// -----------------------------------------------------------------------------
// CAPTURE (Dissertation instrumentation)
// -----------------------------------------------------------------------------

function isInMarkdownCodeFence(doc: vscode.TextDocument, line: number): boolean {
    // Very simple fence tracker: toggles when encountering ``` or ~~~ at start of line.
    // Good enough for dissertation instrumentation; refine later if needed.
    let inFence = false;
    for (let i = 0; i <= line; i++) {
        const t = doc.lineAt(i).text.trim();
        if (t.startsWith("```") || t.startsWith("~~~")) {
            inFence = !inFence;
        }
    }
    return inFence;
}

function isInRstCodeBlock(doc: vscode.TextDocument, line: number): boolean {
    // Heuristic: find the most recent ".. code-block::" (or "::") and see if we're in its indented region.
    // This won’t be perfect, but it’s far better than file-level classification.
    let blockLine = -1;
    for (let i = line; i >= 0; i--) {
        const t = doc.lineAt(i).text;
        const tt = t.trim();
        if (tt.startsWith(".. code-block::") || tt === "::") {
            blockLine = i;
            break;
        }
        // If we hit a non-indented line after searching upward too far, keep going; rst blocks can be separated by blank lines.
    }
    if (blockLine < 0) return false;

    // RST code block content usually begins after optional blank line(s), indented.
    // Determine whether current line is indented relative to block directive line.
    const cur = doc.lineAt(line).text;
    if (cur.trim().length === 0) return false;

    // If it's indented at least one space/tab, treat it as inside block.
    return /^\s+/.test(cur);
}

function classifyAtPosition(doc: vscode.TextDocument, pos: vscode.Position): ActivityKind {
    if (DOC_LANG_IDS.has(doc.languageId)) {
        if (doc.languageId === "markdown") {
            return isInMarkdownCodeFence(doc, pos.line) ? "code" : "doc";
        }
        if (doc.languageId === "restructuredtext") {
            return isInRstCodeBlock(doc, pos.line) ? "code" : "doc";
        }
        // Other doc types: default to doc
        return "doc";
    }
    return "code";
}



// Types for talking to the Rust /capture endpoint.
// This mirrors `CaptureEventWire` in webserver.rs.
interface CaptureEventPayload {
    user_id: string;
    assignment_id?: string;
    group_id?: string;
    file_path?: string;
    event_type: string;
    data: any; // sent as JSON
}

// TODO: replace these with something real (e.g., VS Code settings)
// For now, we hard-code to prove that the pipeline works end-to-end.
const CAPTURE_USER_ID: string = (() => {
    try {
        const u = os.userInfo().username;
        if (u && u.trim().length > 0) {
            return u.trim();
        }
    } catch (_) {
        // fall through
    }

    // Fallbacks (should rarely be needed)
    return (
        process.env["USERNAME"] ||
        process.env["USER"] ||
        "unknown-user"
    );
})();

const CAPTURE_ASSIGNMENT_ID = "demo-assignment";
const CAPTURE_GROUP_ID = "demo-group";

// Base URL for the CodeChat server's /capture endpoint.
// NOTE: keep this in sync with whatever port your server actually uses.
const CAPTURE_SERVER_BASE = "http://127.0.0.1:8080";

// Simple classification of what the user is currently doing.
type ActivityKind = "doc" | "code" | "other";

// Language IDs that we treat as "documentation" for the dissertation metrics.
// You can refine this later if you want.
const DOC_LANG_IDS = new Set<string>([
    "markdown",
    "plaintext",
    "latex",
    "restructuredtext",
]);

// Track the last activity kind and when a reflective-writing (doc) session started.
let lastActivityKind: ActivityKind = "other";
let docSessionStart: number | null = null;

// Heuristic: classify a document as documentation vs. code vs. other.
function classifyDocument(doc: vscode.TextDocument | undefined): ActivityKind {
    if (!doc) {
        return "other";
    }
    if (DOC_LANG_IDS.has(doc.languageId)) {
        return "doc";
    }
    // Everything else we treat as code for now.
    return "code";
}

// Helper to send a capture event to the Rust server.
async function sendCaptureEvent(
    serverBaseUrl: string, // e.g. "http://127.0.0.1:8080"
    eventType: string,
    filePath?: string,
    data: any = {},
): Promise<void> {
    const payload: CaptureEventPayload = {
        user_id: CAPTURE_USER_ID,
        assignment_id: CAPTURE_ASSIGNMENT_ID,
        group_id: CAPTURE_GROUP_ID,
        file_path: filePath,
        event_type: eventType,
        data: {
            ...data,
            session_id: CAPTURE_SESSION_ID,
            client_timestamp_ms: Date.now(),
            client_tz_offset_min: new Date().getTimezoneOffset(),
        },
    };

    try {
        const resp = await fetch(`${serverBaseUrl}/capture`, {
            method: "POST",
            headers: {
                "Content-Type": "application/json",
            },
            body: JSON.stringify(payload),
        });

        if (!resp.ok) {
            console.error(
                "Capture event failed:",
                resp.status,
                await resp.text(),
            );
        }
    } catch (err) {
        console.error("Error sending capture event:", err);
    }
}

// Update activity state, emit switch + doc_session events as needed.
function noteActivity(kind: ActivityKind, filePath?: string) {
    const now = Date.now();

    // Handle entering / leaving a "doc" session.
    if (kind === "doc") {
        if (docSessionStart === null) {
            // Starting a new reflective-writing session.
            docSessionStart = now;
            void sendCaptureEvent(CAPTURE_SERVER_BASE, "session_start", filePath, {
                mode: "doc",
            });
        }
    } else {
        if (docSessionStart !== null) {
            // Ending a reflective-writing session.
            const durationMs = now - docSessionStart;
            docSessionStart = null;
            void sendCaptureEvent(CAPTURE_SERVER_BASE, "doc_session", filePath, {
                duration_ms: durationMs,
                duration_seconds: durationMs / 1000.0,
            });
            void sendCaptureEvent(CAPTURE_SERVER_BASE, "session_end", filePath, {
                mode: "doc",
            });
        }
    }

    // If we switched between doc and code, log a switch_pane event.
    const docOrCode = (k: ActivityKind) => k === "doc" || k === "code";
    if (docOrCode(lastActivityKind) && docOrCode(kind) && kind !== lastActivityKind) {
        void sendCaptureEvent(CAPTURE_SERVER_BASE, "switch_pane", filePath, {
            from: lastActivityKind,
            to: kind,
        });
    }

    lastActivityKind = kind;
}

// Activation/deactivation
// -----------------------------------------------------------------------------
//
// This is invoked when the extension is activated. It either creates a new
// CodeChat Editor Server instance or reveals the currently running one.
export const activate = (context: vscode.ExtensionContext) => {
    context.subscriptions.push(
        vscode.commands.registerCommand(
            "extension.codeChatEditorDeactivate",
            deactivate,
        ),
        vscode.commands.registerCommand(
            "extension.codeChatEditorActivate",
            async () => {
                console_log("CodeChat Editor extension: starting.");

                // CAPTURE: mark the start of an editor session.
                const active = vscode.window.activeTextEditor;
                const startFilePath = active?.document.fileName;
                void sendCaptureEvent(
                    CAPTURE_SERVER_BASE,
                    "session_start",
                    startFilePath,
                    {
                        mode: "vscode_extension",
                    },
                );

                if (!subscribed) {
                    subscribed = true;

                    // Render when the text is changed by listening for the
                    // correct `event
                    // <https://code.visualstudio.com/docs/extensionAPI/vscode-api#Event>`\_.
                    context.subscriptions.push(
                        vscode.workspace.onDidChangeTextDocument((event) => {
                            // VSCode sends empty change events -- ignore these.
                            if (event.contentChanges.length === 0) {
                                return;
                            }
                            if (ignore_text_document_change) {
                                ignore_text_document_change = false;
                                return;
                            }
                            console_log(
                                `CodeChat Editor extension: text changed - ${
                                    event.reason
                                }, ${format_struct(event.contentChanges)}.`,
                            );

                            // CAPTURE: classify this as documentation vs. code and log a write_* event.
                            const doc = event.document;
//                            const kind = classifyDocument(doc);
                            const firstChange = event.contentChanges[0];
                            const pos = firstChange.range.start;
                            const kind = classifyAtPosition(doc, pos);

                            const filePath = doc.fileName;
                            const charsTyped = event.contentChanges
                                .map((c) => c.text.length)
                                .reduce((a, b) => a + b, 0);

                            if (kind === "doc") {
                                void sendCaptureEvent(
                                    CAPTURE_SERVER_BASE,
                                    "write_doc",
                                    filePath,
                                    {
                                        chars_typed: charsTyped,
                                        languageId: doc.languageId,
                                    },
                                );
                            } else if (kind === "code") {
                                void sendCaptureEvent(
                                    CAPTURE_SERVER_BASE,
                                    "write_code",
                                    filePath,
                                    {
                                        chars_typed: charsTyped,
                                        languageId: doc.languageId,
                                    },
                                );
                            }

                            // Update our notion of current activity + doc session.
                            noteActivity(kind, filePath);

                            send_update(true);
                        }),
                    );

                    // Render when the active editor changes.
                    context.subscriptions.push(
                        vscode.window.onDidChangeActiveTextEditor((event) => {
                            // If no text editor is active (for example, the
                            // CodeChat Editor has focus), ignore this update.
                            if (event === undefined) {
                                return;
                            }
                            if (ignore_active_editor_change) {
                                ignore_active_editor_change = false;
                                return;
                            }
                            // Skip an update if we've already sent a
                            // `CurrentFile` for this editor.
                            if (
                                current_editor ===
                                vscode.window.activeTextEditor
                            ) {
                                return;
                            }

                            // CAPTURE: update activity + possible switch_pane/doc_session.
                            const doc = event.document;
                            // const kind = classifyDocument(doc);
                            const pos = event.selection?.active ?? new vscode.Position(0, 0);
                            const kind = classifyAtPosition(doc, pos);

                            const filePath = doc.fileName;
                            noteActivity(kind, filePath);

                            send_update(true);
                        }),
                    );

                    context.subscriptions.push(
                        vscode.window.onDidChangeTextEditorSelection((event) => {
                            if (ignore_selection_change) {
                                ignore_selection_change = false;
                                return;
                            }

                            console_log(
                                "CodeChat Editor extension: sending updated cursor/scroll position.",
                            );

                            // CAPTURE: treat a selection change as "activity" in this document.
                            const doc = event.textEditor.document;
                            // const kind = classifyDocument(doc);
                            const pos = event.selections?.[0]?.active ?? event.textEditor.selection.active;
                            const kind = classifyAtPosition(doc, pos);
                            const filePath = doc.fileName;
                            noteActivity(kind, filePath);

                            send_update(false);
                        }),
                    );

                    // CAPTURE: end of a debug/run session.
                    context.subscriptions.push(
                        vscode.debug.onDidTerminateDebugSession((session) => {
                            const active = vscode.window.activeTextEditor;
                            const filePath = active?.document.fileName;
                            void sendCaptureEvent(
                                CAPTURE_SERVER_BASE,
                                "run_end",
                                filePath,
                                {
                                    sessionName: session.name,
                                    sessionType: session.type,
                                },
                            );
                        }),
                    );

                    // CAPTURE: compile/build end events via VS Code tasks.
                    context.subscriptions.push(
                        vscode.tasks.onDidEndTaskProcess((e) => {
                            const active = vscode.window.activeTextEditor;
                            const filePath = active?.document.fileName;
                            const task = e.execution.task;
                            void sendCaptureEvent(
                                CAPTURE_SERVER_BASE,
                                "compile_end",
                                filePath,
                                {
                                    taskName: task.name,
                                    taskSource: task.source,
                                    exitCode: e.exitCode,
                                },
                            );
                        }),
                    );

                    // CAPTURE: listen for file saves.
                    context.subscriptions.push(
                        vscode.workspace.onDidSaveTextDocument((doc) => {
                            void sendCaptureEvent(
                                CAPTURE_SERVER_BASE,
                                "save",
                                doc.fileName,
                                {
                                    reason: "manual_save",
                                    languageId: doc.languageId,
                                    lineCount: doc.lineCount,
                                },
                            );
                        }),
                    );

                    // CAPTURE: start of a debug/run session.
                    context.subscriptions.push(
                        vscode.debug.onDidStartDebugSession((session) => {
                            const active = vscode.window.activeTextEditor;
                            const filePath = active?.document.fileName;
                            void sendCaptureEvent(
                                CAPTURE_SERVER_BASE,
                                "run",
                                filePath,
                                {
                                    sessionName: session.name,
                                    sessionType: session.type,
                                },
                            );
                        }),
                    );

                    // CAPTURE: compile/build events via VS Code tasks.
                    context.subscriptions.push(
                        vscode.tasks.onDidStartTaskProcess((e) => {
                            const active = vscode.window.activeTextEditor;
                            const filePath = active?.document.fileName;
                            const task = e.execution.task;
                            void sendCaptureEvent(
                                CAPTURE_SERVER_BASE,
                                "compile",
                                filePath,
                                {
                                    taskName: task.name,
                                    taskSource: task.source,
                                    definition: task.definition,
                                    processId: e.processId,
                                },
                            );
                        }),
                    );
                }

                // Get the CodeChat Client's location from the VSCode configuration.
                const codechat_client_location_str = vscode.workspace
                    .getConfiguration("CodeChatEditor.Server")
                    .get("ClientLocation");
                assert(typeof codechat_client_location_str === "string");
                switch (codechat_client_location_str) {
                    case "html":
                        codechat_client_location = CodeChatEditorClientLocation.html;
                        break;

                    case "browser":
                        codechat_client_location = CodeChatEditorClientLocation.browser;
                        break;

                    default:
                        assert(false);
                }

                // Create or reveal the webview panel; if this is an external
                // browser, we'll open it after the client is created.
                if (codechat_client_location === CodeChatEditorClientLocation.html) {
                    if (webview_panel !== undefined) {
                        webview_panel.reveal(undefined, true);
                    } else {
                        webview_panel = vscode.window.createWebviewPanel(
                            "CodeChat Editor",
                            "CodeChat Editor",
                            {
                                preserveFocus: true,
                                viewColumn: vscode.ViewColumn.Beside,
                            },
                            {
                                enableScripts: true,
                                retainContextWhenHidden: true,
                            },
                        );
                        webview_panel.onDidDispose(async () => {
                            console_log("CodeChat Editor extension: shut down webview.");
                            quiet_next_error = true;
                            webview_panel = undefined;
                            await stop_client();
                        });
                    }
                }

                // Provide a simple status display while the server is starting up.
                if (webview_panel !== undefined) {
                    webview_panel.webview.html = "<h1>CodeChat Editor</h1><p>Loading...</p>";
                } else {
                    vscode.window.showInformationMessage(
                        "The CodeChat Editor is loading in an external browser...",
                    );
                }

                // Start the server.
                console_log("CodeChat Editor extension: starting server.");
                codeChatEditorServer = new CodeChatEditorServer();

                const hosted_in_ide =
                    codechat_client_location === CodeChatEditorClientLocation.html;
                console_log(
                    `CodeChat Editor extension: sending message Opened(${hosted_in_ide}).`,
                );
                await codeChatEditorServer.sendMessageOpened(hosted_in_ide);

                if (codechat_client_location === CodeChatEditorClientLocation.browser) {
                    send_update(false);
                }

                while (codeChatEditorServer) {
                    const message_raw = await codeChatEditorServer.getMessage();
                    if (message_raw === null) {
                        console_log("CodeChat Editor extension: queue closed.");
                        break;
                    }

                    const { id, message } = JSON.parse(message_raw) as EditorMessage;
                    console_log(
                        `CodeChat Editor extension: Received data id = ${id}, message = ${format_struct(
                            message,
                        )}.`,
                    );
                    assert(id !== undefined);
                    assert(message !== undefined);
                    if (message === "Closed") {
                        break;
                    }
                    const keys = Object.keys(message);
                    assert(keys.length === 1);
                    const key =
                        keys[0] as KeysOfRustEnum<EditorMessageContents>;
                    const value = Object.values(message)[0];

                    switch (key) {
                        case "Update": {
                            const current_update = value as UpdateMessageContents;
                            const doc = get_document(current_update.file_path);
                            if (doc === undefined) {
                                await sendResult(id, {
                                    NoOpenDocument: current_update.file_path,
                                });
                                break;
                            }
                            if (current_update.contents !== undefined) {
                                const source = current_update.contents.source;

                                ignore_text_document_change = true;
                                ignore_selection_change = true;

                                const wse = new vscode.WorkspaceEdit();

                                if ("Plain" in source) {
                                    wse.replace(
                                        doc.uri,
                                        doc.validateRange(
                                            new vscode.Range(0, 0, doc.lineCount, 0),
                                        ),
                                        source.Plain.doc,
                                    );
                                } else {
                                    assert("Diff" in source);

                                    if (source.Diff.version !== version) {
                                        await sendResult(id, {
                                            OutOfSync: [
                                                version,
                                                source.Diff.version,
                                            ],
                                        });
                                        // Send an `Update` with the full text to
                                        // re-sync the Client.
                                        console_log(
                                            "CodeChat Editor extension: sending update because Client is out of sync.",
                                        );
                                        send_update(true);
                                        break;
                                    }
                                    const diffs = source.Diff.doc;
                                    for (const diff of diffs) {
                                        const from = doc.positionAt(diff.from);
                                        if (diff.to === undefined) {
                                            wse.insert(doc.uri, from, diff.insert);
                                        } else {
                                            const to = doc.positionAt(diff.to);
                                            wse.replace(doc.uri, new Range(from, to), diff.insert);
                                        }
                                    }
                                }
                                await vscode.workspace.applyEdit(wse);
                                ignore_text_document_change = false;
                                ignore_selection_change = false;

                                version = current_update.contents.version;
                            }

                            const editor = get_text_editor(doc);

                            const scroll_line = current_update.scroll_position;
                            if (scroll_line !== undefined && editor) {
                                ignore_selection_change = true;
                                const scroll_position = new vscode.Position(scroll_line - 1, 0);
                                editor.revealRange(
                                    new vscode.Range(scroll_position, scroll_position),
                                    TextEditorRevealType.AtTop,
                                );
                            }

                            const cursor_line = current_update.cursor_position;
                            if (cursor_line !== undefined && editor) {
                                ignore_selection_change = true;
                                const cursor_position = new vscode.Position(cursor_line - 1, 0);
                                editor.selections = [
                                    new vscode.Selection(cursor_position, cursor_position),
                                ];
                            }
                            await sendResult(id);
                            break;
                        }

                        case "CurrentFile": {
                            const current_file = value[0] as string;
                            const is_text = value[1] as boolean | undefined;
                            if (is_text) {
                                let document;
                                try {
                                    document = await vscode.workspace.openTextDocument(current_file);
                                } catch (e) {
                                    await sendResult(id, {
                                        OpenFileFailed: [current_file, (e as Error).toString()],
                                    });
                                    continue;
                                }
                                ignore_active_editor_change = true;
                                current_editor = await vscode.window.showTextDocument(
                                    document,
                                    current_editor?.viewColumn,
                                );
                                ignore_active_editor_change = false;
                                await sendResult(id);
                            } else {
                                // TODO: open using a custom document editor.
                                // See
                                // [openCustomDocument](https://code.visualstudio.com/api/references/vscode-api#CustomEditorProvider.openCustomDocument),
                                // which can evidently be called
                                // [indirectly](https://stackoverflow.com/a/65101181/4374935).
                                // See also
                                // [Built-in Commands](https://code.visualstudio.com/api/references/commands).
                                // For now, simply respond with an OK, since the
                                // following doesn't work.
                                /**
                                    commands
                                        .executeCommand(
                                            "vscode.open",
                                            vscode.Uri.file(current_file),
                                            { viewColumn: current_editor?.viewColumn },
                                        )
                                        .then(
                                            async () => await sendResult(id),
                                            async (reason) =>
                                                await sendResult(id, {
                                                    OpenFileFailed: [current_file, reason],
                                                }),
                                        );
                                */
                                await sendResult(id);
                            }
                            break;
                        }

                        case "Result": {
                            const result_contents = value as MessageResult;
                            if ("Err" in result_contents) {
                                const err = result_contents[
                                    "Err"
                                ] as ResultErrTypes;
                                if (
                                    err instanceof Object &&
                                    "OutOfSync" in err
                                ) {
                                    // Send an update to re-sync the Client.
                                    console.warn(
                                        "Client is out of sync; resyncing.",
                                    );
                                    send_update(true);
                                } else {
                                    // If the client is out of sync, re-sync it.
                                    if (result_contents)
                                        show_error(
                                            `Error in message ${id}: ${JSON.stringify(err)}`,
                                        );
                                }
                            }
                            break;
                        }

                        case "LoadFile": {
                            const [load_file, is_current] = value as [
                                string,
                                boolean,
                            ];
                            // Look through all open documents to see if we have
                            // the requested file.
                            const doc = get_document(load_file);
                            // If we have this file and the request is for the
                            // current file to edit/view in the Client, assign a
                            // version.
                            if (doc !== undefined && is_current) {
                                version = rand();
                            }
                            const load_file_result: null | [string, number] =
                                doc === undefined
                                    ? null
                                    : [doc.getText(), version];
                            console_log(
                                `CodeChat Editor extension: Result(LoadFile(id = ${id}, ${format_struct(load_file_result)}))`,
                            );
                            await codeChatEditorServer.sendResultLoadfile(id, load_file_result);
                            break;
                        }

                        case "ClientHtml": {
                            const client_html = value as string;
                            assert(webview_panel !== undefined);
                            webview_panel.webview.html = client_html;
                            await sendResult(id);
                            send_update(false);
                            break;
                        }

                        default:
                            console.error(
                                `Unhandled message ${key}(${format_struct(value)}`,
                            );
                            break;
                    }
                }
            },
        ),
    );
};

// On deactivation, close everything down.
export const deactivate = async () => {
    console_log("CodeChat Editor extension: deactivating.");

    // CAPTURE: if we were in a doc session, close it out so duration is recorded.
    if (docSessionStart !== null) {
        const now = Date.now();
        const durationMs = now - docSessionStart;
        docSessionStart = null;
        const active = vscode.window.activeTextEditor;
        const filePath = active?.document.fileName;

        await sendCaptureEvent(CAPTURE_SERVER_BASE, "doc_session", filePath, {
            duration_ms: durationMs,
            duration_seconds: durationMs / 1000.0,
            closed_by: "extension_deactivate",
        });
        await sendCaptureEvent(CAPTURE_SERVER_BASE, "session_end", filePath, {
            mode: "doc",
            closed_by: "extension_deactivate",
        });
    }

    // CAPTURE: mark the end of an editor session.
    const active = vscode.window.activeTextEditor;
    const endFilePath = active?.document.fileName;
    await sendCaptureEvent(CAPTURE_SERVER_BASE, "session_end", endFilePath, {
        mode: "vscode_extension",
    });

    await stop_client();
    webview_panel?.dispose();
    console_log("CodeChat Editor extension: deactivated.");
};

// Supporting functions
// -----------------------------------------------------------------------------
//
// Format a complex data structure as a string when in debug mode.
/*eslint-disable-next-line @typescript-eslint/no-explicit-any */
const format_struct = (complex_data_structure: any): string =>
    DEBUG_ENABLED
        ? JSON.stringify(
              // If the struct is `undefined`, print an empty string.
              complex_data_structure ?? "null/undefined",
          ).substring(0, MAX_MESSAGE_LENGTH)
        : "";

// Send a result (a response to a message from the server) back to the server.
const sendResult = async (id: number, result?: ResultErrTypes) => {
    assert(codeChatEditorServer);
    console_log(
        `CodeChat Editor extension: sending Result(id = ${id}, ${format_struct(
            result,
        )}).`,
    );
    try {
        await codeChatEditorServer.sendResult(
            id,
            result === undefined ? undefined : JSON.stringify(result),
        );
    } catch (e) {
        show_error(`Error in sendResult for id ${id}: ${e}.`);
    }
};

// This is called after an event such as an edit, when the CodeChat panel
// becomes visible, or when the current editor changes. Wait a bit in case any
// other events occur, then request a render.
const send_update = (this_is_dirty: boolean) => {
    is_dirty ||= this_is_dirty;
    if (can_render()) {
        if (idle_timer !== undefined) {
            clearTimeout(idle_timer);
        }
        idle_timer = setTimeout(async () => {
            if (can_render()) {
                const ate = vscode.window.activeTextEditor;
                if (ate !== undefined && ate !== current_editor) {
                    current_editor = ate;
                    const current_file = ate.document.fileName;
                    console_log(
                        `CodeChat Editor extension: sending CurrentFile(${current_file}}).`,
                    );
                    try {
                        await codeChatEditorServer!.sendMessageCurrentFile(current_file);
                    } catch (e) {
                        show_error(`Error sending CurrentFile message: ${e}.`);
                    }
                    is_dirty = false;
                    return;
                }

                const cursor_position = current_editor!.selection.active.line + 1;
                const scroll_position =
                    current_editor!.visibleRanges[0].start.line + 1;
                const file_path = current_editor!.document.fileName;

                const option_contents: null | [string, number] = is_dirty
                    ? [current_editor!.document.getText(), (version = rand())]
                    : null;
                is_dirty = false;

                console_log(
                    `CodeChat Editor extension: sending Update(${file_path}, ${cursor_position}, ${scroll_position}, ${format_struct(
                        option_contents,
                    )})`,
                );
                await codeChatEditorServer!.sendMessageUpdatePlain(
                    file_path,
                    option_contents,
                    cursor_position,
                    scroll_position,
                );
            }
        }, autosave_timeout_ms);
    }
};

// Gracefully shut down the render client if possible. Shut down the client as well.
const stop_client = async () => {
    console_log("CodeChat Editor extension: stopping client.");
    if (codeChatEditorServer !== undefined) {
        console_log("CodeChat Editor extension: stopping server.");
        await codeChatEditorServer.stopServer();
        codeChatEditorServer = undefined;
    }

    if (idle_timer !== undefined) {
        clearTimeout(idle_timer);
        idle_timer = undefined;
    }

    current_editor = undefined;
};

// Provide an error message in the panel if possible.
const show_error = (message: string) => {
    if (quiet_next_error) {
        quiet_next_error = false;
        return;
    }
    console.error(`CodeChat Editor extension: ${message}`);
    if (webview_panel !== undefined) {
        if (!webview_panel.webview.html.startsWith("<h1>CodeChat Editor</h1>")) {
            webview_panel.webview.html = "<h1>CodeChat Editor</h1>";
        }
        webview_panel.webview.html += `<p style="white-space: pre-wrap;">${escape(
            message,
        )}</p><p>See the <a href="https://github.com/bjones1/CodeChat_Editor" target="_blank" rel="noreferrer noopener">docs</a>.</p>`;
    } else {
        vscode.window.showErrorMessage(
            message + "\nSee https://github.com/bjones1/CodeChat_Editor.",
        );
    }
};

// Only render if the window and editor are active, we have a valid render client,
// and the webview is visible.
const can_render = () => {
    return (
        (vscode.window.activeTextEditor !== undefined ||
            current_editor !== undefined) &&
        codeChatEditorServer !== undefined &&
        (codechat_client_location === CodeChatEditorClientLocation.browser ||
            webview_panel !== undefined)
    );
};

const get_document = (file_path: string) => {
    for (const doc of vscode.workspace.textDocuments) {
        if (
            (!is_windows && doc.fileName === file_path) ||
            (is_windows && doc.fileName.toUpperCase() === file_path.toUpperCase())
        ) {
            return doc;
        }
    }
    return undefined;
};

const get_text_editor = (doc: TextDocument): TextEditor | undefined => {
    for (const editor of vscode.window.visibleTextEditors) {
        if (editor.document === doc) return editor;
    }
};

/*eslint-disable-next-line @typescript-eslint/no-explicit-any */
const console_log = (...args: any) => {
    if (DEBUG_ENABLED) {
        console.log(...args);
    }
};

function getCurrentUsername(): string {
  try {
    // Most reliable on Windows/macOS/Linux
    const u = os.userInfo().username;
    if (u && u.trim().length > 0) return u.trim();
  } catch (_) {}

  // Fallbacks
  const envUser = process.env["USERNAME"] || process.env["USER"];
  return (envUser && envUser.trim().length > 0) ? envUser.trim() : "unknown-user";
}

