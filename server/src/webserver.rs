// Copyright (C) 2023 Bryan A. Jones.
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
/// `webserver.rs` -- Serve CodeChat Editor Client webpages
/// =======================================================
// Submodules
// ----------
mod filewatcher;
#[cfg(test)]
pub mod tests;
mod vscode;

// Imports
// -------
//
// ### Standard library
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::{self, MAIN_SEPARATOR_STR, Path, PathBuf},
    str::FromStr,
    sync::{Arc, Mutex},
    time::Duration,
};

// ### Third-party
use actix_files;
use actix_web::{
    App, HttpRequest, HttpResponse, HttpServer,
    dev::{ServerHandle, ServiceFactory, ServiceRequest},
    error::Error,
    get,
    http::header::ContentType,
    web,
};
use actix_ws::AggregatedMessage;
use bytes::Bytes;
use dunce::simplified;
use futures_util::StreamExt;
use indoc::{formatdoc, indoc};
use lazy_static::lazy_static;
use log::{LevelFilter, error, info, warn};
use log4rs;
use mime::Mime;
use mime_guess;
use path_slash::{PathBufExt, PathExt};
use serde::{Deserialize, Serialize};
use serde_json;
use tokio::{
    fs::File,
    io::AsyncReadExt,
    select,
    sync::{
        mpsc::{Receiver, Sender},
        oneshot,
    },
    task::JoinHandle,
    time::sleep,
};
use url::Url;
use vscode::{
    serve_vscode_fs, vscode_client_framework, vscode_client_websocket, vscode_ide_websocket,
};

// ### Local
//use crate::capture::EventCapture;
use crate::processing::{
    CodeChatForWeb, TranslationResultsString, source_to_codechat_for_web_string,
};
use filewatcher::{
    filewatcher_browser_endpoint, filewatcher_client_endpoint, filewatcher_root_fs_redirect,
    filewatcher_websocket,
};

// Data structures
// ---------------
//
// ### Data structures supporting a websocket connection between the IDE, this
//
// server, and the CodeChat Editor Client
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
    file_path: PathBuf,
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
    id: f64,
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

    // ### This message may only be sent by the Client to the Server.
    /// Open the provided URL in a web browser. This is used from within
    /// plugins/extensions (such as VSCode), where the Client is prohibited from
    /// opening a new browser tab/window.
    OpenUrl(String),

    // #### These messages may only be sent by the Server.
    /// Ask the IDE if the provided file is loaded. If so, the IDE should
    /// respond by sending a `LoadFile` with the requested file. If not, the
    /// returned `Result` should indicate the error "not loaded". Valid
    /// destinations: IDE.
    LoadFile(PathBuf),
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
    /// success/error.
    Result(MessageResult),
}

/// The contents of a `Result` message.
type MessageResult = Result<
    // The result of the operation, if successful.
    ResultOkTypes,
    // The error message.
    String,
>;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum ResultOkTypes {
    /// Most messages have no result.
    Void,
    /// The `LoadFile` message provides file contents, if available. This
    /// message may only be sent from the IDE to the Server.
    LoadFile(Option<String>),
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
    /// The filesystem path to this file. This is only used by the IDE to
    /// determine which file to apply Update contents to. The Client stores then
    /// then sends it back to the IDE in `Update` messages. This helps deal with
    /// transition times when the IDE and Client have different files loaded,
    /// guaranteeing to updates are still applied to the correct file.
    file_path: String,
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
    // Provide methods to control the server.
    server_handle: Mutex<Option<ServerHandle>>,
    // The number of the next connection ID to assign.
    connection_id: Mutex<u32>,
    // The port this server listens on.
    port: u16,
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

// Macros
// ------
/// Create a macro to report an error when enqueueing an item.
#[macro_export]
macro_rules! oneshot_send {
    // Provide two options: `break` or `break 'label`.
    ($tx: expr_2021) => {
        if let Err(err) = $tx {
            error!("Unable to enqueue: {err:?}");
            break;
        }
    };
    ($tx: expr_2021, $label: tt) => {
        if let Err(err) = $tx {
            error!("Unable to enqueue: {err:?}");
            break $label;
        }
    };
}

#[macro_export]
macro_rules! queue_send {
    ($tx: expr_2021) => {
        $crate::oneshot_send!($tx.await)
    };
    ($tx: expr_2021, $label: tt) => {
        $crate::oneshot_send!($tx.await, $label)
    };
}

/// Globals
/// -------
///
/// The IP address on which the server listens for incoming connections.
pub const IP_ADDRESS: &str = "127.0.0.1";

// The timeout for a reply from a websocket. Use a short timeout to speed up
// unit tests.
const REPLY_TIMEOUT: Duration = if cfg!(test) {
    Duration::from_millis(500)
} else {
    Duration::from_millis(15000)
};

/// The time to wait for a pong from the websocket in response to a ping sent by
/// this server.
const WEBSOCKET_PING_DELAY: Duration = Duration::from_secs(2);

/// The initial value for a message ID.
const INITIAL_MESSAGE_ID: f64 = if cfg!(test) {
    // A simpler value when testing.
    0.0
} else {
    // In production, start with the smallest whole number exactly
    // representable. This is -9007199254740991.
    -((1i64 << f64::MANTISSA_DIGITS) - 1) as f64
};
/// The increment for a message ID. Since the Client, IDE, and Server all
/// increment by this same amount but start at different values, this ensure
/// that message IDs will be unique. (Given a mantissa of 53 bits plus a sign
/// bit, 2^54 seconds = 574 million years before the message ID wraps around
/// assuming an average of 1 message/second.)
const MESSAGE_ID_INCREMENT: f64 = 3.0;

const MATHJAX_TAGS: &str = indoc!(
    r#"
    <script>
        MathJax = {
            // See the [docs](https://docs.mathjax.org/en/latest/options/output/chtml.html#option-descriptions).
            chtml: {
                fontURL: "/static/mathjax-modern-font/chtml/woff",
            },
            tex: {
                inlineMath: [['$', '$'], ['\\(', '\\)']]
            },
        };
    </script>
    <script defer src="/static/mathjax/tex-chtml.js"></script>
    "#
);

lazy_static! {

    // Define the location of the root path, which contains `static/`,
    // `log4rs.yml`, and `hashLocations.json` in a production build, or
    // `client/` and `server/` in a development build.
    static ref ROOT_PATH: PathBuf = {
        let exe_path = env::current_exe().unwrap();
        let exe_dir = exe_path.parent().unwrap();
        #[cfg(not(any(test, debug_assertions)))]
        let root_path = PathBuf::from(exe_dir);
        #[cfg(any(test, debug_assertions))]
        let mut root_path = PathBuf::from(exe_dir);
        // When in debug or running tests, use the layout of the Git repo to
        // find client files. In release mode, we assume the static folder is a
        // subdirectory of the directory containing the executable.
        #[cfg(test)]
        root_path.push("..");
        // Note that `debug_assertions` is also enabled for testing, so this
        // adds to the previous line when running tests.
        #[cfg(debug_assertions)]
        root_path.push("../../..");
        root_path.canonicalize().unwrap()
    };

    // Define the location of static files.
    static ref CLIENT_STATIC_PATH: PathBuf = {
        let mut client_static_path = ROOT_PATH.clone();
        #[cfg(debug_assertions)]
        client_static_path.push("client");

        client_static_path.push("static");
        client_static_path
    };

    // Read in the hashed names of files bundled by esbuild.
    static ref BUNDLED_FILES_MAP: HashMap<String, String> = {
        let mut hl = ROOT_PATH.clone();
        #[cfg(debug_assertions)]
        hl.push("server");
        hl.push("hashLocations.json");
        let json = fs::read_to_string(hl).unwrap();
        let hmm: HashMap<String, String> = serde_json::from_str(&json).unwrap();
        hmm
    };

    static ref CODECHAT_EDITOR_FRAMEWORK_JS: String = BUNDLED_FILES_MAP.get("CodeChatEditorFramework.js").unwrap().to_string();
    static ref CODECHAT_EDITOR_PROJECT_CSS: String = BUNDLED_FILES_MAP.get("CodeChatEditorProject.css").unwrap().to_string();

}

// Webserver functionality
// -----------------------
#[get("/ping")]
async fn ping() -> HttpResponse {
    HttpResponse::Ok().body("pong")
}

#[get("/stop")]
async fn stop(app_state: web::Data<AppState>) -> HttpResponse {
    let Some(ref server_handle) = *app_state.server_handle.lock().unwrap() else {
        error!("Server handle not available to stop server.");
        return HttpResponse::InternalServerError().finish();
    };
    // Don't await this, since that shuts down the server, preventing the
    // following HTTP response. Assign it to a variable to suppress the warning.
    drop(server_handle.stop(true));
    HttpResponse::NoContent().finish()
}

/// Assign an ID to a new connection.
#[get("/id")]
async fn connection_id_endpoint(
    req: HttpRequest,
    body: web::Payload,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
    let (response, mut session, _msg_stream) = actix_ws::handle(&req, body)?;
    actix_rt::spawn(async move {
        if let Err(err) = session
            .text(get_connection_id(&app_state).to_string())
            .await
        {
            error!("Unable to send connection ID: {err}");
        }
        if let Err(err) = session.close(None).await {
            error!("Unable to close connection: {err}");
        }
    });
    Ok(response)
}

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
    // True if the page should enable test mode for Clients it loads.
    is_test_mode: bool,
    // The URL prefix for a websocket connection to the Server.
    ide_path: &str,
    // The ID of the websocket connection.
    connection_id: &str,
    // This returns a response (the Client, or an error).
) -> Result<String, String> {
    // Provide the pathname to the websocket connection. Quote the string using
    // JSON to handle any necessary escapes.
    let ws_url = match serde_json::to_string(&format!("{ide_path}/{connection_id}")) {
        Ok(v) => v,
        Err(err) => {
            return Err(format!(
                "Unable to encode websocket URL for {ide_path}, id {connection_id}: {err}"
            ));
        }
    };

    // Build and return the webpage.
    Ok(formatdoc!(
        r#"
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="UTF-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <title>The CodeChat Editor</title>
                <script type="module">
                    import {{ page_init }} from "/{}"
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
        </html>"#,
        *CODECHAT_EDITOR_FRAMEWORK_JS
    ))
}

// ### Serve file
/// This could be a plain text file (for example, one not recognized as source
/// code that this program supports), a binary file (image/video/etc.), a
/// CodeChat Editor file, or a non-existent file. Determine which type this file
/// is then serve it. Serve a CodeChat Editor Client webpage using the
/// FileWatcher "IDE".
pub async fn filesystem_endpoint(
    request_path: web::Path<(String, String)>,
    req: &HttpRequest,
    app_state: &web::Data<AppState>,
) -> HttpResponse {
    let (connection_id, request_file_path) = request_path.into_inner();
    // On Windows, backslashes in the `request_file_path` will be treated as
    // path separators; however, HTTP does not treat them as path separators.
    // Therefore, re-encode them to prevent inconsistency between the way HTTP
    // and this program interpret file paths.
    let fixed_file_path = request_file_path.replace("\\", "%5C");
    let file_path = match try_canonicalize(&fixed_file_path) {
        Ok(v) => v,
        Err(err) => {
            let msg = format!("Error: unable to convert path {request_file_path}: {err}.");
            error!("{msg}");
            return html_not_found(&msg);
        }
    };

    // Get the `mode` query parameter to determine `is_toc`; default to `false`.
    let query_params: Result<
        web::Query<HashMap<String, String>>,
        actix_web::error::QueryPayloadError,
    > = web::Query::<HashMap<String, String>>::from_query(req.query_string());
    let is_toc =
        query_params.is_ok_and(|query| query.get("mode").is_some_and(|mode| mode == "toc"));
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
            file_path,
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

// Use the provided HTTP request to look for the requested file, returning it as
// an HTTP response. This should be called from within a processing task.
async fn make_simple_http_response(
    // The HTTP request presented to the processing task.
    http_request: &ProcessingTaskHttpRequest,
    // Path to the file currently being edited.
    current_filepath: &Path,
) -> (
    // The response to send back to the HTTP endpoint.
    SimpleHttpResponse,
    // If this file is currently being edited, this is the body of an `Update`
    // message to send.
    Option<EditorMessageContents>,
) {
    // Convert the provided URL back into a file name.
    let file_path = &http_request.file_path;

    // Read the file
    match File::open(file_path).await {
        Err(err) => (
            SimpleHttpResponse::Err(format!("<p>Error opening file {file_path:?}: {err}.")),
            None,
        ),
        Ok(mut fc) => {
            let mut file_contents = String::new();
            match fc.read_to_string(&mut file_contents).await {
                // If this is a binary file (meaning we can't read the contents
                // as UTF-8), just serve it raw; assume this is an
                // image/video/etc.
                Err(_) => (SimpleHttpResponse::Bin(file_path.clone()), None),
                Ok(_) => {
                    text_file_to_response(http_request, current_filepath, file_path, &file_contents)
                        .await
                }
            }
        }
    }
}

// Given a text file, determine the appropriate HTTP response: a Client, or the
// file contents itself (if it's not editable by the Client). If responding with
// a Client, also return an Update message which will provided the contents for
// the Client.
async fn text_file_to_response(
    // The HTTP request presented to the processing task.
    http_request: &ProcessingTaskHttpRequest,
    // Path to the file currently being edited. This path should be cleaned by
    // `try_canonicalize`.
    current_filepath: &Path,
    // Path to this text file. This path should be cleaned by
    // `try_canonicalize`.
    file_path: &Path,
    // Contents of this text file.
    file_contents: &str,
) -> (
    // The response to send back to the HTTP endpoint.
    SimpleHttpResponse,
    // If the response is a Client, also return the appropriate `Update`
    // data to populate the Client with the parsed `file_contents`. In all other
    // cases, return None.
    Option<EditorMessageContents>,
) {
    // Use a lossy conversion, since this is UI display, not filesystem access.
    let Some(file_name) = file_path.file_name() else {
        return (
            SimpleHttpResponse::Err(format!(
                "Path {} has no final component.",
                file_path.to_string_lossy()
            )),
            None,
        );
    };
    let name = escape_html(&file_name.to_string_lossy());

    // Get the locations for bundled files.
    let js_test_suffix = if http_request.is_test_mode {
        "-test"
    } else {
        ""
    };
    let Some(codechat_editor_js) =
        BUNDLED_FILES_MAP.get(&format!("CodeChatEditor{js_test_suffix}.js"))
    else {
        return (
            SimpleHttpResponse::Err(format!("CodeChatEditor{js_test_suffix}.js not found")),
            None,
        );
    };
    let Some(codehat_editor_css) =
        BUNDLED_FILES_MAP.get(&format!("CodeChatEditor{js_test_suffix}.css"))
    else {
        return (
            SimpleHttpResponse::Err(format!("CodeChatEditor{js_test_suffix}.css not found")),
            None,
        );
    };

    // Compare these files, since both have been canonicalized by
    // `try_canonical`.
    let is_current_file = file_path == current_filepath;
    let (translation_results_string, path_to_toc) = if is_current_file || http_request.is_toc {
        source_to_codechat_for_web_string(file_contents, file_path, http_request.is_toc)
    } else {
        // If this isn't the current file, then don't parse it.
        (TranslationResultsString::Unknown, None)
    };
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
            return (SimpleHttpResponse::Err(err_string), None);
        }
        // This is a CodeChat file. The following code wraps the CodeChat for
        // web results in a CodeChat Editor Client webpage.
        TranslationResultsString::CodeChat(codechat_for_web) => codechat_for_web,
        TranslationResultsString::Toc(html) => {
            // The TOC is a simplified web page which requires no additional
            // processing.
            return (
                SimpleHttpResponse::Ok(formatdoc!(
                    r#"
                    <!DOCTYPE html>
                    <html lang="en">
                        <head>
                            <meta charset="UTF-8">
                            <meta name="viewport" content="width=device-width, initial-scale=1">
                            <title>{name} - The CodeChat Editor</title>
                            {MATHJAX_TAGS}
                            <link rel="stylesheet" href="/{codehat_editor_css}">
                        </head>
                        <body class="CodeChat-theme-light">
                            <div class="CodeChat-TOC">
                                {html}
                            </div>
                        </body>
                    </html>"#,
                )),
                None,
            );
        }
    };

    let is_project = path_to_toc.is_some();
    // For project files, add in the sidebar. Convert this from a Windows path
    // to a Posix path if necessary.
    let (sidebar_iframe, sidebar_css) = if is_project {
        (
            format!(
                r#"<iframe src="{}?mode=toc" id="CodeChat-sidebar"></iframe>"#,
                path_to_toc.unwrap().to_slash_lossy()
            ),
            format!(
                r#"<link rel="stylesheet" href="/{}">"#,
                *CODECHAT_EDITOR_PROJECT_CSS
            ),
        )
    } else {
        ("".to_string(), "".to_string())
    };

    // Add testing mode scripts if requested.
    let testing_src = if http_request.is_test_mode {
        r#"
        <link rel="stylesheet" href="https://unpkg.com/mocha/mocha.css" />
        <script src="https://unpkg.com/mocha/mocha.js"></script>
        "#
    } else {
        ""
    };

    // Provided info from the HTTP request, determine the following parameters.
    let Some(raw_dir) = file_path.parent() else {
        return (
            SimpleHttpResponse::Err(format!(
                "Path {} has no parent.",
                file_path.to_string_lossy()
            )),
            None,
        );
    };
    let dir = path_display(raw_dir);
    let Some(file_path) = file_path.to_str() else {
        let msg = format!("Error: unable to convert path {file_path:?} to a string.");
        error!("{msg}");
        return (SimpleHttpResponse::Err(msg), None);
    };
    // Build and return the webpage.
    (
        SimpleHttpResponse::Ok(formatdoc!(
            r#"
            <!DOCTYPE html>
            <html lang="en">
                <head>
                    <meta charset="UTF-8">
                    <meta name="viewport" content="width=device-width, initial-scale=1">
                    <title>{name} - The CodeChat Editor</title>
                    {MATHJAX_TAGS}
                    <script type="module">
                        import {{ page_init }} from "/{codechat_editor_js}"
                        page_init()
                    </script>
                    <link rel="stylesheet" href="/{codehat_editor_css}">
                    {testing_src}
                    {sidebar_css}
                </head>
                <body class="CodeChat-theme-light">
                    {sidebar_iframe}
                    <div id="CodeChat-contents">
                        <div id="CodeChat-top">
                            <div id="CodeChat-filename">
                                <p>
                                    {name} - {dir}
                                </p>
                            </div>
                            <div id="CodeChat-menu"></div>
                        </div>
                        <div id="CodeChat-body"></div>
                        <div id="CodeChat-bottom"></div>
                        <div id="mocha"></div>
                    </div>
                </body>
            </html>"#
        )),
        // If this file is editable and is the main file, send an `Update`. The
        // `simple_http_response` contains the Client.
        Some(EditorMessageContents::Update(UpdateMessageContents {
            file_path: file_path.to_string(),
            contents: Some(codechat_for_web),
            cursor_position: None,
            scroll_position: None,
        })),
    )
}

/// Websockets
/// ----------
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

        // Keep track of pending messages.
        let mut pending_messages: HashMap<u64, JoinHandle<()>> = HashMap::new();

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
        //     should be immediately closed; there's no longer the
        //     opportunity to perform a more controlled shutdown (see the
        //     first case).
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
                                                "Unable to decode JSON message from the CodeChat Editor IDE or client: {err}.\nText was: '{b}'."
                                            );
                                            break;
                                        }
                                        Ok(joint_message) => {
                                            // If this was a `Result`, remove it from
                                            // the pending queue.
                                            if let EditorMessageContents::Result(_) = joint_message.message {
                                                // Cancel the timeout for this result.
                                                if let Some(task) = pending_messages.remove(&joint_message.id.to_bits()) {
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
                                                        message: EditorMessageContents::Result(Err(msg))
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
                                    queue_send!(from_websocket_tx.send(EditorMessage { id: 0.0, message: EditorMessageContents::Closed }));
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
                Some(m) = to_websocket_rx.recv() => {
                    // Pre-process this message.
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
                        // All other messages are added to the pending queue and
                        // assigned a unique id.
                        _ => {
                            let timeout_tx = from_websocket_tx.clone();
                            let waiting_task = actix_rt::spawn(async move {
                                sleep(REPLY_TIMEOUT).await;
                                let msg = format!("Timeout: message id {} unacknowledged.", m.id);
                                error!("{msg}");
                                // Since the websocket failed to send a
                                // `Result`, produce a timeout `Result` for it.
                                'timeout: {
                                        queue_send!(timeout_tx.send(EditorMessage {
                                        id: m.id,
                                        message: EditorMessageContents::Result(Err(msg))
                                    }), 'timeout);
                                }
                            });
                            pending_messages.insert(m.id.to_bits(), waiting_task);
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

// Webserver core
// --------------
#[actix_web::main]
pub async fn main(port: u16) -> std::io::Result<()> {
    run_server(port).await
}

pub async fn run_server(port: u16) -> std::io::Result<()> {
    // Connect to the Capture Database
    //let _event_capture = EventCapture::new("config.json").await?;

    // Pre-load the bundled files before starting the webserver.
    let _ = &*BUNDLED_FILES_MAP;
    let app_data = make_app_data(port);
    let app_data_server = app_data.clone();
    let server = match HttpServer::new(move || configure_app(App::new(), &app_data_server))
        .bind((IP_ADDRESS, port))
    {
        Ok(server) => server.run(),
        Err(err) => {
            error!("Unable to bind to {IP_ADDRESS}:{port} - {err}");
            return Err(err);
        }
    };
    // Store the server handle in the global state.
    *(app_data.server_handle.lock().unwrap()) = Some(server.handle());
    // Start the server.
    server.await
}

pub fn configure_logger(level: LevelFilter) {
    #[cfg(not(debug_assertions))]
    let l4rs = ROOT_PATH.clone();
    #[cfg(debug_assertions)]
    let mut l4rs = ROOT_PATH.clone();
    #[cfg(debug_assertions)]
    l4rs.push("server");
    log4rs::init_file(l4rs.join("log4rs.yml"), Default::default()).unwrap();
    log::set_max_level(level);
}

// Quoting the [docs](https://actix.rs/docs/application#shared-mutable-state),
// "To achieve *globally* shared state, it must be created **outside** of the
// closure passed to `HttpServer::new` and moved/cloned in." Putting this code
// inside `configure_app` places it inside the closure which calls
// `configure_app`, preventing globally shared state.
fn make_app_data(port: u16) -> web::Data<AppState> {
    web::Data::new(AppState {
        server_handle: Mutex::new(None),
        connection_id: Mutex::new(0),
        port,
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
        .service(vscode_client_framework)
        .service(ping)
        .service(stop)
        // Reroute to the filewatcher filesystem for typical user-requested
        // URLs.
        .route("/", web::get().to(filewatcher_root_fs_redirect))
        .route("/fw/fsb", web::get().to(filewatcher_root_fs_redirect))
}

// Utilities
// ---------
//
// Send a response to the client after processing a message from the client.
async fn send_response(client_tx: &Sender<EditorMessage>, id: f64, result: MessageResult) {
    if let Err(err) = client_tx
        .send(EditorMessage {
            id,
            message: EditorMessageContents::Result(result),
        })
        .await
    {
        error!("Unable to enqueue: {err}");
    }
}

// Convert a URL referring to a file in the filesystem into the path to that
// file.
fn url_to_path(
    // The URL for the file.
    url_string: &str,
    // An array of URL path segments; the URL must start with these. They will
    // be dropped from the resulting file's path.
    expected_prefix: &[&str],
    // Output: the resulting path to the file, or a string explaining why an
    // error occurred during conversion.
) -> Result<PathBuf, String> {
    // Parse to a URL, then split it to path segments.
    let url = Url::parse(url_string)
        .map_err(|e| format!("Error: unable to parse URL {url_string}: {e}"))?;
    let path_segments_vec: Vec<_> = url
        .path_segments()
        .ok_or_else(|| format!("Error: URL {url} cannot be a base."))?
        .collect();

    // Make sure the path segments start with the `expected_prefix`.
    let prefix_equal = expected_prefix
        .iter()
        .zip(&path_segments_vec)
        .all(|(a, b)| a == b);
    // The URL should have at least the expected prefix plus one more element
    // (the connection ID).
    if path_segments_vec.len() < expected_prefix.len() + 1 || !prefix_equal {
        return Err(format!("Error: URL {url} has incorrect prefix."));
    }

    // Strip the expected prefix; the remainder is a file path.
    let path_segments_suffix = path_segments_vec[expected_prefix.len() + 1..].to_vec();

    // URL decode each segment; however, re-encode the `\`, since this isn't a
    // valid path separator in a URL but is incorrectly treated as such on
    // Windows.
    let path_segments_suffix_decoded = path_segments_suffix
        .iter()
        .map(|path_segment| {
            urlencoding::decode(path_segment)
                .map_err(|e| format!("Error: unable to decode URL {url_string}: {e}."))
                .map(|path_seg| path_seg.replace("\\", "%5C"))
        })
        .collect::<Result<Vec<String>, String>>()?;

    // Join the segments into a path.
    let path_str = path_segments_suffix_decoded.join(MAIN_SEPARATOR_STR);

    // On non-Windows systems, the path should start with a `/`. Windows paths
    // should already start with a drive letter.
    #[cfg(not(target_os = "windows"))]
    let path_str = "/".to_string() + &path_str;

    try_canonicalize(&path_str)
}

// Given a string representing a file, transform it into a `PathBuf`. Correct it
// as much as possible:
//
// 1.  Convert Linux path separators to this platform's path separators.
// 2.  If the file exists and if this is Windows, correct case based on the
//     actual file's naming (even though the filesystem is case-insensitive;
//     this makes comparisons in the TypeScript simpler).
fn try_canonicalize(file_path: &str) -> Result<PathBuf, String> {
    match PathBuf::from_str(file_path) {
        Err(err) => Err(format!(
            "Error: unable to parse file path {file_path}: {err}."
        )),
        Ok(path_buf) => match path_buf.canonicalize() {
            Ok(p) => Ok(PathBuf::from(simplified(&p))),
            // [Canonicalize](https://doc.rust-lang.org/stable/std/fs/fn.canonicalize.html#errors)
            // fails if the path doesn't exist. For unsaved files, this is
            // expected; in this case, we can't correct case based on the actual
            // file's naming. If the path isn't already absolute, don't make it
            // absolute, since a newly-created, unsaved file (in at least
            // VSCode) doesn't have specific path/location in the filesystem;
            // assuming a path by making it absolute causes the IDE to not
            // recognize the file name with this assume absolute location. On
            // the other hand, if the path is already absolute, then call
            // `absolute` to clean up forward vs. backward slashes.
            Err(_) => {
                if path_buf.is_absolute() {
                    match path::absolute(&path_buf) {
                        Err(err) => Err(format!("Unable to make {path_buf:?} absolute: {err}")),
                        Ok(p) => Ok(p),
                    }
                } else {
                    Ok(path_buf)
                }
            }
        },
    }
}

// Given a file path, convert it to a URL, encoding as necessary.
fn path_to_url(prefix: &str, connection_id: &str, file_path: &Path) -> String {
    // First, convert the path to use forward slashes.
    let pathname = simplified(file_path)
        .to_slash()
        .unwrap()
        // The convert each part of the path to a URL-encoded string. (This
        // avoids encoding the slashes.)
        .split("/")
        .map(|s| urlencoding::encode(s))
        // Then put it all back together.
        .collect::<Vec<_>>()
        .join("/");
    format!("{prefix}/{connection_id}/{pathname}")
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
    formatdoc!(
        r#"
        <!DOCTYPE html>
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
