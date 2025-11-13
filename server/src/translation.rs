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
/// `translation.rs` -- translate messages between the IDE and the Client
/// =====================================================================
///
/// The IDE extension client (IDE for short) and the CodeChat Editor Client (or
/// Editor for short) exchange messages with each other, mediated by the
/// CodeChat Server. The Server forwards messages from one client to the other,
/// translating as necessary (for example, between source code and the Editor
/// format). This module implements the protocol for this forwarding and
/// translation logic; the actuation translation algorithms are implemented in
/// the processing module.
///
/// Overview
/// --------
///
/// ### Architecture
///
/// It uses a set of queues to decouple websocket protocol activity from the
/// core processing needed to translate source code between a CodeChat Editor
/// Client and an IDE client. The following diagram illustrates this approach:
///
/// <graphviz-graph>digraph {
/// ccc -&gt; client_task [ label = "websocket" dir = "both" ]
/// ccc -&gt; http_task [ label = "HTTP\nrequest/response" dir = "both"]
/// client_task -&gt; from_client
/// http_task -&gt; http_to_client
/// http_to_client -&gt; processing
/// processing -&gt; http_from_client
/// http_from_client -&gt; http_task
/// from_client -&gt; processing
/// processing -&gt; to_client
/// to_client -&gt; client_task
/// ide -&gt; ide_task [ dir = "both" ]
/// ide_task -&gt; from_ide
/// from_ide -&gt; processing
/// processing -&gt; to_ide
/// to_ide -&gt; ide_task
/// { rank = same; client_task; http_task }
/// { rank = same; to_client; from_client; http_from_client; http_to_client }
/// { rank = same; to_ide; from_ide }
/// { rank = max; ide }
/// ccc [ label = "CodeChat Editor\nClient"]
/// client_task [ label = "Client websocket\ntask"]
/// http_task [ label = "HTTP endpoint"]
/// from_client [ label = "queue from client" shape="rectangle"]
/// processing [ label = "Processing task" ]
/// to_client [ label = "queue to client" shape="rectangle"]
/// http_to_client [ label = "http queue to client" shape = "rectangle"]
/// http_from_client [ label = "oneshot from client" shape = "box"]
/// ide [ label = "CodeChat Editor\nIDE plugin"]
/// ide_task [ label = "IDE task" ]
/// from_ide [ label = "queue from IDE" shape="rectangle" ]
/// to_ide [ label = "queue to IDE" shape="rectangle" ]
/// }</graphviz-graph>
///
/// The queues use multiple-sender, single receiver (mpsc) types. The exception
/// to this pattern is the HTTP endpoint. This endpoint is invoked with each
/// HTTP request, rather than operating as a single, long-running task. It sends
/// the request to the processing task using an mpsc queue; this request
/// includes a one-shot channel which enables the request to return a response
/// to this specific request instance. The endpoint then returns the provided
/// response.
///
/// ### Protocol
///
/// The following diagrams formally define the forwarding and translation
/// protocol which this module implements.
///
/// *   The startup phase loads the Client framework into a browser:
///
///     <wc-mermaid>
///     sequenceDiagram
///     participant IDE
///     participant Server
///     participant Client
///     note over IDE, Client: Startup
///     IDE -&gt;&gt; Server: Opened(IdeType)
///     Server -&gt;&gt; IDE: Result(String: OK)
///     Server -&gt;&gt; IDE: ClientHtml(String: HTML or URL)
///     IDE -&gt;&gt; Server: Result(String: OK)
///     note over IDE, Client: Open browser (Client framework HTML or URL)
///     loop
///     Client -&gt; Server: HTTP request(/static URL)
///     Server -&gt; Client: HTTP response(/static data)
///     end
///     </wc-mermaid>
///
/// *   If the current file in the IDE changes (including the initial startup,
///     when the change is from no file to the current file), or a link is
///     followed in the Client's iframe:
///
///     <wc-mermaid>
///     sequenceDiagram
///     participant IDE
///     participant Server
///     participant Client
///     alt IDE loads file
///     IDE -&gt;&gt; Client: CurrentFile(String: Path of main.py)
///     opt If Client document is dirty
///     Client -&gt;&gt; IDE: Update(String: contents of main.py)
///     IDE -&gt;&gt; Client: Response(OK)
///     end
///     Client -&gt;&gt; IDE: Response(OK)
///     else Client loads file
///     Client -&gt;&gt; IDE: CurrentFile(String: URL of main.py)
///     IDE -&gt;&gt; Client: Response(OK)
///     end
///     Client -&gt;&gt; Server: HTTP request(URL of main.py)
///     Server -&gt;&gt; IDE: LoadFile(String: path to main.py)
///     IDE -&gt;&gt; Server: Response(LoadFile(String: file contents of main.py))
///     alt main.py is editable
///     Server -&gt;&gt; Client: HTTP response(contents of Client)
///     Server -&gt;&gt; Client: Update(String: contents of main.py)
///     Client -&gt;&gt; Server: Response(OK)
///     loop
///     Client -&gt;&gt; Server: HTTP request(URL of supporting file in main.py)
///     Server -&gt;&gt; IDE: LoadFile(String: path of supporting file)
///     alt Supporting file in IDE
///     IDE -&gt;&gt; Server: Response(LoadFile(contents of supporting file)
///     Server -&gt;&gt; Client: HTTP response(contents of supporting file)
///     else Supporting file not in IDE
///     IDE -&gt;&gt; Server: Response(LoadFile(None))
///     Server -&gt;&gt; Client: HTTP response(contents of supporting file from /// filesystem)
///     end
///     end
///     else main.py not editable and not a project
///     Server -&gt;&gt; Client: HTTP response(contents of main.py)
///     else main.py not editable and is a project
///     Server -&gt;&gt; Client: HTTP response(contents of Client Simple Viewer)
///     Client -&gt;&gt; Server: HTTP request (URL?raw of main.py)
///     Server -&gt;&gt; Client: HTTP response(contents of main.py)
///     end
///     </wc-mermaid>
///
/// *   If the current file's contents in the IDE are edited:
///
///     <wc-mermaid>
///     sequenceDiagram
///     participant IDE
///     participant Server
///     participant Client
///     IDE -&gt;&gt; Server: Update(String: new text contents)
///     alt Main file is editable
///     Server -&gt;&gt; Client: Update(String: new Client contents)
///     else Main file is not editable
///     Server -&gt;&gt; Client: Update(String: new text contents)
///     end
///     Client -&gt;&gt; IDE: Response(String: OK)<br>
///     </wc-mermaid>
///
/// *   If the current file's contents in the Client are edited, the Client
///     sends the IDE an `Update` with the revised contents.
///
/// *   When the PC goes to sleep then wakes up, the IDE client and the Editor
///     client both reconnect to the websocket URL containing their assigned ID.
///
/// *   If the Editor client or the IDE client are closed, they close their
///     websocket, which sends a `Close` message to the other websocket, causes
///     it to also close and ending the session.
///
/// *   If the server is stopped (or crashes), both clients shut down after
///     several reconnect retries.
///
/// ### Editor-overlay filesystem
///
/// When the Client displays a file provided by the IDE, that file may not exist
/// in the filesystem (a newly-created document), the IDE's content may be newer
/// than the filesystem content (an unsaved file), or the file may exist only in
/// the filesystem (for examples, images referenced by a file). The Client loads
/// files by sending HTTP requests to the Server with a URL which includes the
/// path to the desired file. Therefore, the Server must first ask the IDE if it
/// has the requested file; if so, it must deliver the IDE's file contents; if
/// not, it must load thee requested file from the filesystem. This process --
/// fetching from the IDE if possible, then falling back to the filesystem --
/// defines the editor-overlay filesystem.
///
/// #### Message IDs
///
/// The message system connects the IDE, Server, and Client; all three can serve
/// as the source or destination for a message. Any message sent should produce
/// a Response message in return. Therefore, we need globally unique IDs for
/// each message. To achieve this, the Server uses IDs that are multiples of 3
/// (0, 3, 6, ...), the Client multiples of 3 + 1 (1, 4, 7, ...) and the IDE
/// multiples of 3 + 2 (2, 5, 8, ...). A double-precision floating point number
/// (the standard [numeric
/// type](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Data_structures#number_type)
/// in JavaScript) has a 53-bit mantissa, meaning IDs won't wrap around for a
/// very long time.
// Imports
// -------
//
// ### Standard library
use std::{collections::HashMap, ffi::OsStr, fmt::Debug, path::PathBuf};

// ### Third-party
use lazy_static::lazy_static;
use log::{debug, error, warn};
use regex::Regex;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::{fs::File, select, sync::mpsc};

// ### Local
use crate::webserver::{
    EditorMessage, EditorMessageContents, WebAppState, WebsocketQueues, send_response,
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
        SimpleHttpResponse, SimpleHttpResponseError, SyncState, UpdateMessageContents,
        file_to_response, path_to_url, try_canonicalize, try_read_as_text, url_to_path,
    },
};

// Globals
// -------
//
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

#[derive(Debug, thiserror::Error)]
pub enum CreateTranslationQueuesError {
    #[error("Connection ID {0} already in use.")]
    IdInUse(String),
    #[error("IDE queue {0} already in use.")]
    IdeInUse(String),
}

pub struct CreatedTranslationQueues {
    pub from_ide_rx: Receiver<EditorMessage>,
    pub to_ide_tx: Sender<EditorMessage>,
    pub from_client_rx: Receiver<EditorMessage>,
    pub to_client_tx: Sender<EditorMessage>,
}

pub fn create_translation_queues(
    connection_id: String,
    app_state: &WebAppState,
) -> Result<CreatedTranslationQueues, CreateTranslationQueuesError> {
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
        .connection_id
        .lock()
        .unwrap()
        .contains(&connection_id)
    {
        return Err(CreateTranslationQueuesError::IdInUse(connection_id));
    }

    // Now case 2.
    if app_state
        .ide_queues
        .lock()
        .unwrap()
        .contains_key(&connection_id)
    {
        return Err(CreateTranslationQueuesError::IdeInUse(connection_id));
    }

    // Then this is case 1. Add the connection ID to the list of active
    // connections.
    let (from_ide_tx, from_ide_rx) = mpsc::channel(10);
    let (to_ide_tx, to_ide_rx) = mpsc::channel(10);
    assert!(
        app_state
            .ide_queues
            .lock()
            .unwrap()
            .insert(
                connection_id.clone(),
                WebsocketQueues {
                    from_websocket_tx: from_ide_tx,
                    to_websocket_rx: to_ide_rx,
                },
            )
            .is_none()
    );
    let (from_client_tx, from_client_rx) = mpsc::channel(10);
    let (to_client_tx, to_client_rx) = mpsc::channel(10);
    assert!(
        app_state
            .client_queues
            .lock()
            .unwrap()
            .insert(
                connection_id.clone(),
                WebsocketQueues {
                    from_websocket_tx: from_client_tx,
                    to_websocket_rx: to_client_rx,
                },
            )
            .is_none()
    );
    assert!(
        app_state
            .connection_id
            .lock()
            .unwrap()
            .insert(connection_id.clone())
    );

    Ok(CreatedTranslationQueues {
        from_ide_rx,
        to_ide_tx,
        from_client_rx,
        to_client_tx,
    })
}

// This is the processing task for the Visual Studio Code IDE. It handles all
// the core logic to moving data between the IDE and the client.
#[allow(clippy::too_many_arguments)]
pub async fn translation_task(
    connection_id_prefix: String,
    connection_id_raw: String,
    prefix: &'static [&'static str],
    app_state_task: WebAppState,
    shutdown_only: bool,
    allow_source_diffs: bool,
    to_ide_tx: Sender<EditorMessage>,
    mut from_ide_rx: Receiver<EditorMessage>,
    to_client_tx: Sender<EditorMessage>,
    mut from_client_rx: Receiver<EditorMessage>,
) {
    // Start the processing task.
    let connection_id = format!("{connection_id_prefix}{connection_id_raw}");
    if !shutdown_only {
        // Use a [labeled block
        // expression](https://doc.rust-lang.org/reference/expressions/loop-expr.html#labelled-block-expressions)
        // to provide a way to exit the current task.
        'task: {
            let mut current_file = PathBuf::new();
            let mut load_file_requests: HashMap<u64, ProcessingTaskHttpRequest> = HashMap::new();
            debug!("VSCode processing task started.");

            // Create a queue for HTTP requests fo communicate with this task.
            let (from_http_tx, mut from_http_rx) = mpsc::channel(10);
            app_state_task
                .processing_task_queue_tx
                .lock()
                .unwrap()
                .insert(connection_id.to_string(), from_http_tx);

            // Leave space for a server message during the init phase.
            let mut id: f64 = INITIAL_MESSAGE_ID + MESSAGE_ID_INCREMENT;
            // The source code, provided by the IDE. It will use whatever the
            // IDE provides for EOLs, which is stored in `eol` below.
            let mut source_code = String::new();
            let mut code_mirror_doc = String::new();
            // The initial state will be overwritten by the first `Update` or
            // `LoadFile`, so this value doesn't matter.
            let mut eol = EolType::Lf;
            // Some means this contains valid HTML; None means don't use it
            // (since it would have contained Markdown).
            let mut code_mirror_doc_blocks = Some(Vec::new());
            let prefix_str = "/".to_string() + &prefix.join("/");
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
            //
            // Another approach: use revision numbers. Both the IDE and Client
            // start with the same revision number. When either makes an edit,
            // it sends a new revision number along with a diff. If the receiver
            // doesn't have the previous version, it returns a result of error,
            // which prompts the sender to re-send with the full text instead of
            // a diff.
            let mut sync_state = SyncState::OutOfSync;
            loop {
                select! {
                    // Look for messages from the IDE.
                    Some(ide_message) = from_ide_rx.recv() => {
                        debug!("Received IDE message id = {}, message = {}", ide_message.id, debug_shorten(&ide_message.message));
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
                                let ((simple_http_response, option_update), file_contents) = match file_contents_option {
                                    Some(file_contents) => {
                                        // If there are Windows newlines, replace
                                        // with Unix; this is reversed when the
                                        // file is sent back to the IDE.
                                        (file_to_response(&http_request, &current_file, Some(&file_contents), use_pdf_js).await, file_contents)
                                    },
                                    None => {
                                        // The file wasn't available in the IDE.
                                        // Look for it in the filesystem.
                                        match File::open(&http_request.file_path).await {
                                            Err(err) => (
                                                (
                                                    SimpleHttpResponse::Err(SimpleHttpResponseError::Io(err)),
                                                    None,
                                                ),
                                                // There's no file, so return empty
                                                // contents, which will be ignored.
                                                "".to_string()
                                            ),
                                            Ok(mut fc) => {
                                                let option_file_contents = try_read_as_text(&mut fc).await;
                                                (
                                                    file_to_response(
                                                        &http_request,
                                                        &current_file,
                                                        option_file_contents.as_ref(),
                                                        use_pdf_js,
                                                    )
                                                    .await,
                                                    // If the file is binary, return empty
                                                    // contents, which will be ignored.
                                                    option_file_contents.unwrap_or("".to_string())
                                                )
                                            }
                                        }
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
                                    source_code = file_contents;
                                    eol = find_eol_type(&source_code);
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
                                            None => {
                                                queue_send!(to_client_tx.send(EditorMessage {
                                                    id: ide_message.id,
                                                    message: EditorMessageContents::Update(UpdateMessageContents {
                                                        file_path: clean_file_path.to_str().expect("Since the path started as a string, assume it losslessly translates back to a string.").to_string(),
                                                        contents: None,
                                                        cursor_position: update.cursor_position,
                                                        scroll_position: update.scroll_position,
                                                    }),
                                                }));
                                                Ok(ResultOkTypes::Void)
                                            }

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
                                                                        cursor_position: update.cursor_position,
                                                                        scroll_position: update.scroll_position,
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
                                                                        cursor_position: update.cursor_position,
                                                                        scroll_position: update.scroll_position,
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
                                                path_to_url(&prefix_str, Some(&connection_id_raw), &clean_file_path), Some(true)
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
                        debug!("Received Client message id = {}, message = {}", client_message.id, debug_shorten(&client_message.message));
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
                                                Ok(new_source_code) => {
                                                    // Correct EOL endings for use with the
                                                    // IDE.
                                                    let new_source_code_eol = eol_convert(new_source_code, &eol);
                                                    let ccfw = if sync_state == SyncState::InSync && allow_source_diffs {
                                                        Some(CodeChatForWeb {
                                                            metadata: cfw.metadata,
                                                            source: CodeMirrorDiffable::Diff(CodeMirrorDiff {
                                                                // Diff with correct EOLs, so that (for
                                                                // CRLF files as well as LF files) offsets
                                                                // are correct.
                                                                doc: diff_str(&source_code, &new_source_code_eol),
                                                                doc_blocks: vec![],
                                                            }),
                                                        })
                                                    } else {
                                                        Some(CodeChatForWeb {
                                                            metadata: cfw.metadata,
                                                            source: CodeMirrorDiffable::Plain(CodeMirror {
                                                                // We must clone here, so that it can be
                                                                // placed in the TX queue.
                                                                doc: new_source_code_eol.clone(),
                                                                doc_blocks: vec![],
                                                            }),
                                                        })
                                                    };
                                                    source_code = new_source_code_eol;
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
                                let result = match url_to_path(&url_string, prefix) {
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
        }

        debug!("VSCode processing task shutting down.");
        if app_state_task
            .processing_task_queue_tx
            .lock()
            .unwrap()
            .remove(&connection_id)
            .is_none()
        {
            error!("Unable to remove connection ID {connection_id} from processing task queue.");
        }
        if app_state_task
            .client_queues
            .lock()
            .unwrap()
            .remove(&connection_id)
            .is_none()
        {
            error!("Unable to remove connection ID {connection_id} from client queues.");
        }
        if app_state_task
            .ide_queues
            .lock()
            .unwrap()
            .remove(&connection_id)
            .is_none()
        {
            error!("Unable to remove connection ID {connection_id} from IDE queues.");
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

// Provide a simple debug function that prints only the first
// `MAX_MESSAGE_LENGTH` characters of the provided value.
fn debug_shorten<T: Debug>(val: T) -> String {
    if cfg!(debug_assertions) {
        let msg = format!("{:?}", val);
        let max_index = msg
            .char_indices()
            .nth(MAX_MESSAGE_LENGTH)
            .unwrap_or((msg.len(), 'x'))
            .0;
        msg[..max_index].to_string()
    } else {
        "".to_string()
    }
}
