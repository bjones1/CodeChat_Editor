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
mod filewatcher;
mod vscode;

/// ## Imports
///
/// ### Standard library
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, Mutex},
    time::Duration,
};

// ### Third-party
use actix_files;
use actix_web::{
    dev::{ServiceFactory, ServiceRequest},
    error::Error,
    http::header::ContentType,
    web, App, HttpRequest, HttpResponse, HttpServer,
};
use actix_ws::AggregatedMessage;
use bytes::Bytes;
use dunce::simplified;
use futures_util::StreamExt;
use lazy_static::lazy_static;
use log::{error, info, warn};
use log4rs;
use mime::Mime;
use mime_guess;
use path_slash::PathBufExt;
use serde::{Deserialize, Serialize};
use serde_json;
use tokio::{
    select,
    sync::mpsc::{Receiver, Sender},
    sync::oneshot,
    task::JoinHandle,
    time::sleep,
};
use url::Url;
use vscode::{serve_vscode_fs, vscode_client_websocket, vscode_ide_websocket};

// ### Local
use crate::processing::{
    source_to_codechat_for_web_string, CodeChatForWeb, TranslationResultsString,
};
use filewatcher::{
    filewatcher_browser_endpoint, filewatcher_client_endpoint, filewatcher_root_fs_redirect,
    filewatcher_websocket,
};

/// ## Data structures
///
/// ### Data structures supporting a websocket connection between the IDE, this server, and the CodeChat Editor Client
///
/// Provide queues which send data to the IDE and the CodeChat Editor Client.
#[derive(Debug)]
struct WebsocketQueues {
    from_websocket_tx: Sender<EditorMessage>,
    to_websocket_rx: Receiver<EditorMessage>,
}

#[derive(Debug)]
/// Since an `HttpResponse` doesn't implement `Send`, use this as a simply proxy
/// for it. This is used to send a response to the HTTP task to an HTTP request
/// made to that task. Send: String, response
struct ProcessingTaskHttpRequest {
    /// The path of the file requested.
    request_path: PathBuf,
    /// True if this file is a TOC.
    is_toc: bool,
    /// True if test mode is enabled.
    is_test_mode: bool,
    /// A queue to send the response back to the HTTP task.
    response_queue: oneshot::Sender<SimpleHttpResponse>,
}

/// Since an `HttpResponse` doesn't implement `Send`, use this as a proxy to
/// cover all responses to serving a file.
#[derive(Debug)]
enum SimpleHttpResponse {
    /// Return a 200 with the provided string as the HTML body.
    Ok(String),
    /// Return an error (400 status code) with the provided string as the HTML
    /// body.
    Err(String),
    /// Serve the raw file content, using the provided content type.
    Raw(String, Mime),
    /// The file contents are not UTF-8; serve it from the filesystem path
    /// provided.
    Bin(PathBuf),
}

/// Define the data structure used to pass data between the CodeChat Editor
/// Client, the IDE, and the CodeChat Editor Server.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct EditorMessage {
    /// A value unique to this message; it's used to report results
    /// (success/failure) back to the sender.
    id: u32,
    /// The actual message.
    message: EditorMessageContents,
}

/// Define the data structure used to pass data between the CodeChat Editor
/// Client, the CodeChat Editor IDE extension, and the CodeChat Editor Server.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum EditorMessageContents {
    // #### These messages may be sent by either the IDE or the Client.
    /// This sends an update; any missing fields are unchanged. Valid
    /// destinations: IDE, Client.
    Update(UpdateMessageContents),
    /// Specify the current file to edit. Valid destinations: IDE, Client.
    CurrentFile(String),

    // #### These messages may only be sent by the IDE.
    /// This is the first message sent when the IDE starts up. It may only be
    /// sent at startup. Valid destinations: Server.
    Opened(IdeType),
    /// Request the Client to save any unsaved data then close. Valid
    /// destinations: Client.
    RequestClose,

    // #### These messages may only be sent by the Server or the IDE
    /// Ask the IDE if the provided file is loaded. If so, the IDE should
    /// respond by sending a `LoadFile` with the requested file. If not, the
    /// returned `Result` should indicate the error "not loaded". Valid
    /// destinations: IDE.
    LoadFile(PathBuf),

    // #### These messages may only be sent by the Server.
    /// This may only be used to respond to an `Opened` message; it contains the
    /// HTML for the CodeChat Editor Client to display in its built-in browser.
    /// Valid destinations: IDE.
    ClientHtml(String),
    /// Sent when the IDE or Client websocket was closed, indicating that the
    /// unclosed websocket should be closed as well. Therefore, this message
    /// will never be received by the IDE or Client. Valid destinations: Server.
    Closed,

    // #### This message may be sent by anyone.
    /// Sent as a response to any of the above messages, reporting
    /// success/error. None indicates success, while Some contains an error.
    Result(Option<String>, Option<LoadFileResultContents>),
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct LoadFileResultContents {
    /// The path to the file that was queried.
    file_path: PathBuf,
    /// The contents of the file.
    contents: String,
}

/// Specify the type of IDE that this client represents.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum IdeType {
    /// True if the CodeChat Editor will be hosted inside VSCode; false means it
    /// should be hosted in an external browser.
    VSCode(bool),
    /// Another option -- temporary -- to allow for future expansion.
    DeleteMe,
}

/// Contents of the `Update` message.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct UpdateMessageContents {
    /// The contents of this file. TODO: this should be just a string if sent by
    /// the IDE.
    contents: Option<CodeChatForWeb>,
    /// The current cursor position in the file, where 0 = before the first
    /// character in the file and contents.length() = after the last character
    /// in the file. TODO: Selections are not yet supported. TODO: how to get a
    /// cursor location from within a doc block in the Client?
    cursor_position: Option<u32>,
    /// The normalized vertical scroll position in the file, where 0 = top and 1
    /// = bottom.
    scroll_position: Option<f32>,
}

/// ### Data structures used by the webserver
///
/// Define the [state](https://actix.rs/docs/application/#state) available to
/// all endpoints.
pub struct AppState {
    // The number of the next connection ID to assign.
    connection_id: Mutex<u32>,
    // For each connection ID, store a queue tx for the HTTP server to send
    // requests to the processing task for that ID.
    processing_task_queue_tx: Arc<Mutex<HashMap<String, Sender<ProcessingTaskHttpRequest>>>>,
    // For each (connection ID, requested URL) store channel to send the
    // matching response to the HTTP task.
    filewatcher_client_queues: Arc<Mutex<HashMap<String, WebsocketQueues>>>,
    // For each connection ID, store the queues for the VSCode IDE.
    vscode_ide_queues: Arc<Mutex<HashMap<String, WebsocketQueues>>>,
    vscode_client_queues: Arc<Mutex<HashMap<String, WebsocketQueues>>>,
    // Connection IDs that are currently in use.
    vscode_connection_id: Arc<Mutex<HashSet<String>>>,
}

// ## Macros
/// Create a macro to report an error when enqueueing an item.
#[macro_export]
macro_rules! oneshot_send {
    // Provide two options: `break` or `break 'label`.
    ($tx: expr) => {
        if let Err(err) = $tx {
            error!("Unable to enqueue: {err:?}");
            break;
        }
    };
    ($tx: expr, $label: tt) => {
        if let Err(err) = $tx {
            error!("Unable to enqueue: {err:?}");
            break $label;
        }
    };
}

#[macro_export]
macro_rules! queue_send {
    ($tx: expr) => {
        $crate::oneshot_send!($tx.await);
    };
    ($tx: expr, $label: tt) => {
        $crate::oneshot_send!($tx.await, $label);
    };
}

/// ## Globals
/// The IP address on which the server listens for incoming connections.
const IP_ADDRESS: &str = "127.0.0.1";
/// The port on which the server listens for incoming connections.
const IP_PORT: u16 = 8080;

// The timeout for a reply from a websocket. Use a short timeout to speed up
// unit tests.
const REPLY_TIMEOUT: Duration = if cfg!(test) {
    Duration::from_millis(50)
} else {
    Duration::from_millis(2000)
};

/// The time to wait for a pong from the websocket in response to a ping sent by
/// this server.
const WEBSOCKET_PING_DELAY: Duration = Duration::from_secs(2);

lazy_static! {
    // Define the location of static files.
    static ref CLIENT_STATIC_PATH: PathBuf = {
        let exe_path = env::current_exe().unwrap();
        let exe_dir = exe_path.parent().unwrap();
        let mut client_static_path = PathBuf::from(exe_dir);
        // When in debug or running tests, use the layout of the Git repo to find
        // client files. In release mode, we assume the static folder is a
        // subdirectory of the directory containing the executable.
        #[cfg(test)]
        client_static_path.push("..");
        // Note that `debug_assertions` is also enabled for testing, so this adds to
        // the previous line when running tests.
        #[cfg(debug_assertions)]
        client_static_path.push("../../../client");

        client_static_path.push("static");
        client_static_path.canonicalize().unwrap()
    };

    // Read in the hashed names of files bundled by esbuild.
    static ref BUNDLED_FILES_MAP: HashMap<String, String> = {
        let json = fs::read_to_string("hashLocations.json").unwrap();
        let hmm: HashMap<String, String> = serde_json::from_str(&json).unwrap();
        hmm
    };

    // Read in the contents of the CodeChat Editor Framework.
    static ref CODECHAT_EDITOR_FRAMEWORK_JS: String = {
        let mut bfm = CLIENT_STATIC_PATH.clone();
        // The bundled files map start with `static`, so pop that off the client static path to avoid duplication.
        bfm.pop();
        bfm.push(BUNDLED_FILES_MAP.get("CodeChatEditorFramework.js").unwrap());
        fs::read_to_string(bfm).unwrap()
    };

}

/// ## Webserver functionality
/// Return a unique ID for an IDE websocket connection.
fn get_connection_id(app_state: &web::Data<AppState>) -> u32 {
    let mut connection_id = app_state.connection_id.lock().unwrap();
    *connection_id += 1;
    *connection_id
}

// Get the `mode` query parameter to determine `is_test_mode`; default to
// `false`.
pub fn get_test_mode(req: &HttpRequest) -> bool {
    let query_params = web::Query::<HashMap<String, String>>::from_query(req.query_string());
    if let Ok(query) = query_params {
        query.get("test").is_some()
    } else {
        false
    }
}

// Return an instance of the Client.
fn get_client_framework(
    // The HTTP request. Used to extract query parameters to determine if the
    // page is in test mode.
    req: &HttpRequest,
    // The URL prefix for a websocket connection to the Server.
    ide_path: &str,
    // The ID of the websocket connection.
    connection_id: u32, // This returns a response (the Client, or an error).
) -> HttpResponse {
    // Add in content when testing.
    let is_test_mode = get_test_mode(req);

    // Provide the pathname to the websocket connection. Quote the string using
    // JSON to handle any necessary escapes.
    let ws_url = match serde_json::to_string(&format!("{ide_path}/{connection_id}")) {
        Ok(v) => v,
        Err(err) => {
            return html_not_found(&format!(
                "Unable to encode websocket URL for {ide_path}, id {connection_id}: {err}"
            ))
        }
    };

    // Build and return the webpage.
    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"<!DOCTYPE html>
<html lang="en">
    <head>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1">
        <title>The CodeChat Editor</title>
        <script type="module">
            {}
            page_init({ws_url}, {is_test_mode})
        </script>
    </head>
    <body style="margin: 0px; padding: 0px; overflow: hidden">
        <iframe id="CodeChat-iframe"
            style="width:100%; height:100vh; border:none;"
            srcdoc="<!DOCTYPE html>
            <html lang='en'>
                <body style='background-color:#f0f0ff'>
                    <div style='display:flex;justify-content:center;align-items:center;height:95vh;'>
                        <div style='text-align:center;font-family:Trebuchet MS;'>
                            <h1>The CodeChat Editor</h1>
                            <p>Waiting for initial render. Switch the active source code window to begin.</p>
                        </div>
                    </div>
                </body>
            </html>"
        >
        </iframe>
    </body>
</html>
"#, *CODECHAT_EDITOR_FRAMEWORK_JS
        ))
}

// ### Serve file
/// This could be a plain text file (for example, one not recognized as source
/// code that this program supports), a binary file (image/video/etc.), a
/// CodeChat Editor file, or a non-existent file. Determine which type this file
/// is then serve it. Serve a CodeChat Editor Client webpage using the
/// FileWatcher "IDE".
pub async fn filesystem_endpoint(
    path: web::Path<(String, String)>,
    req: &HttpRequest,
    app_state: &web::Data<AppState>,
) -> HttpResponse {
    let (connection_id, file_path) = path.into_inner();
    let request_path = match PathBuf::from_str(&file_path) {
        Ok(v) => v,
        Err(err) => {
            let msg = format!("Error: unable to convert path {file_path}: {err}.");
            error!("{msg}");
            return html_not_found(&msg);
        }
    };

    // Get the `mode` query parameter to determine `is_toc`; default to `false`.
    let query_params: Result<
        web::Query<HashMap<String, String>>,
        actix_web::error::QueryPayloadError,
    > = web::Query::<HashMap<String, String>>::from_query(req.query_string());
    let is_toc = query_params.map_or(false, |query| {
        query.get("mode").map_or(false, |mode| mode == "toc")
    });
    let is_test_mode = get_test_mode(req);

    // Create a one-shot channel used by the processing task to provide a
    // response to this request.
    let (tx, rx) = oneshot::channel();

    let processing_tx = {
        // Get the processing queue; only keep the lock during this block.
        let processing_queue_tx = app_state.processing_task_queue_tx.lock().unwrap();
        let Some(processing_tx) = processing_queue_tx.get(&connection_id) else {
            let msg = format!(
                "Error: no processing task queue for connection id {}.",
                &connection_id
            );
            error!("{msg}");
            return html_not_found(&msg);
        };
        processing_tx.clone()
    };

    // Send it the request.
    if let Err(err) = processing_tx
        .send(ProcessingTaskHttpRequest {
            request_path,
            is_toc,
            is_test_mode,
            response_queue: tx,
        })
        .await
    {
        let msg = format!("Error: unable to enqueue: {err}.");
        error!("{msg}");
        return html_not_found(&msg);
    }

    // Return the response provided by the processing task.
    match rx.await {
        Ok(simple_http_response) => match simple_http_response {
            SimpleHttpResponse::Ok(body) => HttpResponse::Ok()
                .content_type(ContentType::html())
                .body(body),
            SimpleHttpResponse::Err(body) => html_not_found(&body),
            SimpleHttpResponse::Raw(body, content_type) => {
                HttpResponse::Ok().content_type(content_type).body(body)
            }
            SimpleHttpResponse::Bin(path) => {
                match actix_files::NamedFile::open_async(&path).await {
                    Ok(v) => v.into_response(req),
                    Err(err) => html_not_found(&format!("<p>Error opening file {path:?}: {err}.",)),
                }
            }
        },
        Err(err) => html_not_found(&format!("Error: {err}")),
    }
}

async fn serve_file(
    file_path: &Path,
    file_contents: &str,
    is_toc: bool,
    is_current_file: bool,
    is_test_mode: bool,
) -> (SimpleHttpResponse, Option<CodeChatForWeb>) {
    // Provided info from the HTTP request, determine the following parameters.
    let raw_dir = file_path.parent().unwrap();
    // Use a lossy conversion, since this is UI display, not filesystem access.
    let dir = path_display(raw_dir);
    let name = escape_html(&file_path.file_name().unwrap().to_string_lossy());

    // See if this is a CodeChat Editor file.
    let (translation_results_string, path_to_toc) = if is_current_file || is_toc {
        source_to_codechat_for_web_string(file_contents, file_path, is_toc)
    } else {
        // If this isn't the current file, then don't parse it.
        (TranslationResultsString::Unknown, None)
    };
    let is_project = path_to_toc.is_some();
    let codechat_for_web = match translation_results_string {
        // The file type is unknown. Serve it raw.
        TranslationResultsString::Unknown => {
            return (
                SimpleHttpResponse::Raw(
                    file_contents.to_string(),
                    mime_guess::from_path(file_path).first_or_text_plain(),
                ),
                None,
            );
        }
        // Report a lexer error.
        TranslationResultsString::Err(err_string) => {
            return (SimpleHttpResponse::Err(err_string), None)
        }
        // This is a CodeChat file. The following code wraps the CodeChat for
        // web results in a CodeChat Editor Client webpage.
        TranslationResultsString::CodeChat(codechat_for_web) => codechat_for_web,
        TranslationResultsString::Toc(html) => {
            // The TOC is a simplified web page which requires no additional
            // processing.
            return (
                SimpleHttpResponse::Ok(format!(
                    r#"<!DOCTYPE html>
<html lang="en">
    <head>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1">
        <title>{name} - The CodeChat Editor</title>

        <link rel="stylesheet" href="/static/css/CodeChatEditor.css">
        <link rel="stylesheet" href="/static/css/CodeChatEditorSidebar.css">
        </script>
    </head>
    <body>
        {html}
    </body>
</html>"#
                )),
                None,
            );
        }
    };

    // For project files, add in the sidebar. Convert this from a Windows path
    // to a Posix path if necessary.
    let (sidebar_iframe, sidebar_css) = if is_project {
        (
            format!(
                r#"<iframe src="{}?mode=toc" id="CodeChat-sidebar"></iframe>"#,
                path_to_toc.unwrap().to_slash_lossy()
            ),
            r#"<link rel="stylesheet" href="/static/css/CodeChatEditorProject.css">"#,
        )
    } else {
        ("".to_string(), "")
    };

    // Add testing mode scripts if requested.
    let js_test_suffix = if is_test_mode { "-test" } else { "" };
    let testing_src = if is_test_mode {
        r#"
        <link rel="stylesheet" href="https://unpkg.com/mocha/mocha.css" />
        <script src="https://unpkg.com/mocha/mocha.js"></script>
        "#
    } else {
        ""
    };

    // Get the locations for bundled files.
    let codechat_editor_js = BUNDLED_FILES_MAP
        .get(&format!("CodeChatEditor{js_test_suffix}.js"))
        .unwrap();
    let codehat_editor_css = BUNDLED_FILES_MAP
        .get(&format!("CodeChatEditor{js_test_suffix}.css"))
        .unwrap();

    // Build and return the webpage.
    (
        SimpleHttpResponse::Ok(format!(
            r##"<!DOCTYPE html>
<html lang="en">
    <head>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1">
        <title>{name} - The CodeChat Editor</title>
        <script type="module">
            import {{ page_init }} from "/{codechat_editor_js}"
            page_init()
        </script>
        <link rel="stylesheet" href="/{codehat_editor_css}">
        {testing_src}
        {sidebar_css}
    </head>
    <body>
        {sidebar_iframe}
        <div id="CodeChat-contents">
            <div id="CodeChat-top">
                <div id="CodeChat-filename">
                    <p>
                        <button id="CodeChat-save-button">
                            <span class="CodeChat-hotkey">S</span>ave
                        </button>
                        - {name} - {dir}
                    </p>
                </div>
                <div id="CodeChat-menu"></div>
            </div>
            <div id="CodeChat-body"></div>
            <div id="CodeChat-bottom"></div>
            <div id="mocha"></div>
        </div>
    </body>
</html>
"##
        )),
        Some(codechat_for_web),
    )
}

/// ## Websockets
///
/// Each CodeChat Editor IDE instance pairs with a CodeChat Editor Client
/// through the CodeChat Editor Server. Together, these form a joint editor,
/// allowing the user to edit the plain text of the source code in the IDE, or
/// make GUI-enhanced edits of the source code rendered by the CodeChat Editor
/// Client.
async fn client_websocket(
    connection_id: web::Path<String>,
    req: HttpRequest,
    body: web::Payload,
    websocket_queues: Arc<Mutex<HashMap<String, WebsocketQueues>>>,
) -> Result<HttpResponse, Error> {
    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, body)?;

    // Websocket task: start a task to handle receiving `JointMessage` websocket
    // data from the CodeChat Editor Client and forwarding it to the IDE and
    // vice versa. It also handles low-level details (ping/pong, websocket
    // errors/closing).
    actix_rt::spawn(async move {
        msg_stream = msg_stream.max_frame_size(1_000_000);
        let mut aggregated_msg_stream = msg_stream.aggregate_continuations();
        aggregated_msg_stream = aggregated_msg_stream.max_continuation_size(10_000_000);

        // Transfer the queues from the global state to this task.
        let (from_websocket_tx, mut to_websocket_rx) = match websocket_queues
            .lock()
            .unwrap()
            .remove(&connection_id.to_string())
        {
            Some(queues) => (queues.from_websocket_tx.clone(), queues.to_websocket_rx),
            None => {
                error!("No websocket queues for connection id {connection_id}.");
                return;
            }
        };

        // Assign each message unique id.
        let mut id: u32 = 0;
        // Keep track of pending messages.
        let mut pending_messages: HashMap<u32, JoinHandle<()>> = HashMap::new();

        // Shutdown may occur in a controlled process or an immediate websocket
        // close. If the Client needs to close, it can simply close its
        // websocket, since the IDE maintains all state (case 2). However, if
        // the IDE plugin needs to close, it should inform the Client first, so
        // the Client can send the IDE any unsaved data (case 1). However, bad
        // things can also happen; if either websocket connection is closed,
        // then the other websocket should also be immediately closed (also case
        // 2).
        //
        // 1.  The IDE plugin needs to close.
        //     1.  The IDE plugin sends a `Closed` message.
        //     2.  The Client replies with a `Result` message, acknowledging the
        //         close. It sends an `Update` message if necessary to save the
        //         current file.
        //     3.  After receiving the acknowledge from the Update message (if
        //         sent), the Client closes the websocket. The rest of this
        //         sequence is covered in the next case.
        // 2.  Either websocket is closed. In this case, the other websocket
        //     should be immediately closed; there's no longer the opportunity
        //     to perform a more controlled shutdown (see the first case).
        //     1.  The websocket which closed enqueues a `Closed` message for
        //         the other websocket.
        //     2.  When the other websocket receives this message, it closes.
        //
        // True when the websocket's client deliberately closes the websocket;
        // otherwise, closing represents a network interruption (such as the
        // computer going to sleep).
        let mut is_closing = false;
        // True if a ping was sent, but a matching pong wasn't yet received.
        let mut sent_ping = false;

        loop {
            select! {
                // Send pings on a regular basis.
                _ = sleep(WEBSOCKET_PING_DELAY) => {
                    if sent_ping {
                        // If we haven't received the answering pong, the
                        // websocket must be broken.
                        break;
                    }
                    // Send a ping to check that the websocket is still open.
                    // For example, putting a PC to sleep then waking it breaks
                    // the websocket, but the server doesn't detect this without
                    // sending a ping (which then fails).
                    if let Err(err) = session.ping(&Bytes::new()).await {
                        error!("Unable to send ping: {err}");
                        break;
                    }
                    sent_ping = true;
                }

                // Process a message received from the websocket.
                Some(msg_wrapped) = aggregated_msg_stream.next() => {
                    match msg_wrapped {
                        Ok(msg) => {
                            match msg {
                                // Send a pong in response to a ping.
                                AggregatedMessage::Ping(bytes) => {
                                    if let Err(err) = session.pong(&bytes).await {
                                        error!("Unable to send pong: {err}");
                                        break;
                                    }
                                }

                                AggregatedMessage::Pong(_bytes) => {
                                    // Acknowledge the matching pong to the ping
                                    // that was most recently sent.
                                    sent_ping = false;
                                }

                                // Decode text messages as JSON then dispatch
                                // then to the IDE.
                                AggregatedMessage::Text(b) => {
                                    // The CodeChat Editor Client should always
                                    // send valid JSON.
                                    match serde_json::from_str::<EditorMessage>(&b) {
                                        Err(err) => {
                                            error!(
                                                "Unable to decode JSON message from the CodeChat Editor client: {err}"
                                            );
                                            break;
                                        }
                                        Ok(joint_message) => {
                                            // If this was a `Result`, remove it from
                                            // the pending queue.
                                            if let EditorMessageContents::Result(_, _) = joint_message.message {
                                                // Cancel the timeout for this result.
                                                if let Some(task) = pending_messages.remove(&joint_message.id) {
                                                    task.abort();
                                                }
                                            }
                                            // Check for messages that only the server
                                            // can send.
                                            match &joint_message.message {
                                                // Check for an invalid message.
                                                EditorMessageContents::LoadFile(_) |
                                                EditorMessageContents::ClientHtml(_) |
                                                EditorMessageContents::Closed => {
                                                    let msg = format!("Invalid message {joint_message:?}");
                                                    error!("{msg}");
                                                    queue_send!(from_websocket_tx.send(EditorMessage {
                                                        id: joint_message.id,
                                                        message: EditorMessageContents::Result(Some(msg), None)
                                                    }));
                                                },

                                                // Send everything else.
                                                _ => {
                                                    // Send the `JointMessage` to the
                                                    // processing task.
                                                    queue_send!(from_websocket_tx.send(joint_message));
                                                }
                                            }
                                        }
                                    }
                                }

                                // Forward a close message from the client to
                                // the IDE, so that both this websocket
                                // connection and the other connection will both
                                // be closed.
                                AggregatedMessage::Close(reason) => {
                                    info!("Closing per client request: {reason:?}");
                                    is_closing = true;
                                    queue_send!(from_websocket_tx.send(EditorMessage { id: 0, message: EditorMessageContents::Closed }));
                                    break;
                                }

                                other => {
                                    warn!("Unexpected message {other:?}");
                                    break;
                                }
                            }
                        }
                        Err(err) => {
                            error!("websocket receive error {err:?}");
                        }
                    }
                }

                // Forward a message from the processing task to the websocket.
                Some(mut m) = to_websocket_rx.recv() => {
                    // Pre-process this message.
                    match m.message {
                        // If it's a `Result`, no additional processing is
                        // needed.
                        EditorMessageContents::Result(_, _) => {},
                        // A `Closed` message causes the websocket to close.
                        EditorMessageContents::Closed => {
                            info!("Closing per request.");
                            is_closing = true;
                            break;
                        },
                        // All other messages are added to the pending queue and
                        // assigned a unique id.
                        _ => {
                            // Assign the id for the message.
                            m.id = id;
                            id += 1;
                            let timeout_tx = from_websocket_tx.clone();
                            let waiting_task = actix_rt::spawn(async move {
                                sleep(REPLY_TIMEOUT).await;
                                let msg = format!("Timeout: message id {} unacknowledged.", m.id);
                                error!("{msg}");
                                // Since the websocket failed to send a `Result`, produce a timeout `Result` for it.
                                'timeout: {
                                        queue_send!(timeout_tx.send(EditorMessage {
                                        id: m.id,
                                        message: EditorMessageContents::Result(Some(msg), None)
                                    }), 'timeout);
                                }
                            });
                            pending_messages.insert(m.id, waiting_task);
                            info!("ID is {id}.");
                        }
                    }

                    // Send the message to the websocket.
                    match serde_json::to_string(&m) {
                        Ok(s) => {
                            if let Err(err) = session.text(&*s).await {
                                error!("Unable to send: {err}");
                                break;
                            }
                        }
                        Err(err) => {
                            error!("Encoding failure {err}");
                        }
                    }
                }

                else => break,
            }
        }

        // Shut down the session, to stop any incoming messages.
        if let Err(err) = session.close(None).await {
            error!("Unable to close session: {err}");
        }

        // Re-enqueue this unless the client requested the websocket to close.
        if is_closing {
            info!("Websocket closed.");
            to_websocket_rx.close();
            // Drain any remaining messages after closing the queue.
            while let Some(m) = to_websocket_rx.recv().await {
                warn!("Dropped queued message {m:?}");
            }
        } else {
            info!("Websocket re-enqueued.");
            websocket_queues.lock().unwrap().insert(
                connection_id.to_string(),
                WebsocketQueues {
                    from_websocket_tx,
                    to_websocket_rx,
                },
            );
        }

        info!("Websocket exiting.");
    });

    Ok(response)
}

// ## Webserver core
#[actix_web::main]
pub async fn main() -> std::io::Result<()> {
    run_server().await
}

pub async fn run_server() -> std::io::Result<()> {
    // Pre-load the bundled files before starting the webserver.
    let _ = &*BUNDLED_FILES_MAP;
    let _ = &*CODECHAT_EDITOR_FRAMEWORK_JS;
    let app_data = make_app_data();
    let server = match HttpServer::new(move || configure_app(App::new(), &app_data))
        .bind((IP_ADDRESS, IP_PORT))
    {
        Ok(server) => server,
        Err(err) => {
            error!("Unable to bind to {IP_ADDRESS}:{IP_PORT} - {err}");
            return Err(err);
        }
    };
    server.run().await
}

pub fn configure_logger() {
    log4rs::init_file("log4rs.yml", Default::default()).unwrap();
}

// Quoting the [docs](https://actix.rs/docs/application#shared-mutable-state),
// "To achieve _globally_ shared state, it must be created **outside** of the
// closure passed to `HttpServer::new` and moved/cloned in." Putting this code
// inside `configure_app` places it inside the closure which calls
// `configure_app`, preventing globally shared state.
fn make_app_data() -> web::Data<AppState> {
    web::Data::new(AppState {
        connection_id: Mutex::new(0),
        processing_task_queue_tx: Arc::new(Mutex::new(HashMap::new())),
        filewatcher_client_queues: Arc::new(Mutex::new(HashMap::new())),
        vscode_ide_queues: Arc::new(Mutex::new(HashMap::new())),
        vscode_client_queues: Arc::new(Mutex::new(HashMap::new())),
        vscode_connection_id: Arc::new(Mutex::new(HashSet::new())),
    })
}

// Configure the web application. I'd like to make this return an
// `App<AppEntry>`, but `AppEntry` is a private module.
fn configure_app<T>(app: App<T>, app_data: &web::Data<AppState>) -> App<T>
where
    T: ServiceFactory<ServiceRequest, Config = (), Error = Error, InitError = ()>,
{
    app
        // Provide data to all endpoints -- the compiler lexers.
        .app_data(app_data.clone())
        // Serve static files per the
        // [docs](https://actix.rs/docs/static-files).
        .service(actix_files::Files::new(
            "/static",
            CLIENT_STATIC_PATH.as_os_str(),
        ))
        // These endpoints serve the files from the filesystem and the
        // websockets.
        .service(filewatcher_browser_endpoint)
        .service(filewatcher_client_endpoint)
        .service(filewatcher_websocket)
        .service(serve_vscode_fs)
        .service(vscode_ide_websocket)
        .service(vscode_client_websocket)
        // Reroute to the filesystem for typical user-requested URLs.
        .route("/", web::get().to(filewatcher_root_fs_redirect))
        .route("/fw/fsb", web::get().to(filewatcher_root_fs_redirect))
}

// ## Utilities
// Send a response to the client after processing a message from the client.
async fn send_response(client_tx: &Sender<EditorMessage>, id: u32, result: Option<String>) {
    if let Err(err) = client_tx
        .send(EditorMessage {
            id,
            message: EditorMessageContents::Result(result, None),
        })
        .await
    {
        error!("Unable to enqueue: {err}");
    }
}

fn url_to_path(url_string: String) -> Result<PathBuf, String> {
    // Convert this URL back to a file path.
    match urlencoding::decode(&url_string) {
        Err(err) => Err(format!("Error: unable to decode URL {url_string}: {err}.")),
        Ok(url_string) => match Url::parse(&url_string) {
            Err(err) => Err(format!("Error: unable to parse URL {url_string}: {err}")),
            Ok(url) => match url.path_segments() {
                None => Err(format!("Error: URL {url} cannot be a base.")),
                Some(path_segments) => {
                    // Make sure the path segments start with
                    // `/fw/fsc/{connection_id}`.
                    let ps: Vec<_> = path_segments.collect();
                    if ps.len() <= 3 || ps[0] != "fw" || ps[1] != "fsc" {
                        Err(format!("Error: URL {url} has incorrect prefix."))
                    } else {
                        // Strip these first three segments; the
                        // remainder is a file path.
                        let path_str = ps[3..].join("/");
                        match PathBuf::from_str(&path_str) {
                            Err(err) => Err(format!(
                                "Error: unable to parse file path {path_str}: {err}."
                            )),
                            Ok(path_buf) => match path_buf.canonicalize() {
                                Err(err) => {
                                    Err(format!("Unable to canonicalize {path_buf:?}: {err}."))
                                }
                                Ok(p) => Ok(p),
                            },
                        }
                    }
                }
            },
        },
    }
}

// Given a `Path`, transform it into a displayable HTML string (with any
// necessary escaping).
fn path_display(p: &Path) -> String {
    escape_html(&simplified(p).to_string_lossy())
}

// Return a Not Found (404) error with the provided HTML body.
fn html_not_found(msg: &str) -> HttpResponse {
    HttpResponse::NotFound()
        .content_type(ContentType::html())
        .body(html_wrapper(msg))
}

// Wrap the provided HTML body in DOCTYPE/html/head tags.
fn html_wrapper(body: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
    <head>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1">
        <title>The CodeChat Editor</title>
    </head>
    <body>
        {body}
    </body>
</html>"#
    )
}

// Given text, escape it so it formats correctly as HTML. This is a translation
// of Python's `html.escape` function.
fn escape_html(unsafe_text: &str) -> String {
    unsafe_text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
