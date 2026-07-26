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
// =====================================================================
//
// This maintains a websocket connection with the CodeChat Editor Server. The
// accompanying HTML is a full-screen iframe, allowing the Framework to change
// or update the webpage in response to messages received from the websocket, or
// to report navigation events to as a websocket message when the iframe's
// location changes.
//
// Imports
// -------
//
// ### Third-party
import ReconnectingWebSocket from "./third-party/ReconnectingWebSocket.cjs";
import { showToast as showToastCore } from "./show_toast.mjs";

// ### Local
import { assert } from "./assert.mjs";
import {
    consoleLog,
    DEBUG_ENABLED,
    MAX_MESSAGE_LENGTH,
} from "./debug_enabled.mjs";
import {
    CodeChatForWeb,
    EditorMessage,
    EditorMessageContents,
    KeysOfRustEnum,
    MessageResult,
    UpdateMessageContents,
} from "./shared.mjs";
import { onError, onDomContentLoaded } from "./CodeChatEditor.mjs";
import { ResultErrTypes } from "./rust-types/ResultErrTypes.js";
import { CursorPosition } from "./rust-types/CursorPosition.js";

// Websocket
// ---------
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
    wsId = 4;

    // The websocket used by this class. Really a `ReconnectingWebSocket`, but
    // that's not a type.
    ws: WebSocket;

    // A map of message id to (timer id, callback) for all pending messages.
    pendingMessages: Record<
        number,
        {
            timerId: number;
            callback: () => void;
        }
    > = {};

    // The current filename of the file being edited. This is provided by the
    // IDE and passed back to it, but not otherwise used by the Framework.
    currentFilename: string | undefined = undefined;

    // The version number of the current file. This default value will be
    // overwritten when the first `Update` is sent.
    version = 0.0;

    // True when the iframe is loading, so that an `Update` should be postponed
    // until the page load is finished. Otherwise, the page is fully loaded, so
    // the `Update` may be applied immediately.
    isLoading = false;

    // A promise to serialize calls to and from the Client. This is important: a
    // `CurrentFile` requires the Client to save, then switch to a new web page.
    // If an `Update` comes in, it should be applied after the `CurrentFile` has
    // finished executing.
    promise = Promise.resolve();

    constructor(wsUrl: string) {
        // The `ReconnectingWebSocket` doesn't provide ALL the `WebSocket`
        // methods. Ignore this, since we can't use `ReconnectingWebSocket` as a
        // type.
        /// @ts-expect-error("This is legacy, third-party code.")
        this.ws = new ReconnectingWebSocket!(wsUrl);
        // Identify this client on connection.
        this.ws.onopen = () => {
            consoleLog(
                `CodeChat Editor Client: websocket to CodeChat Server open.`,
            );
        };

        // Provide logging to help track down errors.
        this.ws.onerror = (event: Event) => {
            reportError(`CodeChat Editor Client: websocket error.`, event);
        };

        this.ws.onclose = (event: CloseEvent) => {
            consoleLog(
                `CodeChat Editor Client: websocket ${event.wasClean ? "" : "*NOT*"} cleanly closed ${event.reason}. This should only happen on shutdown.`,
            );
            consoleLog(event);
        };

        // Handle websocket messages.
        this.ws.onmessage = (event: MessageEvent) => {
            // Parse the received message, which must be a single element of a
            // dictionary representing an `EditorMessage`.
            const jointMessage = JSON.parse(event.data) as EditorMessage;
            const { id, message } = jointMessage;
            consoleLog(
                `CodeChat Editor Client: received data id = ${id}, message = ${formatStruct(message)}`,
            );
            assert(id !== undefined);
            assert(message !== undefined);
            // A Rust enum variant with no data (e.g. `RequestClose`) is
            // serialized as a bare string rather than a single-key object; a
            // variant with data is serialized as a single-key object. See
            // `KeysOfRustEnum` for the same convention used by its type.
            let key: KeysOfRustEnum<EditorMessageContents>;
            let value: unknown;
            if (typeof message === "string") {
                key = message as KeysOfRustEnum<EditorMessageContents>;
                value = undefined;
            } else {
                const keys = Object.keys(message);
                assert(keys.length === 1);
                key = keys[0] as KeysOfRustEnum<EditorMessageContents>;
                value = Object.values(message)[0];
            }

            // Process this message.
            switch (key) {
                case "Update": {
                    // Load this data in.
                    const currentUpdate = value as UpdateMessageContents;
                    // The rest of this should run after all other messages have
                    // been processed.
                    this.promise = this.promise.finally(async () => {
                        // Check or update the `currentFilename`.
                        if (this.currentFilename === undefined) {
                            this.currentFilename = currentUpdate.file_path;
                        } else if (
                            currentUpdate.file_path !== this.currentFilename
                        ) {
                            const msg = `Ignoring update for ${currentUpdate.file_path} because it's not the current file ${this.currentFilename}.`;
                            reportError(msg);
                            this.sendResult(id, {
                                IgnoredUpdate: [
                                    currentUpdate.file_path,
                                    this.currentFilename,
                                ],
                            });
                            return;
                        }
                        const contents = currentUpdate.contents;
                        const cursorPosition = currentUpdate.cursor_position;
                        if (contents !== undefined) {
                            // Check and update the version. If this is a diff,
                            // ensure the diff was made against the version of
                            // the file we have.
                            if ("Diff" in contents.source) {
                                if (
                                    contents.source.Diff.version !==
                                    this.version
                                ) {
                                    if (currentUpdate.is_re_translation) {
                                        consoleLog(
                                            `Ignoring out-of-sync re-translation update.`,
                                        );
                                    } else {
                                        reportError(
                                            `Out of sync: Client version ${this.version} !== incoming version ${contents.source.Diff.version}.`,
                                        );
                                        this.sendResult(id, {
                                            OutOfSync: [
                                                this.version,
                                                contents.source.Diff.version,
                                            ],
                                        });
                                    }
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
                            if (!this.isLoading) {
                                // Wait until after the DOM is ready, since we
                                // rely on content set in
                                // `onDomContentLoaded` in the Client.
                                await setContent(
                                    contents,
                                    currentUpdate.is_re_translation,
                                    currentUpdate.cursor_position,
                                    currentUpdate.scroll_position,
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
                                        (rootIframe!.onload = async () => {
                                            this.isLoading = false;
                                            await setContent(
                                                contents,
                                                currentUpdate.is_re_translation,
                                                currentUpdate.cursor_position,
                                                currentUpdate.scroll_position,
                                            );
                                            resolve();
                                        }),
                                );
                            }
                        } else {
                            // We might receive a message while the Client is
                            // reloading; during this period, `scrollToLine`
                            // isn't defined.
                            rootIframe!.contentWindow?.CodeChatEditor?.scrollToLine?.(
                                cursorPosition,
                                currentUpdate.scroll_position,
                            );
                        }

                        this.sendResult(id);
                    });
                    break;
                }

                case "CurrentFile": {
                    // Note that we can ignore `value[1]` (if the file is text
                    // or binary); the server only sends text files here.
                    const currentFile = (value as [string, boolean | null])[0];
                    const testSuffix = testMode
                        ? // Append the test parameter correctly, depending if
                          // there are already parameters or not.
                          currentFile.indexOf("?") === -1
                            ? "?test"
                            : "&test"
                        : "";
                    // Execute this after all other messages have been
                    // processed.
                    this.promise = this.promise.finally(async () => {
                        // If the page is still loading, then don't save.
                        // Otherwise, save the editor contents if necessary.
                        const cce = getClient();
                        await cce?.sendUpdate(true);
                        // Now, it's safe to load a new file. Tell the client to
                        // allow this navigation -- the document it contains has
                        // already been saved.
                        if (cce !== undefined) {
                            cce.allow_navigation = true;
                        }
                        this.setRootIframeSrc(currentFile + testSuffix);
                        // The `currentFile` is a URL-encoded path, not a
                        // filesystem path. So, we can't use it for
                        // `currentFilename`. Instead, signal that the
                        // `currentFilename` should be set on the next `Update`
                        // message.
                        this.currentFilename = undefined;
                        this.sendResult(id);
                    });
                    break;
                }

                case "Result": {
                    // If the result has the magic ID, then call a debug
                    // routine.
                    if (id === 1e6 && DEBUG_ENABLED) {
                        rootIframe!.contentWindow?.CodeChatEditor?.doDebug();
                        break;
                    }
                    // Cancel the timer for this message and remove it from
                    // `pendingMessages`.
                    const pendingMessage = this.pendingMessages[id];
                    if (pendingMessage !== undefined) {
                        const { timerId, callback } = pendingMessage;
                        clearTimeout(timerId);
                        callback();
                        delete this.pendingMessages[id];
                    }

                    // Report if this was an error.
                    const resultContents = value as MessageResult;
                    if ("Err" in resultContents) {
                        reportError(
                            `Error in message ${id}: ${JSON.stringify(resultContents.Err)}.`,
                            resultContents.Err,
                        );
                    }
                    break;
                }

                default: {
                    const msg = `Received unhandled message ${key}(${formatStruct(
                        value,
                    )})`;
                    reportError(msg);
                    this.sendResult(id, {
                        ClientIllegalMessageReceived: `${key}(${formatStruct(
                            value,
                        )})`,
                    });
                    break;
                }
            }
        };
    }

    send = (data: string | BufferSource | Blob) => this.ws.send(data);
    /*eslint-disable-next-line @typescript-eslint/no-explicit-any */
    close = (...args: any) => this.ws.close(...args);

    setRootIframeSrc = (url: string) => {
        // Set the new src to (re)load content. At startup, the `srcdoc`
        // attribute shows some welcome text. Remove it so that we can now
        // assign the `src` attribute.
        rootIframe!.removeAttribute("srcdoc");
        rootIframe!.src = url;
        // Track the `isLoading` status.
        this.isLoading = true;
        rootIframe!.onload = () => (this.isLoading = false);
    };

    // Report an error from the server.
    reportServerTimeout = (messageId: number) => {
        delete this.pendingMessages[messageId];
        reportError(`Error: server timeout for message id ${messageId}`);
    };

    // Send a message expecting a result to the server.
    sendMessage = (
        message: EditorMessageContents,
        callback: () => void = () => 0,
    ) => {
        const id = this.wsId;
        // The Client gets every third ID -- the IDE gets another third, while
        // the Server gets the final third.
        this.wsId += 3;
        // Add in the current filename to the message, if it's an `Update`.
        if (typeof message == "object" && "Update" in message) {
            assert(this.currentFilename !== undefined);
            message.Update.file_path = this.currentFilename!;
            // Update the version of this file if it's provided.
            this.version = message.Update.contents?.version ?? this.version;
        }
        consoleLog(
            `CodeChat Editor Client: sent message ${id}, ${formatStruct(message)}`,
        );
        const jm: EditorMessage = {
            id: id,
            message: message,
        };
        this.ws.send(JSON.stringify(jm));
        this.pendingMessages[id] = {
            timerId: window.setTimeout(
                this.reportServerTimeout,
                RESPONSE_TIMEOUT_MS,
                id,
            ),
            callback,
        };
    };

    // This is called by the Client when the user navigates to another webpage.
    currentFile = (url: URL) => {
        // TODO: should we delay execution of user navigation until all previous
        // actions have finished, or ignore them and immediately perform the
        // user navigation?
        this.promise = this.promise.finally(() => {
            if (url.host === window.location.host) {
                // If this points to the Server, then tell the IDE to load a new
                // file.
                this.sendMessage(
                    { CurrentFile: [url.toString(), null] },
                    () => {
                        this.setRootIframeSrc(url.toString());
                    },
                );
            } else {
                // Otherwise, navigate to the provided page.
                this.setRootIframeSrc(url.toString());
            }
            // Read the `currentFilename` from the next `Update` message.
            this.currentFilename = undefined;
        });
    };

    // Send a result (a response to a message from the server) back to the
    // server.
    sendResult = (id: number, result?: ResultErrTypes) => {
        const message: EditorMessageContents = {
            Result: result === undefined ? { Ok: "Void" } : { Err: result },
        };
        consoleLog(
            `CodeChat Editor Client: sending result id = ${id}, message = ${formatStruct(message)}`,
        );
        // We can't simply call `sendMessage` because that function expects a
        // result message back from the server.
        const jm: EditorMessage = {
            id,
            message,
        };
        this.ws.send(JSON.stringify(jm));
    };
}

// Return the `CodeChatEditor` object if the `rootIframe` contains the Client;
// otherwise, this is `undefined`.
const getClient = () => rootIframe?.contentWindow?.CodeChatEditor;

// Assign content to either the Client (if it's loaded) or the webpage (if not)
// in the `rootIframe`.
const setContent = async (
    contents: CodeChatForWeb,
    isReTranslation: boolean,
    cursorPosition?: CursorPosition,
    scrollLine?: number,
) => {
    const client = getClient();
    if (client === undefined) {
        // See if this is the [simple viewer](#Client-simple-viewer). Otherwise,
        // it's just the bare document to replace.
        const contentsElement =
            rootIframe!.contentDocument?.getElementById("CodeChat-contents");
        const cw =
            (contentsElement instanceof HTMLIFrameElement
                ? contentsElement.contentWindow
                : undefined) ?? rootIframe!.contentWindow!;
        cw.document.open();
        assert("Plain" in contents.source);
        cw.document.write(contents.source.Plain.doc);
        cw.document.close();
    } else {
        await rootIframe!.contentWindow!.CodeChatEditor.openLp(
            contents,
            isReTranslation,
            cursorPosition,
            scrollLine,
        );
    }
};

// The iframe element which composes this page.
let rootIframe: HTMLIFrameElement | undefined;

// True when in test mode.
let testMode = false;

// Load the dynamic content into the static page.
export const pageInit = (
    // The pathname for the websocket to use. The remainder of the URL is
    // derived from the hosting page's URL. See the
    // [Location docs](https://developer.mozilla.org/en-US/docs/Web/API/Location)
    // for a nice, interactive definition of the components of a URL.
    wsPathname: string,
    // Test mode flag
    testMode_: boolean,
) => {
    testMode = testMode_;
    onDomContentLoaded(() => {
        // Provide basic error reporting for uncaught errors.
        window.addEventListener("unhandledrejection", onError);
        window.addEventListener("error", onError);

        // If the hosting page uses HTTPS, then use a secure websocket (WSS
        // protocol); otherwise, use an insecure websocket (WS).
        const protocol = window.location.protocol === "http:" ? "ws:" : "wss:";
        // Build a websocket address based on the URL of the current page.
        webSocketComm = new WebSocketComm(
            `${protocol}//${window.location.host}/${wsPathname}`,
        );
        const iframeElement = document.getElementById("CodeChat-iframe");
        assert(iframeElement instanceof HTMLIFrameElement);
        rootIframe = iframeElement;
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

const showToast = (text: string) => {
    if (getClient() === undefined) {
        showToastCore(text);
    } else {
        rootIframe!.contentWindow!.CodeChatEditor.showToast(text);
    }
};

// Format a complex data structure as a string when in debug mode.
/*eslint-disable-next-line @typescript-eslint/no-explicit-any */
export const formatStruct = (complexDataStructure: any): string =>
    DEBUG_ENABLED
        ? JSON.stringify(complexDataStructure).substring(0, MAX_MESSAGE_LENGTH)
        : "";

/*eslint-disable-next-line @typescript-eslint/no-explicit-any */
const reportError = (text: string, ...objs: any) => {
    console.error(text);
    if (objs.length > 0) {
        console.log(...objs);
    }
    showToast(text);
};
