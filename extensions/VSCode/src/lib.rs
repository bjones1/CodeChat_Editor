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
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::ops::DerefMut;
use std::path::PathBuf;

// ### Third-party
use actix_server::{Server, ServerHandle};
use code_chat_editor::webserver::{self, WebAppState, setup_server};
use log::LevelFilter;
use napi::{Error, Status};
use napi_derive::napi;
use std::sync::Arc;
use tokio::sync::Mutex;

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

// Provide a class to start and stop the server.
#[napi]
struct CodeChatEditorServer {
    server: Arc<Mutex<Server>>,
    server_handle: ServerHandle,
    _app_state: WebAppState,
}

#[napi]
// NAPI's C code calls this, which Rust can't see.
#[allow(dead_code)]
impl CodeChatEditorServer {
    #[napi(constructor)]
    pub fn new(port: u16) -> Result<CodeChatEditorServer, Error> {
        let (server, _app_state) = setup_server(
            &SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port),
            None,
        )
        .map_err(|err| Error::new(Status::GenericFailure, err.to_string()))?;
        Ok(CodeChatEditorServer {
            server_handle: server.handle(),
            server: Arc::new(Mutex::new(server)),
            _app_state,
        })
    }

    #[napi]
    pub async fn start_server(&self) -> std::io::Result<()> {
        let mut server_guard = self.server.lock().await;
        server_guard.deref_mut().await
    }

    #[napi]
    pub async fn stop_server(&self) {
        self.server_handle.stop(true).await;
    }
}
