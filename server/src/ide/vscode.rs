use indoc::formatdoc;
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
// Modules
// -------
#[cfg(test)]
pub mod tests;

// Imports
// -------
// ### Standard library
// None.
//
// ### Third-party
use actix_web::{
    HttpRequest, HttpResponse,
    error::{Error, ErrorBadRequest},
    get, web,
};
use log::{debug, error};

// ### Local
use crate::{
    queue_send,
    translation::{CreateTranslationQueuesError, create_translation_queues, translation_task},
    webserver::{
        AppState, EditorMessage, EditorMessageContents, IdeType, ResultOkTypes, client_websocket,
        escape_html, filesystem_endpoint, get_client_framework, get_server_url, html_wrapper,
        send_response,
    },
};

// Globals
// -------
const VSC: &str = "vsc-";

// Code
// ----
#[get("/vsc/ws-ide/{connection_id_raw}")]
pub async fn vscode_ide_websocket(
    connection_id_raw: web::Path<String>,
    req: HttpRequest,
    body: web::Payload,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
    let connection_id_raw = connection_id_raw.to_string();
    let connection_id_str = format!("{VSC}{connection_id_raw}");

    let created_translation_queues_result =
        create_translation_queues(connection_id_str.clone(), app_state.clone());
    let (mut from_ide_rx, to_ide_tx, from_client_rx, to_client_tx) =
        match created_translation_queues_result {
            Err(err) => match err {
                CreateTranslationQueuesError::IdInUse(_) => {
                    return Err(ErrorBadRequest(err.to_string()));
                }
                CreateTranslationQueuesError::IdeInUse => {
                    return client_websocket(
                        connection_id_str.clone(),
                        req,
                        body,
                        app_state.ide_queues.clone(),
                    )
                    .await;
                }
            },
            Ok(tqr) => (
                tqr.from_ide_rx,
                tqr.to_ide_tx,
                tqr.from_client_rx,
                tqr.to_client_tx,
            ),
        };

    let app_state_task = app_state.clone();
    actix_rt::spawn(async move {
        let mut shutdown_only = true;
        'task: {
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
                                    <iframe src="{address}/vsc/cf/{connection_id_raw}" style="width: 100%; height: 100vh; border: none"></iframe>
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
                            webbrowser::open(&format!("{address}/vsc/cf/{connection_id_raw}"))
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
            shutdown_only = false;
        }
        translation_task(
            VSC.to_string(),
            connection_id_raw,
            app_state_task,
            to_ide_tx,
            from_ide_rx,
            to_client_tx,
            from_client_rx,
            shutdown_only,
        )
        .await;
    });

    // Move data between the IDE and the processing task via queues. The
    // websocket connection between the client and the IDE will run in the
    // endpoint for that connection.
    client_websocket(connection_id_str, req, body, app_state.ide_queues.clone()).await
}

/// Serve the Client Framework.
#[get("/vsc/cf/{connection_id}")]
pub async fn vscode_client_framework(connection_id: web::Path<String>) -> HttpResponse {
    HttpResponse::Ok().content_type("text/html").body(
        // Send the HTML for the internal browser.
        match get_client_framework(false, "vsc/ws-client", &connection_id) {
            Ok(web_page) => web_page,
            Err(html_string) => {
                error!("{html_string}");
                html_wrapper(&escape_html(&html_string))
            }
        },
    )
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
        format!("{VSC}{connection_id}"),
        req,
        body,
        app_state.client_queues.clone(),
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
    let (connection_id, file_path) = request_path.into_inner();
    filesystem_endpoint(format!("{VSC}{connection_id}"), file_path, &req, &app_state).await
}
