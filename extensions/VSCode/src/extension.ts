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
import process from "node:process";

// ### Third-party packages
import escape from "escape-html";
import vscode, { commands, Range, TextDocument, TextEditor } from "vscode";
import { CodeChatEditorServer, initServer } from "./index";

// ### Local packages
import {
    EditorMessage,
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
// True to enable additional debug logging.
const DEBUG_ENABLED = true;

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
                console_log("CodeChat Editor extension: starting.");

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
                            send_update(false);
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
                                // Without this, the websocket connection is
                                // dropped when the panel is hidden.
                                retainContextWhenHidden: true,
                            },
                        );
                        webview_panel.onDidDispose(async () => {
                            // Shut down the render client when the webview
                            // panel closes.
                            console_log(
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
                console_log("CodeChat Editor extension: starting server.");
                codeChatEditorServer = new CodeChatEditorServer();

                const hosted_in_ide =
                    codechat_client_location ===
                    CodeChatEditorClientLocation.html;
                console_log(
                    `CodeChat Editor extension: sending message Opened(${hosted_in_ide}).`,
                );
                await codeChatEditorServer.sendMessageOpened(hosted_in_ide);
                // For the external browser, we can immediately send the
                // `CurrentFile` message. For the WebView, we must first wait to
                // receive the HTML for the WebView (the `ClientHtml` message).
                if (
                    codechat_client_location ===
                    CodeChatEditorClientLocation.browser
                ) {
                    send_update(false);
                }

                while (codeChatEditorServer) {
                    const message_raw = await codeChatEditorServer.getMessage();
                    if (message_raw === null) {
                        console_log("CodeChat Editor extension: queue closed.");
                        break;
                    }
                    // Parse the data into a message.
                    const { id, message } = JSON.parse(
                        message_raw,
                    ) as EditorMessage;
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
                    const key = keys[0];
                    const value = Object.values(message)[0];

                    // Process this message.
                    switch (key) {
                        case "Update": {
                            const current_update =
                                value as UpdateMessageContents;
                            const doc = get_document(current_update.file_path);
                            if (doc === undefined) {
                                sendResult(
                                    id,
                                    `No open document for ${current_update.file_path}`,
                                );
                                break;
                            }
                            if (current_update.contents !== undefined) {
                                const source = current_update.contents.source;
                                // Is this plain text, or a diff? This will
                                // produce a change event, which we'll ignore.
                                ignore_text_document_change = true;
                                // Use a workspace edit, since calls to
                                // `TextEditor.edit` must be made to the active
                                // editor only.
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
                                        const from = doc.positionAt(diff.from);
                                        if (diff.to === undefined) {
                                            // This is an insert.
                                            wse.insert(
                                                doc.uri,
                                                from,
                                                diff.insert,
                                            );
                                        } else {
                                            // This is a replace or delete.
                                            const to = doc.positionAt(diff.to);
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
                                    // The VSCode line is zero-based; the
                                    // CodeMirror line is one-based.
                                    line -= 1;
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
                                        new vscode.Range(position, position),
                                    );
                                }
                            }
                            sendResult(id);
                            break;
                        }

                        case "CurrentFile": {
                            const current_file = value[0] as string;
                            const is_text = value[1] as boolean | undefined;
                            if (is_text) {
                                let document;
                                try {
                                    document =
                                        await vscode.workspace.openTextDocument(
                                            current_file,
                                        );
                                } catch (e) {
                                    sendResult(
                                        id,
                                        `Error: unable to open file ${current_file}: ${e}`,
                                    );
                                    continue;
                                }
                                ignore_active_editor_change = true;
                                current_editor =
                                    await vscode.window.showTextDocument(
                                        document,
                                        current_editor?.viewColumn,
                                    );
                                ignore_active_editor_change = false;
                                sendResult(id);
                            } else {
                                // TODO: open using a custom document editor.
                                // See
                                // [openCustomDocument](https://code.visualstudio.com/api/references/vscode-api#CustomEditorProvider.openCustomDocument),
                                // which can evidently be called
                                // [indirectly](https://stackoverflow.com/a/65101181/4374935).
                                // See also [Built-in
                                // Commands](https://code.visualstudio.com/api/references/commands).
                                // For now, simply respond with an OK, since the
                                // following doesn't work.
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
                                            () => sendResult(id),
                                            (reason) =>
                                                sendResult(
                                                    id,
                                                    `Error: unable to open file ${current_file}: ${reason}`,
                                                ),
                                        );
                                }
                                sendResult(id);
                            }
                            break;
                        }

                        case "Result": {
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
                            // Look through all open documents to see if we have
                            // the requested file.
                            const doc = get_document(load_file);
                            const load_file_result =
                                doc === undefined ? null : doc.getText();
                            console_log(
                                `CodeChat Editor extension: Result(LoadFile(${format_struct(load_file_result)}))`,
                            );
                            codeChatEditorServer.sendResultLoadfile(
                                id,
                                load_file_result,
                            );
                            break;
                        }

                        case "ClientHtml": {
                            const client_html = value as string;
                            assert(webview_panel !== undefined);
                            webview_panel.webview.html = client_html;
                            sendResult(id);
                            // Now that the Client is loaded, send the editor's
                            // current file to the server.
                            send_update(false);
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
                }
            },
        ),
    );
};

// On deactivation, close everything down.
export const deactivate = async () => {
    console_log("CodeChat Editor extension: deactivating.");
    await stop_client();
    webview_panel?.dispose();
    console_log("CodeChat Editor extension: deactivated.");
};

// Supporting functions
// --------------------
//
// Format a complex data structure as a string when in debug mode.
const format_struct = (complex_data_structure: any): string =>
    DEBUG_ENABLED
        ? JSON.stringify(complex_data_structure).substring(
              0,
              MAX_MESSAGE_LENGTH,
          )
        : "";

// Send a result (a response to a message from the server) back to the server.
const sendResult = (id: number, result: string | null = null) => {
    assert(codeChatEditorServer);
    console_log(
        `CodeChat Editor extension: sending result ${id}, ${format_struct(result)}.`,
    );
    codeChatEditorServer.sendResult(id, result);
};

// This is called after an event such as an edit, when the CodeChat panel
// becomes visible, or when the current editor changes. Wait a bit in case any
// other events occur, then request a render.
const send_update = (this_is_dirty: boolean) => {
    is_dirty ||= this_is_dirty;
    if (can_render()) {
        // Render after some inactivity: cancel any existing timer, then ...
        if (idle_timer !== undefined) {
            clearTimeout(idle_timer);
        }
        // ... schedule a render after 300 ms.
        idle_timer = setTimeout(async () => {
            if (can_render()) {
                const ate = vscode.window.activeTextEditor!;
                if (ate !== current_editor) {
                    // Send a new current file after a short delay; this allows
                    // the user to rapidly cycle through several editors without
                    // needing to reload the Client with each cycle.
                    current_editor = ate;
                    const current_file = ate!.document.fileName;
                    console_log(
                        `CodeChat Editor extension: sending CurrentFile(${current_file}}).`,
                    );
                    await codeChatEditorServer!.sendMessageCurrentFile(
                        current_file,
                    );
                    // Since we just requested a new file, the contents are
                    // clean by definition.
                    is_dirty = false;
                    // Don't send an updated cursor position until this file is
                    // loaded.
                    return;
                }

                // The
                // [Position](https://code.visualstudio.com/api/references/vscode-api#Position)
                // encodes the line as a zero-based value. In contrast,
                // CodeMirror
                // [Text.line](https://codemirror.net/docs/ref/#state.Text.line)
                // is 1-based.
                const current_line = ate.selection.active.line + 1;
                const file_path = ate.document.fileName;
                const cursor_position = current_line;
                // Send contents only if necessary.
                const option_contents = is_dirty
                    ? ate.document.getText()
                    : null;
                is_dirty = false;
                console_log(
                    `CodeChat Editor extension: sending Update(${file_path}, ${cursor_position}, ${format_struct(cursor_position)})`,
                );
                await codeChatEditorServer!.sendMessageUpdatePlain(
                    file_path,
                    option_contents,
                    cursor_position,
                    null,
                );
            }
        }, 300);
    }
};

// Gracefully shut down the render client if possible. Shut down the client as
// well.
const stop_client = async () => {
    console_log("CodeChat Editor extension: stopping client.");
    if (codeChatEditorServer !== undefined) {
        console_log("CodeChat Editor extension: stopping server.");
        await codeChatEditorServer.stopServer();
        codeChatEditorServer = undefined;
    }

    // Shut the timer down after the client is undefined, to ensure it can't be
    // started again by a call to `start_render()`.
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
        codeChatEditorServer !== undefined &&
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

const console_log = (...args: any) => {
    if (DEBUG_ENABLED) {
        console.log(...args);
    }
};
