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
/// # `vscode.rs` -- Implement server-side functionality for the Visual Studio Code IDE
// ## Imports
//
// ### Standard library
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

// ### Third-party
use actix_web::{
    error::{Error, ErrorBadRequest},
    get, web, HttpRequest, HttpResponse,
};
use log::{error, warn};
use open;
use tokio::{select, sync::mpsc};

// ### Local
use super::{
    client_websocket, get_client_framework, send_response, AppState, EditorMessage,
    EditorMessageContents, IdeType, WebsocketQueues,
};
use crate::{
    oneshot_send, queue_send,
    webserver::{
        escape_html, filesystem_endpoint, html_wrapper, make_simple_http_response, path_to_url,
        text_file_to_response, ProcessingTaskHttpRequest, ResultOkTypes,
    },
};

// ## Code
//
// This is the processing task for the Visual Studio Code IDE. It handles all the core logic to moving data between the IDE and the client.
#[get("/vsc/ws-ide/{connection_id}")]
pub async fn vscode_ide_websocket(
    connection_id: web::Path<String>,
    req: HttpRequest,
    body: web::Payload,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
    let connection_id_str = connection_id.to_string();

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
        .vscode_connection_id
        .lock()
        .unwrap()
        .contains(&connection_id_str)
    {
        let msg = format!("Connection ID {connection_id_str} already in use.");
        error!("{msg}");
        return Err(ErrorBadRequest(msg));
    }

    // Now case 2.
    if app_state
        .vscode_ide_queues
        .lock()
        .unwrap()
        .contains_key(&connection_id_str)
    {
        return client_websocket(
            connection_id,
            req,
            body,
            app_state.vscode_ide_queues.clone(),
        )
        .await;
    }

    // Then this is case 1. Add the connection ID to the list of active
    // connections.
    let (from_ide_tx, mut from_ide_rx) = mpsc::channel(10);
    let (to_ide_tx, to_ide_rx) = mpsc::channel(10);
    assert!(app_state
        .vscode_ide_queues
        .lock()
        .unwrap()
        .insert(
            connection_id_str.clone(),
            WebsocketQueues {
                from_websocket_tx: from_ide_tx,
                to_websocket_rx: to_ide_rx,
            },
        )
        .is_none());
    let (from_client_tx, mut from_client_rx) = mpsc::channel(10);
    let (to_client_tx, to_client_rx) = mpsc::channel(10);
    assert!(app_state
        .vscode_client_queues
        .lock()
        .unwrap()
        .insert(
            connection_id_str.clone(),
            WebsocketQueues {
                from_websocket_tx: from_client_tx,
                to_websocket_rx: to_client_rx,
            },
        )
        .is_none());
    app_state
        .vscode_connection_id
        .lock()
        .unwrap()
        .insert(connection_id_str.clone());

    // Clone variables owned by the processing task.
    let connection_id_task = connection_id_str.clone();
    let app_state_task = app_state.clone();

    // Start the processing task.
    actix_rt::spawn(async move {
        // Use a
        // [labeled block expression](https://doc.rust-lang.org/reference/expressions/loop-expr.html#labelled-block-expressions)
        // to provide a way to exit the current task.
        'task: {
            let mut current_file = PathBuf::new();
            let mut load_file_requests: HashMap<u32, ProcessingTaskHttpRequest> = HashMap::new();

            // Get the first message sent by the IDE.
            let Some(message): std::option::Option<EditorMessage> = from_ide_rx.recv().await else {
                error!("{}", "IDE websocket received no data.");
                break 'task;
            };

            // Make sure it's the `Opened` message.
            let EditorMessageContents::Opened(ide_type) = message.message else {
                let msg = format!("Unexpected message {message:?}");
                error!("{msg}");
                send_response(&to_ide_tx, message.id, Err(msg)).await;

                // Send a `Closed` message to shut down the websocket.
                queue_send!(to_ide_tx.send(EditorMessage { id: 0, message: EditorMessageContents::Closed}), 'task);
                break 'task;
            };

            // Ensure the IDE type (VSCode) is correct.
            match ide_type {
                IdeType::VSCode(is_self_hosted) => {
                    if is_self_hosted {
                        // Send a response (successful) to the `Opened` message.
                        send_response(&to_ide_tx, message.id, Ok(ResultOkTypes::Void)).await;

                        // Send the HTML for the internal browser.
                        let client_html = match get_client_framework(
                            false,
                            "vs/vsc/ws-client",
                            &connection_id_task,
                        ) {
                            Ok(web_page) => web_page,
                            Err(html_string) => {
                                error!("{html_string}");
                                html_wrapper(&escape_html(&html_string))
                            }
                        };
                        queue_send!(to_ide_tx.send(EditorMessage {
                            id: 0,
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
                            if message.id != 0 {
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
                                id: 1,
                                message: EditorMessageContents::Closed
                            }), 'task);
                            break 'task;
                        };
                    } else {
                        // Open the Client in an external browser.
                        if let Err(err) = open::that_detached("https://example.com") {
                            let msg = format!("Unable to open web browser: {err}");
                            error!("{msg}");
                            send_response(&to_ide_tx, message.id, Err(msg)).await;

                            // Send a `Closed` message.
                            queue_send!(to_ide_tx.send(EditorMessage{
                                id: 0,
                                message: EditorMessageContents::Closed
                            }), 'task);
                            break 'task;
                        }
                        // Send a response (successful) to the `Opened` message.
                        send_response(&to_ide_tx, message.id, Ok(ResultOkTypes::Void)).await;
                    }
                }
                _ => {
                    // This is the wrong IDE type. Report then error.
                    let msg = format!("Invalid IDE type: {ide_type:?}");
                    error!("{msg}");
                    send_response(&to_ide_tx, message.id, Err(msg)).await;

                    // Close the connection.
                    queue_send!(to_ide_tx.send(EditorMessage { id: 0, message: EditorMessageContents::Closed}), 'task);
                    break 'task;
                }
            }

            // Create a queue for HTTP requests fo communicate with this task.
            let (from_http_tx, mut from_http_rx) = mpsc::channel(10);
            app_state_task
                .processing_task_queue_tx
                .lock()
                .unwrap()
                .insert(connection_id_task.to_string(), from_http_tx);

            // All further messages are handled in the main loop.
            let mut id: u32 = 1;
            loop {
                select! {
                    // Look for messages from the IDE.
                    Some(ide_message) = from_ide_rx.recv() => {
                        match ide_message.message {
                            // Handle messages that the IDE must not send.
                            EditorMessageContents::Closed |
                            EditorMessageContents::Opened(_) |
                            EditorMessageContents::LoadFile(_) |
                            EditorMessageContents::ClientHtml(_) => {
                                let msg = "IDE must not send this message.";
                                error!("{msg}");
                                send_response(&to_ide_tx, ide_message.id, Err(msg.to_string())).await;
                            },

                            // Handle messages that are simply passed through.
                            EditorMessageContents::RequestClose =>
                                queue_send!(to_client_tx.send(ide_message)),

                            // Pass a `Result` message to the Client, unless it's a `LoadFile` result.
                            EditorMessageContents::Result(ref result) => {
                                let is_loadfile = match result {
                                    // See if this error was produced by a `LoadFile` result.
                                    Err(err) => err.1,
                                    Ok(result_ok) => match result_ok {
                                        ResultOkTypes::Void => false,
                                        ResultOkTypes::LoadFile(_) => true,
                                    }
                                };
                                // Pass the message to the client if this isn't a `LoadFile` result (the only type of result which the Server should handle).
                                if !is_loadfile {
                                    queue_send!(to_client_tx.send(ide_message));
                                    continue;
                                }
                                // Ensure there's an HTTP request for this `LoadFile` result.
                                let Some(http_request) = load_file_requests.remove(&ide_message.id) else {
                                    error!("Error: no HTTP request found for LoadFile result ID {}.", ide_message.id);
                                    break 'task;
                                };

                                // Get the file contents from a `LoadFile` result; otherwise, this is None.
                                let file_contents_option = match result {
                                    Err((err, _)) => {
                                        error!("{err}");
                                        &None
                                    },
                                    Ok(result_ok) => match result_ok {
                                        ResultOkTypes::Void => panic!("LoadFile result should not be void."),
                                        ResultOkTypes::LoadFile(file_contents) => file_contents,
                                    }
                                };

                                // Process the file contents.
                                let (simple_http_response, option_update) = match file_contents_option {
                                    Some(file_contents) =>
                                        text_file_to_response(&http_request, &current_file, &http_request.file_path, file_contents).await,
                                    None =>
                                        // The file wasn't available in the IDE. Look for it in the filesystem.
                                        make_simple_http_response(&http_request, &current_file).await
                                };
                                if let Some(update) = option_update {
                                    // Send the update to the client.
                                    queue_send!(to_client_tx.send(EditorMessage { id: ide_message.id, message: update }));
                                    id += 1;
                                }
                                oneshot_send!(http_request.response_queue.send(simple_http_response));
                            }

                            // Handle the `Update` message.
                            EditorMessageContents::Update(_update) => {
                                // First, see if this update requires a
                                // different working directory. If so, split it
                                // into two parts.
                            }

                            // Update the current file; translate it to a URL then pass it to the Client.
                            EditorMessageContents::CurrentFile(file_path) => {
                                queue_send!(to_client_tx.send(EditorMessage {
                                    id: ide_message.id,
                                    message: EditorMessageContents::CurrentFile(
                                        format!("/vsc/fs/{connection_id_task}/{}", path_to_url(Path::new(&file_path)))
                                    )
                                }));
                                current_file = file_path.into();
                            }
                        }
                    },

                    // Handle HTTP requests.
                    Some(http_request) = from_http_rx.recv() => {
                        // Store the ID and request, which are needed to send a response when the `LoadFile` result is received.
                        let file_path = http_request.file_path.clone();
                        load_file_requests.insert(id, http_request);

                        // Convert the request into a `LoadFile` message.
                        queue_send!(to_ide_tx.send(EditorMessage {
                            id,
                            message: EditorMessageContents::LoadFile(file_path)
                        }));
                        id += 1;
                    }

                    // Handle messages from the client.
                    Some(client_message) = from_client_rx.recv() => {
                        match client_message.message {
                            // Handle messages that the client must not send.
                            EditorMessageContents::Closed |
                            EditorMessageContents::Opened(_) |
                            EditorMessageContents::LoadFile(_) |
                            EditorMessageContents::ClientHtml(_) => {
                                let msg = "Client must not send this message.";
                                error!("{msg}");
                                send_response(&to_client_tx, client_message.id, Err(msg.to_string())).await;
                            },

                                send_response(&to_client_tx, client_message.id, Err((msg.to_string(), false))).await;
                            },

                            // Handle messages that are simply passed through.
                            EditorMessageContents::RequestClose |
                            EditorMessageContents::Result(_) => queue_send!(to_ide_tx.send(client_message)),

                            // Handle the `Update` message.
                            EditorMessageContents::Update(_update) => {
                            },

                            // Update the current file; translate it to a URL then pass it to the IDE.
                            EditorMessageContents::CurrentFile(_file_path) => {
                            }
                        }
                    },

                    else => break
                }
            }

            if app_state_task
                .processing_task_queue_tx
                .lock()
                .unwrap()
                .remove(&connection_id_task)
                .is_none()
            {
                error!("Unable to remove connection ID {connection_id_task} from processing task queue.");
            }
            if app_state_task
                .vscode_client_queues
                .lock()
                .unwrap()
                .remove(&connection_id_task)
                .is_none()
            {
                error!("Unable to remove connection ID {connection_id_task} from client queues.");
            }
            if app_state_task
                .vscode_ide_queues
                .lock()
                .unwrap()
                .remove(&connection_id_task)
                .is_none()
            {
                error!("Unable to remove connection ID {connection_id_task} from IDE queues.");
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
        }
    });

    // Move data between the IDE and the processing task via queues. The websocket connection between the client and the IDE will run in the endpoint for that connection.
    client_websocket(
        connection_id,
        req,
        body,
        app_state.vscode_ide_queues.clone(),
    )
    .await
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

// ## Tests
#[cfg(test)]
mod test {
    use std::io::Error;

    use actix_rt::task::JoinHandle;
    use assertables::{assert_ends_with, assert_starts_with};
    use futures_util::{SinkExt, StreamExt};
    use lazy_static::lazy_static;
    use minreq;
    use tokio::io::{AsyncRead, AsyncWrite};
    use tokio_tungstenite::{
        connect_async, tungstenite::http::StatusCode, tungstenite::protocol::Message,
        WebSocketStream,
    };

    use super::super::{
        run_server, EditorMessage, EditorMessageContents, IdeType, IP_ADDRESS, IP_PORT,
    };
    use crate::{
        cast, prep_test_dir,
        processing::{CodeChatForWeb, CodeMirror, SourceFileMetadata},
        test_utils::{check_logger_errors, configure_testing_logger},
        webserver::{ResultOkTypes, UpdateMessageContents},
    };

    lazy_static! {
        // Run a single webserver for all tests.
        static ref webserver_handle: JoinHandle<Result<(), Error>> =
            actix_rt::spawn(async move { run_server().await });
    }

    // Send a message via a websocket.
    async fn send_message<S: AsyncRead + AsyncWrite + Unpin>(
        ws_stream: &mut WebSocketStream<S>,
        message: &EditorMessage,
    ) {
        ws_stream
            .send(Message::Text(serde_json::to_string(message).unwrap()))
            .await
            .unwrap();
    }

    // Read a message from a websocket.
    async fn read_message<S: AsyncRead + AsyncWrite + Unpin>(
        ws_stream: &mut WebSocketStream<S>,
    ) -> EditorMessage {
        let msg_txt = loop {
            let msg = ws_stream.next().await.unwrap().unwrap();
            match msg {
                Message::Close(_) => panic!("Unexpected close message."),
                Message::Ping(_) => ws_stream.send(Message::Pong(vec![])).await.unwrap(),
                Message::Pong(_) => panic!("Unexpected pong message."),
                Message::Text(txt) => break txt,
                Message::Binary(_) => panic!("Unexpected binary message."),
                Message::Frame(_) => panic!("Unexpected frame message."),
            }
        };
        serde_json::from_str(&msg_txt).expect(&format!("Unable to convert '{msg_txt}' to JSON."))
    }

    // Test incorrect inputs: two connections with the same ID, sending the
    // wrong first message.
    #[actix_web::test]
    async fn test_vscode_ide_websocket1() {
        configure_testing_logger();
        // Ensure the webserver is running.
        let _ = &*webserver_handle;

        // Connect to the VSCode IDE websocket.
        let (mut ws_ide, _) = connect_async(format!(
            "ws://{IP_ADDRESS}:{IP_PORT}/vsc/ws-ide/test-connection-id1"
        ))
        .await
        .expect("Failed to connect");

        // Start a second connection; verify that it fails.
        let err = connect_async(format!(
            "ws://{IP_ADDRESS}:{IP_PORT}/vsc/ws-ide/test-connection-id1"
        ))
        .await
        .expect_err("Should fail to connect");
        let response = cast!(err, tokio_tungstenite::tungstenite::Error::Http);
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        // Note: we can't check the logs, since the server runs in a separate
        // thread. Changing the logger to log across threads means we get logs
        // from other tests (which run in parallel by default). The benefit of
        // running all tests single-threaded plus fixing the logger is low.
        //
        // Send a message that's not an `Opened` message.
        send_message(
            &mut ws_ide,
            &EditorMessage {
                id: 0,
                message: EditorMessageContents::Update(UpdateMessageContents {
                    contents: None,
                    cursor_position: None,
                    scroll_position: None,
                }),
            },
        )
        .await;

        // Get the response. It should be an error.
        let em = read_message(&mut ws_ide).await;
        let result = cast!(em.message, EditorMessageContents::Result);

        assert_starts_with!(cast!(&result, Err), "Unexpected message");

        // Next, expect the websocket to be closed.
        let err = &ws_ide.next().await.unwrap().unwrap();
        assert_eq!(*err, Message::Close(None));

        check_logger_errors(0);
    }

    // Test opening the Client in an external browser.
    #[actix_web::test]
    async fn test_vscode_ide_websocket2() {
        configure_testing_logger();
        // Ensure the webserver is running.
        let _ = &*webserver_handle;

        // Connect to the VSCode IDE websocket.
        let (mut ws_ide, _) = connect_async(format!(
            "ws://{IP_ADDRESS}:{IP_PORT}/vsc/ws-ide/test-connection-id2"
        ))
        .await
        .expect("Failed to connect");

        // Send the `Opened` message.
        send_message(
            &mut ws_ide,
            &EditorMessage {
                id: 0,
                message: EditorMessageContents::Opened(IdeType::VSCode(false)),
            },
        )
        .await;

        // Get the response. It should be success.
        let em = read_message(&mut ws_ide).await;
        assert_eq!(
            cast!(em.message, EditorMessageContents::Result),
            Ok(ResultOkTypes::Void)
        );

        check_logger_errors(0);
    }

    // Test opening the Client in the VSCode browser.
    #[actix_web::test]
    async fn test_vscode_ide_websocket3() {
        configure_testing_logger();
        let (temp_dir, test_dir) = prep_test_dir!();
        // Ensure the webserver is running.
        let _ = &*webserver_handle;

        // Connect to the VSCode IDE websocket.
        let (mut ws_ide, _) = connect_async(format!(
            "ws://{IP_ADDRESS}:{IP_PORT}/vsc/ws-ide/test-connection-id3"
        ))
        .await
        .expect("Failed to connect");

        // 1. Send the `Opened` message.
        //
        // Message ids: IDE - 100->200, Server - 0, Client - 1000.
        send_message(
            &mut ws_ide,
            &EditorMessage {
                id: 100,
                message: EditorMessageContents::Opened(IdeType::VSCode(true)),
            },
        )
        .await;

        // Get the response. It should be success.
        let em = read_message(&mut ws_ide).await;
        assert_eq!(
            em,
            EditorMessage {
                id: 100,
                message: EditorMessageContents::Result(Ok(ResultOkTypes::Void)),
            }
        );

        // 2. Next, wait for the next message -- the HTML.
        //
        // Message ids: IDE - 200, Server - 0->1, Client - 1000.
        let em = read_message(&mut ws_ide).await;
        assert_starts_with!(
            cast!(&em.message, EditorMessageContents::ClientHtml),
            "<!DOCTYPE html>"
        );
        assert_eq!(em.id, 0);

        // Send a success response to this message.
        send_message(
            &mut ws_ide,
            &EditorMessage {
                id: 0,
                message: EditorMessageContents::Result(Ok(ResultOkTypes::Void)),
            },
        )
        .await;

        // 3. Fetch a non-existent file and verify the response returns an error.
        //
        // Do this is a thread, since the request generates a message that requires a response in order to complete.
        let test_dir_thread = test_dir.clone();
        let join_handle = thread::spawn(move || {
            assert_eq!(
                minreq::get(format!(
                    "http://localhost:8080/vsc/fs/test-connection-id3/{}/none.py",
                    test_dir_thread.to_str().unwrap()
                ))
                .send()
                .unwrap()
                .status_code,
                404
            )
        });

        // This should produce a `LoadFile` message.
        //
        // Message ids: IDE - 200, Server - 1->2, Client - 1000.
        let em = read_message(&mut ws_ide).await;
        let msg = cast!(em.message, EditorMessageContents::LoadFile);
        assert_ends_with!(msg.to_string_lossy(), "/none.py");
        assert_eq!(em.id, 1);

        // Reply to the `LoadFile` message -- the file isn't present.
        send_message(
            &mut ws_ide,
            &EditorMessage {
                id: 1,
                message: EditorMessageContents::Result(Ok(ResultOkTypes::LoadFile(None))),
            },
        )
        .await;

        // This should cause the request to complete.
        join_handle.join().unwrap();

        // Create a websocket to emulate the client.
        let (mut ws_client, _) = connect_async(format!(
            "ws://{IP_ADDRESS}:{IP_PORT}/vsc/ws-client/test-connection-id3"
        ))
        .await
        .expect("Failed to connect");

        // 4. Send a `CurrentFile` message with a file to edit that exists only in the IDE.
        //
        // Message ids: IDE - 200->300, Server - 2, Client - 1000.
        send_message(
            &mut ws_ide,
            &EditorMessage {
                id: 200,
                message: EditorMessageContents::CurrentFile(format!(
                    "{}/only-in-ide.py",
                    test_dir.to_str().unwrap()
                )),
            },
        )
        .await;

        // This should be passed to the Client.
        let em = read_message(&mut ws_client).await;
        assert_eq!(em.id, 200);
        assert_ends_with!(
            cast!(&em.message, EditorMessageContents::CurrentFile),
            "/only-in-ide.py"
        );

        // The Client should send a response.
        send_message(
            &mut ws_client,
            &EditorMessage {
                id: 200,
                message: EditorMessageContents::Result(Ok(ResultOkTypes::Void)),
            },
        )
        .await;

        // The IDE should receive it.
        let em = read_message(&mut ws_ide).await;
        assert_eq!(
            em,
            EditorMessage {
                id: 200,
                message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
            }
        );

        // 5. The Client should send a GET request for this file.
        let test_dir_thread = test_dir.clone();
        let join_handle = thread::spawn(move || {
            assert_eq!(
                minreq::get(format!(
                    "http://localhost:8080/vsc/fs/test-connection-id3/{}/only-in-ide.py",
                    test_dir_thread.to_str().unwrap()
                ))
                .send()
                .unwrap()
                .status_code,
                200
            )
        });

        // This should produce a `LoadFile` message.
        //
        // Message ids: IDE - 200, Server - 2->3, Client - 1000.
        let em = read_message(&mut ws_ide).await;
        let msg = cast!(em.message, EditorMessageContents::LoadFile);
        assert_ends_with!(msg.to_string_lossy(), "/only-in-ide.py");
        assert_eq!(em.id, 2);

        // Reply to the `LoadFile` message with the file's contents.
        send_message(
            &mut ws_ide,
            &EditorMessage {
                id: 2,
                message: EditorMessageContents::Result(Ok(ResultOkTypes::LoadFile(Some(
                    "testing".to_string(),
                )))),
            },
        )
        .await;
        join_handle.join().unwrap();

        // This should also produce an `Update` message.
        //
        // Message ids: IDE - 200, Server - 3, Client - 1000.
        let em = read_message(&mut ws_client).await;
        assert_eq!(
            em,
            EditorMessage {
                id: 2,
                message: EditorMessageContents::Update(UpdateMessageContents {
                    contents: Some(CodeChatForWeb {
                        metadata: SourceFileMetadata {
                            mode: "python".to_string(),
                        },
                        source: CodeMirror {
                            doc: "testing".to_string(),
                            doc_blocks: vec![],
                        },
                    }),
                    cursor_position: None,
                    scroll_position: None,
                })
            }
        );
        send_message(
            &mut ws_client,
            &EditorMessage {
                id: 2,
                message: EditorMessageContents::Result(Ok(ResultOkTypes::Void)),
            },
        )
        .await;

        // 5. Send an `Update` message with the contents of this file.
        send_message(
            &mut ws_ide,
            &EditorMessage {
                id: 3,
                message: EditorMessageContents::Update(UpdateMessageContents {
                    contents: Some(CodeChatForWeb {
                        metadata: SourceFileMetadata {
                            mode: "python".to_string(),
                        },
                        source: CodeMirror {
                            doc: "print('Hello, world!')".to_string(),
                            doc_blocks: vec![],
                        },
                    }),
                    cursor_position: None,
                    scroll_position: None,
                }),
            },
        )
        .await;

        /*
        // This should become one update to load the correct URL/directory, then another with the actual file contents.
        let em = read_message(&mut ws_client).await;
        assert_eq!(
            cast!(em.message, EditorMessageContents::Update),
            UpdateMessageContents {
                contents: None,
                cursor_position: None,
                scroll_position: None,
            }
        );
        */

        check_logger_errors(0);
        // Report any errors produced when removing the temporary directory.
        temp_dir.close().unwrap();
    }
}
