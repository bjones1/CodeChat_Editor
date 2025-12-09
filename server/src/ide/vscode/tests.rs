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
/// `test.rs` -- Unit tests for the vscode interface
/// ================================================
// Imports
// -------
use std::{
    env,
    fs::{self, File},
    io::{Error, Read},
    net::SocketAddr,
    path::{self, Path, PathBuf},
    thread::{self, JoinHandle},
    time::{Duration, SystemTime},
};

use assert_fs::TempDir;
use assertables::{assert_contains, assert_ends_with, assert_starts_with};
use dunce::simplified;
use futures_util::{SinkExt, StreamExt};
use indoc::indoc;
use lazy_static::lazy_static;
use minreq;
use path_slash::PathExt;
use pretty_assertions::assert_eq;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
    select,
    time::sleep,
};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{http::StatusCode, protocol::Message},
};

use crate::webserver::{
    EditorMessage, EditorMessageContents, INITIAL_CLIENT_MESSAGE_ID, INITIAL_IDE_MESSAGE_ID,
    INITIAL_MESSAGE_ID, IdeType, MESSAGE_ID_INCREMENT, ResultErrTypes,
};
use crate::{
    cast,
    processing::{
        CodeChatForWeb, CodeMirror, CodeMirrorDiff, CodeMirrorDiffable, CodeMirrorDocBlock,
        CodeMirrorDocBlockTransaction, SourceFileMetadata, StringDiff,
    },
    test_utils::{_prep_test_dir, check_logger_errors, configure_testing_logger},
    webserver::{ResultOkTypes, UpdateMessageContents, drop_leading_slash},
};
use crate::{
    translation::{EolType, find_eol_type},
    webserver::main,
};

// Globals
// -------
lazy_static! {
    // Run a single webserver for all tests.
    static ref WEBSERVER_HANDLE: JoinHandle<Result<(), Error>> =
        thread::spawn(|| {
            main(
                None,
                &SocketAddr::new("127.0.0.1".parse().unwrap(), IP_PORT),
                None,
                log::LevelFilter::Debug
            )
        });
}

// Constants
// ---------
/// The default port on which the server listens for incoming connections.
pub const IP_PORT: u16 = 8080;

// Support functions
// -----------------
/// Send a message via a websocket.
async fn send_message<S: AsyncRead + AsyncWrite + Unpin>(
    ws_stream: &mut WebSocketStream<S>,
    message: &EditorMessage,
) {
    ws_stream
        .send(Message::Text(
            serde_json::to_string(message).unwrap().into(),
        ))
        .await
        .unwrap();
}

/// Read a message from a websocket.
async fn read_message<S: AsyncRead + AsyncWrite + Unpin>(
    ws_stream: &mut WebSocketStream<S>,
) -> EditorMessage {
    let now = SystemTime::now();
    let msg_txt = loop {
        let msg = select! {
            data = ws_stream.next() => data.unwrap().unwrap(),
            _ = sleep(Duration::from_secs(3) - now.elapsed().unwrap()) => panic!("Timeout waiting for message")
        };
        match msg {
            Message::Close(_) => panic!("Unexpected close message."),
            Message::Ping(_) => ws_stream.send(Message::Pong(vec![].into())).await.unwrap(),
            Message::Pong(_) => panic!("Unexpected pong message."),
            Message::Text(txt) => break txt,
            Message::Binary(_) => panic!("Unexpected binary message."),
            Message::Frame(_) => panic!("Unexpected frame message."),
        }
    };
    serde_json::from_str(&msg_txt)
        .unwrap_or_else(|_| panic!("Unable to convert '{msg_txt}' to JSON."))
}

type WebSocketStreamTcp = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn connect_async_server(prefix: &str, connection_id: &str) -> WebSocketStreamTcp {
    connect_async(format!("ws://127.0.0.1:{IP_PORT}{prefix}/{connection_id}",))
        .await
        .expect("Failed to connect")
        .0
}

async fn connect_async_ide(connection_id: &str) -> WebSocketStreamTcp {
    connect_async_server("/vsc/ws-ide", connection_id).await
}

async fn connect_async_client(connection_id: &str) -> WebSocketStreamTcp {
    connect_async_server("/vsc/ws-client", connection_id).await
}

/// Open the Client in the VSCode browser. (Although, for testing, the Client
/// isn't opened at all.)
///
/// Message ids at function end: IDE - 4, Server - 3, Client - 2.
async fn open_client<S: AsyncRead + AsyncWrite + Unpin>(ws_ide: &mut WebSocketStream<S>) {
    // 1.  Send the `Opened` message.
    //
    // Message ids: IDE - 1->4, Server - 0, Client - 2.
    send_message(
        ws_ide,
        &EditorMessage {
            id: 1.0,
            message: EditorMessageContents::Opened(IdeType::VSCode(true)),
        },
    )
    .await;

    // Get the response. It should be success.
    assert_eq!(
        read_message(ws_ide).await,
        EditorMessage {
            id: 1.0,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void)),
        }
    );

    // 2.  Next, wait for the next message -- the HTML.
    //
    // Message ids: IDE - 4, Server - 0->3, Client - 2.
    let em = read_message(ws_ide).await;
    assert_starts_with!(
        cast!(&em.message, EditorMessageContents::ClientHtml),
        "<!DOCTYPE html>"
    );
    assert_eq!(em.id, 0.0);

    // Send a success response to this message.
    send_message(
        ws_ide,
        &EditorMessage {
            id: 0.0,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void)),
        },
    )
    .await;
}

/// Perform all the setup for testing the Server via IDE and Client websockets.
/// This should be invoked by the `prep_test!` macro; otherwise, test files
/// won't be found.
async fn _prep_test(
    connection_id: &str,
    test_full_name: &str,
) -> (TempDir, PathBuf, WebSocketStreamTcp, WebSocketStreamTcp) {
    configure_testing_logger();
    let (temp_dir, test_dir) = _prep_test_dir(test_full_name);
    // Ensure the webserver is running.
    let _ = &*WEBSERVER_HANDLE;
    let now = SystemTime::now();
    while now.elapsed().unwrap().as_millis() < 100 {
        if minreq::get(format!("http://127.0.0.1:{IP_PORT}/ping",))
            .send()
            .is_ok_and(|response| response.as_bytes() == b"pong")
        {
            break;
        }
        sleep(Duration::from_millis(10)).await;
    }

    // Connect to the VSCode IDE websocket.
    let ws_ide = connect_async_ide(connection_id).await;
    let ws_client = connect_async_client(connection_id).await;

    (temp_dir, test_dir, ws_ide, ws_client)
}

/// This calls `_prep_test` with the current function name. It must be a macro,
/// so that it's called with the test function's name; calling it inside
/// `_prep_test` would give the wrong name.
macro_rules! prep_test {
    ($connection_id: ident) => {{
        use crate::function_name;
        _prep_test($connection_id, function_name!())
    }};
}

// Tests
// -----
/// Test incorrect inputs: two connections with the same ID, sending the wrong
/// first message.
#[actix_web::test]
async fn test_vscode_ide_websocket1() {
    let connection_id = "test-connection-id1";
    let (_, _, mut ws_ide, _) = prep_test!(connection_id).await;

    // Start a second connection; verify that it fails.
    let err = connect_async(format!(
        "ws://127.0.0.1:{IP_PORT}/vsc/ws-ide/{connection_id}",
    ))
    .await
    .expect_err("Should fail to connect");
    let response = cast!(err, tokio_tungstenite::tungstenite::Error::Http);
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Note: we can't check the logs, since the server runs in a separate
    // thread. Changing the logger to log across threads means we get logs from
    // other tests (which run in parallel by default). The benefit of running
    // all tests single-threaded plus fixing the logger is low.
    //
    // Send a message that's not an `Opened` message.
    send_message(
        &mut ws_ide,
        &EditorMessage {
            id: INITIAL_IDE_MESSAGE_ID,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: "".to_string(),
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

    matches!(cast!(&result, Err), ResultErrTypes::ClientIllegalMessage);

    // Next, expect the websocket to be closed.
    let err = &ws_ide.next().await.unwrap().unwrap();
    assert_eq!(*err, Message::Close(None));

    check_logger_errors(0);
}

/// Test opening the Client in an external browser.
#[actix_web::test]
async fn test_vscode_ide_websocket2() {
    // Running this in CI on Windows causes the test suite to never exit. Avoid
    // this. See the
    // [docs](https://docs.github.com/en/actions/reference/workflows-and-actions/variables)
    // on GitHub Actions environment variables.
    if env::var("RUNNER_OS") == Ok("Windows".to_string()) {
        // TODO: call a function here which tells the test running we're
        // skipping this test.
        return;
    }

    let connection_id = "test-connection-id2";
    let (_, _, mut ws_ide, _) = prep_test!(connection_id).await;

    // Send the `Opened` message.
    send_message(
        &mut ws_ide,
        &EditorMessage {
            id: INITIAL_IDE_MESSAGE_ID,
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

/// Fetch a non-existent file and verify the response returns an error.
#[actix_web::test]
async fn test_vscode_ide_websocket3() {
    let connection_id = "test-connection-id3";
    let (temp_dir, test_dir, mut ws_ide, _) = prep_test!(connection_id).await;
    open_client(&mut ws_ide).await;

    let file_path = test_dir.join("none.py");
    let file_path_str = drop_leading_slash(&file_path.to_slash().unwrap()).to_string();

    // Since we expect a 404 error, wait some to ensure the 404 results from not
    // finding the requested file, instead of the web server not being fully
    // ready.
    sleep(Duration::from_millis(500)).await;

    // Do this is a thread, since the request generates a message that requires
    // a response in order to complete.
    let file_path_str_thread = file_path_str.clone();
    let join_handle = thread::spawn(move || {
        assert_eq!(
            minreq::get(format!(
                "http://localhost:{IP_PORT}/vsc/fs/{connection_id}/{file_path_str_thread}",
            ))
            .send()
            .unwrap()
            .status_code,
            404
        )
    });

    // The HTTP request produces a `LoadFile` message.
    //
    // Message ids: IDE - 0, Server - 1->2, Client - 0.
    let em = read_message(&mut ws_ide).await;
    let msg = cast!(em.message, EditorMessageContents::LoadFile);
    // Compare these as strings -- we want to ensure the path separator is
    // correct for the current platform.
    assert_eq!(file_path.to_string_lossy(), msg.to_string_lossy());
    assert_eq!(em.id, INITIAL_MESSAGE_ID + MESSAGE_ID_INCREMENT);

    // Reply to the `LoadFile` message -- the file isn't present.
    send_message(
        &mut ws_ide,
        &EditorMessage {
            id: INITIAL_IDE_MESSAGE_ID,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::LoadFile(None))),
        },
    )
    .await;

    // This should cause the HTTP request to complete by receiving the response
    // (file not found).
    join_handle.join().unwrap();

    check_logger_errors(0);
    // Report any errors produced when removing the temporary directory.
    temp_dir.close().unwrap();
}

/// Fetch a file that exists, but using backslashes. This should still fail,
/// even on Windows.
#[actix_web::test]
async fn test_vscode_ide_websocket3a() {
    let connection_id = "test-connection-id3a";
    let (temp_dir, test_dir, mut ws_ide, _) = prep_test!(connection_id).await;
    open_client(&mut ws_ide).await;

    let file_path = test_dir.join("test.py");
    // Force the path separator to be Window-style for this test, even on
    // non-Windows platforms.
    let file_path_str = file_path.to_str().unwrap().to_string().replace("/", "\\");

    // Since we expect a 404 error, wait some to ensure the 404 results from not
    // finding the requested file, instead of the web server not being fully
    // ready.
    sleep(Duration::from_millis(500)).await;

    // Do this is a thread, since the request generates a message that requires
    // a response in order to complete.
    let file_path_str_thread = file_path_str.clone();
    let join_handle = thread::spawn(move || {
        assert_eq!(
            minreq::get(format!(
                "http://localhost:{IP_PORT}/vsc/fs/{connection_id}/{file_path_str_thread}",
            ))
            .send()
            .unwrap()
            .status_code,
            404
        )
    });

    // The HTTP request produces a `LoadFile` message.
    //
    // Message ids: IDE - 0, Server - 0->2, Client - 0.
    let em = read_message(&mut ws_ide).await;
    cast!(em.message, EditorMessageContents::LoadFile);
    // Skip comparing the file names, due to the backslash encoding.
    assert_eq!(em.id, INITIAL_MESSAGE_ID + MESSAGE_ID_INCREMENT);

    // Reply to the `LoadFile` message -- the file isn't present.
    send_message(
        &mut ws_ide,
        &EditorMessage {
            id: INITIAL_IDE_MESSAGE_ID,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::LoadFile(None))),
        },
    )
    .await;

    // This should cause the HTTP request to complete by receiving the response
    // (file not found).
    join_handle.join().unwrap();

    check_logger_errors(0);
    // Report any errors produced when removing the temporary directory.
    temp_dir.close().unwrap();
}

/// Send a `CurrentFile` message with a file to edit that exists only in the
/// IDE.
#[actix_web::test]
async fn test_vscode_ide_websocket8() {
    let connection_id = "test-connection-id8";
    let (temp_dir, test_dir, mut ws_ide, mut ws_client) = prep_test!(connection_id).await;
    open_client(&mut ws_ide).await;

    // Message ids: IDE - 0->1, Server - 0, Client - 0.
    let file_path = test_dir.join("only-in-ide.py");
    let file_path_str = file_path.to_str().unwrap().to_string();
    send_message(
        &mut ws_ide,
        &EditorMessage {
            id: INITIAL_IDE_MESSAGE_ID,
            message: EditorMessageContents::CurrentFile(file_path_str.clone(), None),
        },
    )
    .await;

    // This should be passed to the Client.
    let em = read_message(&mut ws_client).await;
    assert_eq!(em.id, INITIAL_IDE_MESSAGE_ID);
    assert_ends_with!(
        cast!(
            &em.message,
            EditorMessageContents::CurrentFile,
            file_name,
            is_text
        )
        .0,
        "/only-in-ide.py"
    );

    // The Client should send a response.
    send_message(
        &mut ws_client,
        &EditorMessage {
            id: INITIAL_IDE_MESSAGE_ID,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void)),
        },
    )
    .await;

    // The IDE should receive it.
    assert_eq!(
        read_message(&mut ws_ide).await,
        EditorMessage {
            id: INITIAL_IDE_MESSAGE_ID,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );

    // The Client should send a GET request for this file.
    let file_path_thread = file_path.clone();
    let join_handle = thread::spawn(move || {
        assert_eq!(
            minreq::get(format!(
                "http://localhost:{IP_PORT}/vsc/fs/{connection_id}/{}",
                drop_leading_slash(&file_path_thread.to_slash().unwrap())
            ))
            .send()
            .unwrap()
            .status_code,
            200
        )
    });

    // This should produce a `LoadFile` message.
    //
    // Message ids: IDE - 1, Server - 1->2, Client - 0.
    let em = read_message(&mut ws_ide).await;
    let msg = cast!(em.message, EditorMessageContents::LoadFile);
    assert_eq!(
        path::absolute(Path::new(&msg)).unwrap(),
        path::absolute(&file_path).unwrap()
    );
    assert_eq!(em.id, INITIAL_MESSAGE_ID + MESSAGE_ID_INCREMENT);

    // Reply to the `LoadFile` message with the file's contents.
    send_message(
        &mut ws_ide,
        &EditorMessage {
            id: INITIAL_MESSAGE_ID + MESSAGE_ID_INCREMENT,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::LoadFile(Some((
                "# testing".to_string(),
                0.0,
            ))))),
        },
    )
    .await;
    join_handle.join().unwrap();

    // This should also produce an `Update` message sent from the Server.
    //
    // Message ids: IDE - 1, Server - 2->3, Client - 0.
    assert_eq!(
        read_message(&mut ws_client).await,
        EditorMessage {
            id: INITIAL_MESSAGE_ID + 2.0 * MESSAGE_ID_INCREMENT,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: file_path_str.clone(),
                contents: Some(CodeChatForWeb {
                    metadata: SourceFileMetadata {
                        mode: "python".to_string(),
                    },
                    source: CodeMirrorDiffable::Plain(CodeMirror {
                        doc: "\n".to_string(),
                        doc_blocks: vec![CodeMirrorDocBlock {
                            from: 0,
                            to: 1,
                            indent: "".to_string(),
                            delimiter: "#".to_string(),
                            contents: "<p>testing</p>\n".to_string()
                        }],
                    }),
                    version: 0.0,
                }),
                cursor_position: None,
                scroll_position: None,
            })
        }
    );
    send_message(
        &mut ws_client,
        &EditorMessage {
            id: INITIAL_MESSAGE_ID + 2.0 * MESSAGE_ID_INCREMENT,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void)),
        },
    )
    .await;

    // The message, though a result for the `Update` sent by the Server, will
    // still be echoed back to the IDE.
    assert_eq!(
        read_message(&mut ws_ide).await,
        EditorMessage {
            id: INITIAL_MESSAGE_ID + 2.0 * MESSAGE_ID_INCREMENT,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );

    check_logger_errors(0);
    // Report any errors produced when removing the temporary directory.
    temp_dir.close().unwrap();
}

/// Send an `Update` message from the IDE.
#[actix_web::test]
async fn test_vscode_ide_websocket7() {
    let connection_id = "test-connection-id7";
    let (temp_dir, test_dir, mut ws_ide, mut ws_client) = prep_test!(connection_id).await;
    open_client(&mut ws_ide).await;

    // Set the current file, so a subsequent `Update` message can be translated.
    //
    // Message ids: IDE - 0, Server - 1, Client - 0->1.
    let file_path = test_dir.join("test.py");
    let file_path_str = file_path.to_str().unwrap().to_string();
    send_message(
        &mut ws_client,
        &EditorMessage {
            id: INITIAL_CLIENT_MESSAGE_ID,
            message: EditorMessageContents::CurrentFile(
                format!(
                    "http://localhost:{IP_PORT}/vsc/fs/{connection_id}/{}",
                    &file_path.to_slash().unwrap(),
                ),
                None,
            ),
        },
    )
    .await;
    let em = read_message(&mut ws_ide).await;
    let (cf, is_text) = cast!(
        em.message,
        EditorMessageContents::CurrentFile,
        file_name,
        is_text
    );
    assert_eq!(path::absolute(Path::new(&cf)).unwrap(), file_path);
    // Since the file doesn't exist, it's classified as binary by default.
    assert_eq!(is_text, Some(false));
    assert_eq!(em.id, INITIAL_CLIENT_MESSAGE_ID);

    send_message(
        &mut ws_ide,
        &EditorMessage {
            id: INITIAL_CLIENT_MESSAGE_ID,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void)),
        },
    )
    .await;
    assert_eq!(
        read_message(&mut ws_client).await,
        EditorMessage {
            id: INITIAL_CLIENT_MESSAGE_ID,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );

    // Send an `Update` message.
    //
    // Message ids: IDE - 0->1, Server - 1, Client - 1.
    send_message(
        &mut ws_ide,
        &EditorMessage {
            id: INITIAL_IDE_MESSAGE_ID,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: file_path_str.clone(),
                contents: Some(CodeChatForWeb {
                    metadata: SourceFileMetadata {
                        mode: "python".to_string(),
                    },
                    source: CodeMirrorDiffable::Plain(CodeMirror {
                        doc: "# more".to_string(),
                        doc_blocks: vec![],
                    }),
                    version: 0.0,
                }),
                cursor_position: None,
                scroll_position: None,
            }),
        },
    )
    .await;
    assert_eq!(
        read_message(&mut ws_client).await,
        EditorMessage {
            id: INITIAL_IDE_MESSAGE_ID,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: file_path_str.clone(),
                contents: Some(CodeChatForWeb {
                    metadata: SourceFileMetadata {
                        mode: "python".to_string(),
                    },
                    source: CodeMirrorDiffable::Plain(CodeMirror {
                        doc: "\n".to_string(),
                        doc_blocks: vec![CodeMirrorDocBlock {
                            from: 0,
                            to: 1,
                            indent: "".to_string(),
                            delimiter: "#".to_string(),
                            contents: "<p>more</p>\n".to_string()
                        }]
                    }),
                    version: 0.0,
                }),
                cursor_position: None,
                scroll_position: None,
            })
        }
    );
    send_message(
        &mut ws_client,
        &EditorMessage {
            id: INITIAL_IDE_MESSAGE_ID,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void)),
        },
    )
    .await;
    assert_eq!(
        read_message(&mut ws_ide).await,
        EditorMessage {
            id: INITIAL_IDE_MESSAGE_ID,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );

    // Send a message with an update that produces a diff.
    //
    // Message ids: IDE - 1->2, Server - 1, Client - 1.
    send_message(
        &mut ws_ide,
        &EditorMessage {
            id: INITIAL_IDE_MESSAGE_ID + 2.0 * MESSAGE_ID_INCREMENT,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: file_path_str.clone(),
                contents: Some(CodeChatForWeb {
                    metadata: SourceFileMetadata {
                        mode: "python".to_string(),
                    },
                    source: CodeMirrorDiffable::Plain(CodeMirror {
                        doc: indoc!(
                            "
                            # more
                            code
                            # most"
                        )
                        .to_string(),
                        doc_blocks: vec![],
                    }),
                    version: 1.0,
                }),
                cursor_position: None,
                scroll_position: None,
            }),
        },
    )
    .await;
    assert_eq!(
        read_message(&mut ws_client).await,
        EditorMessage {
            id: INITIAL_IDE_MESSAGE_ID + 2.0 * MESSAGE_ID_INCREMENT,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: file_path_str.clone(),
                contents: Some(CodeChatForWeb {
                    metadata: SourceFileMetadata {
                        mode: "python".to_string(),
                    },
                    source: CodeMirrorDiffable::Diff(CodeMirrorDiff {
                        doc: vec![StringDiff {
                            from: 1,
                            to: None,
                            insert: "code\n\n".to_string()
                        }],
                        doc_blocks: vec![CodeMirrorDocBlockTransaction::Add(CodeMirrorDocBlock {
                            from: 6,
                            to: 7,
                            indent: "".to_string(),
                            delimiter: "#".to_string(),
                            contents: "<p>most</p>\n".to_string()
                        })],
                        version: 0.0,
                    }),
                    version: 1.0,
                }),
                cursor_position: None,
                scroll_position: None,
            })
        }
    );
    send_message(
        &mut ws_client,
        &EditorMessage {
            id: INITIAL_IDE_MESSAGE_ID + 2.0 * MESSAGE_ID_INCREMENT,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void)),
        },
    )
    .await;
    assert_eq!(
        read_message(&mut ws_ide).await,
        EditorMessage {
            id: INITIAL_IDE_MESSAGE_ID + 2.0 * MESSAGE_ID_INCREMENT,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );

    check_logger_errors(0);
    // Report any errors produced when removing the temporary directory.
    temp_dir.close().unwrap();
}

/// Send an `Update` message from the Client.
#[actix_web::test]
async fn test_vscode_ide_websocket6() {
    let connection_id = "test-connection-id6";
    let (temp_dir, test_dir, mut ws_ide, mut ws_client) = prep_test!(connection_id).await;
    open_client(&mut ws_ide).await;

    // Message ids: IDE - 0, Server - 1, Client - 0->1.
    let file_path = test_dir.join("foo.py").to_string_lossy().to_string();
    send_message(
        &mut ws_client,
        &EditorMessage {
            id: INITIAL_CLIENT_MESSAGE_ID,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: file_path.clone(),
                contents: Some(CodeChatForWeb {
                    metadata: SourceFileMetadata {
                        mode: "python".to_string(),
                    },
                    source: CodeMirrorDiffable::Plain(CodeMirror {
                        doc: "\n".to_string(),
                        doc_blocks: vec![CodeMirrorDocBlock {
                            from: 0,
                            to: 1,
                            indent: "".to_string(),
                            delimiter: "#".to_string(),
                            contents: "less\n".to_string(),
                        }],
                    }),
                    version: 0.0,
                }),
                cursor_position: None,
                scroll_position: None,
            }),
        },
    )
    .await;
    assert_eq!(
        read_message(&mut ws_ide).await,
        EditorMessage {
            id: INITIAL_CLIENT_MESSAGE_ID,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path,
                contents: Some(CodeChatForWeb {
                    metadata: SourceFileMetadata {
                        mode: "python".to_string(),
                    },
                    source: CodeMirrorDiffable::Plain(CodeMirror {
                        doc: "# less\n".to_string(),
                        doc_blocks: vec![],
                    }),
                    version: 0.0,
                }),
                cursor_position: None,
                scroll_position: None,
            })
        }
    );
    send_message(
        &mut ws_ide,
        &EditorMessage {
            id: INITIAL_CLIENT_MESSAGE_ID,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void)),
        },
    )
    .await;
    assert_eq!(
        read_message(&mut ws_client).await,
        EditorMessage {
            id: INITIAL_CLIENT_MESSAGE_ID,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );

    check_logger_errors(0);
    // Report any errors produced when removing the temporary directory.
    temp_dir.close().unwrap();
}

/// Send a `CurrentFile` message from the Client, requesting a file that exists
/// on disk, but not in the IDE.
#[actix_web::test]
async fn test_vscode_ide_websocket4() {
    let connection_id = "test-connection-id4";
    let (temp_dir, test_dir, mut ws_ide, mut ws_client) = prep_test!(connection_id).await;
    open_client(&mut ws_ide).await;

    // Message ids: IDE - 0, Server - 1, Client - 0->1.
    let file_path_temp = fs::canonicalize(test_dir.join("test.py")).unwrap();
    let file_path = simplified(&file_path_temp);
    let file_path_str = file_path.to_str().unwrap().to_string();
    send_message(
        &mut ws_client,
        &EditorMessage {
            id: INITIAL_CLIENT_MESSAGE_ID,
            message: EditorMessageContents::CurrentFile(
                format!(
                    "http://localhost:{IP_PORT}/vsc/fs/{connection_id}/{}",
                    &file_path.to_slash().unwrap()
                ),
                None,
            ),
        },
    )
    .await;
    assert_eq!(
        read_message(&mut ws_ide).await,
        EditorMessage {
            id: INITIAL_CLIENT_MESSAGE_ID,
            message: EditorMessageContents::CurrentFile(file_path_str.clone(), Some(true))
        }
    );

    send_message(
        &mut ws_ide,
        &EditorMessage {
            id: INITIAL_CLIENT_MESSAGE_ID,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void)),
        },
    )
    .await;
    assert_eq!(
        read_message(&mut ws_client).await,
        EditorMessage {
            id: INITIAL_CLIENT_MESSAGE_ID,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );

    // The Client should send a GET request for this file.
    let test_dir_thread = test_dir.clone();
    let join_handle = thread::spawn(move || {
        // Get the file itself.
        assert_eq!(
            minreq::get(format!(
                "http://localhost:{IP_PORT}/vsc/fs/{connection_id}/{}/{}",
                test_dir_thread.to_slash().unwrap(),
                // On Windows, send incorrect case for this file; the server
                // should correct it.
                if cfg!(windows) { "Test.py" } else { "test.py" }
            ))
            .send()
            .unwrap()
            .status_code,
            200
        );
    });

    // This should produce a `LoadFile` message.
    //
    // Message ids: IDE - 0, Server - 1->2, Client - 1.
    let em = read_message(&mut ws_ide).await;
    let msg = cast!(em.message, EditorMessageContents::LoadFile);
    assert_eq!(fs::canonicalize(&msg).unwrap(), file_path_temp);
    assert_eq!(em.id, INITIAL_MESSAGE_ID + MESSAGE_ID_INCREMENT);

    // Reply to the `LoadFile` message: the IDE doesn't have the file.
    send_message(
        &mut ws_ide,
        &EditorMessage {
            id: INITIAL_MESSAGE_ID + MESSAGE_ID_INCREMENT,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::LoadFile(None))),
        },
    )
    .await;
    join_handle.join().unwrap();

    // This should also produce an `Update` message sent from the Server.
    //
    // Message ids: IDE - 0, Server - 2->3, Client - 0.
    //
    // Since the version is randomly generated, copy that from the received message.
    let msg = read_message(&mut ws_client).await;
    assert_eq!(
        msg,
        EditorMessage {
            id: INITIAL_MESSAGE_ID + 2.0 * MESSAGE_ID_INCREMENT,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: file_path_str.clone(),
                contents: Some(CodeChatForWeb {
                    metadata: SourceFileMetadata {
                        mode: "python".to_string(),
                    },
                    source: CodeMirrorDiffable::Plain(CodeMirror {
                        doc: "\n".to_string(),
                        doc_blocks: vec![CodeMirrorDocBlock {
                            from: 0,
                            to: 1,
                            indent: "".to_string(),
                            delimiter: "#".to_string(),
                            contents: "<p>test.py</p>\n".to_string()
                        }],
                    }),
                    version: cast!(&msg.message, EditorMessageContents::Update)
                        .contents
                        .as_ref()
                        .unwrap()
                        .version,
                }),
                cursor_position: None,
                scroll_position: None,
            })
        }
    );
    send_message(
        &mut ws_client,
        &EditorMessage {
            id: INITIAL_MESSAGE_ID + 2.0 * MESSAGE_ID_INCREMENT,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void)),
        },
    )
    .await;
    assert_eq!(
        read_message(&mut ws_ide).await,
        EditorMessage {
            id: INITIAL_MESSAGE_ID + 2.0 * MESSAGE_ID_INCREMENT,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void)),
        }
    );

    // Simulate a related fetch for a project -- the `toc.md` file.
    let test_dir_thread = test_dir.clone();
    let join_handle = thread::spawn(move || {
        assert_eq!(
            minreq::get(format!(
                "http://localhost:{IP_PORT}/vsc/fs/{connection_id}/{}/toc.md",
                test_dir_thread.to_slash().unwrap()
            ))
            .send()
            .unwrap()
            .status_code,
            200
        );
    });

    // This should also produce a `LoadFile` message.
    //
    // Message ids: IDE - 0, Server - 3->4, Client - 0.
    let em = read_message(&mut ws_ide).await;
    let msg = cast!(em.message, EditorMessageContents::LoadFile);
    assert_eq!(
        fs::canonicalize(&msg).unwrap(),
        fs::canonicalize(test_dir.join("toc.md")).unwrap()
    );
    assert_eq!(em.id, INITIAL_MESSAGE_ID + 3.0 * MESSAGE_ID_INCREMENT);

    // Reply to the `LoadFile` message: the IDE doesn't have the file.
    send_message(
        &mut ws_ide,
        &EditorMessage {
            id: INITIAL_MESSAGE_ID + 3.0 * MESSAGE_ID_INCREMENT,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::LoadFile(None))),
        },
    )
    .await;
    join_handle.join().unwrap();

    // What makes sense here? If the IDE didn't load the file, either the Client shouldn't edit it or the Client should switch to using a filewatcher for edits.
    /*
    // Send an update from the Client, which should produce a diff.
    //
    // Message ids: IDE - 0, Server - 4, Client - 0->1.
    send_message(
        &mut ws_client,
        &EditorMessage {
            id: INITIAL_CLIENT_MESSAGE_ID + MESSAGE_ID_INCREMENT,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: file_path_str.clone(),
                contents: Some(CodeChatForWeb {
                    metadata: SourceFileMetadata {
                        mode: "python".to_string(),
                    },
                    source: CodeMirrorDiffable::Plain(CodeMirror {
                        doc: "More\n".to_string(),
                        doc_blocks: vec![CodeMirrorDocBlock {
                            from: 5,
                            to: 6,
                            indent: "".to_string(),
                            delimiter: "#".to_string(),
                            contents: "<p>test.py</p>".to_string(),
                        }],
                    }),
                    version: 1.0,
                }),
                cursor_position: None,
                scroll_position: None,
            }),
        },
    )
    .await;
    assert_eq!(
        read_message(&mut ws_ide).await,
        EditorMessage {
            id: INITIAL_CLIENT_MESSAGE_ID + MESSAGE_ID_INCREMENT,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: file_path_str.clone(),
                contents: Some(CodeChatForWeb {
                    metadata: SourceFileMetadata {
                        mode: "python".to_string(),
                    },
                    source: CodeMirrorDiffable::Diff(CodeMirrorDiff {
                        doc: vec![StringDiff {
                            from: 0,
                            to: None,
                            insert: format!("More{}", if cfg!(windows) { "\r\n" } else { "\n" }),
                        }],
                        doc_blocks: vec![],
                        version: 0.0,
                    }),
                    version: 1.0,
                }),
                cursor_position: None,
                scroll_position: None,
            })
        }
    );
    send_message(
        &mut ws_ide,
        &EditorMessage {
            id: INITIAL_CLIENT_MESSAGE_ID + MESSAGE_ID_INCREMENT,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void)),
        },
    )
    .await;
    assert_eq!(
        read_message(&mut ws_client).await,
        EditorMessage {
            id: INITIAL_CLIENT_MESSAGE_ID + MESSAGE_ID_INCREMENT,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );
    */

    check_logger_errors(0);
    // Report any errors produced when removing the temporary directory.
    temp_dir.close().unwrap();
}

/// Send a `CurrentFile` message from the Client, requesting a binary file that
/// exists on disk, but not in the IDE.
#[actix_web::test]
async fn test_vscode_ide_websocket4a() {
    let connection_id = "test-connection-id4a";
    let (temp_dir, test_dir, mut ws_ide, mut ws_client) = prep_test!(connection_id).await;
    open_client(&mut ws_ide).await;

    // Message ids: IDE - 0, Server - 1, Client - 0->1.
    let hw = "helloworld.pdf";
    let file_path_temp = fs::canonicalize(test_dir.join(hw)).unwrap();
    let file_path = simplified(&file_path_temp);
    send_message(
        &mut ws_client,
        &EditorMessage {
            id: INITIAL_CLIENT_MESSAGE_ID,
            message: EditorMessageContents::CurrentFile(
                format!(
                    "http://localhost:{IP_PORT}/vsc/fs/{connection_id}/{}",
                    &file_path.to_slash().unwrap()
                ),
                None,
            ),
        },
    )
    .await;

    assert_eq!(
        read_message(&mut ws_ide).await,
        EditorMessage {
            id: INITIAL_CLIENT_MESSAGE_ID,
            message: EditorMessageContents::CurrentFile(
                file_path.to_str().unwrap().to_string(),
                // `helloworld.pdf` is a text file! (But perhaps should mark all
                // PDFs as binary, regardless?)
                Some(true)
            )
        }
    );

    send_message(
        &mut ws_ide,
        &EditorMessage {
            id: INITIAL_CLIENT_MESSAGE_ID,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void)),
        },
    )
    .await;
    assert_eq!(
        read_message(&mut ws_client).await,
        EditorMessage {
            id: INITIAL_CLIENT_MESSAGE_ID,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );

    // The Client should send a GET request for this file.
    let mut test_dir_thread = test_dir.clone();
    let join_handle = thread::spawn(move || {
        // Read the file.
        let response = minreq::get(format!(
            "http://localhost:{IP_PORT}/vsc/fs/{connection_id}/{}/{hw}",
            test_dir_thread.to_slash().unwrap(),
        ))
        .send()
        .unwrap();
        assert_eq!(response.status_code, 200);
        // Since this isn't a project, the response should be just the image.
        test_dir_thread.push(hw);
        let mut helloworld_pdf_data = vec![];
        File::open(test_dir_thread)
            .unwrap()
            .read_to_end(&mut helloworld_pdf_data)
            .unwrap();
        assert_eq!(response.as_bytes().to_vec(), helloworld_pdf_data);
    });

    // This should produce a `LoadFile` message.
    //
    // Message ids: IDE - 0, Server - 1->2, Client - 1.
    let em = read_message(&mut ws_ide).await;
    let msg = cast!(em.message, EditorMessageContents::LoadFile);
    assert_eq!(fs::canonicalize(&msg).unwrap(), file_path_temp);
    assert_eq!(em.id, INITIAL_MESSAGE_ID + MESSAGE_ID_INCREMENT);

    // Reply to the `LoadFile` message: the IDE doesn't have the file.
    send_message(
        &mut ws_ide,
        &EditorMessage {
            id: INITIAL_MESSAGE_ID + MESSAGE_ID_INCREMENT,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::LoadFile(None))),
        },
    )
    .await;
    join_handle.join().unwrap();

    check_logger_errors(0);
    // Report any errors produced when removing the temporary directory.
    temp_dir.close().unwrap();
}

/// Send a `CurrentFile` message from the Client, requesting a PDF that exists
/// on disk, but not in the IDE, inside a project.
#[actix_web::test]
async fn test_vscode_ide_websocket4b() {
    let connection_id = "test-connection-id4b";
    let (temp_dir, test_dir, mut ws_ide, mut ws_client) = prep_test!(connection_id).await;
    open_client(&mut ws_ide).await;

    // Message ids: IDE - 0, Server - 1, Client - 0->1.
    let hw = "helloworld.pdf";
    let file_path_temp = fs::canonicalize(test_dir.join(hw)).unwrap();
    let file_path = simplified(&file_path_temp);
    send_message(
        &mut ws_client,
        &EditorMessage {
            id: INITIAL_CLIENT_MESSAGE_ID,
            message: EditorMessageContents::CurrentFile(
                format!(
                    "http://localhost:{IP_PORT}/vsc/fs/{connection_id}/{}",
                    &file_path.to_slash().unwrap()
                ),
                None,
            ),
        },
    )
    .await;

    assert_eq!(
        read_message(&mut ws_ide).await,
        EditorMessage {
            id: INITIAL_CLIENT_MESSAGE_ID,
            message: EditorMessageContents::CurrentFile(
                file_path.to_str().unwrap().to_string(),
                // `helloworld.pdf` is a text file! (But perhaps should mark all
                // PDFs as binary, regardless?)
                Some(true)
            )
        }
    );

    send_message(
        &mut ws_ide,
        &EditorMessage {
            id: INITIAL_CLIENT_MESSAGE_ID,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void)),
        },
    )
    .await;
    assert_eq!(
        read_message(&mut ws_client).await,
        EditorMessage {
            id: INITIAL_CLIENT_MESSAGE_ID,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );

    // The Client should send a GET request for this file.
    let mut test_dir_thread = test_dir.clone();
    let join_handle = thread::spawn(move || {
        // Read the file.
        let response = minreq::get(format!(
            "http://localhost:{IP_PORT}/vsc/fs/{connection_id}/{}/{hw}",
            test_dir_thread.to_slash().unwrap(),
        ))
        .send()
        .unwrap();
        assert_eq!(response.status_code, 200);
        // This is a project; the response should be a Client Simple Viewer.
        assert_contains!(
            response.as_str().unwrap(),
            r#"<iframe src="/static/pdfjs-main.html?"#
        );

        // Now, request the PDF as a raw file.
        let response = minreq::get(format!(
            "http://localhost:{IP_PORT}/vsc/fs/{connection_id}/{}/{hw}?raw",
            test_dir_thread.to_slash().unwrap(),
        ))
        .send()
        .unwrap();
        assert_eq!(response.status_code, 200);
        test_dir_thread.push(hw);
        let mut helloworld_pdf_data = vec![];
        File::open(test_dir_thread)
            .unwrap()
            .read_to_end(&mut helloworld_pdf_data)
            .unwrap();
        assert_eq!(response.as_bytes(), helloworld_pdf_data);
    });

    // This should produce a `LoadFile` message.
    //
    // Message ids: IDE - 0, Server - 1->2, Client - 1.
    let em = read_message(&mut ws_ide).await;
    let msg = cast!(em.message, EditorMessageContents::LoadFile);
    assert_eq!(fs::canonicalize(&msg).unwrap(), file_path_temp);
    assert_eq!(em.id, INITIAL_MESSAGE_ID + MESSAGE_ID_INCREMENT);

    // Reply to the `LoadFile` message: the IDE doesn't have the file.
    send_message(
        &mut ws_ide,
        &EditorMessage {
            id: INITIAL_MESSAGE_ID + MESSAGE_ID_INCREMENT,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::LoadFile(None))),
        },
    )
    .await;
    join_handle.join().unwrap();

    check_logger_errors(0);
    // Report any errors produced when removing the temporary directory.
    temp_dir.close().unwrap();
}

/// Send a `RequestClose` message to the Client, then close the Client.
#[actix_web::test]
async fn test_vscode_ide_websocket5() {
    let connection_id = "test-connection-id5";
    let (temp_dir, _, mut ws_ide, mut ws_client) = prep_test!(connection_id).await;
    open_client(&mut ws_ide).await;

    // Message ids: IDE - 4->7, Server - 3, Client - 2.
    //
    // Send the `RequestClose` message.
    send_message(
        &mut ws_ide,
        &EditorMessage {
            id: 4.0,
            message: EditorMessageContents::RequestClose,
        },
    )
    .await;
    assert_eq!(
        read_message(&mut ws_client).await,
        EditorMessage {
            id: 4.0,
            message: EditorMessageContents::RequestClose
        }
    );
    send_message(
        &mut ws_client,
        &EditorMessage {
            id: 4.0,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void)),
        },
    )
    .await;
    assert_eq!(
        read_message(&mut ws_ide).await,
        EditorMessage {
            id: 4.0,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void)),
        }
    );

    // Close the Client websocket.
    ws_client.close(None).await.unwrap();
    loop {
        match ws_ide.next().await.unwrap().unwrap() {
            Message::Ping(_) => ws_ide.send(Message::Pong(vec![].into())).await.unwrap(),
            Message::Close(_) => break,
            _ => panic!("Unexpected message."),
        }
    }

    check_logger_errors(0);
    // Report any errors produced when removing the temporary directory.
    temp_dir.close().unwrap();
}

/// Close the IDE.
#[actix_web::test]
async fn test_vscode_ide_websocket9() {
    let connection_id = "test-connection-id9";
    let (temp_dir, _, mut ws_ide, mut ws_client) = prep_test!(connection_id).await;
    open_client(&mut ws_ide).await;

    ws_ide.close(None).await.unwrap();
    loop {
        match ws_client.next().await.unwrap().unwrap() {
            Message::Ping(_) => ws_client.send(Message::Pong(vec![].into())).await.unwrap(),
            Message::Close(_) => break,
            _ => panic!("Unexpected message."),
        }
    }

    check_logger_errors(0);
    // Report any errors produced when removing the temporary directory.
    temp_dir.close().unwrap();
}

#[test]
fn test_find_eoltypes() {
    assert_eq!(
        find_eol_type(""),
        if cfg!(windows) {
            EolType::Crlf
        } else {
            EolType::Lf
        }
    );
    assert_eq!(find_eol_type("Testing\nOne, two, three"), EolType::Lf);
    assert_eq!(find_eol_type("Testing\r\nOne, two, three"), EolType::Crlf);
}
