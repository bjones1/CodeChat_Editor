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
use actix_web::{error::Error, get, web, HttpRequest, HttpResponse};
use log::error;
use open;
use tokio::sync::mpsc;

// ### Local
use super::{
    client_websocket, create_timeout, send_response, AppState, EditorMessage,
    EditorMessageContents, IdeType, ProcessingQueues,
};

use crate::queue_send;

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
        // Use a [labeled block expression](https://doc.rust-lang.org/reference/expressions/loop-expr.html#labelled-block-expressions) to provide a way to exit the current task.
        'task: {
            // Get the first message sent by the IDE.
            let Some(message): std::option::Option<EditorMessage> = from_ide_rx.recv().await else {
                error!("{}", "IDE websocket received no data.");
                break 'task;
            };

            // Make sure it's the `Opened` message.
            let EditorMessageContents::Opened(ide_type) = message.message else {
                let msg = format!("Unexpected message {message:?}");
                error!("{msg}");
                send_response(&to_ide_tx, message.id, &msg).await;

                // Send a `Closing` message.
                queue_send!(to_ide_tx.send(EditorMessage { id: 0, message: EditorMessageContents::Closing}), 'task);
                create_timeout(&app_state_recv, 0);
                break 'task;
            };

            // Ensure the IDE type (VSCode) is correct.
            match ide_type {
                IdeType::VSCode(is_self_hosted) => {
                    if is_self_hosted {
                        // Send a response (successful) to the `Opened` message.
                        send_response(&to_ide_tx, message.id, "").await;
                        queue_send!(to_ide_tx.send(EditorMessage { id: 0, message: EditorMessageContents::ClientHtml("testing".to_string())}), 'task);
                        create_timeout(&app_state_recv, 0);
                    } else {
                        // Open the Client in an external browser.
                        if let Err(err) = open::that_detached("https://example.com") {
                            let msg = format!("Unable to open web browser: {err}");
                            error!("{msg}");
                            send_response(&to_ide_tx, message.id, &msg).await;

                            // Send a `Closing` message.
                            queue_send!(to_ide_tx.send(EditorMessage { id: 0, message: EditorMessageContents::Closing}), 'task);
                            create_timeout(&app_state_recv, 0);

                            break 'task;
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
                    queue_send!(to_ide_tx.send(EditorMessage { id: 0, message: EditorMessageContents::Closing}), 'task);
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
        }
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

// ## Tests
#[cfg(test)]
mod test {
    use actix_web::{App, HttpServer};
    use assertables::assert_starts_with;
    use assertables::assert_starts_with_as_result;
    use futures_util::stream::FusedStream;
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

    use super::super::{configure_app, make_app_data, EditorMessage, EditorMessageContents};
    use crate::{cast, test_utils::configure_testing_logger, webserver::UpdateMessageContents};

    #[actix_web::test]
    async fn test_vscode_ide_websocket() {
        configure_testing_logger();

        // Start the full webserver, so we can send non-test requests to it. (The test library doesn't provide a way to send websocket requests that I know of.)
        // One problem: since the server gets run in a separate thread, we can't examine the logs.
        let app_data = make_app_data();
        let webserver_handle = actix_rt::spawn(async move {
            HttpServer::new(move || configure_app(App::new(), &app_data))
                .bind(("127.0.0.1", 8080))?
                // No need to create a bunch of threads for testing.
                .workers(1)
                .run()
                .await
        });

        // Connect to the VSCode IDE websocket.
        let (mut ws_stream, _) = connect_async("ws://127.0.0.1:8080/vsc/ws-ide/test-connection-id")
            .await
            .expect("Failed to connect");

        // Note: we can't check the logs, since the server runs in a separate thread. Changing the logger to log across threads means we get logs from other tests (which run in parallel by default). The benefit of running all tests single-threaded plus fixing the logger is low.
        //
        // Send a message that's not an `Opened` message.
        ws_stream
            .send(Message::Text(
                serde_json::to_string(&EditorMessage {
                    id: 0,
                    message: EditorMessageContents::Update(UpdateMessageContents {
                        path: None,
                        contents: None,
                        cursor_position: None,
                        scroll_position: None,
                    }),
                })
                .unwrap(),
            ))
            .await
            .unwrap();

        // Get the response. It should be an error.
        let em: EditorMessage = serde_json::from_str(
            &ws_stream
                .next()
                .await
                .unwrap()
                .unwrap()
                .into_text()
                .unwrap(),
        )
        .unwrap();
        let EditorMessageContents::Result(result) = em.message else {
            panic!();
        };
        assert_starts_with!(result, "Unexpected message");

        // Next, expect a closing message.
        let em: EditorMessage = serde_json::from_str(
            &ws_stream
                .next()
                .await
                .unwrap()
                .unwrap()
                .into_text()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(em.message, EditorMessageContents::Closing);

        // Send a response to the closing message.
        ws_stream
            .send(Message::Text(
                serde_json::to_string(&EditorMessage {
                    id: 0,
                    message: EditorMessageContents::Result("".to_string()),
                })
                .unwrap(),
            ))
            .await
            .unwrap();

        // Close the connection. TODO: check that the webserver closes it.
        ws_stream.close(None).await.unwrap();

        // Shut down the webserver.
        webserver_handle.abort();
    }
}
