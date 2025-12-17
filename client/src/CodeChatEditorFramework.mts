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
// `CodeChatEditorFramework.mts` -- the CodeChat Editor Client Framework
// =============================================================================
//
// This maintains a websocket connection between the CodeChat Editor Server. The
// accompanying HTML is a full-screen iframe, allowing the Framework to change
// or update the webpage in response to messages received from the websocket, or
// to report navigation events to as a websocket message when the iframe's
// location changes.
//
// Imports
// -----------------------------------------------------------------------------
//
// ### Third-party
import ReconnectingWebSocket from "./third-party/ReconnectingWebSocket.cjs";
import { show_toast as show_toast_core } from "./show_toast.mjs";

// ### Local
import { assert } from "./assert.mjs";
import { DEBUG_ENABLED, MAX_MESSAGE_LENGTH } from "./debug_enabled.mjs";
import {
    CodeChatForWeb,
    EditorMessage,
    EditorMessageContents,
    KeysOfRustEnum,
    MessageResult,
    UpdateMessageContents,
} from "./shared_types.mjs";
import {
    console_log,
    on_error,
    on_dom_content_loaded,
} from "./CodeChatEditor.mjs";
import { ResultErrTypes } from "./rust-types/ResultErrTypes.js";

// Websocket
// -----------------------------------------------------------------------------
//
// This code communicates with the CodeChat Editor Server via its websocket
// interface.
//
// The timeout for a websocket `Response`, in ms.
const RESPONSE_TIMEOUT_MS = 15000;

// An instance of the websocket communication class.
let webSocketComm: WebSocketComm;

class WebSocketComm {
    // Use a unique ID for each websocket message sent. See the Implementation
    // section on Message IDs for more information.
    ws_id = 4;

    // The websocket used by this class. Really a `ReconnectingWebSocket`, but
    // that's not a type.
    ws: WebSocket;

    // A map of message id to (timer id, callback) for all pending messages.
    pending_messages: Record<
        number,
        {
            timer_id: number;
            callback: () => void;
        }
    > = {};

    // The current filename of the file being edited. This is provided by the
    // IDE and passed back to it, but not otherwise used by the Framework.
    current_filename: string | undefined = undefined;

    // The version number of the current file. This default value will be
    // overwritten when the first `Update` is sent.
    version = 0.0;

    // True when the iframe is loading, so that an `Update` should be postponed
    // until the page load is finished. Otherwise, the page is fully loaded, so
    // the `Update` may be applied immediately.
    is_loading = false;

    // A promise to serialize calls to and from the Client. This is important: a
    // `CurrentFile` requires the Client to save, then switch to a new web page.
    // If an `Update` comes in, it should be applied after the `CurrentFile` has
    // finished executing.
    promise = Promise.resolve();

    constructor(ws_url: string) {
        // The `ReconnectingWebSocket` doesn't provide ALL the `WebSocket`
        // methods. Ignore this, since we can't use `ReconnectingWebSocket` as a
        // type.
        /// @ts-expect-error("This is legacy, third-party code.")
        this.ws = new ReconnectingWebSocket!(ws_url);
        // Identify this client on connection.
        this.ws.onopen = () => {
            console_log(
                `CodeChat Editor Client: websocket to CodeChat Server open.`,
            );
        };

        // Provide logging to help track down errors.
        this.ws.onerror = (event: Event) => {
            report_error(`CodeChat Editor Client: websocket error.`, event);
        };

        this.ws.onclose = (event: CloseEvent) => {
            console_log(
                `CodeChat Editor Client: websocket ${event.wasClean ? "" : "*NOT*"} cleanly closed ${event.reason}. This should only happen on shutdown.`,
            );
            console_log(event);
        };

        // Handle websocket messages.
        this.ws.onmessage = (event: MessageEvent) => {
            // Parse the received message, which must be a single element of a
            // dictionary representing an `EditorMessage`.
            const joint_message = JSON.parse(event.data) as EditorMessage;
            const { id, message } = joint_message;
            console_log(
                `CodeChat Editor Client: received data id = ${id}, message = ${format_struct(message)}`,
            );
            assert(id !== undefined);
            assert(message !== undefined);
            const keys = Object.keys(message);
            assert(keys.length === 1);
            const key = keys[0] as KeysOfRustEnum<EditorMessageContents>;
            const value = Object.values(message)[0];

            // Process this message.
            switch (key) {
                case "Update": {
                    // Load this data in.
                    const current_update = value as UpdateMessageContents;
                    // The rest of this should run after all other messages have
                    // been processed.
                    this.promise = this.promise.finally(async () => {
                        // Check or update the `current_filename`.
                        if (this.current_filename === undefined) {
                            this.current_filename = current_update.file_path;
                        } else if (
                            current_update.file_path !== this.current_filename
                        ) {
                            const msg = `Ignoring update for ${current_update.file_path} because it's not the current file ${this.current_filename}.`;
                            report_error(msg);
                            this.send_result(id, {
                                IgnoredUpdate: [
                                    current_update.file_path,
                                    this.current_filename,
                                ],
                            });
                            return;
                        }
                        const contents = current_update.contents;
                        const cursor_position = current_update.cursor_position;
                        if (contents !== undefined) {
                            // Check and update the version. If this is a diff,
                            // ensure the diff was made against the version of
                            // the file we have.
                            if ("Diff" in contents.source) {
                                if (
                                    contents.source.Diff.version !==
                                    this.version
                                ) {
                                    report_error(
                                        `Out of sync: Client version ${this.version} !== incoming version ${contents.source.Diff.version}.`,
                                    );
                                    this.send_result(id, {
                                        OutOfSync: [
                                            this.version,
                                            contents.source.Diff.version,
                                        ],
                                    });
                                    return;
                                }
                            }
                            this.version = contents.version;
                            // I'd prefer to use a system-maintained value to
                            // determine the ready state of the iframe, such as
                            // [readyState](https://developer.mozilla.org/en-US/docs/Web/API/Document/readyState).
                            // However, this value only applies to the initial
                            // load of the iframe; it doesn't change when the
                            // iframe's `src` attribute is changed. So, we have
                            // to track this manually instead.
                            if (!this.is_loading) {
                                // Wait until after the DOM is ready, since we
                                // rely on content set in
                                // `on_dom_content_loaded` in the Client.
                                await set_content(
                                    contents,
                                    current_update.cursor_position,
                                );
                            } else {
                                // If the page is still loading, wait until the
                                // load completes before updating the editable
                                // contents.
                                //
                                // Construct the promise to use; this causes the
                                // `onload` callback to be set immediately.
                                await new Promise<void>(
                                    (resolve) =>
                                        (root_iframe!.onload = async () => {
                                            this.is_loading = false;
                                            await set_content(
                                                contents,
                                                current_update.cursor_position,
                                                current_update.scroll_position,
                                            );
                                            resolve();
                                        }),
                                );
                            }
                        } else {
                            // We might receive a message while the Client is
                            // reloading; during this period, `scroll_to_line`
                            // isn't defined.
                            root_iframe!.contentWindow?.CodeChatEditor?.scroll_to_line?.(
                                cursor_position,
                                current_update.scroll_position,
                            );
                        }

                        this.send_result(id);
                    });
                    break;
                }

                case "CurrentFile": {
                    // Note that we can ignore `value[1]` (if the file is text
                    // or binary); the server only sends text files here.
                    const current_file = value[0] as string;
                    const testSuffix = testMode
                        ? // Append the test parameter correctly, depending if
                          // there are already parameters or not.
                          current_file.indexOf("?") === -1
                            ? "?test"
                            : "&test"
                        : "";
                    // Execute this after all other messages have been
                    // processed.
                    this.promise = this.promise.finally(async () => {
                        // If the page is still loading, then don't save.
                        // Otherwise, save the editor contents if necessary.
                        const cce = get_client();
                        await cce?.on_save(true);
                        // Now, it's safe to load a new file. Tell the client to
                        // allow this navigation -- the document it contains has
                        // already been saved.
                        if (cce !== undefined) {
                            cce.allow_navigation = true;
                        }
                        this.set_root_iframe_src(current_file + testSuffix);
                        // The `current_file` is a URL-encoded path, not a
                        // filesystem path. So, we can't use it for
                        // `current_filename`. Instead, signal that the
                        // `current_filename` should be set on the next `Update`
                        // message.
                        this.current_filename = undefined;
                        this.send_result(id);
                    });
                    break;
                }

                case "Result": {
                    // Cancel the timer for this message and remove it from
                    // `pending_messages`.
                    const pending_message = this.pending_messages[id];
                    if (pending_message !== undefined) {
                        const { timer_id, callback } =
                            this.pending_messages[id];
                        clearTimeout(timer_id);
                        callback();
                        delete this.pending_messages[id];
                    }

                    // Report if this was an error.
                    const result_contents = value as MessageResult;
                    if ("Err" in result_contents) {
                        report_error(
                            `Error in message ${id}: ${JSON.stringify(result_contents.Err)}.`,
                            result_contents.Err,
                        );
                    }
                    break;
                }

                default: {
                    const msg = `Received unhandled message ${key}(${format_struct(
                        value,
                    )})`;
                    report_error(msg);
                    this.send_result(id, {
                        ClientIllegalMessageReceived: `${key}(${format_struct(
                            value,
                        )})`,
                    });
                    break;
                }
            }
        };
    }

    /*eslint-disable-next-line @typescript-eslint/no-explicit-any */
    send = (data: any) => this.ws.send(data);
    /*eslint-disable-next-line @typescript-eslint/no-explicit-any */
    close = (...args: any) => this.ws.close(...args);

    set_root_iframe_src = (url: string) => {
        // Set the new src to (re)load content. At startup, the `srcdoc`
        // attribute shows some welcome text. Remove it so that we can now
        // assign the `src` attribute.
        root_iframe!.removeAttribute("srcdoc");
        root_iframe!.src = url;
        // Track the `is_loading` status.
        this.is_loading = true;
        root_iframe!.onload = () => (this.is_loading = false);
    };

    // Report an error from the server.
    report_server_timeout = (message_id: number) => {
        delete this.pending_messages[message_id];
        report_error(`Error: server timeout for message id ${message_id}`);
    };

    // Send a message expecting a result to the server.
    send_message = (
        message: EditorMessageContents,
        callback: () => void = () => 0,
    ) => {
        const id = this.ws_id;
        // The Client gets every third ID -- the IDE gets another third, while
        // the Server gets the final third.
        this.ws_id += 3;
        // Add in the current filename to the message, if it's an `Update`.
        if (typeof message == "object" && "Update" in message) {
            assert(this.current_filename !== undefined);
            message.Update.file_path = this.current_filename!;
            // Update the version of this file if it's provided.
            this.version = message.Update.contents?.version ?? this.version;
        }
        console_log(
            `CodeChat Editor Client: sent message ${id}, ${format_struct(message)}`,
        );
        const jm: EditorMessage = {
            id: id,
            message: message,
        };
        this.ws.send(JSON.stringify(jm));
        this.pending_messages[id] = {
            timer_id: window.setTimeout(
                this.report_server_timeout,
                RESPONSE_TIMEOUT_MS,
                id,
            ),
            callback,
        };
    };

    // This is called by the Client when the user navigates to another webpage.
    current_file = (url: URL) => {
        // TODO: should we delay execution of user navigation until all previous
        // actions have finished, or ignore them and immediately perform the
        // user navigation?
        this.promise = this.promise.finally(() => {
            if (url.host === window.location.host) {
                // If this points to the Server, then tell the IDE to load a new
                // file.
                this.send_message(
                    { CurrentFile: [url.toString(), null] },
                    () => {
                        this.set_root_iframe_src(url.toString());
                    },
                );
            } else {
                // Otherwise, navigate to the provided page.
                this.set_root_iframe_src(url.toString());
            }
            // Read the `current_filename` from the next `Update` message.
            this.current_filename = undefined;
        });
    };

    // Send a result (a response to a message from the server) back to the
    // server.
    send_result = (id: number, result?: ResultErrTypes) => {
        const message: EditorMessageContents = {
            Result: result === undefined ? { Ok: "Void" } : { Err: result },
        };
        console_log(
            `CodeChat Client: sending result id = ${id}, message = ${format_struct(message)}`,
        );
        // We can't simply call `send_message` because that function expects a
        // result message back from the server.
        const jm: EditorMessage = {
            id,
            message,
        };
        this.ws.send(JSON.stringify(jm));
    };
}

// Return the `CodeChatEditor` object if the `root_iframe` contains the Client;
// otherwise, this is `undefined`.
const get_client = () => root_iframe?.contentWindow?.CodeChatEditor;

// Assign content to either the Client (if it's loaded) or the webpage (if not)
// in the `root_iframe`.
const set_content = async (
    contents: CodeChatForWeb,
    cursor_line?: number,
    scroll_line?: number,
) => {
    const client = get_client();
    if (client === undefined) {
        // See if this is the [simple viewer](#Client-simple-viewer). Otherwise,
        // it's just the bare document to replace.
        const cw =
            (
                root_iframe!.contentDocument?.getElementById(
                    "CodeChat-contents",
                ) as HTMLIFrameElement | undefined
            )?.contentWindow ?? root_iframe!.contentWindow!;
        cw.document.open();
        assert("Plain" in contents.source);
        cw.document.write(contents.source.Plain.doc);
        cw.document.close();
    } else {
        await root_iframe!.contentWindow!.CodeChatEditor.open_lp(
            contents,
            cursor_line,
            scroll_line,
        );
    }
};

// The iframe element which composes this page.
let root_iframe: HTMLIFrameElement | undefined;

// True when in test mode.
let testMode = false;

// Load the dynamic content into the static page.
export const page_init = (
    // The pathname for the websocket to use. The remainder of the URL is
    // derived from the hosting page's URL. See the
    // [Location docs](https://developer.mozilla.org/en-US/docs/Web/API/Location)
    // for a nice, interactive definition of the components of a URL.
    ws_pathname: string,
    // Test mode flag
    testMode_: boolean,
) => {
    testMode = testMode_;
    on_dom_content_loaded(() => {
        // Provide basic error reporting for uncaught errors.
        window.addEventListener("unhandledrejection", on_error);
        window.addEventListener("error", on_error);

        // If the hosting page uses HTTPS, then use a secure websocket (WSS
        // protocol); otherwise, use an insecure websocket (WS).
        const protocol = window.location.protocol === "http:" ? "ws:" : "wss:";
        // Build a websocket address based on the URL of the current page.
        webSocketComm = new WebSocketComm(
            `${protocol}//${window.location.host}/${ws_pathname}`,
        );
        root_iframe = document.getElementById(
            "CodeChat-iframe",
        )! as HTMLIFrameElement;
        window.CodeChatEditorFramework = {
            webSocketComm,
        };
    });
};

// Tell TypeScript about the global namespace this program defines.
declare global {
    interface Window {
        CodeChatEditorFramework: {
            webSocketComm: WebSocketComm;
        };
        CodeChatEditor_test: unknown;
    }
}

const show_toast = (text: string) => {
    if (get_client() === undefined) {
        show_toast_core(text);
    } else {
        root_iframe!.contentWindow!.CodeChatEditor.show_toast(text);
    }
};

// Format a complex data structure as a string when in debug mode.
/*eslint-disable-next-line @typescript-eslint/no-explicit-any */
export const format_struct = (complex_data_structure: any): string =>
    DEBUG_ENABLED
        ? JSON.stringify(complex_data_structure).substring(
              0,
              MAX_MESSAGE_LENGTH,
          )
        : "";

/*eslint-disable-next-line @typescript-eslint/no-explicit-any */
const report_error = (text: string, ...objs: any) => {
    console.error(text);
    if (objs !== undefined) {
        console.log(...objs);
    }
    show_toast(text);
};
