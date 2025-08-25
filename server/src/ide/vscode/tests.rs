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
    io::Error,
    net::SocketAddr,
    path::PathBuf,
    time::{Duration, SystemTime},
};

use actix_rt::task::JoinHandle;
use assert_fs::TempDir;
use assertables::assert_starts_with;
use futures_util::{SinkExt, StreamExt};
use lazy_static::lazy_static;
use minreq;
use pretty_assertions::assert_eq;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
    select,
    time::sleep,
};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async, tungstenite::protocol::Message,
};

use crate::translation::{EolType, find_eol_type};
use crate::webserver::{EditorMessage, EditorMessageContents, IdeType, run_server, tests::IP_PORT};
use crate::{
    cast,
    test_utils::{_prep_test_dir, check_logger_errors, configure_testing_logger},
    webserver::ResultOkTypes,
};

// Globals
// -------
lazy_static! {
    // Run a single webserver for all tests.
    static ref WEBSERVER_HANDLE: JoinHandle<Result<(), Error>> =
        actix_rt::spawn(async move { run_server(&SocketAddr::new("127.0.0.1".parse().unwrap(), IP_PORT), None).await });
}

// Send a message via a websocket.
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

// Support functions
// -----------------
//
// Read a message from a websocket.
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

// Open the Client in the VSCode browser. (Although, for testing, the Client
// isn't opened at all.)
//
// Message ids at function end: IDE - 4, Server - 3, Client - 2.
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

// Perform all the setup for testing the Server via IDE and Client websockets.
// This should be invoked by the `prep_test!` macro; otherwise, test files won't
// be found.
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
            .is_ok()
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

// This calls `_prep_test` with the current function name. It must be a macro,
// so that it's called with the test function's name; calling it inside
// `_prep_test` would give the wrong name.
macro_rules! prep_test {
    ($connection_id: ident) => {{
        use crate::function_name;
        _prep_test($connection_id, function_name!())
    }};
}

// Tests
// -----
//
// Test incorrect inputs: two connections with the same ID, sending the wrong
// first message.
// Close the IDE.
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
