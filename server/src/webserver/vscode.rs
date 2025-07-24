/// Copyright (C) 2025 Bryan A. Jones.
///
/// This file is part of the CodeChat Editor. The CodeChat Editor is free
/// software: you can redistribute it and/or modify it under the terms of the
/// GNU General Public License as published by the Free Software Foundation,
/// either version 3 of the License, or (at your option) any later version.
///
/// The CodeChat Editor is distributed in the hope that it will be useful, but
/// WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY
/// or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for
/// more details.
///
/// You should have received a copy of the GNU General Public License along with
/// the CodeChat Editor. If not, see
/// [http://www.gnu.org/licenses](http://www.gnu.org/licenses).
///
/// `vscode.rs` -- Implement server-side functionality for the Visual Studio
/// Code IDE
/// ========================================================================
// Submodules
// ----------
#[cfg(test)]
pub mod tests;

// Imports
// -------
//
// ### Standard library
use std::{cmp::min, collections::HashMap, ffi::OsStr, path::PathBuf};

// ### Third-party
use actix_web::{
    HttpRequest, HttpResponse,
    error::{Error, ErrorBadRequest},
    get, web,
};
use indoc::formatdoc;
use lazy_static::lazy_static;
use log::{debug, error, warn};
use regex::Regex;
use tokio::{fs::File, select, sync::mpsc};

// ### Local
use super::{
    AppState, EditorMessage, EditorMessageContents, IdeType, WebsocketQueues, client_websocket,
    get_client_framework, send_response,
};
use crate::{
    oneshot_send,
    processing::{
        CodeChatForWeb, CodeMirror, CodeMirrorDiff, CodeMirrorDiffable, SourceFileMetadata,
        TranslationResultsString, codechat_for_web_to_source, diff_code_mirror_doc_blocks,
        diff_str, source_to_codechat_for_web_string,
    },
    queue_send,
    webserver::{
        INITIAL_MESSAGE_ID, MESSAGE_ID_INCREMENT, ProcessingTaskHttpRequest, ResultOkTypes,
        SyncState, UpdateMessageContents, escape_html, file_to_response, filesystem_endpoint,
        get_server_url, html_wrapper, make_simple_http_response, path_to_url, try_canonicalize,
        try_read_as_text, url_to_path,
    },
};

// Globals
// -------
const VSCODE_PATH_PREFIX: &[&str] = &["vsc", "fs"];
// The max length of a message to show in the console.
const MAX_MESSAGE_LENGTH: usize = 300;

lazy_static! {
        /// A regex to determine the type of the first EOL. See 'PROCESSINGS1.
    pub static ref EOL_FINDER: Regex = Regex::new("[^\r\n]*(\r?\n)").unwrap();

}

// Data structures
// ---------------
#[derive(Clone, Debug, PartialEq)]
pub enum EolType {
    Lf,
    Crlf,
}

// Code
// ----
pub fn find_eol_type(s: &str) -> EolType {
    match EOL_FINDER.captures(s) {
        // Assume a line type for strings with no newlines.
        None => {
            if cfg!(windows) {
                EolType::Crlf
            } else {
                EolType::Lf
            }
        }
        Some(captures) => match captures.get(1) {
            None => panic!("No capture group!"),
            Some(match_) => {
                if match_.as_str() == "\n" {
                    EolType::Lf
                } else {
                    EolType::Crlf
                }
            }
        },
    }
}

// This is the processing task for the Visual Studio Code IDE. It handles all
// the core logic to moving data between the IDE and the client.
#[get("/vsc/ws-ide/{connection_id}")]
pub async fn vscode_ide_websocket(
    connection_id: web::Path<String>,
    req: HttpRequest,
    body: web::Payload,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
    let connection_id_str = connection_id.to_string();

    // There are three cases for this `connection_id`:
    //
    // 1.  It hasn't been used before. In this case, create the appropriate
    //     queues and start websocket and processing tasks.
    // 2.  It's in use, but was disconnected. In this case, re-use the queues
    //     and start the websocket task; the processing task is still running.
    // 3.  It's in use by another IDE. This is an error, but I don't have a way
    //     to detect it yet.
    //
    // Check case 3.
    if app_state
        .vscode_connection_id
        .lock()
        .unwrap()
        .contains(&connection_id_str)
    {
        let msg = format!("Connection ID {connection_id_str} already in use.");
        error!("{msg}");
        return Err(ErrorBadRequest(msg));
    }

    // Now case 2.
    if app_state
        .vscode_ide_queues
        .lock()
        .unwrap()
        .contains_key(&connection_id_str)
    {
        return client_websocket(
            connection_id,
            req,
            body,
            app_state.vscode_ide_queues.clone(),
        )
        .await;
    }

    // Then this is case 1. Add the connection ID to the list of active
    // connections.
    let (from_ide_tx, mut from_ide_rx) = mpsc::channel(10);
    let (to_ide_tx, to_ide_rx) = mpsc::channel(10);
    assert!(
        app_state
            .vscode_ide_queues
            .lock()
            .unwrap()
            .insert(
                connection_id_str.clone(),
                WebsocketQueues {
                    from_websocket_tx: from_ide_tx,
                    to_websocket_rx: to_ide_rx,
                },
            )
            .is_none()
    );
    let (from_client_tx, mut from_client_rx) = mpsc::channel(10);
    let (to_client_tx, to_client_rx) = mpsc::channel(10);
    assert!(
        app_state
            .vscode_client_queues
            .lock()
            .unwrap()
            .insert(
                connection_id_str.clone(),
                WebsocketQueues {
                    from_websocket_tx: from_client_tx,
                    to_websocket_rx: to_client_rx,
                },
            )
            .is_none()
    );
    app_state
        .vscode_connection_id
        .lock()
        .unwrap()
        .insert(connection_id_str.clone());

    // Clone variables owned by the processing task.
    let connection_id_task = connection_id_str;
    let app_state_task = app_state.clone();

    // Start the processing task.
    actix_rt::spawn(async move {
        // Use a [labeled block
        // expression](https://doc.rust-lang.org/reference/expressions/loop-expr.html#labelled-block-expressions)
        // to provide a way to exit the current task.
        'task: {
            let mut current_file = PathBuf::new();
            let mut load_file_requests: HashMap<u64, ProcessingTaskHttpRequest> = HashMap::new();
            debug!("VSCode processing task started.");

            // Get the first message sent by the IDE.
            let Some(first_message): std::option::Option<EditorMessage> = from_ide_rx.recv().await
            else {
                error!("{}", "IDE websocket received no data.");
                break 'task;
            };

            // Make sure it's the `Opened` message.
            let EditorMessageContents::Opened(ide_type) = first_message.message else {
                let msg = format!("Unexpected message {first_message:?}");
                error!("{msg}");
                send_response(&to_ide_tx, first_message.id, Err(msg)).await;

                // Send a `Closed` message to shut down the websocket.
                queue_send!(to_ide_tx.send(EditorMessage { id: 0.0, message: EditorMessageContents::Closed}), 'task);
                break 'task;
            };
            debug!("Received IDE Opened message.");

            // Ensure the IDE type (VSCode) is correct.
            match ide_type {
                IdeType::VSCode(is_self_hosted) => {
                    // Get the address for the server.
                    let port = app_state_task.port;
                    let address = match get_server_url(port).await {
                        Ok(address) => address,
                        Err(err) => {
                            error!("{err:?}");
                            break 'task;
                        }
                    };
                    if is_self_hosted {
                        // Send a response (successful) to the `Opened` message.
                        debug!(
                            "Sending response = OK to IDE Opened message, id {}.",
                            first_message.id
                        );
                        send_response(&to_ide_tx, first_message.id, Ok(ResultOkTypes::Void)).await;

                        // Send the HTML for the internal browser.
                        let client_html = formatdoc!(
                            r#"
                            <!DOCTYPE html>
                            <html>
                                <head>
                                </head>
                                <body style="margin: 0px; padding: 0px; overflow: hidden">
                                    <iframe src="{address}/vsc/cf/{connection_id_task}" style="width: 100%; height: 100vh; border: none"></iframe>
                                </body>
                            </html>"#
                        );
                        debug!("Sending ClientHtml message to IDE: {client_html}");
                        queue_send!(to_ide_tx.send(EditorMessage {
                            id: 0.0,
                            message: EditorMessageContents::ClientHtml(client_html)
                        }), 'task);

                        // Wait for the response.
                        let Some(message) = from_ide_rx.recv().await else {
                            error!("{}", "IDE websocket received no data.");
                            break 'task;
                        };

                        // Make sure it's the `Result` message with no errors.
                        let res =
                            // First, make sure the ID matches.
                            if message.id != 0.0 {
                                Err(format!("Unexpected message ID {}.", message.id))
                            } else {
                                match message.message {
                                    EditorMessageContents::Result(message_result) => match message_result {
                                        Err(err) => Err(format!("Error in ClientHtml: {err}")),
                                        Ok(result_ok) =>
                                            if let ResultOkTypes::Void = result_ok {
                                                Ok(())
                                            } else {
                                                Err(format!(
                                                    "Unexpected message LoadFile contents {result_ok:?}."
                                                ))
                                            }
                                    },
                                    _ => Err(format!("Unexpected message {message:?}")),
                                }
                            };
                        if let Err(err) = res {
                            error!("{err}");
                            // Send a `Closed` message.
                            queue_send!(to_ide_tx.send(EditorMessage {
                                id: 1.0,
                                message: EditorMessageContents::Closed
                            }), 'task);
                            break 'task;
                        };
                    } else {
                        // Open the Client in an external browser.
                        if let Err(err) =
                            webbrowser::open(&format!("{address}/vsc/cf/{connection_id_task}"))
                        {
                            let msg = format!("Unable to open web browser: {err}");
                            error!("{msg}");
                            send_response(&to_ide_tx, first_message.id, Err(msg)).await;

                            // Send a `Closed` message.
                            queue_send!(to_ide_tx.send(EditorMessage{
                                id: 0.0,
                                message: EditorMessageContents::Closed
                            }), 'task);
                            break 'task;
                        }
                        // Send a response (successful) to the `Opened` message.
                        send_response(&to_ide_tx, first_message.id, Ok(ResultOkTypes::Void)).await;
                    }
                }
                _ => {
                    // This is the wrong IDE type. Report then error.
                    let msg = format!("Invalid IDE type: {ide_type:?}");
                    error!("{msg}");
                    send_response(&to_ide_tx, first_message.id, Err(msg)).await;

                    // Close the connection.
                    queue_send!(to_ide_tx.send(EditorMessage { id: 0.0, message: EditorMessageContents::Closed}), 'task);
                    break 'task;
                }
            }

            // Create a queue for HTTP requests fo communicate with this task.
            let (from_http_tx, mut from_http_rx) = mpsc::channel(10);
            app_state_task
                .processing_task_queue_tx
                .lock()
                .unwrap()
                .insert(connection_id_task.to_string(), from_http_tx);

            // All further messages are handled in the main loop.
            let mut id: f64 = INITIAL_MESSAGE_ID + MESSAGE_ID_INCREMENT;
            let mut source_code = String::new();
            let mut code_mirror_doc = String::new();
            // The initial state will be overwritten by the first `Update` or
            // `LoadFile`, so this value doesn't matter.
            let mut eol = EolType::Lf;
            // Some means this contains valid HTML; None means don't use it
            // (since it would have contained Markdown).
            let mut code_mirror_doc_blocks = Some(Vec::new());
            // To send a diff from Server to Client or vice versa, we need to
            // ensure they are in sync:
            //
            // 1.  IDE update -> Server -> Client or Client update -> Server ->
            //     IDE: the Server and Client sync is pending. Client response
            //     -> Server -> IDE or IDE response -> Server -> Client: the
            //     Server and Client are synced.
            // 2.  IDE current file -> Server -> Client or Client current file
            //     -> Server -> IDE: Out of sync.
            //
            // It's only safe to send a diff when the most recent sync is
            // achieved. So, we need to track the ID of the most recent IDE ->
            // Client update or Client -> IDE update, if one is in flight. When
            // complete, mark the connection as synchronized. Since all IDs are
            // unique, we can use a single variable to store the ID.
            //
            // Currently, when the Client sends an update, mark the connection
            // as out of sync, since the update contains not HTML in the doc
            // blocks, but Markdown. When Turndown is moved from JavaScript to
            // Rust, this can be changed, since both sides will have HTML in the
            // doc blocks.
            let mut sync_state = SyncState::OutOfSync;
            loop {
                select! {
                    // Look for messages from the IDE.
                    Some(ide_message) = from_ide_rx.recv() => {
                        let msg = format!("{:?}", ide_message.message);
                        debug!("Received IDE message id = {}, message = {}", ide_message.id, &msg[..min(MAX_MESSAGE_LENGTH, msg.len())]);
                        match ide_message.message {
                            // Handle messages that the IDE must not send.
                            EditorMessageContents::Opened(_) |
                            EditorMessageContents::OpenUrl(_) |
                            EditorMessageContents::LoadFile(_) |
                            EditorMessageContents::ClientHtml(_) => {
                                let msg = "IDE must not send this message.";
                                error!("{msg}");
                                send_response(&to_ide_tx, ide_message.id, Err(msg.to_string())).await;
                            },

                            // Handle messages that are simply passed through.
                            EditorMessageContents::Closed |
                            EditorMessageContents::RequestClose => {
                                debug!("Forwarding it to the Client.");
                                queue_send!(to_client_tx.send(ide_message))
                            },

                            // Pass a `Result` message to the Client, unless
                            // it's a `LoadFile` result.
                            EditorMessageContents::Result(ref result) => {
                                let is_loadfile = match result {
                                    // See if this error was produced by a
                                    // `LoadFile` result.
                                    Err(_) => load_file_requests.contains_key(&ide_message.id.to_bits()),
                                    Ok(result_ok) => match result_ok {
                                        ResultOkTypes::Void => false,
                                        ResultOkTypes::LoadFile(_) => true,
                                    }
                                };
                                // Pass the message to the client if this isn't
                                // a `LoadFile` result (the only type of result
                                // which the Server should handle).
                                if !is_loadfile {
                                    debug!("Forwarding it to the Client.");
                                    // If this was confirmation from the IDE
                                    // that it received the latest update, then
                                    // mark the IDE as synced.
                                    if sync_state == SyncState::Pending(ide_message.id) {
                                        sync_state = SyncState::InSync;
                                    }
                                    queue_send!(to_client_tx.send(ide_message));
                                    continue;
                                }
                                // Ensure there's an HTTP request for this
                                // `LoadFile` result.
                                let Some(http_request) = load_file_requests.remove(&ide_message.id.to_bits()) else {
                                    error!("Error: no HTTP request found for LoadFile result ID {}.", ide_message.id);
                                    break 'task;
                                };

                                // Take ownership of the result after sending it
                                // above (which requires ownership).
                                let EditorMessageContents::Result(result) = ide_message.message else {
                                    error!("{}", "Not a result.");
                                    break;
                                };
                                // Get the file contents from a `LoadFile`
                                // result; otherwise, this is None.
                                let file_contents_option = match result {
                                    Err(err) => {
                                        error!("{err}");
                                        None
                                    },
                                    Ok(result_ok) => match result_ok {
                                        ResultOkTypes::Void => panic!("LoadFile result should not be void."),
                                        ResultOkTypes::LoadFile(file_contents) => file_contents,
                                    }
                                };

                                // Process the file contents. Since VSCode
                                // doesn't have a PDF viewer, determine if this
                                // is a PDF file. (TODO: look at the magic
                                // number also -- "%PDF").
                                let use_pdf_js = http_request.file_path.extension() == Some(OsStr::new("pdf"));
                                let (simple_http_response, option_update) = match file_contents_option {
                                    Some(file_contents) => {
                                        // If there are Windows newlines, replace
                                        // with Unix; this is reversed when the
                                        // file is sent back to the IDE.
                                        eol = find_eol_type(&file_contents);
                                        let file_contents = file_contents.replace("\r\n", "\n");
                                        let ret = file_to_response(&http_request, &current_file, Some(&file_contents), use_pdf_js).await;
                                        source_code = file_contents;
                                        ret
                                    },
                                    None => {
                                        // The file wasn't available in the IDE.
                                        // Look for it in the filesystem.
                                        make_simple_http_response(&http_request, &current_file, use_pdf_js).await
                                    }
                                };
                                if let Some(update) = option_update {
                                    let Some(ref tmp) = update.contents else {
                                        error!("None.");
                                        break;
                                    };
                                    let CodeMirrorDiffable::Plain(ref plain) = tmp.source else {
                                        error!("Not plain!");
                                        break;
                                    };
                                    // We must clone here, since the original is
                                    // placed in the TX queue.
                                    code_mirror_doc = plain.doc.clone();
                                    code_mirror_doc_blocks = Some(plain.doc_blocks.clone());
                                    sync_state = SyncState::Pending(id);

                                    debug!("Sending Update to Client, id = {id}.");
                                    queue_send!(to_client_tx.send(EditorMessage {
                                        id,
                                        message: EditorMessageContents::Update(update)
                                    }));
                                    id += MESSAGE_ID_INCREMENT;
                                }
                                debug!("Sending HTTP response.");
                                oneshot_send!(http_request.response_queue.send(simple_http_response));
                            }

                            // Handle the `Update` message.
                            EditorMessageContents::Update(update) => {
                                // Normalize the provided file name.
                                let result = match try_canonicalize(&update.file_path) {
                                    Err(err) => Err(err),
                                    Ok(clean_file_path) => {
                                        match update.contents {
                                            None => Err("TODO: support for updates without contents.".to_string()),
                                            Some(contents) => {
                                                match contents.source {
                                                    CodeMirrorDiffable::Diff(_diff) => Err("TODO: support for updates with diffable sources.".to_string()),
                                                    CodeMirrorDiffable::Plain(code_mirror) => {
                                                        // If there are Windows newlines, replace
                                                        // with Unix; this is reversed when the
                                                        // file is sent back to the IDE.
                                                        eol = find_eol_type(&code_mirror.doc);
                                                        let doc_normalized_eols = code_mirror.doc.replace("\r\n", "\n");
                                                        // Translate the file.
                                                        let (translation_results_string, _path_to_toc) =
                                                        source_to_codechat_for_web_string(&doc_normalized_eols, &current_file, false);
                                                        match translation_results_string {
                                                            TranslationResultsString::CodeChat(ccfw) => {
                                                                // Send the new translated contents.
                                                                debug!("Sending translated contents to Client.");
                                                                let CodeMirrorDiffable::Plain(ref ccfw_source_plain) = ccfw.source else {
                                                                    error!("{}", "Unexpected diff value.");
                                                                    break;
                                                                };
                                                                // Send a diff if possible (only when the
                                                                // Client's contents are synced with the
                                                                // IDE).
                                                                let contents = Some(
                                                                    if let Some(cmdb) = code_mirror_doc_blocks &&
                                                                     sync_state == SyncState::InSync {
                                                                        let doc_diff = diff_str(&code_mirror_doc, &ccfw_source_plain.doc);
                                                                        let code_mirror_diff = diff_code_mirror_doc_blocks(&cmdb, &ccfw_source_plain.doc_blocks);
                                                                        CodeChatForWeb {
                                                                            // Clone needed here, so we can copy it
                                                                            // later.
                                                                            metadata: ccfw.metadata.clone(),
                                                                            source: CodeMirrorDiffable::Diff(CodeMirrorDiff {
                                                                                doc: doc_diff,
                                                                                doc_blocks: code_mirror_diff
                                                                            })
                                                                        }
                                                                    } else {
                                                                        // We must make a clone to put in the TX
                                                                        // queue; this allows us to keep the
                                                                        // original below to use with the next
                                                                        // diff.
                                                                        ccfw.clone()
                                                                    }
                                                                );
                                                                queue_send!(to_client_tx.send(EditorMessage {
                                                                    id: ide_message.id,
                                                                    message: EditorMessageContents::Update(UpdateMessageContents {
                                                                        file_path: clean_file_path.to_str().expect("Since the path started as a string, assume it losslessly translates back to a string.").to_string(),
                                                                        contents,
                                                                        cursor_position: None,
                                                                        scroll_position: None,
                                                                    }),
                                                                }));
                                                                // Update to the latest code after
                                                                // computing diffs. To avoid ownership
                                                                // problems, re-define `ccfw_source_plain`.
                                                                let CodeMirrorDiffable::Plain(ccfw_source_plain) = ccfw.source else {
                                                                    error!("{}", "Unexpected diff value.");
                                                                    break;
                                                                };
                                                                source_code = code_mirror.doc;
                                                                code_mirror_doc = ccfw_source_plain.doc;
                                                                code_mirror_doc_blocks = Some(ccfw_source_plain.doc_blocks);
                                                                // Mark the Client as unsynced until this
                                                                // is acknowledged.
                                                                sync_state = SyncState::Pending(ide_message.id);
                                                                Ok(ResultOkTypes::Void)
                                                            }
                                                            // TODO
                                                            TranslationResultsString::Binary => Err("TODO".to_string()),
                                                            TranslationResultsString::Err(err) => Err(format!("Error translating source to CodeChat: {err}").to_string()),
                                                            TranslationResultsString::Unknown => {
                                                                // Send the new raw contents.
                                                                debug!("Sending translated contents to Client.");
                                                                queue_send!(to_client_tx.send(EditorMessage {
                                                                    id: ide_message.id,
                                                                    message: EditorMessageContents::Update(UpdateMessageContents {
                                                                        file_path: clean_file_path.to_str().expect("Since the path started as a string, assume it losslessly translates back to a string.").to_string(),
                                                                        contents: Some(CodeChatForWeb {
                                                                            metadata: SourceFileMetadata {
                                                                                // Since this is raw data, `mode` doesn't
                                                                                // matter.
                                                                                mode: "".to_string(),
                                                                            },
                                                                            source: CodeMirrorDiffable::Plain(CodeMirror {
                                                                                doc: code_mirror.doc,
                                                                                doc_blocks: vec![]
                                                                            })
                                                                        }),
                                                                        cursor_position: None,
                                                                        scroll_position: None,
                                                                    }),
                                                                }));
                                                                Ok(ResultOkTypes::Void)
                                                            },
                                                            TranslationResultsString::Toc(_) => {
                                                                Err("Error: source incorrectly recognized as a TOC.".to_string())
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                };
                                // If there's an error, then report it;
                                // otherwise, the message is passed to the
                                // Client, which will provide the result.
                                if let Err(err) = &result {
                                    error!("{err}");
                                    send_response(&to_ide_tx, ide_message.id, result).await;
                                }
                            }

                            // Update the current file; translate it to a URL
                            // then pass it to the Client.
                            EditorMessageContents::CurrentFile(file_path, _is_text) => {
                                debug!("Translating and forwarding it to the Client.");
                                match try_canonicalize(&file_path) {
                                    Ok(clean_file_path) => {
                                        queue_send!(to_client_tx.send(EditorMessage {
                                            id: ide_message.id,
                                            message: EditorMessageContents::CurrentFile(
                                                path_to_url("/vsc/fs", Some(&connection_id_task), &clean_file_path), Some(true)
                                            )
                                        }));
                                        current_file = file_path.into();
                                        // Since this is a new file, mark it as
                                        // unsynced.
                                        sync_state = SyncState::OutOfSync;
                                    }
                                    Err(err) => {
                                        let msg = format!(
                                            "Unable to canonicalize file name {}: {err}", &file_path
                                        );
                                        error!("{msg}");
                                        send_response(&to_client_tx, ide_message.id, Err(msg)).await;
                                    }
                                }
                            }
                        }
                    },

                    // Handle HTTP requests.
                    Some(http_request) = from_http_rx.recv() => {
                        debug!("Received HTTP request for {:?} and sending LoadFile to IDE, id = {id}.", http_request.file_path);
                        // Convert the request into a `LoadFile` message.
                        queue_send!(to_ide_tx.send(EditorMessage {
                            id,
                            message: EditorMessageContents::LoadFile(http_request.file_path.clone())
                        }));
                        // Store the ID and request, which are needed to send a
                        // response when the `LoadFile` result is received.
                        load_file_requests.insert(id.to_bits(), http_request);
                        id += MESSAGE_ID_INCREMENT;
                    }

                    // Handle messages from the client.
                    Some(client_message) = from_client_rx.recv() => {
                        let msg = format!("{:?}", client_message.message);
                        debug!("Received Client message id = {}, message = {}", client_message.id, &msg[..min(MAX_MESSAGE_LENGTH, msg.len())]);
                        match client_message.message {
                            // Handle messages that the client must not send.
                            EditorMessageContents::Opened(_) |
                            EditorMessageContents::LoadFile(_) |
                            EditorMessageContents::RequestClose |
                            EditorMessageContents::ClientHtml(_) => {
                                let msg = "Client must not send this message.";
                                error!("{msg}");
                                send_response(&to_client_tx, client_message.id, Err(msg.to_string())).await;
                            },

                            // Handle messages that are simply passed through.
                            EditorMessageContents::Closed |
                            EditorMessageContents::Result(_) => {
                                debug!("Forwarding it to the IDE.");
                                // If this result confirms that the Client
                                // received the most recent IDE update, then
                                // mark the documents as synced.
                                if sync_state == SyncState::Pending(client_message.id) {
                                    sync_state = SyncState::InSync;
                                }
                                queue_send!(to_ide_tx.send(client_message))
                            },

                            // Open a web browser when requested.
                            EditorMessageContents::OpenUrl(url) => {
                                // This doesn't work in Codespaces. TODO: send
                                // this back to the VSCode window, then call
                                // `vscode.env.openExternal(vscode.Uri.parse(url))`.
                                if let Err(err) = webbrowser::open(&url) {
                                    let msg = format!("Unable to open web browser to URL {url}: {err}");
                                    error!("{msg}");
                                    send_response(&to_client_tx, client_message.id, Err(msg)).await;
                                } else {
                                    send_response(&to_client_tx, client_message.id, Ok(ResultOkTypes::Void)).await;
                                }
                            },

                            // Handle the `Update` message.
                            EditorMessageContents::Update(update_message_contents) => {
                                debug!("Forwarding translation of it to the IDE.");
                                match try_canonicalize(&update_message_contents.file_path) {
                                    Err(err) => {
                                        let msg = format!(
                                            "Unable to canonicalize file name {}: {err}", &update_message_contents.file_path
                                        );
                                        error!("{msg}");
                                        send_response(&to_client_tx, client_message.id, Err(msg)).await;
                                        continue;
                                    }
                                    Ok(clean_file_path) => {
                                        let codechat_for_web = match update_message_contents.contents {
                                            None => None,
                                            Some(cfw) => match codechat_for_web_to_source(
                                                &cfw)
                                            {
                                                Ok(result) => {
                                                    let ccfw = if sync_state == SyncState::InSync {
                                                        Some(CodeChatForWeb {
                                                            metadata: cfw.metadata,
                                                            source: CodeMirrorDiffable::Diff(CodeMirrorDiff {
                                                                // Diff with correct EOLs, so that (for
                                                                // CRLF files as well as LF files) offsets
                                                                // are correct.
                                                                doc: diff_str(&eol_convert(source_code, &eol), &eol_convert(result.clone(), &eol)),
                                                                doc_blocks: vec![],
                                                            }),
                                                        })
                                                    } else {
                                                        Some(CodeChatForWeb {
                                                            metadata: cfw.metadata,
                                                            source: CodeMirrorDiffable::Plain(CodeMirror {
                                                                // We must clone here, so that it can be
                                                                // placed in the TX queue.
                                                                doc: eol_convert(result.clone(), &eol),
                                                                doc_blocks: vec![],
                                                            }),
                                                        })
                                                    };
                                                    // Store the document with Unix-style EOLs
                                                    // (LFs).
                                                    source_code = result;
                                                    let CodeMirrorDiffable::Plain(cmd) = cfw.source else {
                                                        // TODO: support diffable!
                                                        error!("No diff!");
                                                        break;
                                                    };
                                                    code_mirror_doc = cmd.doc;
                                                    // TODO: instead of `cmd.doc_blocks`, use
                                                    // `None` to indicate that the doc blocks
                                                    // contain Markdown instead of HTML.
                                                    code_mirror_doc_blocks = None;
                                                    ccfw
                                                },
                                                Err(message) => {
                                                    let msg = format!(
                                                        "Unable to translate to source: {message}"
                                                    );
                                                    error!("{msg}");
                                                    send_response(&to_client_tx, client_message.id, Err(msg)).await;
                                                    continue;
                                                }
                                            },
                                        };
                                        queue_send!(to_ide_tx.send(EditorMessage {
                                            id: client_message.id,
                                            message: EditorMessageContents::Update(UpdateMessageContents {
                                                file_path: clean_file_path.to_str().expect("Since the path started as a string, assume it losslessly translates back to a string.").to_string(),
                                                contents: codechat_for_web,
                                                cursor_position: update_message_contents.cursor_position,
                                                scroll_position: update_message_contents.scroll_position,
                                            })
                                        }));
                                        // Mark the IDE contents as out of sync
                                        // until this message is received.
                                        sync_state = SyncState::Pending(client_message.id);
                                    }
                                }
                            },

                            // Update the current file; translate it to a URL
                            // then pass it to the IDE.
                            EditorMessageContents::CurrentFile(url_string, _is_text) => {
                                debug!("Forwarding translated path to IDE.");
                                let result = match url_to_path(&url_string, VSCODE_PATH_PREFIX) {
                                    Err(err) => Err(format!("Unable to convert URL to path: {err}")),
                                    Ok(file_path) => {
                                        match file_path.to_str() {
                                            None => Err("Unable to convert path to string.".to_string()),
                                            Some(file_path_string) => {
                                                // Use a [binary file
                                                // sniffer](#binary-file-sniffer) to
                                                // determine if the file is text or binary.
                                                let is_text = if let Ok(mut fc) = File::open(&file_path).await {
                                                    try_read_as_text(&mut fc).await.is_some()
                                                } else {
                                                    false
                                                };
                                                queue_send!(to_ide_tx.send(EditorMessage {
                                                    id: client_message.id,
                                                    message: EditorMessageContents::CurrentFile(file_path_string.to_string(), Some(is_text))
                                                }));
                                                current_file = file_path;
                                                // Mark the IDE as out of sync, since this
                                                // is a new file.
                                                sync_state = SyncState::OutOfSync;
                                                Ok(())
                                            }
                                        }
                                    }
                                };
                                if let Err(msg) = result {
                                    error!("{msg}");
                                    send_response(&to_client_tx, client_message.id, Err(msg)).await;
                                }
                            }
                        }
                    },

                    else => break
                }
            }

            debug!("VSCode processing task shutting down.");
            if app_state_task
                .processing_task_queue_tx
                .lock()
                .unwrap()
                .remove(&connection_id_task)
                .is_none()
            {
                error!(
                    "Unable to remove connection ID {connection_id_task} from processing task queue."
                );
            }
            if app_state_task
                .vscode_client_queues
                .lock()
                .unwrap()
                .remove(&connection_id_task)
                .is_none()
            {
                error!("Unable to remove connection ID {connection_id_task} from client queues.");
            }
            if app_state_task
                .vscode_ide_queues
                .lock()
                .unwrap()
                .remove(&connection_id_task)
                .is_none()
            {
                error!("Unable to remove connection ID {connection_id_task} from IDE queues.");
            }

            from_ide_rx.close();
            from_ide_rx.close();

            // Drain any remaining messages after closing the queue.
            while let Some(m) = from_ide_rx.recv().await {
                warn!("Dropped queued message {m:?}");
            }
            while let Some(m) = from_client_rx.recv().await {
                warn!("Dropped queued message {m:?}");
            }
            debug!("VSCode processing task exited.");
        }
    });

    // Move data between the IDE and the processing task via queues. The
    // websocket connection between the client and the IDE will run in the
    // endpoint for that connection.
    client_websocket(
        connection_id,
        req,
        body,
        app_state.vscode_ide_queues.clone(),
    )
    .await
}

pub fn get_vscode_client_framework(connection_id: &str) -> String {
    // Send the HTML for the internal browser.
    match get_client_framework(false, "vsc/ws-client", connection_id) {
        Ok(web_page) => web_page,
        Err(html_string) => {
            error!("{html_string}");
            html_wrapper(&escape_html(&html_string))
        }
    }
}

/// Serve the Client Framework.
#[get("/vsc/cf/{connection_id}")]
pub async fn vscode_client_framework(connection_id: web::Path<String>) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body(get_vscode_client_framework(&connection_id))
}

/// Define a websocket handler for the CodeChat Editor Client.
#[get("/vsc/ws-client/{connection_id}")]
pub async fn vscode_client_websocket(
    connection_id: web::Path<String>,
    req: HttpRequest,
    body: web::Payload,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
    client_websocket(
        connection_id,
        req,
        body,
        app_state.vscode_client_queues.clone(),
    )
    .await
}

// Respond to requests for the filesystem.
#[get("/vsc/fs/{connection_id}/{file_path:.*}")]
async fn serve_vscode_fs(
    request_path: web::Path<(String, String)>,
    req: HttpRequest,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    filesystem_endpoint(request_path, &req, &app_state).await
}

// If a string is encoded using CRLFs (Windows style), convert it to LFs only
// (Unix style).
fn eol_convert(s: String, eol_type: &EolType) -> String {
    if eol_type == &EolType::Crlf {
        s.replace("\n", "\r\n")
    } else {
        s
    }
}

