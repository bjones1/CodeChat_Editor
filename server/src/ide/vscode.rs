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
/// `vscode.rs` -- Implement server-side functionality for the Visual Studio
/// Code IDE
/// ========================================================================
use log::error;

use crate::webserver::{
    AppState, client_websocket, escape_html, filesystem_endpoint, get_client_framework,
    html_wrapper,
};
use actix_web::{HttpRequest, HttpResponse, error::Error, get, web};

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
