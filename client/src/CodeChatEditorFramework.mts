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
// ## Imports

// ### JavaScript/TypeScript
//
// #### Third-party
import ReconnectingWebSocket from "./ReconnectingWebSocket.cjs";

// ## Websocket
//
// This code communicates with the CodeChat Editor Server via its websocket
// interface.
//
// ### Message types
//
// These mirror the same definitions in the Rust webserver, so that the two can
// exchange messages.
interface EditorMessage {
    id: number,
    message: EditorMessageContents
}

interface EditorMessageContents {
    Update?: UpdateMessageContents,
    CurrentFile?: string,
    Load?: string,
    Result?: string
    RequestClose?: null
}

let webSocketComm: WebSocketComm

class WebSocketComm {
    // Use a unique ID for each websocket message sent.
    ws_id = 0;
    // The websocket used by this class. Really a `ReconnectingWebSocket`, but
    // that's not a type.
    ws: WebSocket
    // A map of message id to timer id for all pending messages.
    pending_messages: Record<number, number> = {}
    // True when the iframe is loading, so that an `Update` should be postponed until the page load is finished. Otherwise, the page is fully loaded, so the `Update` may be applied immediately.
    onloading = false

    constructor(ws_url: string) {
        // The `ReconnectingWebSocket` doesn't provide ALL the `WebSocket`
        // methods. Ignore this, since we can't use `ReconnectingWebSocket` as a
        // type.
        /// @ts-ignore
        this.ws = new ReconnectingWebSocket!(ws_url);
        // Identify this client on connection.
        this.ws.onopen = () => {
            console.log(`CodeChat Client: websocket to CodeChat Server open.`);
        };

        // Provide logging to help track down errors.
        this.ws.onerror = (event: any) => {
            console.error(`CodeChat Client: websocket error ${event}.`);
        };

        this.ws.onclose = (event: any) => {
            console.log(
                `CodeChat Client: websocket closed by event type ${event.type}: ${event.detail}. This should only happen on shutdown.`,
            );
        };

        // Handle websocket messages.
        this.ws.onmessage = (event: any) => {
            // Parse the received message, which must be a single element of a
            // dictionary representing a `JointMessage`.
            const joint_message = JSON.parse(event.data) as EditorMessage
            const { id: id, message: message } = joint_message
            console.assert(id !== undefined)
            console.assert(message !== undefined)
            const keys = Object.keys(message)
            console.assert(keys.length === 1)
            const key = keys[0];
            const value = Object.values(message)[0]
            const root_iframe = get_root_iframe()!

            // Process this message.
            switch (key) {
                case "Update":
                    // Load this data in.
                    const current_update = value as UpdateMessageContents;
                    console.log(`Update(cursor_position: ${current_update.cursor_position}, scroll_position: ${current_update.scroll_position})`)

                    let result = ""
                    const contents = current_update.contents
                    if (contents !== null && contents !== undefined) {
                        // If the page is still loading, wait until the load completed before updating the editable contents.
                        if (this.onloading) {
                            root_iframe.onload = () => {
                                /// @ts-ignore
                                root_iframe.contentWindow.CodeChatEditor.open_lp(contents)
                                this.onloading = false
                            }
                        } else {
                            /// @ts-ignore
                            root_iframe.contentWindow.CodeChatEditor.open_lp(contents)
                        }
                    } else {
                        // TODO: handle scroll/cursor updates.
                        result = `Unhandled Update message: ${current_update}`
                        console.log(result)
                    }

                    this.send_result(id, result)
                    break;

                case "CurrentFile":
                    const current_file = value as string;
                    console.log(`CurrentFile(${current_file})`)
                    // Set the new src to (re)load content. At startup, the ``srcdoc`` attribute shows some welcome text. Remove it so that we can now assign the ``src`` attribute.
                    root_iframe.removeAttribute("srcdoc")
                    root_iframe.src = current_file
                    // There's no easy way to determine when the iframe's DOM is ready. This is a kludgy workaround -- set a flag.
                    this.onloading = true
                    root_iframe.onload = () => this.onloading = false
                    this.send_result(id, "")
                    break;

                case "Result":
                    // Cancel the timer for this message and remove it from
                    // `pending_messages`.
                    const timer_id = this.pending_messages[id]
                    if (timer_id !== undefined) {
                        clearTimeout(timer_id)
                        delete this.pending_messages[id]
                    }

                    // Report if this was an error.
                    const err = value as string;
                    if (value !== "") {
                        console.log(`Error in message ${id}: ${err}.`)
                    }
                    break;

                default:
                    console.log(`Unhandled message ${key}(${value})`);
                    break;
            }
        };
    }

    send = (data: any) => this.ws.send(data)
    close = (...args: any) => this.ws.close(...args)

    // Report an error from the server.
    report_server_timeout = (message_id: number) => {
        delete this.pending_messages[message_id]
        console.log(`Error: server timeout for message id ${message_id}`)
    }

    // Send a message expecting a result to the server.
    send_message = (id: number, message: EditorMessageContents) => {
        const jm: EditorMessage = {
            id: id,
            message: message
        }
        this.ws.send(JSON.stringify(jm))
        this.pending_messages[id] = setTimeout(this.report_server_timeout, 2000, id)
    }

    // Send a result (a response to a message from the server) back to the
    // server.
    send_result = (id: number, result: string = "") => {
        // We can't simply call `send_message` because that function expects a
        // result message back from the server.
        const jm: EditorMessage = {
            id: id,
            message: {
                Result: result
            }
        }
        this.ws.send(JSON.stringify(jm))
    }

}

const get_root_iframe = () => document.getElementById("CodeChat-iframe")! as HTMLIFrameElement

// Load the dynamic content into the static page.
export const page_init = (
    // The pathname for the websocket to use. The remainder of the URL is
    // derived from the hosting page's URL. See the
    // [Location docs](https://developer.mozilla.org/en-US/docs/Web/API/Location)
    // for a nice, interactive definition of the components of a URL.
    ws_pathname: string
) => {
    on_dom_content_loaded(async () => {
        // If the hosting page uses HTTPS, then use a secure websocket (WSS
        // protocol); otherwise, use an insecure websocket (WS).
        const protocol = window.location.protocol === "http:" ? "ws:" : "wss:";
        // Build a websocket address based on the URL of the current page.
        webSocketComm = new WebSocketComm(`${protocol}//${window.location.host}/${ws_pathname}`)
        window.CodeChatEditorFramework = {
            webSocketComm
        }
    });
}

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

// Tell TypeScript about the global namespace this program defines.
declare global {
    interface Window {
        CodeChatEditorFramework: {
            webSocketComm: WebSocketComm
        }
        CodeChatEditor_test: any
    }
}
