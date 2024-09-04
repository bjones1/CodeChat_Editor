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
    env,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;

// ### Third-party
use actix_files;
use actix_web::{
    dev::{ServiceFactory, ServiceRequest},
    error::Error,
    get,
    http::header::{self, ContentType},
    web, App, HttpRequest, HttpResponse, HttpServer, Responder,
};
use actix_ws::AggregatedMessage;
use bytes::Bytes;
use futures_util::StreamExt;
use lazy_static::lazy_static;
use log::{error, info, warn};
use log4rs;
use path_slash::PathBufExt;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json;
use tokio::{
    fs::{self, DirEntry},
    select,
    sync::mpsc::{Receiver, Sender},
    task::JoinHandle,
    time::sleep,
};
use urlencoding::{self, encode};
use vscode::{serve_vscode_fs, vscode_client_websocket, vscode_ide_websocket};
#[cfg(target_os = "windows")]
use win_partitions::win_api::get_logical_drive;

// ### Local
use crate::lexer::LanguageLexersCompiled;
use crate::lexer::{compile_lexers, supported_languages::get_language_lexer_vec};
use crate::processing::TranslationResultsString;
use crate::processing::{source_to_codechat_for_web_string, CodeChatForWeb};
use filewatcher::{filewatcher_websocket, serve_filewatcher};

// ## Macros
/// Create a macro to report an error when enqueueing an item.
#[macro_export]
macro_rules! queue_send {
    // Provide two options: `break` or `break 'label`.
    ($tx: expr) => {
        if let Err(err) = $tx.await {
            error!("Unable to enqueue: {err}");
            break;
        }
    };
    ($tx: expr, $label: tt) => {
        if let Err(err) = $tx.await {
            error!("Unable to enqueue: {err}");
            break $label;
        }
    };
}

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
/// Client, the IDE, and the CodeChat Editor Server.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum EditorMessageContents {
    /// This is the first message sent when the IDE starts up. The client should
    /// not send this message.
    Opened(IdeType),
    /// This sends an update; any missing fields are unchanged.
    Update(UpdateMessageContents),
    /// Only the CodeChat Client editor may send this; it requests the IDE to
    /// load the provided file. The IDE should respond by sending an `Update`
    /// with the requested file.
    LoadFile(PathBuf),
    /// Only the server may send this to the IDE. It contains the HTML for the
    /// CodeChat Editor Client to display in its built-in browser.
    ClientHtml(String),
    /// Sent when the IDE or client websocket was closed, indicating that the
    /// unclosed websocket should be closed as well.
    Closed,
    /// Send by the IDE to request the client to save any unsaved data then
    /// close.
    RequestClose,
    /// Sent as a response to any of the above messages, reporting
    /// success/error. An empty string indicates success; otherwise, the string
    /// contains the error message.
    Result(String),
}

/// Specify the type of IDE that this client represents.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum IdeType {
    /// True if the CodeChat Editor will be hosted inside VSCode; false means it
    /// should be hosted in an external browser.
    VSCode(bool),
}

/// Contents of the `Update` message.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct UpdateMessageContents {
    /// An absolute path to the file currently in use.
    path: Option<PathBuf>,
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
    lexers: LanguageLexersCompiled,
    // The number of the next connection ID to assign.
    connection_id: Mutex<u32>,
    // For each connection ID, store the queues for the FileWatcher IDE.
    filewatcher_client_queues: Arc<Mutex<HashMap<String, WebsocketQueues>>>,
    // For each connection ID, store the queues for the VSCode IDE.
    vscode_ide_queues: Arc<Mutex<HashMap<String, WebsocketQueues>>>,
    vscode_client_queues: Arc<Mutex<HashMap<String, WebsocketQueues>>>,
    // Connection IDs that are currently in use.
    vscode_connection_id: Arc<Mutex<HashSet<String>>>,
}

// ## Globals
lazy_static! {
    /// Matches a bare drive letter. Only needed on Windows.
    static ref DRIVE_LETTER_REGEX: Regex = Regex::new("^[a-zA-Z]:$").unwrap();
}
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

/// ## File browser endpoints
///
/// The file browser provides a very crude interface, allowing a user to select
/// a file from the local filesystem for editing. Long term, this should be
/// replaced by something better.
///
/// Redirect from the root of the filesystem to the actual root path on this OS.
async fn _root_fs_redirect() -> impl Responder {
    HttpResponse::TemporaryRedirect()
        .insert_header((header::LOCATION, "/fw/fs/"))
        .finish()
}

/// Dispatch to support functions which serve either a directory listing, a
/// CodeChat Editor file, or a normal file.
///
/// Omit code coverage -- this is a temporary interface, until IDE integration
/// replaces this.
#[cfg(not(tarpaulin_include))]
#[get("/fw/fs/{path:.*}")]
async fn serve_filewatcher_fs(
    req: HttpRequest,
    app_state: web::Data<AppState>,
    orig_path: web::Path<String>,
) -> impl Responder {
    let mut fixed_path = orig_path.to_string();
    #[cfg(target_os = "windows")]
    // On Windows, a path of `drive_letter:` needs a `/` appended.
    if DRIVE_LETTER_REGEX.is_match(&fixed_path) {
        fixed_path += "/";
    } else if fixed_path.is_empty() {
        // If there's no drive letter yet, we will always use `dir_listing` to
        // select a drive.
        return dir_listing("", Path::new("")).await;
    }
    // All other cases (for example, `C:\a\path\to\file.txt`) are OK.

    // For Linux/OS X, prepend a slash, so that `a/path/to/file.txt` becomes
    // `/a/path/to/file.txt`.
    #[cfg(not(target_os = "windows"))]
    let fixed_path = "/".to_string() + &fixed_path;

    // Handle any
    // [errors](https://doc.rust-lang.org/std/fs/fn.canonicalize.html#errors).
    let canon_path = match Path::new(&fixed_path).canonicalize() {
        Ok(p) => p,
        Err(err) => {
            return html_not_found(&format!(
                "<p>The requested path <code>{fixed_path}</code> is not valid: {err}.</p>"
            ))
        }
    };
    if canon_path.is_dir() {
        return dir_listing(orig_path.as_str(), &canon_path).await;
    } else if canon_path.is_file() {
        return serve_filewatcher(&canon_path, &req, app_state).await;
    }

    // It's not a directory or a file...we give up. For simplicity, don't handle
    // symbolic links.
    html_not_found(&format!(
        "<p>The requested path <code>{}</code> is not a directory or a file.</p>",
        path_display(&canon_path)
    ))
}

/// ### Directory browser
///
/// Create a web page listing all files and subdirectories of the provided
/// directory.
///
/// Omit code coverage -- this is a temporary interface, until IDE integration
/// replaces this.
#[cfg(not(tarpaulin_include))]
async fn dir_listing(web_path: &str, dir_path: &Path) -> HttpResponse {
    // Special case on Windows: list drive letters.
    #[cfg(target_os = "windows")]
    if dir_path == Path::new("") {
        // List drive letters in Windows
        let mut drive_html = String::new();
        let logical_drives = match get_logical_drive() {
            Ok(v) => v,
            Err(err) => return html_not_found(&format!("Unable to list drive letters: {}.", err)),
        };
        for drive_letter in logical_drives {
            drive_html.push_str(&format!(
                "<li><a href='/fw/fs/{drive_letter}:/'>{drive_letter}:</a></li>\n"
            ));
        }

        return HttpResponse::Ok()
            .content_type(ContentType::html())
            .body(html_wrapper(&format!(
                "<h1>Drives</h1>
<ul>
{drive_html}
</ul>
"
            )));
    }

    // List each file/directory with appropriate links.
    let mut unwrapped_read_dir = match fs::read_dir(dir_path).await {
        Ok(p) => p,
        Err(err) => {
            return html_not_found(&format!(
                "<p>Unable to list the directory {}: {err}/</p>",
                path_display(dir_path)
            ))
        }
    };

    // Get a listing of all files and directories
    let mut files: Vec<DirEntry> = Vec::new();
    let mut dirs: Vec<DirEntry> = Vec::new();
    loop {
        match unwrapped_read_dir.next_entry().await {
            Ok(v) => {
                if let Some(dir_entry) = v {
                    let file_type = match dir_entry.file_type().await {
                        Ok(x) => x,
                        Err(err) => {
                            return html_not_found(&format!(
                                "<p>Unable to determine the type of {}: {err}.",
                                path_display(&dir_entry.path()),
                            ))
                        }
                    };
                    if file_type.is_file() {
                        files.push(dir_entry);
                    } else {
                        // Group symlinks with dirs.
                        dirs.push(dir_entry);
                    }
                } else {
                    break;
                }
            }
            Err(err) => {
                return html_not_found(&format!("<p>Unable to read file in directory: {err}."))
            }
        };
    }
    // Sort them -- case-insensitive on Windows, normally on Linux/OS X.
    #[cfg(target_os = "windows")]
    let file_name_key = |a: &DirEntry| {
        Ok::<String, std::ffi::OsString>(a.file_name().into_string()?.to_lowercase())
    };
    #[cfg(not(target_os = "windows"))]
    let file_name_key =
        |a: &DirEntry| Ok::<String, std::ffi::OsString>(a.file_name().into_string()?);
    files.sort_unstable_by_key(file_name_key);
    dirs.sort_unstable_by_key(file_name_key);

    // Put this on the resulting webpage. List directories first.
    let mut dir_html = String::new();
    for dir in dirs {
        let dir_name = match dir.file_name().into_string() {
            Ok(v) => v,
            Err(err) => {
                return html_not_found(&format!(
                    "<p>Unable to decode directory name '{err:?}' as UTF-8."
                ))
            }
        };
        let encoded_dir = encode(&dir_name);
        dir_html += &format!(
            "<li><a href='/fw/fs/{web_path}{}{encoded_dir}'>{dir_name}</a></li>\n",
            // If this is a raw drive letter, then the path already ends with a
            // slash, such as `C:/`. Don't add a second slash in this case.
            // Otherwise, add a slash to make `C:/foo` into `C:/foo/`.
            //
            // Likewise, the Linux root path of `/` already ends with a slash,
            // while all other paths such a `/foo` don't. To detect this, look
            // for an empty `web_path`.
            if web_path.ends_with('/') || web_path.is_empty() {
                ""
            } else {
                "/"
            }
        );
    }

    // List files second.
    let mut file_html = String::new();
    for file in files {
        let file_name = match file.file_name().into_string() {
            Ok(v) => v,
            Err(err) => {
                return html_not_found(&format!("<p>Unable to decode file name {err:?} as UTF-8.",))
            }
        };
        let encoded_file = encode(&file_name);
        file_html += &format!(
            "<li><a href=\"/fw/fs/{web_path}/{encoded_file}\" target=\"_blank\">{file_name}</a></li>\n"
        );
    }
    let body = format!(
        "<h1>Directory {}</h1>
<h2>Subdirectories</h2>
<ul>
{dir_html}
</ul>
<h2>Files</h2>
<ul>
{file_html}
</ul>
",
        path_display(dir_path)
    );

    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(html_wrapper(&body))
}

#[async_trait]
trait ProcessingTask {
    async fn processing_task(
        &self,
        file_path: &Path,
        app_state: web::Data<AppState>,
        codechat_for_web: CodeChatForWeb,
    ) -> u32;
}

async fn serve_file(
    file_path: &Path,
    file_contents: &str,
    ide_path: &str,
    req: &HttpRequest,
    app_state: web::Data<AppState>,
    // Rust doesn't allow async function pointers. This is a workaround from
    // [SO](https://stackoverflow.com/a/76983770/16038919).
    processing_task: impl ProcessingTask,
) -> HttpResponse {
    let (name, dir, _mode, is_test_mode, is_toc) = parse_web(file_path, req);

    // See if this is a CodeChat Editor file.
    let (translation_results_string, path_to_toc) =
        source_to_codechat_for_web_string(file_contents, file_path, is_toc, &app_state.lexers);
    let is_project = path_to_toc.is_some();
    let codechat_for_web = match translation_results_string {
        // The file type is unknown. Serve it raw.
        TranslationResultsString::Unknown => {
            match actix_files::NamedFile::open_async(file_path).await {
                Ok(v) => {
                    let res = v.into_response(req);
                    return res;
                }
                Err(err) => {
                    return html_not_found(&format!(
                        "<p>Error opening file {}: {err}.",
                        path_display(file_path),
                    ))
                }
            }
        }
        // Report a lexer error.
        TranslationResultsString::Err(err_string) => return html_not_found(&err_string),
        // This is a CodeChat file. The following code wraps the CodeChat for
        // web results in a CodeChat Editor Client webpage.
        TranslationResultsString::CodeChat(codechat_for_web) => codechat_for_web,
        TranslationResultsString::Toc(html) => {
            // The TOC is a simplified web page which requires no additional
            // processing. The script ensures that all hyperlinks notify the
            // encoding page (the CodeChat Editor Client), allowing it to save
            // before navigating.
            return HttpResponse::Ok()
                .content_type(ContentType::html())
                .body(format!(
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
{}
</body>
</html>
"#,
                    // Look for any script tags and prevent these from causing
                    // problems.
                    html.replace("</script>", "<\\/script>")
                ));
        }
    };

    let connection_id = processing_task
        .processing_task(file_path, app_state, codechat_for_web)
        .await;

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

    // Add in content when testing.
    let testing_src = if is_test_mode {
        r#"
        <link rel="stylesheet" href="https://unpkg.com/mocha/mocha.css" />
        <script src="https://unpkg.com/mocha/mocha.js"></script>
        "#
    } else {
        ""
    };

    // Build and return the webpage.
    let js_test_suffix = if is_test_mode { "-test" } else { "" };
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
    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r##"<!DOCTYPE html>
<html lang="en">
    <head>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1">
        <title>{name} - The CodeChat Editor</title>

        <link rel="stylesheet" href="/static/bundled/CodeChatEditor.css">
        <script type="module">
            import {{ page_init }} from "/static/bundled/CodeChatEditor{js_test_suffix}.js"
            page_init({ws_url})
        </script>
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
        ))
}

// Provided info from the HTTP request, determine the following parameters.
fn parse_web(
    file_path: &Path,
    req: &HttpRequest,
) -> (
    // The name of the file, as a string.
    String,
    // THe path to the file, as a string.
    String,
    // The rendering mode for this file (view, test, etc.)
    String,
    // True if this web page wants to run unit tests.
    bool,
    // True if this file should be rendered as a table of contents.
    bool,
) {
    let raw_dir = file_path.parent().unwrap();
    // Use a lossy conversion, since this is UI display, not filesystem access.
    let dir = escape_html(path_display(raw_dir).as_str());
    let name = escape_html(&file_path.file_name().unwrap().to_string_lossy());

    // Get the `mode` and `test` query parameters.
    let empty_string = "".to_string();
    let query_params = web::Query::<HashMap<String, String>>::from_query(req.query_string());
    let (mode, is_test_mode) = match query_params {
        Ok(query) => (
            query.get("mode").unwrap_or(&empty_string).clone(),
            query.get("test").is_some(),
        ),
        Err(_err) => (empty_string, false),
    };
    let is_toc = mode == "toc";

    (name, dir, mode, is_test_mode, is_toc)
}

// Send a response to the client after processing a message from the client.
async fn send_response(client_tx: &Sender<EditorMessage>, id: u32, result: &str) {
    if let Err(err) = client_tx
        .send(EditorMessage {
            id,
            message: EditorMessageContents::Result(result.to_string()),
        })
        .await
    {
        error!("Unable to enqueue: {err}");
    }
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
                                            if let EditorMessageContents::Result(_) = joint_message.message {
                                                // Cancel the timeout for this result.
                                                if let Some(task) = pending_messages.remove(&joint_message.id) {
                                                    task.abort();
                                                }
                                            }
                                            // Send the `JointMessage` to the
                                            // processing task.
                                            queue_send!(from_websocket_tx.send(joint_message));
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
                    // Assign the id for the message.
                    m.id = id;
                    id += 1;

                    // Process this message.
                    match m.message {
                        // If it's a `Result`, no additional processing is
                        // needed.
                        EditorMessageContents::Result(_) => {},
                        // A `Closed` message causes the websocket to close.
                        EditorMessageContents::Closed => {
                            info!("Closing per request.");
                            is_closing = true;
                            break;
                        },
                        // All other messages are added to the pending queue.
                        _ => {
                            let waiting_task = actix_rt::spawn(async move {
                                sleep(REPLY_TIMEOUT).await;
                                error!("Timeout: message id {} unacknowledged.", m.id);
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

// ## Webserver startup
#[actix_web::main]
pub async fn main() -> std::io::Result<()> {
    run_server().await
}

pub async fn run_server() -> std::io::Result<()> {
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

// ## Utilities
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
        lexers: compile_lexers(get_language_lexer_vec()),
        connection_id: Mutex::new(0),
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
    client_static_path = client_static_path.canonicalize().unwrap();

    app
        // Provide data to all endpoints -- the compiler lexers.
        .app_data(app_data.clone())
        // Serve static files per the
        // [docs](https://actix.rs/docs/static-files).
        .service(actix_files::Files::new(
            "/static",
            client_static_path.as_os_str(),
        ))
        // These endpoints serve the files from the filesystem and the websockets.
        .service(serve_filewatcher_fs)
        .service(filewatcher_websocket)
        .service(serve_vscode_fs)
        .service(vscode_ide_websocket)
        .service(vscode_client_websocket)
        // Reroute to the filesystem for typical user-requested URLs.
        .route("/", web::get().to(_root_fs_redirect))
        .route("/fw/fs", web::get().to(_root_fs_redirect))
}

// Given a `Path`, transform it into a displayable string.
fn path_display(p: &Path) -> String {
    let path_orig = p.to_string_lossy();
    if cfg!(windows) {
        // On Windows, the returned path starts with `\\?\` per the
        // [docs](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file#win32-file-namespaces).
        path_orig[4..].to_string()
    } else {
        path_orig.to_string()
    }
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
        "<!DOCTYPE html>
<html lang=\"en\">
    <head>
        <meta charset=\"UTF-8\">
        <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">
        <title>The CodeChat Editor</title>
    </head>
    <body>
        {body}
    </body>
</html>"
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
