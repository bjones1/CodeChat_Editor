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
/// `ide.rs` -- Provide interfaces with common IDEs
/// ===============================================
pub mod filewatcher;
pub mod vscode;

// Imports
// -------
//
// ### Standard library
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    thread,
    time::Duration,
};

// ### Third-party
use actix_server::{Server, ServerHandle};
use rand::random;
use tokio::{
    runtime::Handle,
    select,
    sync::{
        Mutex,
        mpsc::{self, Receiver, Sender},
    },
    task::JoinHandle,
    time::sleep,
};

// ### Local
use crate::{
    ide::vscode::{connection_id_raw_to_str, vscode_ide_core},
    processing::{CodeChatForWeb, CodeMirror, CodeMirrorDiffable, SourceFileMetadata},
    translation::{CreatedTranslationQueues, create_translation_queues},
    webserver::{
        self, EditorMessage, EditorMessageContents, INITIAL_IDE_MESSAGE_ID, MESSAGE_ID_INCREMENT,
        REPLY_TIMEOUT_MS, ResultOkTypes, UpdateMessageContents, WebAppState, setup_server,
    },
};

// Code
// ----
//
// Using this macro is critical -- otherwise, the Actix system doesn't get
// correctly initialized, which makes calls to `actix_rt::spawn` fail. In
// addition, this ensures that the server runs in a separate thread, rather than
// depending on the extension to yield it time to run in the current thread.
#[actix_web::main]
async fn start_server(
    connection_id_raw: String,
    app_state_task: WebAppState,
    translation_queues: CreatedTranslationQueues,
    server: Server,
) -> std::io::Result<()> {
    vscode_ide_core(connection_id_raw, app_state_task, translation_queues);
    server.await
}

// Provide a class to start and stop the server. All its fields are opaque,
// since only Rust should use them.
pub struct CodeChatEditorServer {
    server_handle: ServerHandle,
    from_ide_tx: Sender<EditorMessage>,
    to_ide_rx: Arc<Mutex<Receiver<EditorMessage>>>,
    current_id: Arc<Mutex<f64>>,
    pending_messages: Arc<Mutex<HashMap<u64, JoinHandle<()>>>>,
    expired_messages_tx: Sender<f64>,
    expired_messages_rx: Arc<Mutex<Receiver<f64>>>,
}

impl CodeChatEditorServer {
    pub fn new() -> std::io::Result<CodeChatEditorServer> {
        // Start the server.
        let (server, app_state) = setup_server(
            // A port of 0 requests the OS to assign an open port.
            &SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0),
            None,
        )?;
        let server_handle = server.handle();

        // Start a thread to translate between this IDE and a Client.
        let connection_id_raw = random::<u64>().to_string();
        let connection_id = connection_id_raw_to_str(&connection_id_raw);
        let app_state_task = app_state.clone();
        let translation_queues = create_translation_queues(
            connection_id_raw_to_str(connection_id_raw.as_str()),
            &app_state,
        )
        .map_err(|err| std::io::Error::other(format!("Unable to create queues: {err}")))?;
        thread::spawn(move || {
            start_server(
                connection_id_raw,
                app_state_task,
                translation_queues,
                server,
            )
        });

        // Get the IDE queues created by this task for use with the `get`/`put`
        // methods.
        let websocket_queues = app_state
            .ide_queues
            .lock()
            .map_err(|e| std::io::Error::other(format!("Unable to lock queue: {e}")))?
            .remove(&connection_id)
            .ok_or_else(|| {
                std::io::Error::other(format!("Unable to find queue named {connection_id}"))
            })?;

        let (expired_messages_tx, expired_messages_rx) = mpsc::channel(100);
        Ok(CodeChatEditorServer {
            server_handle,
            from_ide_tx: websocket_queues.from_websocket_tx,
            to_ide_rx: Arc::new(Mutex::new(websocket_queues.to_websocket_rx)),
            // Use a unique ID for each websocket message sent. See the
            // Implementation section on Message IDs for more information.
            current_id: Arc::new(Mutex::new(INITIAL_IDE_MESSAGE_ID)),
            pending_messages: Arc::new(Mutex::new(HashMap::new())),
            expired_messages_tx,
            expired_messages_rx: Arc::new(Mutex::new(expired_messages_rx)),
        })
    }

    // This returns an error if the conversion to JSON fails, `None` if the
    // queue is closed, or a JSON-encoded string containing the message
    // otherwise.
    pub async fn get_message(&self) -> Option<EditorMessage> {
        // Get a message -- either an expired message result or an incoming
        // message.
        let mut to_ide_rx = self.to_ide_rx.lock().await;
        let mut expired_messages_rx = self.expired_messages_rx.lock().await;
        select! {
            Some(m) = to_ide_rx.recv() => {
                // Cancel the timer on this pending message.
                if let Some(task) = self.pending_messages.lock().await.remove(&m.id.to_bits()) {
                    task.abort();
                }
                // Return it.
                Some(m)
            },
            Some(id) = expired_messages_rx.recv() =>
                // Report this unacknowledged message.
                Some(
                    EditorMessage {
                        id,
                        message: EditorMessageContents::Result(Err(format!("Timeout: message id {id} unacknowledged.")))
                    }
                ),
            else => None,
        }
    }

    // Like `get_message`, but with a timeout.
    pub async fn get_message_timeout(&self, timeout: Duration) -> Option<EditorMessage> {
        select! {
            _ = sleep(timeout) => None,
            v = self.get_message() => v
        }
    }

    // Send the provided message contents; add in an ID and add this to the list
    // of pending messages. This produces a timeout of a matching `Result`
    // message isn't received with the timeout.
    async fn send_message_timeout(
        &self,
        editor_message_contents: EditorMessageContents,
    ) -> std::io::Result<f64> {
        // Get and update the current ID.
        let id = {
            let mut id = self.current_id.lock().await;
            let old_id = *id;
            *id += MESSAGE_ID_INCREMENT;
            old_id
        };
        // Build the resulting message to send.
        let editor_message = EditorMessage {
            id,
            message: editor_message_contents,
        };

        // Start a timeout in case the message isn't acknowledged.
        let expired_messages_tx = self.expired_messages_tx.clone();
        // Important: there's already a Tokio runtime since this is an async
        // function. Use that to spawn a new task; there's not an Actix
        // System/Arbiter running in this thread.
        let waiting_task = Handle::current().spawn(async move {
            sleep(REPLY_TIMEOUT_MS).await;
            // Since the websocket failed to send a `Result`, produce a timeout
            // `Result` for it.
            match expired_messages_tx.send(id).await {
                Ok(join_handle) => join_handle,
                Err(err) => {
                    eprintln!("Error -- unable to send expired message: {err}");
                }
            }
        });
        // Add this to the list of pending message.
        self.pending_messages
            .lock()
            .await
            .insert(editor_message.id.to_bits(), waiting_task);

        self.send_message_raw(editor_message).await?;
        Ok(id)
    }

    // Send a message with no timeout or other additional steps.
    async fn send_message_raw(&self, editor_message: EditorMessage) -> std::io::Result<()> {
        self.from_ide_tx
            .send(editor_message)
            .await
            .map_err(|e| std::io::Error::other(e.to_string()))
    }

    pub async fn send_message_opened(&self, hosted_in_ide: bool) -> std::io::Result<f64> {
        self.send_message_timeout(EditorMessageContents::Opened(webserver::IdeType::VSCode(
            hosted_in_ide,
        )))
        .await
    }

    // Send a `CurrentFile` message. The other parameter (true if text/false if
    // binary/None if ignored) is ignored by the server, so it's always sent as
    // `None`.
    pub async fn send_message_current_file(&self, url: String) -> std::io::Result<f64> {
        self.send_message_timeout(EditorMessageContents::CurrentFile(url, None))
            .await
    }

    // Send an `Update` message, optionally with plain text (instead of a diff)
    // containing the source code from the IDE.
    pub async fn send_message_update_plain(
        &self,
        file_path: String,
        // `null` to send no source code; a string to send the source code.
        option_contents: Option<String>,
        cursor_position: Option<u32>,
        scroll_position: Option<f64>,
    ) -> std::io::Result<f64> {
        self.send_message_timeout(EditorMessageContents::Update(UpdateMessageContents {
            file_path,
            contents: option_contents.map(|contents| CodeChatForWeb {
                metadata: SourceFileMetadata {
                    mode: "".to_string(),
                },
                source: CodeMirrorDiffable::Plain(CodeMirror {
                    doc: contents,
                    doc_blocks: vec![],
                }),
            }),
            cursor_position,
            scroll_position: scroll_position.map(|x| x as f32),
        }))
        .await
    }

    // Send either an Ok(Void) or an Error result to the Client.
    pub async fn send_result(
        &self,
        id: f64,
        message_result: Option<String>,
    ) -> std::io::Result<()> {
        let editor_message = EditorMessage {
            id,
            message: webserver::EditorMessageContents::Result(
                if let Some(message_result) = message_result {
                    Err(message_result)
                } else {
                    Ok(ResultOkTypes::Void)
                },
            ),
        };
        self.send_message_raw(editor_message).await
    }

    pub async fn send_result_loadfile(
        &self,
        id: f64,
        load_file: Option<String>,
    ) -> std::io::Result<()> {
        self.send_message_raw(EditorMessage {
            id,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::LoadFile(load_file))),
        })
        .await
    }

    // This returns after the server shuts down.
    pub async fn stop_server(&self) {
        self.server_handle.stop(true).await;
        // Stop all running timers.
        for (_id, join_handle) in self.pending_messages.lock().await.drain() {
            join_handle.abort();
        }
        // Since the server is closing, don't report any expired message.
        self.expired_messages_rx.lock().await.close();
    }
}
