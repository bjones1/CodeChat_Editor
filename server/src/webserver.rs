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
/// `webserver.rs` -- Serve CodeChat Editor Client webpages
/// ============================================================================
// Submodules
// -----------------------------------------------------------------------------
#[cfg(test)]
pub mod tests;

// Imports
// -----------------------------------------------------------------------------
//
// ### Standard library
use std::{
    collections::{HashMap, HashSet},
    env, fs, io,
    net::SocketAddr,
    path::{self, MAIN_SEPARATOR_STR, Path, PathBuf},
    str::FromStr,
    string::FromUtf8Error,
    sync::{Arc, Mutex},
    time::Duration,
};

// ### Third-party
use actix_files;
use actix_web::{
    App, HttpRequest, HttpResponse, HttpServer,
    dev::{Server, ServerHandle, ServiceFactory, ServiceRequest},
    error::Error,
    get,
    http::header::{ContentType, DispositionType},
    middleware,
    web::{self, Data},
};
use actix_web_httpauth::{extractors::basic::BasicAuth, middleware::HttpAuthentication};
use actix_ws::AggregatedMessage;
use bytes::Bytes;
use dunce::simplified;
use futures_util::StreamExt;
use indoc::{concatdoc, formatdoc};
use lazy_static::lazy_static;
use log::{LevelFilter, error, info, warn};
use log4rs::{self, config::load_config_file};
use mime::Mime;
use mime_guess;
use path_slash::{PathBufExt, PathExt};
use serde::{Deserialize, Serialize};
use serde_json;
use tokio::{
    fs::File,
    io::AsyncReadExt,
    process::Command,
    select,
    sync::{
        mpsc::{Receiver, Sender},
        oneshot,
    },
    task::JoinHandle,
    time::sleep,
};
use ts_rs::TS;
use url::Url;

// ### Local
//use crate::capture::EventCapture;
use crate::{
    ide::{
        filewatcher::{
            filewatcher_browser_endpoint, filewatcher_client_endpoint,
            filewatcher_root_fs_redirect, filewatcher_websocket,
        },
        vscode::{
            serve_vscode_fs, vscode_client_framework, vscode_client_websocket, vscode_ide_websocket,
        },
    },
    processing::{
        CodeChatForWeb, SourceToCodeChatForWebError, TranslationResultsString, find_path_to_toc,
        source_to_codechat_for_web_string,
    },
};

// Data structures
// -----------------------------------------------------------------------------
//
// ### Data structures supporting a websocket connection between the IDE, this
//
// server, and the CodeChat Editor Client
/// Provide queues which send data to the IDE and the CodeChat Editor Client.
#[derive(Debug)]
pub struct WebsocketQueues {
    pub from_websocket_tx: Sender<EditorMessage>,
    pub to_websocket_rx: Receiver<EditorMessage>,
}

#[derive(Debug)]
/// Since an `HttpResponse` doesn't implement `Send`, use this as a simply proxy
/// for it. This is used to send a response to the HTTP task to an HTTP request
/// made to that task. Send: String, response
pub struct ProcessingTaskHttpRequest {
    /// The URL provided by this request.
    pub url: String,
    /// The path of the file requested.
    pub file_path: PathBuf,
    /// Flags for this file: none, TOC, raw.
    flags: ProcessingTaskHttpRequestFlags,
    /// True if test mode is enabled.
    is_test_mode: bool,
    /// A queue to send the response back to the HTTP task.
    pub response_queue: oneshot::Sender<SimpleHttpResponse>,
}

#[derive(Debug, PartialEq)]
enum ProcessingTaskHttpRequestFlags {
    // No flags provided.
    None,
    // This file is a TOC.
    Toc,
    /// This should be sent as the raw file.
    Raw,
}

/// Since an `HttpResponse` doesn't implement `Send`, use this as a proxy to
/// cover all responses to serving a file.
#[derive(Debug)]
pub enum SimpleHttpResponse {
    /// Return a 200 with the provided string as the HTML body.
    Ok(String),
    /// Return an error as the HTML body.
    Err(SimpleHttpResponseError),
    /// Serve the raw file content, using the provided content type.
    Raw(String, Mime),
    /// The file contents are not UTF-8; serve it from the filesystem path
    /// provided.
    Bin(PathBuf),
}

// List all the possible errors when responding to an HTTP request. See
// [The definitive guide to error handling in Rust](https://www.howtocodeit.com/articles/the-definitive-guide-to-rust-error-handling).
#[derive(Debug, thiserror::Error)]
pub enum SimpleHttpResponseError {
    #[error("Error opening file")]
    Io(#[from] io::Error),
    #[error("Project path {0:?} has no final component.")]
    ProjectPathShort(PathBuf),
    #[error("Path {0:?} cannot be translated to a string.")]
    PathNotString(PathBuf),
    #[error("Path {0:?} is not a project.")]
    PathNotProject(PathBuf),
    #[error("Bundled file {0} not found.")]
    BundledFileNotFound(String),
    #[error("Lexer error: {0}.")]
    LexerError(#[from] SourceToCodeChatForWebError),
}

/// Define the data structure used to pass data between the CodeChat Editor
/// Client, the IDE, and the CodeChat Editor Server.
#[derive(Debug, Serialize, Deserialize, PartialEq, TS)]
#[ts(export)]
pub struct EditorMessage {
    /// A value unique to this message; it's used to report results
    /// (success/failure) back to the sender.
    pub id: f64,
    /// The actual message.
    pub message: EditorMessageContents,
}

/// Define the data structure used to pass data between the CodeChat Editor
/// Client, the CodeChat Editor IDE extension, and the CodeChat Editor Server.
#[derive(Debug, Serialize, Deserialize, PartialEq, TS)]
#[ts(export)]
pub enum EditorMessageContents {
    // #### These messages may be sent by either the IDE or the Client.
    /// This sends an update; any missing fields are unchanged. Valid
    /// destinations: IDE, Client.
    Update(UpdateMessageContents),
    /// Specify the current file to edit. Valid destinations: IDE, Client.
    CurrentFile(
        // A path/URL to this file.
        String,
        // True if the file is text; False if it's binary; None if the file's
        // type hasn't been determined. This is only used by the IDE, which
        // needs to know whether it's opening a text file or a binary file. When
        // sending this message, the IDE and Client can both send `None`; the
        // Server will determine the value if needed.
        Option<bool>,
    ),

    // #### These messages may only be sent by the IDE.
    /// This is the first message sent when the IDE starts up. It may only be
    /// sent at startup. Valid destinations: Server.
    Opened(IdeType),
    /// Request the Client to save any unsaved data then close. Valid
    /// destinations: Client.
    RequestClose,

    // #### This message may only be sent by the Client to the Server.
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

/// The contents of a `Result` message. We can't export this type, since `ts-rs`
/// only supports structs and enums.
pub type MessageResult = Result<
    // The result of the operation, if successful.
    ResultOkTypes,
    // The error message.
    ResultErrTypes,
>;

#[derive(Debug, Serialize, Deserialize, PartialEq, TS)]
pub enum ResultOkTypes {
    /// Most messages have no result.
    Void,
    /// The `LoadFile` message provides file contents and a revision number, if
    /// available. This message may only be sent from the IDE to the Server.
    LoadFile(Option<(String, f64)>),
}

#[derive(Debug, Serialize, Deserialize, TS, PartialEq, thiserror::Error)]
pub enum ResultErrTypes {
    #[error("File out of sync; update rejected")]
    OutOfSync,
    #[error("IDE must not send this message")]
    IdeIllegalMessage,
    #[error("Client not allowed to send this message")]
    ClientIllegalMessage,
    #[error("Client must not receive this message: {0}")]
    ClientIllegalMessageReceived(String),
    #[error("timeout: message id {0} unacknowledged")]
    MessageTimeout(f64),
    #[error("unable to convert path {0:?} to string")]
    NoPathToString(PathBuf),
    // We can't pass the full error, since it's not serializable.
    #[error("unable to convert URL {0} to path: {1}")]
    UrlToPathError(String, String),
    #[error("unable to canonicalize path: {0}")]
    TryCanonicalizeError(String),
    #[error("source incorrectly recognized as a TOC")]
    NotToc,
    #[error("unable to translate source to CodeChat: {0}")]
    CannotTranslateSource(String),
    #[error("unable to translate CodeChat to source: {0}")]
    CannotTranslateCodeChat(String),
    #[error("TODO: support for updates with diffable sources")]
    TodoDiffSupport,
    #[error("TODO: support for binary files")]
    TodoBinarySupport,
    #[error("unable to open web browser: {0}")]
    WebBrowserOpenFailed(String),
    #[error("unexpected message {0}")]
    UnexpectedMessage(String),
    #[error("invalid IDE type: {0:?}")]
    InvalidIdeType(IdeType),
    #[error("update for file '{0}' doesn't match current file '{1:?}'")]
    WrongFileUpdate(String, Option<PathBuf>),
    #[error("file watcher error: {0}")]
    FileWatchingError(String),
    #[error("unable to unwatch file '{0}': {1}")]
    FileUnwatchError(PathBuf, String),
    #[error("unable to save file '{0}': {1}")]
    SaveFileError(PathBuf, String),
    #[error("unable to watch file '{0}': {1}")]
    FileWatchError(PathBuf, String),
    #[error("ignoring update for {0} because it's not the current file {1}")]
    IgnoredUpdate(String, String),
    #[error("no open document for {0}")]
    NoOpenDocument(String),
    #[error("unable to open file {0}: {1}")]
    OpenFileFailed(String, String),
}

/// Specify the type of IDE that this client represents.
#[derive(Debug, Serialize, Deserialize, PartialEq, TS)]
pub enum IdeType {
    /// True if the CodeChat Editor will be hosted inside VSCode; false means it
    /// should be hosted in an external browser.
    VSCode(bool),
    /// Another option -- temporary -- to allow for future expansion.
    DeleteMe,
}

/// Contents of the `Update` message.
#[derive(Debug, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, optional_fields)]
pub struct UpdateMessageContents {
    /// The filesystem path to this file. This is only used by the IDE to
    /// determine which file to apply Update contents to. The Client stores then
    /// then sends it back to the IDE in `Update` messages. This helps deal with
    /// transition times when the IDE and Client have different files loaded,
    /// guaranteeing to updates are still applied to the correct file.
    pub file_path: String,
    /// The contents of this file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contents: Option<CodeChatForWeb>,
    /// The line in the file where the cursor is located. TODO: Selections are
    /// not yet supported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor_position: Option<u32>,
    /// The line at the top of the screen.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scroll_position: Option<f32>,
}

/// ### Data structures used by the webserver
///
/// Define the [state](https://actix.rs/docs/application/#state) available to
/// all endpoints.
pub struct AppState {
    /// Provide methods to control the server.
    server_handle: Mutex<Option<ServerHandle>>,
    /// The number of the next connection ID to assign for the filewatcher.
    pub filewatcher_next_connection_id: Mutex<u32>,
    /// The port this server listens on.
    pub port: Arc<Mutex<u16>>,
    /// For each connection ID, store a queue tx for the HTTP server to send
    /// requests to the processing task for that ID.
    pub processing_task_queue_tx: Arc<Mutex<HashMap<String, Sender<ProcessingTaskHttpRequest>>>>,
    /// For each connection ID, store the queues for the IDE and Client.
    pub ide_queues: Arc<Mutex<HashMap<String, WebsocketQueues>>>,
    pub client_queues: Arc<Mutex<HashMap<String, WebsocketQueues>>>,
    /// Connection IDs that are currently in use.
    pub connection_id: Arc<Mutex<HashSet<String>>>,
    /// The auth credentials if authentication is used.
    credentials: Option<Credentials>,
}

pub type WebAppState = web::Data<AppState>;

#[derive(Clone)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

// Macros
// -----------------------------------------------------------------------------
/// Create a macro to report an error when enqueueing an item.
#[macro_export]
macro_rules! queue_send {
    ($tx: expr) => {
        if let Err(err) = $tx.await {
            error!("Unable to enqueue: {err:?}");
            break;
        }
    };
    ($tx: expr, $label: tt) => {
        if let Err(err) = $tx.await {
            error!("Unable to enqueue: {err:?}");
            break $label;
        }
    };
}

#[macro_export]
macro_rules! queue_send_func {
    ($tx: expr) => {
        if let Err(err) = $tx.await {
            error!("Unable to enqueue: {err:?}");
            return false;
        }
    };
}

/// Globals
/// ----------------------------------------------------------------------------
// The timeout for a reply from a websocket, in ms. Use a short timeout to speed
// up unit tests.
pub const REPLY_TIMEOUT_MS: Duration = if cfg!(test) {
    Duration::from_millis(500)
} else {
    Duration::from_millis(1500000)
};

/// The time to wait for a pong from the websocket in response to a ping sent by
/// this server.
const WEBSOCKET_PING_DELAY: Duration = Duration::from_secs(2);

/// A few message IDs reserve for used during startup or for sending errors.
pub const RESERVED_MESSAGE_ID: f64 = 0.0;
/// The initial value for the server's message ID.
pub const INITIAL_MESSAGE_ID: f64 = RESERVED_MESSAGE_ID + 3.0;
// The initial value for a Client.
pub const INITIAL_CLIENT_MESSAGE_ID: f64 = INITIAL_MESSAGE_ID + 1.0;
// The initial value for an IDE.
pub const INITIAL_IDE_MESSAGE_ID: f64 = INITIAL_CLIENT_MESSAGE_ID + 1.0;
/// The increment for a message ID. Since the Client, IDE, and Server all
/// increment by this same amount but start at different values, this ensures
/// that message IDs will be unique. (Given a mantissa of 53 bits plus a sign
/// bit, 2^54 seconds = 574 million years before the message ID wraps around
/// assuming an average of 1 message/second.)
pub const MESSAGE_ID_INCREMENT: f64 = 3.0;

/// Synchronization state between the Client, Server, and IDE.
#[derive(PartialEq)]
pub enum SyncState {
    /// Indicates the Client, IDE, and server's documents are identical.
    InSync,
    /// An Update message is in flight; the documents are out of sync until the
    /// response to the Update is received.
    Pending(f64),
    /// A CurrentFile message was sent, guaranteeing that documents are out of
    /// sync.
    OutOfSync,
}

const MATHJAX_TAGS: &str = concatdoc!(
    r#"
    <script>
        MathJax = {"#,
    // See the
    // [docs](https://docs.mathjax.org/en/latest/options/output/chtml.html#option-descriptions),
    // [postFilters](https://docs.mathjax.org/en/latest/options/output/index.html#output-postfilters);
    // see also the
    // [TinyMCE non-editable class](https://www.tiny.cloud/docs/tinymce/latest/non-editable-content-options/#noneditable_class).
    // After some experimentation, I discovered:
    //
    // * Setting the `classList` had no effect. I still think it's a good idea
    //   for the future, though.
    // * I can't use the `postFilter` to enclose this in a span with the
    //   appropriate class; MathJax disallows editing the `mjx-container`
    //   element.
    // * Simply setting `contentEditable` is what actually works.
    r#"
            chtml: {
                fontURL: "/static/mathjax-newcm-font/chtml/woff2",
            },
            output: {
                postFilters: [(obj) => {
                    obj.data.classList.add("mceNonEditable");
                    obj.data.contentEditable = false;
                }],
            },
            tex: {
                inlineMath: [['$', '$'], ['\\(', '\\)']]
            },
        };
    </script>"#,
    // Per the
    // [MathJax docs](https://docs.mathjax.org/en/latest/web/components/combined.html#tex-chtml),
    // enable tex input and HTML output.
    r#"
    <script defer src="/static/mathjax/tex-chtml.js"></script>"#
);

lazy_static! {
    pub static ref ROOT_PATH: Arc<Mutex<PathBuf>> = Arc::new(Mutex::new(PathBuf::new()));

    // Define the location of static files.
    static ref CLIENT_STATIC_PATH: PathBuf = {
        let mut client_static_path = ROOT_PATH.lock().unwrap().clone();
        #[cfg(debug_assertions)]
        client_static_path.push("client");

        client_static_path.push("static");
        client_static_path
    };

    // Read in the hashed names of files bundled by esbuild.
    static ref BUNDLED_FILES_MAP: HashMap<String, String> = {
        let mut hl = ROOT_PATH.lock().unwrap().clone();
        #[cfg(debug_assertions)]
        hl.push("server");
        hl.push("hashLocations.json");
        let json = fs::read_to_string(hl.clone()).unwrap_or_else(|_| format!(r#"{{"error": "Unable to read {:#?}"}}"#, hl.to_string_lossy()));
        let hmm: HashMap<String, String> = serde_json::from_str(&json).unwrap_or_else(|_| HashMap::new());
        hmm
    };

    static ref CODECHAT_EDITOR_FRAMEWORK_JS: String = BUNDLED_FILES_MAP.get("CodeChatEditorFramework.js").cloned().unwrap_or("Not found".to_string());
    static ref CODECHAT_EDITOR_PROJECT_CSS: String = BUNDLED_FILES_MAP.get("CodeChatEditorProject.css").cloned().unwrap_or("Not found".to_string());
}

// Define the location of the root path, which contains `static/`, `log4rs.yml`,
// and `hashLocations.json` in a production build, or `client/` and `server/` in
// a development build.
pub fn set_root_path(
    // The path where this extension's files reside. `None` if this is running
    // an a standalone server, instead of as an extension loaded by an IDE.
    extension_base_path: Option<&Path>,
) -> io::Result<()> {
    // If the extension provided a base path, use that; otherwise, get the path
    // to this executable.
    let exe_path;
    let exe_dir = if let Some(ed) = extension_base_path {
        ed
    } else {
        exe_path = env::current_exe().unwrap();
        exe_path.parent().unwrap()
    };
    #[cfg(not(any(test, debug_assertions)))]
    let root_path = PathBuf::from(exe_dir);
    #[cfg(any(test, debug_assertions))]
    let mut root_path = PathBuf::from(exe_dir);
    // When in debug or running tests, use the layout of the Git repo to find
    // client files. In release mode, we assume the static folder is a
    // subdirectory of the directory containing the executable.
    #[cfg(test)]
    // In development, this extra directory level for the extension isn't
    // needed.
    if extension_base_path.is_none() {
        root_path.push("..");
    }
    // Note that `debug_assertions` is also enabled for testing, so this adds to
    // the previous line when running tests.
    #[cfg(debug_assertions)]
    root_path.push(if extension_base_path.is_some() {
        "../.."
    } else {
        "../../.."
    });
    *ROOT_PATH.lock().unwrap() = root_path.canonicalize()?;
    Ok(())
}

// Webserver functionality
// -----------------------------------------------------------------------------
#[get("/ping")]
async fn ping() -> HttpResponse {
    HttpResponse::Ok().body("pong")
}

#[get("/stop")]
async fn stop(app_state: WebAppState) -> HttpResponse {
    let Some(ref server_handle) = *app_state.server_handle.lock().unwrap() else {
        error!("Server handle not available to stop server.");
        return HttpResponse::InternalServerError().finish();
    };
    // Don't await this, since that shuts down the server, preventing the
    // following HTTP response. Assign it to a variable to suppress the warning.
    drop(server_handle.stop(true));
    HttpResponse::NoContent().finish()
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
pub fn get_client_framework(
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
    connection_id: String,
    request_file_path: String,
    req: &HttpRequest,
    app_state: &WebAppState,
) -> HttpResponse {
    // On Windows, backslashes in the `request_file_path` will be treated as
    // path separators; however, HTTP does not treat them as path separators.
    // Therefore, re-encode them to prevent inconsistency between the way HTTP
    // and this program interpret file paths. On OS X/Linux, the path starts
    // with a leading slash, which gets absorbed into the URL to prevent a URL
    // such as "/fw/fsc/1//foo/bar/...". Restore it here.
    #[cfg(target_os = "windows")]
    let fixed_file_path = request_file_path.replace("\\", "%5C");
    // On OS X/Linux, the path starts with a leading slash, which gets absorbed
    // into the URL to prevent a URL such as "/fw/fsc/1//foo/bar/...". Restore
    // it here.
    #[cfg(not(target_os = "windows"))]
    let fixed_file_path = format!("/{request_file_path}");
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
    let is_toc = query_params
        .as_ref()
        .is_ok_and(|query| query.get("mode").is_some_and(|mode| mode == "toc"));
    let is_raw = query_params
        .as_ref()
        .is_ok_and(|query| query.get("raw").is_some());
    let is_test_mode = get_test_mode(req);
    let flags = if is_toc {
        // Both flags should never be set.
        assert!(!is_raw);
        ProcessingTaskHttpRequestFlags::Toc
    } else if is_raw {
        ProcessingTaskHttpRequestFlags::Raw
    } else {
        ProcessingTaskHttpRequestFlags::None
    };

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
            url: req.path().to_string(),
            file_path,
            flags,
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
            SimpleHttpResponse::Err(body) => html_not_found(&format!("{body}")),
            SimpleHttpResponse::Raw(body, content_type) => {
                HttpResponse::Ok().content_type(content_type).body(body)
            }
            SimpleHttpResponse::Bin(path) => {
                match actix_files::NamedFile::open_async(&path).await {
                    Ok(mut v) => {
                        if path.extension().is_some_and(|ext| ext == "pdf") {
                            let mut cd = v.content_disposition().clone();
                            cd.disposition = DispositionType::Inline;
                            v = v.set_content_disposition(cd);
                        }
                        v.into_response(req)
                    }
                    Err(err) => html_not_found(&format!("<p>Error opening file {path:?}: {err}.",)),
                }
            }
        },
        Err(err) => html_not_found(&format!("Error: {err}")),
    }
}

// Determine if the provided file is text or binary. If text, return it as a
// Unicode string. If binary, return None.
pub async fn try_read_as_text(file: &mut File) -> Option<String> {
    let mut file_contents = String::new();
    // TODO: this is a rather crude way to detect if a file is binary. It's
    // probably slow for large file (the
    // [underlying code](https://github.com/tokio-rs/tokio/blob/master/tokio/src/io/util/read_to_string.rs#L57)
    // looks like it reads the entire file to memory, then converts that to
    // UTF-8). Find a heuristic sniffer instead, such as
    // [libmagic](https://docs.rs/magic/0.13.0-alpha.3/magic/).
    if file.read_to_string(&mut file_contents).await.is_ok() {
        Some(file_contents)
    } else {
        None
    }
}

// Given a text file, determine the appropriate HTTP response: a Client, or the
// file contents itself (if it's not editable by the Client). If responding with
// a Client, also return an Update message which will provided the contents for
// the Client.
pub async fn file_to_response(
    // The HTTP request presented to the processing task.
    http_request: &ProcessingTaskHttpRequest,
    // The version of this file.
    version: f64,
    // Path to the file currently being edited. This path should be cleaned by
    // `try_canonicalize`.
    current_filepath: &Path,
    // Contents of this file, if it's text; None if it was binary data.
    file_contents: Option<&String>,
    // True to use the PDF.js viewer for this file.
    use_pdf_js: bool,
) -> (
    // The response to send back to the HTTP endpoint.
    SimpleHttpResponse,
    // If the response is a Client, also return the appropriate `Update` data to
    // populate the Client with the parsed `file_contents`. In all other cases,
    // return None.
    Option<UpdateMessageContents>,
) {
    // Use a lossy conversion, since this is UI display, not filesystem access.
    let file_path = &http_request.file_path;
    let Some(file_name) = file_path.file_name() else {
        return (
            SimpleHttpResponse::Err(SimpleHttpResponseError::ProjectPathShort(
                file_path.to_path_buf(),
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
    let codechat_editor_js_name = format!("CodeChatEditor{js_test_suffix}.js");
    let Some(codechat_editor_js) = BUNDLED_FILES_MAP.get(&codechat_editor_js_name) else {
        return (
            SimpleHttpResponse::Err(SimpleHttpResponseError::BundledFileNotFound(
                codechat_editor_js_name,
            )),
            None,
        );
    };
    let codechat_editor_css_name = format!("CodeChatEditor{js_test_suffix}.css");
    let Some(codehat_editor_css) = BUNDLED_FILES_MAP.get(&codechat_editor_css_name) else {
        return (
            SimpleHttpResponse::Err(SimpleHttpResponseError::BundledFileNotFound(
                codechat_editor_css_name,
            )),
            None,
        );
    };

    // Compare these files, since both have been canonicalized by
    // `try_canonical`.
    let is_current_file = file_path == current_filepath;
    let is_toc = http_request.flags == ProcessingTaskHttpRequestFlags::Toc;
    let translation_results = if let Some(file_contents_text) = file_contents {
        if is_current_file || is_toc {
            source_to_codechat_for_web_string(
                // Ensure we work with Unix-style (LF only) files, since other
                // line endings break the translation process.
                &file_contents_text.replace("\r\n", "\n"),
                file_path,
                version,
                is_toc,
            )
        } else {
            // If this isn't the current file, then don't parse it.
            Ok((TranslationResultsString::Unknown, None))
        }
    } else {
        Ok((
            TranslationResultsString::Binary,
            find_path_to_toc(file_path),
        ))
    };
    let (translation_results_string, path_to_toc) = match translation_results {
        // Report a lexer error.
        Err(err) => {
            return (
                SimpleHttpResponse::Err(SimpleHttpResponseError::LexerError(err)),
                None,
            );
        }
        Ok(tr) => tr,
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

    // Do we need to respond with a [simple viewer](#Client-simple-viewer)?
    if (translation_results_string == TranslationResultsString::Binary
        || translation_results_string == TranslationResultsString::Unknown)
        && is_project
        && is_current_file
        && http_request.flags != ProcessingTaskHttpRequestFlags::Raw
    {
        let Some(file_name) = file_name.to_str() else {
            return (
                SimpleHttpResponse::Err(SimpleHttpResponseError::PathNotString(PathBuf::from(
                    file_name,
                ))),
                None,
            );
        };
        return (
            make_simple_viewer(
                http_request,
                &if use_pdf_js {
                    // For the [PDF.js viewer](#pdf.js), pass the file to view
                    // as the query parameter.
                    format!(
                        r#"<iframe src="/static/pdfjs-main.html?{}" style="height: 100vh; border: 0px" id="CodeChat-contents"></iframe>"#,
                        http_request.url
                    )
                } else {
                    format!(
                        r#"<iframe src="{file_name}?raw" style="height: 100vh" id="CodeChat-contents"></iframe>"#
                    )
                },
            ),
            None,
        );
    }

    let codechat_for_web = match translation_results_string {
        // The file type is binary. Ask the HTTP server to serve it raw.
        TranslationResultsString::Binary => return
            (SimpleHttpResponse::Bin(file_path.to_path_buf()), None)
        ,
        // The file type is unknown. Serve it raw.
        TranslationResultsString::Unknown => {
            return (
                SimpleHttpResponse::Raw(
                    file_contents.unwrap().to_string(),
                    mime_guess::from_path(file_path).first_or_text_plain(),
                ),
                None,
            );
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

    // Provided info from the HTTP request, determine the following parameters.
    let Some(raw_dir) = file_path.parent() else {
        return (
            SimpleHttpResponse::Err(SimpleHttpResponseError::ProjectPathShort(
                file_path.to_path_buf(),
            )),
            None,
        );
    };
    let dir = path_display(raw_dir);
    let Some(file_path) = file_path.to_str() else {
        return (
            SimpleHttpResponse::Err(SimpleHttpResponseError::PathNotString(
                file_path.to_path_buf(),
            )),
            None,
        );
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
                    <script type="module">import "/{codechat_editor_js}"</script>
                    <link rel="stylesheet" href="/{codehat_editor_css}">
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
        Some(UpdateMessageContents {
            file_path: file_path.to_string(),
            contents: Some(codechat_for_web),
            cursor_position: None,
            scroll_position: None,
        }),
    )
}

// Create a [Client Simple Viewer](#Client-simple-viewer) (which shows just the
// TOC, then whatever HTML is provided). This is useful to show images/videos,
// unsupported text files, error messages, etc. when inside a project.
fn make_simple_viewer(http_request: &ProcessingTaskHttpRequest, html: &str) -> SimpleHttpResponse {
    // Use a lossy conversion, since this is UI display, not filesystem access.
    let file_path = &http_request.file_path;
    let Some(file_name) = file_path.file_name() else {
        return SimpleHttpResponse::Err(SimpleHttpResponseError::ProjectPathShort(
            file_path.to_path_buf(),
        ));
    };
    let Some(file_name) = file_name.to_str() else {
        return SimpleHttpResponse::Err(SimpleHttpResponseError::PathNotString(
            file_path.to_path_buf(),
        ));
    };
    let file_name = escape_html(file_name);

    let Some(path_to_toc) = find_path_to_toc(file_path) else {
        return SimpleHttpResponse::Err(SimpleHttpResponseError::PathNotProject(
            file_path.to_path_buf(),
        ));
    };
    let Some(path_to_toc) = path_to_toc.to_str() else {
        return SimpleHttpResponse::Err(SimpleHttpResponseError::PathNotString(
            path_to_toc.to_path_buf(),
        ));
    };
    let path_to_toc = escape_html(path_to_toc);

    SimpleHttpResponse::Ok(
        // The JavaScript is a stripped-down version of
        // [on\_navigate from CodeChatEditor.mts](../../client/src/CodeChatEditor.mts).
        formatdoc!(
            r#"
                <!DOCTYPE html>
                <html lang="en">
                    <head>
                        <meta charset="UTF-8">
                        <meta name="viewport" content="width=device-width, initial-scale=1">
                        <title>{file_name} - The CodeChat Editor</title>
                        <script>
                            const on_navigate = (navigateEvent) => {{
                                if (navigateEvent.hashChange ||
                                    navigateEvent.downloadRequest ||
                                    navigateEvent.formData ||
                                    !navigateEvent.canIntercept
                                ) {{
                                    return;
                                }}
                                navigateEvent.intercept();
                                navigation.removeEventListener("navigate", on_navigate);
                                parent.window.CodeChatEditorFramework.webSocketComm.current_file(new URL(navigateEvent.destination.url));
                            }};

                            const on_load_func = () => {{
                                document.getElementById("CodeChat-sidebar").contentWindow.navigation.addEventListener("navigate", on_navigate);
                            }};
                            if (document.readyState === "loading") {{
                                document.addEventListener("DOMContentLoaded", on_load_func);
                            }} else {{
                                on_load_func();
                            }}
                        </script>
                        <link rel="stylesheet" href="/{}">
                    </head>
                    <body class="CodeChat-theme-light">
                        <iframe src="{path_to_toc}?mode=toc" id="CodeChat-sidebar"></iframe>
                        {html}
                    </body>
                </html>"#,
            *CODECHAT_EDITOR_PROJECT_CSS
        ),
    )
}

/// Websockets
/// ----------------------------------------------------------------------------
///
/// Each CodeChat Editor IDE instance pairs with a CodeChat Editor Client
/// through the CodeChat Editor Server. Together, these form a joint editor,
/// allowing the user to edit the plain text of the source code in the IDE, or
/// make GUI-enhanced edits of the source code rendered by the CodeChat Editor
/// Client.
pub fn client_websocket(
    connection_id: String,
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
        let (from_websocket_tx, mut to_websocket_rx) =
            match websocket_queues.lock().unwrap().remove(&connection_id) {
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
        // 1. The IDE plugin needs to close.
        //    1. The IDE plugin sends a `Closed` message.
        //    2. The Client replies with a `Result` message, acknowledging the
        //       close. It sends an `Update` message if necessary to save the
        //       current file.
        //    3. After receiving the acknowledge from the Update message (if
        //       sent), the Client closes the websocket. The rest of this
        //       sequence is covered in the next case.
        // 2. Either websocket is closed. In this case, the other websocket
        //    should be immediately closed; there's no longer the opportunity to
        //    perform a more controlled shutdown (see the first case).
        //    1. The websocket which closed enqueues a `Closed` message for the
        //       other websocket.
        //    2. When the other websocket receives this message, it closes.
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
                                                    let err = ResultErrTypes::ClientIllegalMessage;
                                                    error!("{err}");
                                                    queue_send!(from_websocket_tx.send(EditorMessage {
                                                        id: joint_message.id,
                                                        message: EditorMessageContents::Result(Err(err))
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
                                sleep(REPLY_TIMEOUT_MS).await;
                                let err = ResultErrTypes::MessageTimeout(m.id);
                                error!("{err}");
                                // Since the websocket failed to send a
                                // `Result`, produce a timeout `Result` for it.
                                'timeout: {
                                        queue_send!(timeout_tx.send(EditorMessage {
                                        id: m.id,
                                        message: EditorMessageContents::Result(Err(err))
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
            to_websocket_rx.close();
            // Stop all timers.
            for (_id, join_handle) in pending_messages.drain() {
                join_handle.abort();
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
// -----------------------------------------------------------------------------
#[actix_web::main]
pub async fn main(
    extension_base_path: Option<&Path>,
    addr: &SocketAddr,
    credentials: Option<Credentials>,
    level: LevelFilter,
) -> std::io::Result<()> {
    init_server(extension_base_path, level)?;
    let server = setup_server(addr, credentials)?.0;
    server.await
}

// Perform global init of the server. This must only be called once; it must be
// called before the server is run.
pub fn init_server(
    // If provided, the path to the location of this extension's files. This is
    // used to locate static files for the webserver, etc. When None, assume the
    // default layout.
    extension_base_path: Option<&Path>,
    level: LevelFilter,
) -> std::io::Result<()> {
    set_root_path(extension_base_path)?;
    // The unit tests include a test logger; don't config the logger in a test
    // build.
    #[cfg(test)]
    let _ = level;
    #[cfg(not(test))]
    configure_logger(level).map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(())
}

// Set up the server so it's ready to run, but don't start it yet. This does
// check that the assigned `addr` is available, returning an error if not.
pub fn setup_server(
    addr: &SocketAddr,
    credentials: Option<Credentials>,
) -> std::io::Result<(Server, Data<AppState>)> {
    // Connect to the Capture Database
    //let _event_capture = EventCapture::new("config.json").await?;

    // Pre-load the bundled files before starting the webserver.
    let _ = &*BUNDLED_FILES_MAP;
    let app_data = make_app_data(credentials);
    let app_data_server = app_data.clone();
    let server = match HttpServer::new(move || {
        let auth = HttpAuthentication::with_fn(basic_validator);
        configure_app(
            App::new().wrap(middleware::Condition::new(
                app_data_server.credentials.is_some(),
                auth,
            )),
            &app_data_server,
        )
    })
    // We only have one user; don't spawn lots of threads.
    .workers(1)
    .bind(addr)
    {
        Ok(server) => {
            // Store the port in the global state. Use the port of the first
            // bound address.
            *app_data.port.lock().unwrap() = server.addrs()[0].port();
            server.run()
        }
        Err(err) => {
            error!("Unable to bind to {addr} - {err}");
            return Err(err);
        }
    };
    // Store the server handle in the global state.
    *(app_data.server_handle.lock().unwrap()) = Some(server.handle());

    Ok((server, app_data))
}

// Use HTTP basic authentication (if provided) to mediate access.
async fn basic_validator(
    req: ServiceRequest,
    credentials: BasicAuth,
) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    // Get the provided credentials.
    let expected_credentials = &req
        .app_data::<WebAppState>()
        .unwrap()
        .credentials
        .as_ref()
        .unwrap();
    if credentials.user_id() == expected_credentials.username
        && credentials.password() == Some(&expected_credentials.password)
    {
        Ok(req)
    } else {
        Err((
            actix_web::error::ErrorUnauthorized("Incorrect username or password."),
            req,
        ))
    }
}

pub fn configure_logger(level: LevelFilter) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(not(debug_assertions))]
    let l4rs = ROOT_PATH.lock().unwrap().clone();
    #[cfg(debug_assertions)]
    let mut l4rs = ROOT_PATH.lock().unwrap().clone();
    #[cfg(debug_assertions)]
    l4rs.push("server");
    let config_file = l4rs.join("log4rs.yml");
    let mut config = match load_config_file(&config_file, Default::default()) {
        Ok(c) => c,
        Err(err) => return Err(err.into()),
    };
    config.root_mut().set_level(level);
    log4rs::init_config(config)?;
    Ok(())
}

// Quoting the [docs](https://actix.rs/docs/application#shared-mutable-state),
// "To achieve *globally* shared state, it must be created **outside** of the
// closure passed to `HttpServer::new` and moved/cloned in." Putting this code
// inside `configure_app` places it inside the closure which calls
// `configure_app`, preventing globally shared state.
pub fn make_app_data(credentials: Option<Credentials>) -> WebAppState {
    web::Data::new(AppState {
        server_handle: Mutex::new(None),
        filewatcher_next_connection_id: Mutex::new(0),
        // Use a dummy value until the server binds to a port.
        port: Arc::new(Mutex::new(0)),
        processing_task_queue_tx: Arc::new(Mutex::new(HashMap::new())),
        ide_queues: Arc::new(Mutex::new(HashMap::new())),
        client_queues: Arc::new(Mutex::new(HashMap::new())),
        connection_id: Arc::new(Mutex::new(HashSet::new())),
        credentials,
    })
}

// Configure the web application. I'd like to make this return an
// `App<AppEntry>`, but `AppEntry` is a private module.
pub fn configure_app<T>(app: App<T>, app_data: &WebAppState) -> App<T>
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
// -----------------------------------------------------------------------------
//
// Send a response to the client after processing a message from the client.
pub async fn send_response(client_tx: &Sender<EditorMessage>, id: f64, result: MessageResult) {
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

#[derive(Debug, thiserror::Error)]
pub enum UrlToPathError {
    #[error("unable to parse URL")]
    ParseError(#[from] url::ParseError),
    #[error("URL {0} cannot be a base.")]
    NotBase(String),
    #[error("URL {0} has incorrect prefix.")]
    IncorrectPrefix(String),
    #[error("unable to decode URL")]
    UnableToDecode(#[from] FromUtf8Error),
    #[error(transparent)]
    UrlNotFile(#[from] TryCanonicalizeError),
}

// Convert a URL referring to a file in the filesystem into the path to that
// file.
pub fn url_to_path(
    // The URL for the file.
    url_string: &str,
    // An array of URL path segments; the URL must start with these. They will
    // be dropped from the resulting file's path.
    expected_prefix: &[&str],
    // Output: the resulting path to the file, or a string explaining why an
    // error occurred during conversion.
) -> Result<PathBuf, UrlToPathError> {
    // Parse to a URL, then split it to path segments.
    let url = Url::parse(url_string)?;
    let path_segments_vec: Vec<_> = url
        .path_segments()
        .ok_or_else(|| UrlToPathError::NotBase(url_string.to_string()))?
        .collect();

    // Make sure the path segments start with the `expected_prefix`.
    let prefix_equal = expected_prefix
        .iter()
        .zip(&path_segments_vec)
        .all(|(a, b)| a == b);
    // The URL should have at least the expected prefix plus one more element
    // (the connection ID).
    if path_segments_vec.len() < expected_prefix.len() + 1 || !prefix_equal {
        return Err(UrlToPathError::IncorrectPrefix(url_string.to_string()));
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
                .map_err(UrlToPathError::UnableToDecode)
                .map(|path_seg| path_seg.replace("\\", "%5C"))
        })
        .collect::<Result<Vec<String>, UrlToPathError>>()?;

    // Join the segments into a path.
    let path_str = path_segments_suffix_decoded.join(MAIN_SEPARATOR_STR);

    // On non-Windows systems, the path should start with a `/`. Windows paths
    // should already start with a drive letter.
    #[cfg(not(target_os = "windows"))]
    let path_str = "/".to_string() + &path_str;

    try_canonicalize(&path_str).map_err(UrlToPathError::UrlNotFile)
}

#[derive(Debug, thiserror::Error)]
pub enum TryCanonicalizeError {
    #[error("unable to parse {file_path} into file path: {error}")]
    ParseFailure { file_path: String, error: String },
    #[error("unable to make file path absolute")]
    CannotAbsolute(#[from] io::Error),
}
// Given a string representing a file, transform it into a `PathBuf`. Correct it
// as much as possible:
//
// 1. Convert Linux path separators to this platform's path separators.
// 2. If the file exists and if this is Windows, correct case based on the
//    actual file's naming (even though the filesystem is case-insensitive; this
//    makes comparisons in the TypeScript simpler).
pub fn try_canonicalize(file_path: &str) -> Result<PathBuf, TryCanonicalizeError> {
    match PathBuf::from_str(file_path) {
        Err(err) => Err(TryCanonicalizeError::ParseFailure {
            file_path: file_path.to_string(),
            error: err.to_string(),
        }),
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
                        Err(err) => Err(TryCanonicalizeError::CannotAbsolute(err)),
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
pub fn path_to_url(prefix: &str, connection_id: Option<&str>, file_path: &Path) -> String {
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
    // On Windows, path names start with a drive letter. On Linux/OS X, they
    // start with a forward slash -- don't put a double forward slash in the
    // resulting path.
    let pathname = drop_leading_slash(&pathname);
    if let Some(connection_id) = connection_id {
        format!("{prefix}/{connection_id}/{pathname}")
    } else {
        format!("{prefix}/{pathname}")
    }
}

// Given a string (which is probably a pathname), drop the leading slash if it's
// present.
pub fn drop_leading_slash(path_: &str) -> &str {
    if path_.starts_with("/") {
        let mut chars = path_.chars();
        chars.next();
        chars.as_str()
    } else {
        path_
    }
}

// Given a `Path`, transform it into a displayable HTML string (with any
// necessary escaping).
pub fn path_display(p: &Path) -> String {
    escape_html(&simplified(p).to_string_lossy())
}

// Return a Not Found (404) error with the provided HTML body.
pub fn html_not_found(msg: &str) -> HttpResponse {
    HttpResponse::NotFound()
        .content_type(ContentType::html())
        .body(html_wrapper(msg))
}

// Wrap the provided HTML body in DOCTYPE/html/head tags.
pub fn html_wrapper(body: &str) -> String {
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
pub fn escape_html(unsafe_text: &str) -> String {
    unsafe_text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// This lists all errors produced by calling `get_server_url`. TODO: rework and
// re-think the overall error framework. How should I group errors?
#[derive(Debug, thiserror::Error)]
pub enum GetServerUrlError {
    #[error("Expected environment variable not found.")]
    Io(#[from] env::VarError),
    #[error("Error running process.")]
    Process(#[from] std::io::Error),
    #[error("Process exit status {0:?} indicates error.")]
    NonZeroExitStatus(Option<i32>),
}

// Determine the URL for this server; supports running locally and in a GitHub
// Codespace.
pub async fn get_server_url(port: u16) -> Result<String, GetServerUrlError> {
    // This is always true in a GitHub Codespace per the
    // [docs](https://docs.github.com/en/codespaces/developing-in-codespaces/default-environment-variables-for-your-codespace#list-of-default-environment-variables).
    if env::var("CODESPACES") == Ok("true".to_string()) {
        let codespace_name = env::var("CODESPACE_NAME")?;
        let codespace_domain = env::var("GITHUB_CODESPACES_PORT_FORWARDING_DOMAIN")?;
        // Use the GitHub CLI to
        // [forward this port](https://docs.github.com/en/codespaces/developing-in-a-codespace/using-github-codespaces-with-github-cli#modify-ports-in-a-codespace).
        let status = Command::new("gh")
            .args([
                "codespace",
                "ports",
                "visibility",
                &format!("{port}:public"),
                "-c",
                &codespace_name,
            ])
            .status()
            .await?;
        if !status.success() {
            Err(GetServerUrlError::NonZeroExitStatus(status.code()))
        } else {
            Ok(format!(
                "https://{codespace_name}-{port}.{codespace_domain}"
            ))
        }
    } else {
        // We're running locally, so use localhost.
        Ok(format!("http://127.0.0.1:{port}"))
    }
}
