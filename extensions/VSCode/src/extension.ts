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
// `extension.ts` - The CodeChat Editor Visual Studio Code extension
// =================================================================
//
// This extension creates a webview, then uses a websocket connection to the
// CodeChat Editor Server and Client to render editor text in that webview.
//
// Imports
// -------
//
// ### Node.js packages
import assert from "assert";
import child_process from "node:child_process";
import process from "node:process";

// ### Third-party packages
import escape from "escape-html";
import vscode, {
    commands,
    Position,
    Range,
    TextDocument,
    TextEditor,
} from "vscode";
import { WebSocket } from "ws";

// ### Local packages
import {
    CodeChatForWeb,
    CodeMirror,
    EditorMessage,
    EditorMessageContents,
    MessageResult,
    UpdateMessageContents,
} from "../../../client/src/shared_types.mjs";

// Globals
// -------
enum CodeChatEditorClientLocation {
    html,
    browser,
}
// The max length of a message to show in the console.
const MAX_MESSAGE_LENGTH = 200;
// The timeout for a websocket `Response`.
const RESPONSE_TIMEOUT = 15000;
// True to enable additional debug logging.
const DEBUG_ENABLED = false;

// True on Windows, false on OS X / Linux.
const is_windows = process.platform === "win32";

// These globals are truly global: only one is needed for this entire plugin.
let websocket: WebSocket | undefined;
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
// Use a unique ID for each websocket message sent. See the Implementation
// section on Message IDs for more information.
let message_id = -9007199254740989;
// A map of message id to (timer id, callback) for all pending messages.
const pending_messages: Record<
    number,
    {
        timer_id: NodeJS.Timeout;
        callback: (succeeded: boolean) => void;
    }
> = {};
// The text editor containing the current file.
let current_editor: vscode.TextEditor | undefined;
// True to ignore the next change event, which is produced by applying an
// `Update` from the Client.
let ignore_text_document_change = false;
// True to ignore the next active editor change event, since a `CurrentFile`
// message from the Client caused this change.
let ignore_active_editor_change = false;
// True to ignore the next text selection change, since updates to the
// cursor or scroll position from the Client trigged this change.
let ignore_selection_change = false;
// True to not report the next error.
let quiet_next_error = false;
// True if the editor contents have changed (are dirty) from the perspective
// of the CodeChat Editor (not if the contents are saved to disk).
let is_dirty = false;

// Activation/deactivation
// -----------------------
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
                console.log("CodeChat Editor extension starting.");

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
                            console.log(
                                `CodeChat Editor extension: text changed - ${
                                    event.reason
                                }, ${format_struct(event.contentChanges)}.`,
                            );
                            send_update(true);
                        }),
                    );

                    // Render when the active editor changes.
                    context.subscriptions.push(
                        vscode.window.onDidChangeActiveTextEditor((_event) => {
                            if (ignore_active_editor_change) {
                                ignore_active_editor_change = false;
                                return;
                            }
                            current_file();
                        }),
                    );

                    context.subscriptions.push(
                        vscode.window.onDidChangeTextEditorSelection(
                            (_event) => {
                                if (ignore_selection_change) {
                                    ignore_selection_change = false;
                                    return;
                                }
                                send_update(false);
                            },
                        ),
                    );
                }

                // Get the CodeChat Client's location from the VSCode
                // configuration.
                const codechat_client_location_str = vscode.workspace
                    .getConfiguration("CodeChatEditor.Server")
                    .get("ClientLocation");
                assert(typeof codechat_client_location_str === "string");
                switch (codechat_client_location_str) {
                    case "html":
                        codechat_client_location =
                            CodeChatEditorClientLocation.html;
                        break;

                    case "browser":
                        codechat_client_location =
                            CodeChatEditorClientLocation.browser;
                        break;

                    default:
                        assert(false);
                }

                // Create or reveal the webview panel; if this is an external
                // browser, we'll open it after the client is created.
                if (
                    codechat_client_location ===
                    CodeChatEditorClientLocation.html
                ) {
                    if (webview_panel !== undefined) {
                        // As below, don't take the focus when revealing.
                        webview_panel.reveal(undefined, true);
                    } else {
                        // Create a webview panel.
                        webview_panel = vscode.window.createWebviewPanel(
                            "CodeChat Editor",
                            "CodeChat Editor",
                            {
                                // Without this, the focus becomes this webview;
                                // setting this allows the code window open
                                // before this command was executed to retain
                                // the focus and be immediately rendered.
                                preserveFocus: true,
                                // Put this in the a column beside the current
                                // column.
                                viewColumn: vscode.ViewColumn.Beside,
                            },
                            // See
                            // [WebViewOptions](https://code.visualstudio.com/api/references/vscode-api#WebviewOptions).
                            {
                                enableScripts: true,
                                // Per the
                                // [docs](https://code.visualstudio.com/api/references/vscode-api#WebviewOptions),
                                // "If a webview accesses localhost content, we
                                // recommend that you specify port mappings even
                                // if the `webviewPort` and `extensionHostPort`
                                // ports are the same."
                                portMapping: [
                                    {
                                        extensionHostPort: get_port(),
                                        webviewPort: get_port(),
                                    },
                                ],
                                // Without this, the websocket connection is
                                // dropped when the panel is hidden.
                                retainContextWhenHidden: true,
                            },
                        );
                        webview_panel.onDidDispose(async () => {
                            // Shut down the render client when the webview
                            // panel closes.
                            console.log(
                                "CodeChat Editor extension: shut down webview.",
                            );
                            // Closing the webview abruptly closes the Client,
                            // which produces an error. Don't report it.
                            quiet_next_error = true;
                            webview_panel = undefined;
                            await stop_client();
                        });

                        // Render when the webview panel is shown.
                        webview_panel.onDidChangeViewState(
                            (
                                _event: vscode.WebviewPanelOnDidChangeViewStateEvent,
                            ) => {
                                // Only render if the webview was activated;
                                // this event also occurs when it's deactivated.
                                if (webview_panel?.active) {
                                    send_update(true);
                                }
                            },
                        );
                    }
                }

                // Provide a simple status display while the CodeChat Editor
                // Server is starting up.
                if (webview_panel !== undefined) {
                    // If we have an ID, then the GUI is already running; don't
                    // replace it.
                    webview_panel.webview.html =
                        "<h1>CodeChat Editor</h1><p>Loading...</p>";
                } else {
                    vscode.window.showInformationMessage(
                        "The CodeChat Editor is loading in an external browser...",
                    );
                }

                // Start the server.
                try {
                    console.log("CodeChat Editor extension: starting server.");
                    await run_server(["start"]);
                } catch (err) {
                    assert(err instanceof Error);
                    show_error(err.message);
                    return;
                }

                if (websocket === undefined) {
                    console.log(
                        "CodeChat Editor extension: opening websocket.",
                    );

                    // Connect to the CodeChat Editor Server.
                    websocket = new WebSocket(
                        `ws://localhost:${get_port()}/vsc/ws-ide/${Math.random()}`,
                    );

                    let was_error: boolean = false;

                    websocket.on("error", (err: ErrorEvent) => {
                        was_error = true;
                        show_error(
                            `Error communicating with the CodeChat Editor Server: ${err.message}. Re-run the CodeChat Editor extension to restart it.`,
                        );
                        // The close event will be [emitted
                        // next](https://nodejs.org/api/net.html#net_event_error_1);
                        // that will handle cleanup.
                    });

                    websocket.on("close", (hadError: CloseEvent) => {
                        console.log(
                            "CodeChat Editor extension: closing websocket connection.",
                        );
                        // If there was an error, the event handler above
                        // already provided the message. Note: the [parameter
                        // hadError](https://nodejs.org/api/net.html#net_event_close_1)
                        // only applies to transmission errors, not to any other
                        // errors which trigger the error callback. Therefore,
                        // I'm using the `was_error` flag instead to catch
                        // non-transmission errors.
                        if (!was_error && hadError) {
                            show_error(
                                "The connection to the CodeChat Editor Server was closed due to a transmission error. Re-run the CodeChat Editor extension to restart it.",
                            );
                        }
                        websocket = undefined;
                        idle_timer = undefined;
                    });

                    websocket.on("open", () => {
                        console.log(
                            "CodeChat Editor extension: connected to server.",
                        );
                        assert(websocket !== undefined);
                        send_message({
                            Opened: {
                                VSCode:
                                    codechat_client_location ===
                                    CodeChatEditorClientLocation.html,
                            },
                        });
                        // For the external browser, we can immediately send the
                        // `CurrentFile` message. For the WebView, we must first
                        // wait to receive the HTML for the WebView (the
                        // `ClientHtml` message).
                        if (
                            codechat_client_location ===
                            CodeChatEditorClientLocation.browser
                        ) {
                            current_file();
                        }
                    });

                    websocket.on("message", (data) => {
                        // Parse the data into a message.
                        const { id, message } = JSON.parse(
                            data.toString(),
                        ) as EditorMessage;
                        console.log(
                            `CodeChat Editor extension: Received data id = ${id}, message = ${format_struct(
                                message,
                            )}.`,
                        );
                        assert(id !== undefined);
                        assert(message !== undefined);
                        const keys = Object.keys(message);
                        console.assert(keys.length === 1);
                        const key = keys[0];
                        const value = Object.values(message)[0];

                        // Process this message.
                        switch (key) {
                            case "Update": {
                                const current_update =
                                    value as UpdateMessageContents;
                                const doc = get_document(
                                    current_update.file_path,
                                );
                                if (doc === undefined) {
                                    send_result(id, {
                                        Err: "No open document for this file.",
                                    });
                                    break;
                                }
                                if (current_update.contents !== undefined) {
                                    const source =
                                        current_update.contents.source;
                                    // Is this plain text, or a diff? This will
                                    // produce a change event, which we'll
                                    // ignore.
                                    ignore_text_document_change = true;
                                    // Use a workspace edit, since calls to
                                    // `TextEditor.edit` must be made to the
                                    // active editor only.
                                    const wse = new vscode.WorkspaceEdit();
                                    if ("Plain" in source) {
                                        wse.replace(
                                            doc.uri,
                                            doc.validateRange(
                                                new vscode.Range(
                                                    0,
                                                    0,
                                                    doc.lineCount,
                                                    0,
                                                ),
                                            ),
                                            source.Plain.doc,
                                        );
                                    } else {
                                        assert("Diff" in source);
                                        const diffs = source.Diff.doc;
                                        for (const diff of diffs) {
                                            // Convert from character offsets from the
                                            // beginning of the document to a
                                            // `Position` (line, then offset on that
                                            // line) needed by VSCode.
                                            const from = doc.positionAt(
                                                diff.from,
                                            );
                                            if (diff.to === undefined) {
                                                // This is an insert.
                                                wse.insert(
                                                    doc.uri,
                                                    from,
                                                    diff.insert,
                                                );
                                            } else {
                                                // This is a replace or delete.
                                                const to = doc.positionAt(
                                                    diff.to,
                                                );
                                                wse.replace(
                                                    doc.uri,
                                                    new Range(from, to),
                                                    diff.insert,
                                                );
                                            }
                                        }
                                    }
                                    vscode.workspace
                                        .applyEdit(wse)
                                        .then(
                                            () =>
                                                (ignore_text_document_change = false),
                                        );
                                }
                                // Update the cursor position if provided.
                                let line = current_update.cursor_position;
                                if (line !== undefined) {
                                    const editor = get_text_editor(doc);
                                    if (editor) {
                                        ignore_selection_change = true;
                                        // The VSCode line is zero-based; the CodeMirror line is one-based.
                                        line -= 1;
                                        console.log(`Moving to line ${line}.`);
                                        const position = new vscode.Position(
                                            line,
                                            line,
                                        );
                                        editor.selections = [
                                            new vscode.Selection(
                                                position,
                                                position,
                                            ),
                                        ];
                                        editor.revealRange(
                                            new vscode.Range(
                                                position,
                                                position,
                                            ),
                                        );
                                    }
                                }
                                send_result(id);
                                break;
                            }

                            case "CurrentFile": {
                                const current_file = value[0] as string;
                                const is_text = value[1] as boolean | undefined;
                                if (is_text) {
                                    vscode.workspace
                                        .openTextDocument(current_file)
                                        .then(
                                            (document) => {
                                                ignore_active_editor_change = true;
                                                vscode.window.showTextDocument(
                                                    document,
                                                    current_editor?.viewColumn,
                                                );
                                                send_result(id);
                                            },
                                            (reason) =>
                                                send_result(id, {
                                                    Err: `Error: unable to open file ${current_file}: ${reason}`,
                                                }),
                                        );
                                } else {
                                    // TODO: open using a custom document
                                    // editor. See
                                    // [openCustomDocument](https://code.visualstudio.com/api/references/vscode-api#CustomEditorProvider.openCustomDocument),
                                    // which can evidently be called
                                    // [indirectly](https://stackoverflow.com/a/65101181/4374935).
                                    // See also [Built-in
                                    // Commands](https://code.visualstudio.com/api/references/commands).
                                    // For now, simply respond with an OK, since
                                    // the following doesn't work.
                                    if (false) {
                                        commands
                                            .executeCommand(
                                                "vscode.open",
                                                vscode.Uri.file(current_file),
                                                {
                                                    viewColumn:
                                                        current_editor?.viewColumn,
                                                },
                                            )
                                            .then(
                                                () => send_result(id),
                                                (reason) =>
                                                    send_result(id, {
                                                        Err: `Error: unable to open file ${current_file}: ${reason}`,
                                                    }),
                                            );
                                    }
                                    send_result(id);
                                }
                                break;
                            }

                            case "Result": {
                                // Cancel the timer for this message and remove
                                // it from `pending_messages`.
                                const pending_message = pending_messages[id];
                                if (pending_message !== undefined) {
                                    const { timer_id, callback } =
                                        pending_messages[id];
                                    clearTimeout(timer_id);
                                    // eslint-disable-next-line
                                    // n/no-callback-literal
                                    callback(true);
                                    delete pending_messages[id];
                                }

                                // Report if this was an error.
                                const result_contents = value as MessageResult;
                                if ("Err" in result_contents) {
                                    show_error(
                                        `Error in message ${id}: ${result_contents.Err}`,
                                    );
                                }
                                break;
                            }

                            case "LoadFile": {
                                const load_file = value as string;
                                // Look through all open documents to see if we
                                // have the requested file.
                                const doc = get_document(load_file);
                                const load_file_result =
                                    doc === undefined ? null : doc.getText();
                                send_result(id, {
                                    Ok: {
                                        LoadFile: load_file_result,
                                    },
                                });
                                break;
                            }

                            case "ClientHtml": {
                                const client_html = value as string;
                                assert(webview_panel !== undefined);
                                webview_panel.webview.html = client_html;
                                send_result(id);
                                // Now that the Client is loaded, send the
                                // editor's current file to the server.
                                current_file();
                                break;
                            }

                            default:
                                console.error(
                                    `Unhandled message ${key}(${format_struct(
                                        value,
                                    )}`,
                                );
                                break;
                        }
                    });
                } else {
                    console.log(
                        "CodeChat Editor extension: connection already pending, so a new client wasn't created.",
                    );
                }
            },
        ),
    );
};

// On deactivation, close everything down.
export const deactivate = async () => {
    console.log("CodeChat extension: deactivating.");
    await stop_client();
    webview_panel?.dispose();
    console.log("CodeChat extension: deactivated.");
};

// Supporting functions
// --------------------
//
// Send a message expecting a result to the server.
const send_message = (
    message: EditorMessageContents,
    callback: (succeeded: boolean) => void = (_) => 0,
) => {
    const id = message_id;
    message_id += 3;
    const jm: EditorMessage = {
        id,
        message,
    };
    assert(websocket);
    console.log(
        `CodeChat Editor extension: sending message ${format_struct(jm)}.`,
    );
    websocket.send(JSON.stringify(jm));
    pending_messages[id] = {
        timer_id: setTimeout(report_server_timeout, RESPONSE_TIMEOUT, id),
        callback,
    };
};

// Format a complex data structure as a string when in debug mode.
const format_struct = (complex_data_structure: any): string =>
    DEBUG_ENABLED
        ? JSON.stringify(complex_data_structure).substring(
              0,
              MAX_MESSAGE_LENGTH,
          )
        : "";

// Report an error from the server.
const report_server_timeout = (message_id: number) => {
    // Invoke the callback with an error.
    pending_messages[message_id]?.callback(false);

    // Remove the message from the pending messages and report the error.
    delete pending_messages[message_id];
    console.error(`Error: server timeout for message id ${message_id}`);
};

// Send a result (a response to a message from the server) back to the server.
const send_result = (id: number, result: MessageResult = { Ok: "Void" }) => {
    // We can't simply call `send_message` because that function expects a
    // result message back from the server.
    const jm: EditorMessage = {
        id,
        message: {
            Result: result,
        },
    };
    assert(websocket);
    console.log(
        `CodeChat Editor extension: sending result ${JSON.stringify(
            jm,
        ).substring(0, MAX_MESSAGE_LENGTH)}.`,
    );
    websocket.send(JSON.stringify(jm));
};

// This is called after an event such as an edit, or when the CodeChat panel
// becomes visible. Wait a bit in case any other events occur, then request a
// render.
const send_update = (this_is_dirty: boolean) => {
    is_dirty ||= this_is_dirty;
    if (can_render()) {
        // Render after some inactivity: cancel any existing timer, then ...
        if (idle_timer !== undefined) {
            clearTimeout(idle_timer);
        }
        // ... schedule a render after 300 ms.
        idle_timer = setTimeout(() => {
            if (can_render()) {
                const ate = vscode.window.activeTextEditor!;
                // The [Position](https://code.visualstudio.com/api/references/vscode-api#Position)
                // encodes the line as a zero-based value. In contrast, CodeMirror [Text.line](https://codemirror.net/docs/ref/#state.Text.line)
                // is 1-based.
                const current_line = ate.selection.active.line + 1;
                const Update: UpdateMessageContents = {
                    file_path: ate.document.fileName,
                    cursor_position: current_line,
                };
                // Send contents only if necessary.
                if (is_dirty) {
                    Update.contents = {
                        metadata: { mode: "" },
                        source: {
                            Plain: {
                                doc: ate.document.getText(),
                                doc_blocks: [],
                            },
                        },
                    };
                    is_dirty = false;
                }
                send_message({
                    Update,
                });
            }
        }, 300);
    }
};

const current_file = () => {
    // Only send a new current file is there's a change.
    const ate = vscode.window.activeTextEditor;
    if (can_render() && ate !== current_editor) {
        current_editor = ate;
        send_message({
            CurrentFile: [ate!.document.fileName, null],
        });
    }
};

// Gracefully shut down the render client if possible. Shut down the client as
// well.
const stop_client = async () => {
    console.log("CodeChat Editor extension: stopping client.");
    if (websocket !== undefined) {
        console.log("CodeChat Editor extension: ending connection.");
        websocket?.close();
        websocket = undefined;
    }

    // Shut the timer down after the client is undefined, to ensure it can't be
    // started again by a call to `start_render()`.
    if (idle_timer !== undefined) {
        clearTimeout(idle_timer);
        idle_timer = undefined;
    }

    // Shut down the server.
    try {
        await run_server(["stop"]);
    } catch (err) {
        assert(err instanceof Error);
        console.error(
            `CodeChat Editor Client: error on server shutdown - ${err.message}`,
        );
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
        // If the panel was displaying other content, reset it for errors.
        if (
            !webview_panel.webview.html.startsWith("<h1>CodeChat Editor</h1>")
        ) {
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

// Only render if the window and editor are active, we have a valid render
// client, and the webview is visible.
const can_render = () => {
    return (
        vscode.window.activeTextEditor !== undefined &&
        websocket !== undefined &&
        (codechat_client_location === CodeChatEditorClientLocation.browser ||
            webview_panel !== undefined)
    );
};

const get_document = (file_path: string) => {
    // Look through all open documents to see if we have the requested file.
    for (const doc of vscode.workspace.textDocuments) {
        // Make the possibly incorrect assumption that only Windows filesystems
        // are case-insensitive; I don't know how to easily determine the
        // case-sensitivity of the current filesystem without extra probing code
        // (write a file in mixed case, try to open it in another mixed case.)
        // Per [How to Work with Different
        // Filesystems](https://nodejs.org/en/learn/manipulating-files/working-with-different-filesystems#filesystem-behavior),
        // "Be wary of inferring filesystem behavior from `process.platform`.
        // For example, do not assume that because your program is running on
        // Darwin that you are therefore working on a case-insensitive
        // filesystem (HFS+), as the user may be using a case-sensitive
        // filesystem (HFSX)."
        //
        // The same article
        // [recommends](https://nodejs.org/en/learn/manipulating-files/working-with-different-filesystems#be-prepared-for-slight-differences-in-comparison-functions)
        // using `toUpperCase` for case-insensitive filename comparisons.
        if (
            (!is_windows && doc.fileName === file_path) ||
            (is_windows &&
                doc.fileName.toUpperCase() === file_path.toUpperCase())
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

const get_port = (): number => {
    const port = vscode.workspace
        .getConfiguration("CodeChatEditor.Server")
        .get("Port");
    assert(typeof port === "number");
    return port;
};

const run_server = (args: string[]) => {
    // Get the command from the VSCode configuration.
    let codechat_editor_server_command = vscode.workspace
        .getConfiguration("CodeChatEditor.Server")
        .get("Command");
    assert(typeof codechat_editor_server_command === "string");

    // If not specified, use the packaged binary.
    if (codechat_editor_server_command === "") {
        const ext = vscode.extensions.getExtension(
            "CodeChat.codechat-editor-client",
        );
        assert(ext !== undefined);
        codechat_editor_server_command =
            ext.extensionPath + "/server/codechat-editor-server";
    }

    let stdout = "";
    let stderr = "";
    return new Promise((resolve, reject) => {
        const server_process = child_process.spawn(
            codechat_editor_server_command as string,
            ["--port", get_port().toString()].concat(args),
        );
        server_process.on("error", (err: NodeJS.ErrnoException) => {
            const msg =
                err.code === "ENOENT"
                    ? `Error - cannot find the file ${err.path}`
                    : err;
            reject(
                new Error(`While starting the CodeChat Editor Server: ${msg}.`),
            );
        });

        server_process.on("exit", (code, signal) => {
            const exit_str = code ? `code ${code}` : `signal ${signal}`;
            if (code === 0) {
                resolve("");
            } else {
                reject(
                    new Error(
                        `${stdout}\n${stderr}\n\nCodeChat Editor Server exited with ${exit_str}.\n`,
                    ),
                );
            }
        });

        assert(server_process.stdout !== null);
        server_process.stdout.on("data", (chunk) => {
            stdout += chunk.toString();
        });

        assert(server_process.stderr !== null);
        server_process.stderr.on("data", (chunk) => {
            stderr += chunk.toString();
        });
    });
};
