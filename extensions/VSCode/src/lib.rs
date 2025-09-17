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
// `lib.rs` -- Interface to the CodeChat Editor for VSCode
// =======================================================
//
// Imports
// -------
//
// ### Standard library
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::Arc,
    thread,
};

// ### Third-party
use actix_server::{Server, ServerHandle};
use code_chat_editor::{
    ide::vscode::{connection_id_raw_to_str, vscode_ide_core},
    translation::{CreatedTranslationQueues, create_translation_queues},
    webserver::{self, EditorMessage, WebAppState, setup_server},
};
use log::LevelFilter;
use napi::{Error, Status};
use napi_derive::napi;
use rand::random;
use tokio::sync::{
    Mutex,
    mpsc::{Receiver, Sender},
};

// Code
// ----
//
// This must be called only once, before constructing the `CodeChatEditorServer`
// class.
#[napi]
pub fn init_server(extension_base_path: String) -> Result<(), Error> {
    webserver::init_server(
        Some(&PathBuf::from(extension_base_path)),
        LevelFilter::Debug,
    )
    .map_err(|err| Error::new(Status::GenericFailure, err.to_string()))
}

// Using this macro is critical -- otherwise, the Actix system doesn't get
// correctly initialized, which makes calls to `actix_rt::spawn` fail.
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
#[napi]
struct CodeChatEditorServer {
    server_handle: ServerHandle,
    from_ide_tx: Sender<EditorMessage>,
    to_ide_rx: Arc<Mutex<Receiver<EditorMessage>>>,
}

#[napi]
// NAPI's C code calls this, which Rust can't see.
#[allow(dead_code)]
impl CodeChatEditorServer {
    #[napi(constructor)]
    pub fn new(port: u16) -> Result<CodeChatEditorServer, Error> {
        // Start the server.
        let (server, app_state) = setup_server(
            &SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port),
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
        .map_err(|err| Error::new(Status::GenericFailure, err.to_string()))?;
        thread::spawn(move || {
            start_server(
                connection_id_raw,
                app_state_task,
                translation_queues,
                server,
            )
        });

        // Get the IDE queues created by this task.
        let websocket_queues = app_state
            .ide_queues
            .lock()
            .map_err(|e| std::io::Error::other(format!("Unable to lock queue: {e}")))?
            .remove(&connection_id)
            .ok_or_else(|| {
                std::io::Error::other(format!("Unable to find queue named {connection_id}"))
            })?;

        Ok(CodeChatEditorServer {
            server_handle,
            from_ide_tx: websocket_queues.from_websocket_tx,
            to_ide_rx: Arc::new(Mutex::new(websocket_queues.to_websocket_rx)),
        })
    }

    #[napi]
    pub async fn get_message(&self) -> Result<Option<String>, Error> {
        match self.to_ide_rx.lock().await.recv().await {
            Some(editor_message) => match serde_json::to_string(&editor_message) {
                Ok(v) => Ok(Some(v)),
                Err(err) => Err(Error::new(Status::GenericFailure, err.to_string())),
            },
            None => Ok(None),
        }
    }

    #[napi]
    pub async fn send_message(&self, message: String) -> std::io::Result<()> {
        let editor_message = serde_json::from_str::<EditorMessage>(&message)?;
        self.from_ide_tx
            .send(editor_message)
            .await
            .map_err(|e| std::io::Error::other(e.to_string()))
    }

    #[napi]
    pub async fn stop_server(&self) {
        self.server_handle.stop(true).await;
    }
}
