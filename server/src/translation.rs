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
/// ============================================================================
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
/// ----------------------------------------------------------------------------
///
/// ### Architecture
///
/// It uses a set of queues to decouple websocket protocol activity from the
/// core processing needed to translate source code between a CodeChat Editor
/// Client and an IDE client. The following diagram illustrates this approach:
///
/// ```graphviz
/// digraph {
/// ccc -> client_task [ label = "websocket" dir = "both" ]
/// ccc -> http_task [ label = "HTTP\nrequest/response" dir = "both"]
/// client_task -> from_client
/// http_task -> http_to_client
/// http_to_client -> processing
/// processing -> http_from_client
/// http_from_client -> http_task
/// from_client -> processing
/// processing -> to_client
/// to_client -> client_task
/// ide -> ide_task [ dir = "both" ]
/// ide_task -> from_ide
/// from_ide -> processing
/// processing -> to_ide
/// to_ide -> ide_task
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
/// }
/// ```
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
/// * The startup phase loads the Client framework into a browser:
///
///   ```mermaid
///   sequenceDiagram
///   participant IDE
///   participant Server
///   participant Client
///   note over IDE, Client: Startup
///   IDE ->> Server: Opened(IdeType)
///   Server ->> IDE: Result(String: OK)
///   Server ->> IDE: ClientHtml(String: HTML or URL)
///   IDE ->> Server: Result(String: OK)
///   note over IDE, Client: Open browser (Client framework HTML or URL)
///   loop
///   Client -> Server: HTTP request(/static URL)
///   Server -> Client: HTTP response(/static data)
///   end
///   ```
///
/// * If the current file in the IDE changes (including the initial startup,
///   when the change is from no file to the current file), or a link is
///   followed in the Client's iframe:
///
///   ```mermaid
///   sequenceDiagram
///   participant IDE
///   participant Server
///   participant Client
///   alt IDE loads file
///   IDE ->> Client: CurrentFile(String: Path of main.py)
///   opt If Client document is dirty
///   Client ->> IDE: Update(String: contents of main.py)
///   IDE ->> Client: Response(OK)
///   end
///   Client ->> IDE: Response(OK)
///   else Client loads file
///   Client ->> IDE: CurrentFile(String: URL of main.py)
///   IDE ->> Client: Response(OK)
///   end
///   Client ->> Server: HTTP request(URL of main.py)
///   Server ->> IDE: LoadFile(String: path to main.py)
///   IDE ->> Server: Response(LoadFile(String: file contents of main.py))
///   alt main.py is editable
///   Server ->> Client: HTTP response(contents of Client)
///   Server ->> Client: Update(String: contents of main.py)
///   Client ->> Server: Response(OK)
///   loop
///   Client ->> Server: HTTP request(URL of supporting file in main.py)
///   Server ->> IDE: LoadFile(String: path of supporting file)
///   alt Supporting file in IDE
///   IDE ->> Server: Response(LoadFile(contents of supporting file)
///   Server ->> Client: HTTP response(contents of supporting file)
///   else Supporting file not in IDE
///   IDE ->> Server: Response(LoadFile(None))
///   Server ->> Client: HTTP response(contents of supporting file from /// filesystem)
///   end
///   end
///   else main.py not editable and not a project
///   Server ->> Client: HTTP response(contents of main.py)
///   else main.py not editable and is a project
///   Server ->> Client: HTTP response(contents of Client Simple Viewer)
///   Client ->> Server: HTTP request (URL?raw of main.py)
///   Server ->> Client: HTTP response(contents of main.py)
///   end
///   ```
///
/// * If the current file's contents in the IDE are edited:
///
///   ```mermaid
///   sequenceDiagram
///   participant IDE
///   participant Server
///   participant Client
///   IDE ->> Server: Update(String: new text contents)
///   alt Main file is editable
///     Server ->> Client: Update(String: new Client contents)
///   else Main file is not editable
///     Server ->> Client: Update(String: new text contents)
///   end
///   Client ->> IDE: Response(String: OK)
///   ```
///
/// * If the current file's contents in the Client are edited, the Client sends
///   the IDE an `Update` with the revised contents.
///
/// * When the PC goes to sleep then wakes up, the IDE client and the Editor
///   client both reconnect to the websocket URL containing their assigned ID.
///
/// * If the Editor client or the IDE client are closed, they close their
///   websocket, which sends a `Close` message to the other websocket, causes it
///   to also close and ending the session.
///
/// * If the server is stopped (or crashes), both clients shut down after
///   several reconnect retries.
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
/// (the standard
/// [numeric type](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Data_structures#number_type)
/// in JavaScript) has a 53-bit mantissa, meaning IDs won't wrap around for a
/// very long time.
// Imports
// -----------------------------------------------------------------------------
//
// ### Standard library
use std::{collections::HashMap, ffi::OsStr, fmt::Debug, path::PathBuf};

// ### Third-party
use lazy_static::lazy_static;
use log::{debug, error, warn};
use rand::random;
use regex::Regex;
use tokio::{
    fs::File,
    select,
    sync::mpsc::{self, Receiver, Sender},
};

// ### Local
use crate::{
    lexer::supported_languages::MARKDOWN_MODE,
    processing::{
        CodeChatForWeb, CodeMirror, CodeMirrorDiff, CodeMirrorDiffable, CodeMirrorDocBlock,
        CodeMirrorDocBlockVec, SourceFileMetadata, TranslationResultsString,
        codechat_for_web_to_source, diff_code_mirror_doc_blocks, diff_str,
        source_to_codechat_for_web_string,
    },
    queue_send, queue_send_func,
    webserver::{
        EditorMessage, EditorMessageContents, INITIAL_MESSAGE_ID, MESSAGE_ID_INCREMENT,
        ProcessingTaskHttpRequest, ProcessingTaskHttpRequestFlags, ResultErrTypes, ResultOkTypes,
        SimpleHttpResponse, SimpleHttpResponseError, UpdateMessageContents, WebAppState,
        WebsocketQueues, file_to_response, path_to_url, send_response, try_canonicalize,
        try_read_as_text, url_to_path,
    },
};

// Globals
// -----------------------------------------------------------------------------
//
// The max length of a message to show in the console.
const MAX_MESSAGE_LENGTH: usize = 500;

lazy_static! {
        /// A regex to determine the type of the first EOL. See 'PROCESSINGS\`.
    pub static ref EOL_FINDER: Regex = Regex::new("[^\r\n]*(\r?\n)").unwrap();
}

// Data structures
// -----------------------------------------------------------------------------
#[derive(Clone, Debug, PartialEq)]
pub enum EolType {
    Lf,
    Crlf,
}

// Code
// -----------------------------------------------------------------------------
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
    // 1. It hasn't been used before. In this case, create the appropriate
    //    queues and start websocket and processing tasks.
    // 2. It's in use, but was disconnected. In this case, re-use the queues and
    //    start the websocket task; the processing task is still running.
    // 3. It's in use by another IDE. This is an error, but I don't have a way
    //    to detect it yet.
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

/// This holds the state used by the main loop of the translation task; this
/// allows factoring out lengthy contents in the loop into subfunctions.
struct TranslationTask {
    // These parameters are passed to us.
    connection_id_raw: String,
    prefix: &'static [&'static str],
    allow_source_diffs: bool,
    to_ide_tx: Sender<EditorMessage>,
    from_ide_rx: Receiver<EditorMessage>,
    to_client_tx: Sender<EditorMessage>,
    from_client_rx: Receiver<EditorMessage>,
    from_http_rx: Receiver<ProcessingTaskHttpRequest>,

    // These parameters are internal state.
    /// The file currently loaded in the Client.
    current_file: PathBuf,
    /// A map of `LoadFile` requests sent to the IDE, awaiting its response.
    load_file_requests: HashMap<u64, ProcessingTaskHttpRequest>,
    /// The id for messages created by the server. Leave space for a server
    /// message during the init phase.
    id: f64,
    /// The source code, provided by the IDE. It will use whatever the IDE
    /// provides for EOLs, which is stored in `eol` below.
    source_code: String,
    code_mirror_doc: String,
    eol: EolType,
    /// Some means this contains valid HTML; None means don't use it (since it
    /// would have contained Markdown).
    code_mirror_doc_blocks: Option<Vec<CodeMirrorDocBlock>>,
    prefix_str: String,
    /// To support sending diffs, we must provide a way to determine if the
    /// sender and receiver have the same file contents before applying a diff.
    /// File contents can become unsynced due to:
    ///
    /// 1. A dropped/lost message between the IDE and Client.
    /// 2. Edits to file contents in two locations before updates from one
    ///    location (the Client, for example) propagate to the other location
    ///    (the IDE).
    ///
    /// Therefore, assign each file a version number. All files are sent with a
    /// unique, randomly-generated version number which define the file's
    /// version after this update is applied. Diffs also include the version
    /// number of the file before applying the diff; the
    // receiver's current version number must match with the sender's
    /// pre-diff version number in order to apply the diff. When the versions
    /// don't match, the IDE must send a full text file to the Server and Client
    /// to re-sync. When a file is first loaded, its version number is None,
    /// signaling that the sender must always provide the full text, not a diff.
    version: f64,
    /// Has the full (non-diff) version of the current file been sent? Don't
    /// send diffs until this is sent.
    sent_full: bool,
}

/// This is the processing task for the Visual Studio Code IDE. It handles all
/// the core logic to moving data between the IDE and the client.
#[allow(clippy::too_many_arguments)]
pub async fn translation_task(
    connection_id_prefix: String,
    connection_id_raw: String,
    prefix: &'static [&'static str],
    app_state: WebAppState,
    shutdown_only: bool,
    allow_source_diffs: bool,
    to_ide_tx: Sender<EditorMessage>,
    mut from_ide_rx: Receiver<EditorMessage>,
    to_client_tx: Sender<EditorMessage>,
    mut from_client_rx: Receiver<EditorMessage>,
) {
    let connection_id = format!("{connection_id_prefix}{connection_id_raw}");
    if !shutdown_only {
        debug!("VSCode processing task started.");

        // Create a queue for HTTP requests fo communicate with this task.
        let (from_http_tx, from_http_rx) = mpsc::channel(10);
        app_state
            .processing_task_queue_tx
            .lock()
            .unwrap()
            .insert(connection_id.to_string(), from_http_tx);

        let mut continue_loop = true;
        let mut tt = TranslationTask {
            connection_id_raw,
            prefix,
            allow_source_diffs,
            to_ide_tx,
            from_ide_rx,
            to_client_tx,
            from_client_rx,
            from_http_rx,
            current_file: PathBuf::new(),
            load_file_requests: HashMap::new(),
            id: INITIAL_MESSAGE_ID + MESSAGE_ID_INCREMENT,
            source_code: String::new(),
            code_mirror_doc: String::new(),
            // The initial state will be overwritten by the first `Update` or
            // `LoadFile`, so this value doesn't matter.
            eol: EolType::Lf,
            // Some means this contains valid HTML; None means don't use it
            // (since it would have contained Markdown).
            code_mirror_doc_blocks: Some(Vec::new()),
            prefix_str: "/".to_string() + &prefix.join("/"),
            version: 0.0,
            // Don't send diffs until this is sent.
            sent_full: false,
        };
        while continue_loop {
            select! {
                // Look for messages from the IDE.
                Some(ide_message) = tt.from_ide_rx.recv() => {
                    debug!("Received IDE message id = {}, message = {}", ide_message.id, debug_shorten(&ide_message.message));
                    match ide_message.message {
                        // Handle messages that the IDE must not send.
                        EditorMessageContents::Opened(_) |
                        EditorMessageContents::OpenUrl(_) |
                        EditorMessageContents::LoadFile(..) |
                        EditorMessageContents::ClientHtml(_) => {
                            let err = ResultErrTypes::IdeIllegalMessage;
                            error!("{err:?}");
                            send_response(&tt.to_ide_tx, ide_message.id, Err(err)).await;
                        },

                        // Handle messages that are simply passed through.
                        EditorMessageContents::Closed |
                        EditorMessageContents::RequestClose => {
                            debug!("Forwarding it to the Client.");
                            queue_send!(tt.to_client_tx.send(ide_message))
                        },

                        EditorMessageContents::Result(_) => continue_loop = tt.ide_result(ide_message).await,
                        EditorMessageContents::Update(_) => continue_loop = tt.ide_update(ide_message).await,

                        // Update the current file; translate it to a URL then
                        // pass it to the Client.
                        EditorMessageContents::CurrentFile(file_path, _is_text) => {
                            debug!("Translating and forwarding it to the Client.");
                            match try_canonicalize(&file_path) {
                                Ok(clean_file_path) => {
                                    queue_send!(tt.to_client_tx.send(EditorMessage {
                                        id: ide_message.id,
                                        message: EditorMessageContents::CurrentFile(
                                            path_to_url(&tt.prefix_str, Some(&tt.connection_id_raw), &clean_file_path), Some(true)
                                        )
                                    }));
                                    tt.current_file = file_path.into();
                                    // Since this is a new file, mark it as
                                    // unsent in full.
                                    tt.sent_full = false;
                                }
                                Err(err) => {
                                    error!("{err:?}");
                                    send_response(&tt.to_client_tx, ide_message.id, Err(ResultErrTypes::TryCanonicalizeError(err.to_string()))).await;
                                }
                            }
                        }
                    }
                },

                // Handle HTTP requests.
                Some(http_request) = tt.from_http_rx.recv() => {
                    debug!("Received HTTP request for {:?} and sending LoadFile to IDE, id = {}.", http_request.file_path, tt.id);
                    // Convert the request into a `LoadFile` message.
                    queue_send!(tt.to_ide_tx.send(EditorMessage {
                        id: tt.id,
                        message: EditorMessageContents::LoadFile
                            (http_request.file_path.clone(),
                            // Assign a version to this `LoadFile` request only
                            // if it's the current file and loaded as the file
                            // to edit, not as the sidebar TOC. We can us a
                            // simple comparison, since both file names have
                            // already been canonicalized.
                            http_request.file_path == tt.current_file &&
                            http_request.flags == ProcessingTaskHttpRequestFlags::None
                        )
                    }));
                    // Store the ID and request, which are needed to send a
                    // response when the `LoadFile` result is received.
                    tt.load_file_requests.insert(tt.id.to_bits(), http_request);
                    tt.id += MESSAGE_ID_INCREMENT;
                }

                // Handle messages from the client.
                Some(client_message) = tt.from_client_rx.recv() => {
                    debug!("Received Client message id = {}, message = {}", client_message.id, debug_shorten(&client_message.message));
                    match client_message.message {
                        // Handle messages that the client must not send.
                        EditorMessageContents::Opened(_) |
                        EditorMessageContents::LoadFile(..) |
                        EditorMessageContents::RequestClose |
                        EditorMessageContents::ClientHtml(_) => {
                            let err = ResultErrTypes::ClientIllegalMessage;
                            error!("{err:?}");
                            send_response(&tt.to_client_tx, client_message.id, Err(err)).await;
                        },

                        // Handle messages that are simply passed through.
                        EditorMessageContents::Closed => {
                            debug!("Forwarding it to the IDE.");
                            queue_send!(tt.to_ide_tx.send(client_message))
                        },

                        EditorMessageContents::Result(ref result) => {
                            debug!("Forwarding it to the IDE.");
                            // If the Client can't read our diff, send the full
                            // text next time.
                            if matches!(result, Err(ResultErrTypes::OutOfSync(..))) {
                                tt.sent_full = false;
                            }
                            queue_send!(tt.to_ide_tx.send(client_message))
                        },

                        // Open a web browser when requested.
                        EditorMessageContents::OpenUrl(url) => {
                            // This doesn't work in Codespaces. TODO: send this
                            // back to the VSCode window, then call
                            // `vscode.env.openExternal(vscode.Uri.parse(url))`.
                            if let Err(err) = webbrowser::open(&url) {
                                let err = ResultErrTypes::WebBrowserOpenFailed(err.to_string());
                                error!("{err:?}");
                                send_response(&tt.to_client_tx, client_message.id, Err(err)).await;
                            } else {
                                send_response(&tt.to_client_tx, client_message.id, Ok(ResultOkTypes::Void)).await;
                            }
                        },

                        EditorMessageContents::Update(_) => continue_loop = tt.client_update(client_message).await,

                        // Update the current file; translate it to a URL then
                        // pass it to the IDE.
                        EditorMessageContents::CurrentFile(url_string, _is_text) => {
                            debug!("Forwarding translated path to IDE.");
                            let result = match url_to_path(&url_string, tt.prefix) {
                                Err(err) => Err(ResultErrTypes::UrlToPathError(url_string.to_string(), err.to_string())),
                                Ok(file_path) => {
                                    match file_path.to_str() {
                                        None => Err(ResultErrTypes::NoPathToString(file_path)),
                                        Some(file_path_string) => {
                                            // Use a
                                            // [binary file sniffer](#binary-file-sniffer)
                                            // to determine if the file is text or
                                            // binary.
                                            let is_text = if let Ok(mut fc) = File::open(&file_path).await {
                                                try_read_as_text(&mut fc).await.is_some()
                                            } else {
                                                false
                                            };
                                            queue_send!(tt.to_ide_tx.send(EditorMessage {
                                                id: client_message.id,
                                                message: EditorMessageContents::CurrentFile(file_path_string.to_string(), Some(is_text))
                                            }));
                                            tt.current_file = file_path;
                                            // Since this is a new file, the full text
                                            // hasn't been sent yet.
                                            tt.sent_full = false;
                                            Ok(())
                                        }
                                    }
                                }
                            };
                            if let Err(msg) = result {
                                error!("{msg}");
                                send_response(&tt.to_client_tx, client_message.id, Err(msg)).await;
                            }
                        }
                    }
                },

                else => break
            }
        }
        (from_ide_rx, from_client_rx) = (tt.from_ide_rx, tt.from_client_rx);
    }
    debug!("VSCode processing task shutting down.");
    if app_state
        .processing_task_queue_tx
        .lock()
        .unwrap()
        .remove(&connection_id)
        .is_none()
    {
        error!("Unable to remove connection ID {connection_id} from processing task queue.");
    }
    if app_state
        .client_queues
        .lock()
        .unwrap()
        .remove(&connection_id)
        .is_none()
    {
        error!("Unable to remove connection ID {connection_id} from client queues.");
    }
    if app_state
        .ide_queues
        .lock()
        .unwrap()
        .remove(&connection_id)
        .is_none()
    {
        error!("Unable to remove connection ID {connection_id} from IDE queues.");
    }

    from_ide_rx.close();
    from_client_rx.close();

    // Drain any remaining messages after closing the queue.
    while let Some(m) = from_ide_rx.recv().await {
        warn!("Dropped queued message {m:?}");
    }
    while let Some(m) = from_client_rx.recv().await {
        warn!("Dropped queued message {m:?}");
    }
    debug!("VSCode processing task exited.");
}

// These provide translation for messages passing through the Server.
impl TranslationTask {
    // Pass a `Result` message to the Client, unless it's a `LoadFile` result.
    async fn ide_result(&mut self, ide_message: EditorMessage) -> bool {
        let EditorMessageContents::Result(ref result) = ide_message.message else {
            panic!("Should only be called with a result.");
        };
        let is_loadfile = match result {
            // See if this error was produced by a `LoadFile` result.
            Err(_) => self
                .load_file_requests
                .contains_key(&ide_message.id.to_bits()),
            Ok(result_ok) => match result_ok {
                ResultOkTypes::Void => false,
                ResultOkTypes::LoadFile(_) => true,
            },
        };
        // Pass the message to the client if this isn't a `LoadFile` result (the
        // only type of result which the Server should handle).
        if !is_loadfile {
            debug!("Forwarding it to the Client.");
            // If the Server can't read our diff, send the full text next time.
            if matches!(result, Err(ResultErrTypes::OutOfSync(..))) {
                self.sent_full = false;
            }
            queue_send_func!(self.to_client_tx.send(ide_message));
            return true;
        }
        // Ensure there's an HTTP request for this `LoadFile` result.
        let Some(http_request) = self.load_file_requests.remove(&ide_message.id.to_bits()) else {
            error!(
                "Error: no HTTP request found for LoadFile result ID {}.",
                ide_message.id
            );
            return true;
        };

        // Take ownership of the result after sending it above (which requires
        // ownership).
        let EditorMessageContents::Result(result) = ide_message.message else {
            panic!("Not a result.");
        };
        // Get the file contents from a `LoadFile` result; otherwise, this is
        // None.
        let file_contents_option = match result {
            Err(err) => {
                error!("{err:?}");
                None
            }
            Ok(result_ok) => match result_ok {
                ResultOkTypes::Void => panic!("LoadFile result should not be void."),
                ResultOkTypes::LoadFile(file_contents) => file_contents,
            },
        };

        // Process the file contents. Since VSCode doesn't have a PDF viewer,
        // determine if this is a PDF file. (TODO: look at the magic number also
        // -- "%PDF").
        let use_pdf_js = http_request.file_path.extension() == Some(OsStr::new("pdf"));
        let ((simple_http_response, option_update), file_contents) = match file_contents_option {
            Some((file_contents, new_version)) => {
                // Only pay attention to the version if this is an editable
                // Client file.
                if http_request.file_path == self.current_file
                    && http_request.flags == ProcessingTaskHttpRequestFlags::None
                {
                    self.version = new_version;
                }
                // The IDE just sent the full contents; we're sending full
                // contents to the Client.
                self.sent_full = true;
                (
                    file_to_response(
                        &http_request,
                        new_version,
                        &self.current_file,
                        Some(&file_contents),
                        use_pdf_js,
                    )
                    .await,
                    file_contents,
                )
            }
            None => {
                // The file wasn't available in the IDE. Look for it in the
                // filesystem.
                match File::open(&http_request.file_path).await {
                    Err(err) => (
                        (
                            SimpleHttpResponse::Err(SimpleHttpResponseError::Io(err)),
                            None,
                        ),
                        // There's no file, so return empty contents, which will
                        // be ignored.
                        "".to_string(),
                    ),
                    Ok(mut fc) => {
                        let option_file_contents = try_read_as_text(&mut fc).await;
                        (
                            file_to_response(
                                &http_request,
                                self.version,
                                &self.current_file,
                                option_file_contents.as_ref(),
                                use_pdf_js,
                            )
                            .await,
                            // If the file is binary, return empty contents,
                            // which will be ignored.
                            option_file_contents.unwrap_or("".to_string()),
                        )
                    }
                }
            }
        };
        if let Some(update) = option_update {
            let Some(ref tmp) = update.contents else {
                panic!("Contents must always be provided.");
            };
            let CodeMirrorDiffable::Plain(ref plain) = tmp.source else {
                panic!("Diff not supported.");
            };
            self.source_code = file_contents;
            self.eol = find_eol_type(&self.source_code);
            // We must clone here, since the original is placed in the TX queue.
            self.code_mirror_doc = plain.doc.clone();
            self.code_mirror_doc_blocks = Some(plain.doc_blocks.clone());

            debug!("Sending Update from LoadFile to Client, id = {}.", self.id);
            queue_send_func!(self.to_client_tx.send(EditorMessage {
                id: self.id,
                message: EditorMessageContents::Update(update)
            }));
            self.id += MESSAGE_ID_INCREMENT;
        }
        debug!("Sending HTTP response.");
        if let Err(err) = http_request.response_queue.send(simple_http_response) {
            error!("Unable to enqueue: {err:?}");
            return false;
        }

        true
    }

    async fn ide_update(&mut self, ide_message: EditorMessage) -> bool {
        let EditorMessageContents::Update(update) = ide_message.message else {
            panic!("Expected update message.");
        };
        // Normalize the provided file name.
        let result = match try_canonicalize(&update.file_path) {
            Err(err) => Err(ResultErrTypes::TryCanonicalizeError(err.to_string())),
            Ok(clean_file_path) => {
                match update.contents {
                    None => {
                        queue_send_func!(self.to_client_tx.send(EditorMessage {
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
                            CodeMirrorDiffable::Diff(_diff) => Err(ResultErrTypes::TodoDiffSupport),
                            CodeMirrorDiffable::Plain(code_mirror) => {
                                // If there are Windows newlines, replace with
                                // Unix; this is reversed when the file is sent
                                // back to the IDE.
                                self.eol = find_eol_type(&code_mirror.doc);
                                let doc_normalized_eols = code_mirror.doc.replace("\r\n", "\n");
                                // Translate the file.
                                match source_to_codechat_for_web_string(
                                    &doc_normalized_eols,
                                    &self.current_file,
                                    contents.version,
                                    false,
                                ) {
                                    Err(err) => {
                                        Err(ResultErrTypes::CannotTranslateSource(err.to_string()))
                                    }
                                    Ok((translation_results_string, _path_to_toc)) => {
                                        match translation_results_string {
                                            TranslationResultsString::CodeChat(ccfw) => {
                                                // Send the new translated contents.
                                                debug!("Sending translated contents to Client.");
                                                let CodeMirrorDiffable::Plain(
                                                    ref code_mirror_translated,
                                                ) = ccfw.source
                                                else {
                                                    panic!("Unexpected diff value.");
                                                };
                                                // Send a diff if possible.
                                                let client_contents = if self.sent_full {
                                                    self.diff_code_mirror(
                                                        ccfw.metadata.clone(),
                                                        self.version,
                                                        ccfw.version,
                                                        code_mirror_translated,
                                                    )
                                                } else {
                                                    self.sent_full = true;
                                                    ccfw.clone()
                                                };
                                                queue_send_func!(self.to_client_tx.send(EditorMessage {
                                                    id: ide_message.id,
                                                    message: EditorMessageContents::Update(UpdateMessageContents {
                                                        file_path: clean_file_path.to_str().expect("Since the path started as a string, assume it losslessly translates back to a string.").to_string(),
                                                        contents: Some(client_contents),
                                                        cursor_position: update.cursor_position,
                                                        scroll_position: update.scroll_position,
                                                    }),
                                                }));
                                                // Update to the latest code after
                                                // computing diffs. To avoid ownership
                                                // problems, re-define `ccfw_source_plain`.
                                                let CodeMirrorDiffable::Plain(
                                                    code_mirror_translated,
                                                ) = ccfw.source
                                                else {
                                                    panic!("{}", "Unexpected diff value.");
                                                };
                                                self.source_code = code_mirror.doc;
                                                self.code_mirror_doc = code_mirror_translated.doc;
                                                self.code_mirror_doc_blocks =
                                                    Some(code_mirror_translated.doc_blocks);
                                                // Update to the version of the file just
                                                // sent.
                                                self.version = contents.version;
                                                Ok(ResultOkTypes::Void)
                                            }
                                            // TODO
                                            TranslationResultsString::Binary => {
                                                Err(ResultErrTypes::TodoBinarySupport)
                                            }
                                            TranslationResultsString::Unknown => {
                                                // Send the new raw contents.
                                                debug!("Sending translated contents to Client.");
                                                queue_send_func!(self.to_client_tx.send(EditorMessage {
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
                                                            }),
                                                            version: contents.version
                                                        }),
                                                        cursor_position: update.cursor_position,
                                                        scroll_position: update.scroll_position,
                                                    }),
                                                }));
                                                Ok(ResultOkTypes::Void)
                                            }
                                            TranslationResultsString::Toc(_) => {
                                                Err(ResultErrTypes::NotToc)
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        };
        // If there's an error, then report it; otherwise, the message is passed
        // to the Client, which will provide the result.
        if let Err(err) = &result {
            error!("{err:?}");
            send_response(&self.to_ide_tx, ide_message.id, result).await;
        }

        true
    }

    /// Return a `CodeChatForWeb` struct containing a diff between
    /// `self.code_mirror_doc` / `self.code_mirror_doc_blocks` and
    /// `code_mirror_translated`.
    fn diff_code_mirror(
        &self,
        // The `metadata` and `version` fields will be copied from this to the
        // returned `CodeChatForWeb` struct.
        metadata: SourceFileMetadata,
        // The version number of the previous (before) data. Typically,
        // `self.version`.
        before_version: f64,
        // The version number for the resulting return struct.
        version: f64,
        // This provides the after data for the diff; before data comes from
        // `self.code_mirror` / `self.code_mirror_doc`.
        code_mirror_after: &CodeMirror,
    ) -> CodeChatForWeb {
        assert!(self.sent_full);
        let doc_diff = diff_str(&self.code_mirror_doc, &code_mirror_after.doc);
        let Some(ref cmdb) = self.code_mirror_doc_blocks else {
            panic!("Should have diff of doc blocks!");
        };
        let doc_blocks_diff = diff_code_mirror_doc_blocks(cmdb, &code_mirror_after.doc_blocks);
        CodeChatForWeb {
            // Clone needed here, so we can copy it later.
            metadata,
            source: CodeMirrorDiffable::Diff(CodeMirrorDiff {
                doc: doc_diff,
                doc_blocks: doc_blocks_diff,
                // The diff was made between the before version (this) and the
                // after version (`ccfw.version`).
                version: before_version,
            }),
            version,
        }
    }

    async fn client_update(&mut self, client_message: EditorMessage) -> bool {
        let EditorMessageContents::Update(update_message_contents) = client_message.message else {
            panic!("Expected update message.");
        };
        debug!("Forwarding translation of it to the IDE.");
        match try_canonicalize(&update_message_contents.file_path) {
            Err(err) => {
                let err = ResultErrTypes::TryCanonicalizeError(err.to_string());
                error!("{err:?}");
                send_response(&self.to_client_tx, client_message.id, Err(err)).await;
                return true;
            }
            Ok(clean_file_path) => {
                let codechat_for_web = match update_message_contents.contents {
                    None => None,
                    Some(cfw) => match codechat_for_web_to_source(&cfw) {
                        Ok(new_source_code) => {
                            // Update the stored CodeMirror data structures with
                            // what we just received. This must be updated
                            // before we can translate back to check for changes
                            // (the next step).
                            let CodeMirrorDiffable::Plain(code_mirror) = cfw.source else {
                                // TODO: support diffable!
                                panic!("Diff not supported.");
                            };
                            self.code_mirror_doc = code_mirror.doc;
                            self.code_mirror_doc_blocks = Some(code_mirror.doc_blocks);
                            // We may need to change this version if we send a
                            // diff back to the Client.
                            let mut cfw_version = cfw.version;

                            // Translate back to the Client to see if there are
                            // any changes after this conversion. Only check
                            // CodeChat documents, not Markdown docs.
                            if cfw.metadata.mode != MARKDOWN_MODE
                                && let Ok(ccfws) = source_to_codechat_for_web_string(
                                    &new_source_code,
                                    &clean_file_path,
                                    cfw.version,
                                    false,
                                )
                                && let TranslationResultsString::CodeChat(ccfw) = ccfws.0
                                && let CodeMirrorDiffable::Plain(code_mirror_translated) =
                                    ccfw.source
                                && self.sent_full
                            {
                                // Determine if the re-translation includes
                                // changes (such as line wrapping in doc blocks
                                // which changes line numbering, creation of a
                                // new doc block from previous code block text,
                                // or updates from future document intelligence
                                // such as renamed headings, etc.) For doc
                                // blocks that haven't been edited by TinyMCE,
                                // this is easy; equality is sufficient. Doc
                                // blocks that have been edited are a different
                                // case: TinyMCE removes newlines, causing a lot
                                // of "changes" to re-insert these. Therefore,
                                // use the following approach:
                                //
                                // 1. Compare the `doc` values. If they differ,
                                //    then the the Client needs an update.
                                // 2. Compare each code block using simple
                                //    equality. If this fails, compare the doc
                                //    block text excluding newlines. If still
                                //    different, then the Client needs an
                                //    update.
                                if code_mirror_translated.doc != self.code_mirror_doc
                                    || !doc_block_compare(
                                        &code_mirror_translated.doc_blocks,
                                        self.code_mirror_doc_blocks.as_ref().unwrap(),
                                    )
                                {
                                    // Use a whole number to avoid encoding
                                    // differences with fractional values.
                                    cfw_version = random::<u64>() as f64;
                                    // The Client needs an update.
                                    let client_contents = self.diff_code_mirror(
                                        cfw.metadata.clone(),
                                        cfw.version,
                                        cfw_version,
                                        &code_mirror_translated,
                                    );
                                    debug!(
                                        "Sending re-translation update id = {} back to the Client.",
                                        self.id
                                    );
                                    queue_send_func!(self.to_client_tx.send(EditorMessage {
                                        id: self.id,
                                        message: EditorMessageContents::Update(
                                            UpdateMessageContents {
                                                file_path: update_message_contents.file_path,
                                                contents: Some(client_contents),
                                                // Don't change the current position, since
                                                // the Client editing position should be
                                                // left undisturbed.
                                                cursor_position: None,
                                                scroll_position: None
                                            }
                                        )
                                    }));
                                    self.id += MESSAGE_ID_INCREMENT;
                                    // Update with what was just sent to the
                                    // client.
                                    self.code_mirror_doc = code_mirror_translated.doc;
                                    self.code_mirror_doc_blocks =
                                        Some(code_mirror_translated.doc_blocks);
                                }
                            };
                            // Correct EOL endings for use with the IDE.
                            let new_source_code_eol = eol_convert(new_source_code, &self.eol);
                            let ccfw = if self.sent_full && self.allow_source_diffs {
                                Some(CodeChatForWeb {
                                    metadata: cfw.metadata,
                                    source: CodeMirrorDiffable::Diff(CodeMirrorDiff {
                                        // Diff with correct EOLs, so that (for
                                        // CRLF files as well as LF files) offsets
                                        // are correct.
                                        doc: diff_str(&self.source_code, &new_source_code_eol),
                                        doc_blocks: vec![],
                                        version: self.version,
                                    }),
                                    version: cfw_version,
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
                                    version: cfw_version,
                                })
                            };
                            self.version = cfw_version;
                            self.source_code = new_source_code_eol;
                            ccfw
                        }
                        Err(message) => {
                            let err = ResultErrTypes::CannotTranslateCodeChat(message.to_string());
                            error!("{err:?}");
                            send_response(&self.to_client_tx, client_message.id, Err(err)).await;
                            return true;
                        }
                    },
                };
                debug!("Sending update id = {}", client_message.id);
                queue_send_func!(self.to_ide_tx.send(EditorMessage {
                    id: client_message.id,
                    message: EditorMessageContents::Update(UpdateMessageContents {
                        file_path: clean_file_path.to_str().expect("Since the path started as a string, assume it losslessly translates back to a string.").to_string(),
                        contents: codechat_for_web,
                        cursor_position: update_message_contents.cursor_position,
                        scroll_position: update_message_contents.scroll_position,
                    })
                }));
            }
        }

        true
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

// Given a vector of two doc blocks, compare them, ignoring newlines.
fn doc_block_compare(a: &CodeMirrorDocBlockVec, b: &CodeMirrorDocBlockVec) -> bool {
    if a.len() != b.len() {
        return false;
    }

    a.iter().zip(b).all(|el| {
        let a = el.0;
        let b = el.1;
        a.from == b.from
            && a.to == b.to
            && a.indent == b.indent
            && a.delimiter == b.delimiter
            && (a.contents == b.contents
                // TinyMCE replaces newlines inside paragraphs with a space; for
                // a crude comparison, translate all newlines back to spaces,
                // then ignore leading/trailing newlines.
                || map_newlines_to_spaces(&a.contents).eq(map_newlines_to_spaces(&b.contents)))
    })
}

fn map_newlines_to_spaces<'a>(
    s: &'a str,
) -> std::iter::Map<std::str::Chars<'a>, impl FnMut(char) -> char> {
    s.trim()
        .chars()
        .map(|c: char| if c == '\n' { ' ' } else { c })
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

// Tests
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use crate::{processing::CodeMirrorDocBlock, translation::doc_block_compare};

    #[test]
    fn test_x1() {
        let before = vec![CodeMirrorDocBlock {
            from: 0,
            to: 20,
            indent: "".to_string(),
            delimiter: "//".to_string(),
            contents: "<p>Copyright (C) 2025 Bryan A. Jones.</p>\n<p>This file is part of the CodeChat Editor. The CodeChat Editor is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.</p>\n<p>The CodeChat Editor is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.</p>\n<p>You should have received a copy of the GNU General Public License along with the CodeChat Editor. If not, see <a href=\"http://www.gnu.org/licenses\">http://www.gnu.org/licenses</a>.</p>\n<h1><code>debug_enable.mts</code> -- Configure debug features</h1>\n<p>True to enable additional debug logging.</p>".to_string(),
        }];
        let after = vec![CodeMirrorDocBlock {
            from: 0,
            to: 20,
            indent: "".to_string(),
            delimiter: "//".to_string(),
            contents: "<p>Copyright (C) 2025 Bryan A. Jones.</p>\n<p>This file is part of the CodeChat Editor. The CodeChat Editor is free\nsoftware: you can redistribute it and/or modify it under the terms of the GNU\nGeneral Public License as published by the Free Software Foundation, either\nversion 3 of the License, or (at your option) any later version.</p>\n<p>The CodeChat Editor is distributed in the hope that it will be useful, but\nWITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or\nFITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more\ndetails.</p>\n<p>You should have received a copy of the GNU General Public License along with\nthe CodeChat Editor. If not, see\n<a href=\"http://www.gnu.org/licenses\">http://www.gnu.org/licenses</a>.</p>\n<h1><code>debug_enable.mts</code> -- Configure debug features</h1>\n<p>True to enable additional debug logging.</p>\n".to_string(),
        }];
        assert!(doc_block_compare(&before, &after));
    }
}
