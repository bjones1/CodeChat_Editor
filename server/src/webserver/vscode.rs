/// Copyright (C) 2023 Bryan A. Jones.
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
/// # `webserver.rs` -- Serve CodeChat Editor Client webpages
// ## Imports
//
// ### Standard library
//
// None.
//
// ### Third-party
use actix_web::{
    error::{Error, ErrorMisdirectedRequest},
    get,
    http::header::{self, ContentDisposition},
    web, HttpRequest, HttpResponse,
};
use log::{error, info, warn};
use open;
use tokio::sync::mpsc;

// ### Local
use super::{
    client_websocket, create_timeout, html_not_found, send_response, AppState, EditorMessage,
    EditorMessageContents, IdeType, ProcessingQueues,
};

// ## Code
#[get("/vsc/ws-ide/{connection_id}")]
pub async fn vscode_ide_websocket(
    connection_id: web::Path<String>,
    req: HttpRequest,
    body: web::Payload,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
    let (from_ide_tx, mut from_ide_rx) = mpsc::channel(10);
    let (to_ide_tx, to_ide_rx) = mpsc::channel(10);

    // Wait for the open message and respond, then end the task. Effectively,
    // this is a (pre-)processing task that exits as soon as the IDE has enough
    // information to launch the full processing task.
    let app_state_recv = app_state.clone();
    let connection_id_str = connection_id.to_string();
    actix_rt::spawn(async move {
        // Get the first message sent by the IDE.
        let Some(message): std::option::Option<EditorMessage> = from_ide_rx.recv().await else {
            error!("{}", "IDE websocket received no data.");
            return;
        };

        // Make sure it's the `Opened` message.
        let EditorMessageContents::Opened(ide_type) = message.message else {
            let msg = format!("Unexpected message {message:?}");
            error!("{msg}");
            send_response(&to_ide_tx, message.id, &msg).await;

            // Send a `Closing` message.
            if let Err(err) = to_ide_tx
                .send(EditorMessage {
                    id: 0,
                    message: EditorMessageContents::Closing,
                })
                .await
            {
                let msg = format!("Unable to enqueue: {err}");
                error!("{msg}");
                return;
            }
            create_timeout(&app_state_recv, 0);

            return;
        };

        // Ensure the IDE type (VSCode) is correct.
        match ide_type {
            IdeType::VSCode(is_self_hosted) => {
                if is_self_hosted {
                    // Send a response (successful) to the `Opened` message.
                    send_response(&to_ide_tx, message.id, "").await;
                    if let Err(err) = to_ide_tx
                        .send(EditorMessage {
                            id: 0,
                            message: EditorMessageContents::ClientHtml("testing".to_string()),
                        })
                        .await
                    {
                        let msg = format!("Unable to enqueue: {err}");
                        error!("{msg}");
                        return;
                    }
                    create_timeout(&app_state_recv, 0);
                } else {
                    // Open the Client in an external browser.
                    if let Err(err) = open::that_detached("https://example.com") {
                        let msg = format!("Unable to open web browser: {err}");
                        error!("{msg}");
                        send_response(&to_ide_tx, message.id, &msg).await;

                        // Send a `Closing` message.
                        if let Err(err) = to_ide_tx
                            .send(EditorMessage {
                                id: 0,
                                message: EditorMessageContents::Closing,
                            })
                            .await
                        {
                            let msg = format!("Unable to enqueue: {err}");
                            error!("{msg}");
                            return;
                        }
                        create_timeout(&app_state_recv, 0);

                        return;
                    }
                    send_response(&to_ide_tx, message.id, "").await;
                }
            }
            _ => {
                // This is the wrong IDE type. Report then error.
                let msg = format!("Invalid IDE type: {ide_type:?}");
                error!("{msg}");
                send_response(&to_ide_tx, message.id, &msg).await;

                // Close the connection.
                if let Err(err) = to_ide_tx
                    .send(EditorMessage {
                        id: 0,
                        message: EditorMessageContents::Closing,
                    })
                    .await
                {
                    let msg = format!("Unable to enqueue: {err}");
                    error!("{msg}");
                    return;
                }
                create_timeout(&app_state_recv, 0);
            }
        }

        // The web page containing the Client will soon be opened. Provide the
        // info for a processing task which connects the IDE to the Client.
        // TODO: a bit of a race condition -- what is the client webpage is
        // opened before this code run? (Not likely, though.)
        app_state_recv
            .vscode_processing_queues
            .lock()
            .unwrap()
            .insert(
                connection_id_str,
                ProcessingQueues {
                    from_ide_rx,
                    to_ide_tx,
                },
            );
    });

    // Move data between the IDE and the processing task via queues.
    client_websocket(
        connection_id,
        req,
        body,
        app_state.clone(),
        from_ide_tx,
        to_ide_rx,
    )
    .await
}
