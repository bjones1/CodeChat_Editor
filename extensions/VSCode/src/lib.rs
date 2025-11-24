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
use std::path::PathBuf;

// ### Third-party
use log::LevelFilter;
use napi::{Error, Status};
use napi_derive::napi;

// ### Local
use code_chat_editor::{ide, webserver};

// Code
// ----
#[napi]
pub fn init_server(extension_base_path: String) -> Result<(), Error> {
    webserver::init_server(
        Some(&PathBuf::from(extension_base_path)),
        LevelFilter::Debug,
    )
    .map_err(|err| Error::new(Status::GenericFailure, err.to_string()))
}

#[napi]
struct CodeChatEditorServer(ide::CodeChatEditorServer);

#[napi]
// NAPI's C code calls this, which Rust can't see.
#[allow(dead_code)]
impl CodeChatEditorServer {
    #[napi(constructor)]
    pub fn new() -> Result<CodeChatEditorServer, Error> {
        Ok(CodeChatEditorServer(ide::CodeChatEditorServer::new()?))
    }

    // This returns an error if the conversion to JSON fails, `None` if the
    // queue is closed, or a JSON-encoded string containing the message
    // otherwise.
    #[napi]
    pub async fn get_message(&self) -> Result<Option<String>, Error> {
        let editor_message = self.0.get_message().await;

        // Encode then deliver it.
        match editor_message {
            Some(editor_message) => match serde_json::to_string(&editor_message) {
                Ok(v) => Ok(Some(v)),
                Err(err) => Err(Error::new(Status::GenericFailure, err.to_string())),
            },
            None => Ok(None),
        }
    }

    #[napi]
    pub async fn send_message_opened(&self, hosted_in_ide: bool) -> std::io::Result<f64> {
        self.0.send_message_opened(hosted_in_ide).await
    }

    #[napi]
    pub async fn send_message_current_file(&self, url: String) -> std::io::Result<f64> {
        self.0.send_message_current_file(url).await
    }

    #[napi]
    pub async fn send_message_update_plain(
        &self,
        file_path: String,
        // `null` to send no source code; a string to send the source code.
        option_contents: Option<(String, f64)>,
        cursor_position: Option<u32>,
        scroll_position: Option<f64>,
    ) -> std::io::Result<f64> {
        self.0
            .send_message_update_plain(file_path, option_contents, cursor_position, scroll_position)
            .await
    }

    #[napi]
    pub async fn send_result(
        &self,
        id: f64,
        // If provided, a JSON-encoded `ResultErrTypes`.
        message_result: Option<String>,
    ) -> std::io::Result<()> {
        let message = if let Some(err_json) = message_result {
            match serde_json::from_str(&err_json) {
                Ok(v) => Some(v),
                Err(err) => return Err(std::io::Error::other(err.to_string())),
            }
        } else {
            None
        };
        self.0.send_result(id, message).await
    }

    #[napi]
    pub async fn send_result_loadfile(
        &self,
        id: f64,
        load_file: Option<(String, f64)>,
    ) -> std::io::Result<()> {
        self.0.send_result_loadfile(id, load_file).await
    }

    // This returns after the server shuts down.
    #[napi]
    pub async fn stop_server(&self) {
        self.0.stop_server().await
    }
}
