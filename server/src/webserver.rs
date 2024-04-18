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
///
/// TODO: auto-reload when the current file changes on disk. Use
/// [notify](https://docs.rs/notify/latest/notify/).
///
/// ## Imports
///
/// ### Standard library
use std::{
    borrow::Cow,
    collections::{HashMap, VecDeque},
    env,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Mutex,
};

// ### Third-party
use actix_files;
use actix_web::{
    get,
    http::header,
    http::header::{ContentDisposition, ContentType},
    put, web, App, Error, HttpRequest, HttpResponse, HttpServer, Responder,
};
use actix_ws::Message;
use bytes::Bytes;
use futures_util::StreamExt;
use lazy_static::lazy_static;
use path_slash::PathBufExt;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json;
use tokio::{
    fs::{self, DirEntry, File},
    io::AsyncReadExt,
    sync::mpsc,
    sync::mpsc::{Receiver, Sender},
};
use urlencoding::{self, encode};
#[cfg(target_os = "windows")]
use win_partitions::win_api::get_logical_drive;

// ### Local
use crate::lexer::LanguageLexersCompiled;
use crate::lexer::{compile_lexers, supported_languages::get_language_lexer_vec};
use crate::processing::{codechat_for_web_to_source, source_to_codechat_for_web_string};

/// ## Data structures
///
/// ### Translation between a local (traditional) source file and its web-editable, client-side representation
#[derive(Debug, Serialize, Deserialize, PartialEq)]
/// <a id="LexedSourceFile"></a>Define the JSON data structure used to represent
/// a source file in a web-editable format.
pub struct CodeChatForWeb<'a> {
    pub metadata: SourceFileMetadata,
    pub source: CodeMirror<'a>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
/// <a id="SourceFileMetadata"></a>Metadata about a source file sent along with
/// it both to and from the client. TODO: currently, this is too simple to
/// justify a struct. This allows for future growth -- perhaps the valid types
/// of comment delimiters?
pub struct SourceFileMetadata {
    pub mode: String,
}

pub type CodeMirrorDocBlocks<'a> = Vec<(
    // From -- the starting character this doc block is anchored to.
    usize,
    // To -- the ending character this doc block is anchored ti.
    usize,
    // Indent. This might be a borrowed reference or an owned reference. When
    // the lexer transforms code and doc blocks into this CodeMirror format, a
    // borrow from those existing doc blocks is more efficient. However,
    // deserialization from JSON requires ownership, since the Actix web
    // framework doesn't provide a place to borrow from. The following variables
    // are clone-on-write for the same reason.
    Cow<'a, String>,
    // delimiter
    Cow<'a, String>,
    // contents
    Cow<'a, String>,
)>;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
/// The format used by CodeMirror to serialize/deserialize editor contents.
/// TODO: Link to JS code where this data structure is defined.
pub struct CodeMirror<'a> {
    /// The document being edited.
    pub doc: String,
    /// Doc blocks
    pub doc_blocks: CodeMirrorDocBlocks<'a>,
}

/// This defines the structure of JSON responses returned by the `save_source`
/// endpoint. TODO: Link to where this is used in the JS.
#[derive(Serialize)]
struct ErrorResponse {
    success: bool,
    message: String,
}

/// This enum contains the results of translating a source file to a string
/// rendering of the CodeChat Editor format.
#[derive(Debug, PartialEq)]
pub enum TranslationResultsString {
    // This file is unknown to and therefore not supported by the CodeChat
    // Editor.
    Unknown,
    // This is a CodeChat Editor file but it contains errors that prevent its
    // translation. The string contains the error message.
    Err(String),
    // A CodeChat Editor file; the struct contains the file's contents
    // translated to CodeMirror.
    CodeChat(String),
}

// ## Globals
lazy_static! {
    /// Matches a bare drive letter. Only needed on Windows.
    static ref DRIVE_LETTER_REGEX: Regex = Regex::new("^[a-zA-Z]:$").unwrap();
}

/// ## Save endpoint
#[put("/fs/{path:.*}")]
/// The Save button in the CodeChat Editor Client posts to this endpoint.
async fn save_source(
    // The path to save this file to. See
    // [Path](https://actix.rs/docs/extractors#path), which extracts parameters
    // from the request's path.
    encoded_path: web::Path<String>,
    // The source file to save, in web format. See
    // [JSON](https://actix.rs/docs/extractors#json), which deserializes the
    // request body into the provided struct (here, `CodeChatForWeb`).
    codechat_for_web: web::Json<CodeChatForWeb<'_>>,
    // Lexer info, needed to transform the `CodeChatForWeb` into source code.
    // See
    // [Data](https://docs.rs/actix-web/4.3.1/actix_web/web/struct.Data.html),
    // which provides access to application-wide data. (TODO: link to where this
    // is defined.)
    app_state: web::Data<AppState>,
) -> impl Responder {
    // Translate from the CodeChatForWeb format to the contents of a source
    // file.
    let language_lexers_compiled = &app_state.lexers;
    let file_contents =
        match codechat_for_web_to_source(codechat_for_web.into_inner(), language_lexers_compiled) {
            Ok(r) => r,
            Err(message) => return save_source_response(false, &message),
        };

    // Save this string to a file. Add a leading slash for Linux/OS X: this
    // changes from `foo/bar.c` to `/foo/bar.c`. Windows paths already starts
    // with a drive letter, such as `C:\foo\bar.c`, so no changes are needed.
    let save_file_path = if cfg!(windows) {
        "".to_string()
    } else {
        "/".to_string()
    } + &encoded_path;
    match fs::write(save_file_path.to_string(), file_contents).await {
        Ok(v) => v,
        Err(err) => {
            return save_source_response(
                false,
                &format!("Unable to save file {}: {}.", save_file_path, err),
            )
        }
    }

    save_source_response(true, "")
}

/// A convenience method to fill out then return the `ErrorResponse` struct from
/// the `save_source` endpoint.
fn save_source_response(success: bool, message: &str) -> HttpResponse {
    let response = ErrorResponse {
        success,
        message: message.to_string(),
    };
    let body = serde_json::to_string(&response).unwrap();
    if success {
        HttpResponse::Ok()
            .content_type(ContentType::json())
            .body(body)
    } else {
        HttpResponse::UnprocessableEntity()
            .content_type(ContentType::json())
            .body(body)
    }
}

/// ## Load endpoints
///
/// Redirect from the root of the filesystem to the actual root path on this OS.
async fn _root_fs_redirect() -> impl Responder {
    HttpResponse::TemporaryRedirect()
        .insert_header((header::LOCATION, "/fs/"))
        .finish()
}

/// The load endpoint: dispatch to support functions which serve either a
/// directory listing, a CodeChat Editor file, or a normal file.
#[get("/fs/{path:.*}")]
async fn serve_fs(
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
                "<p>The requested path <code>{}</code> is not valid: {}.</p>",
                fixed_path, err
            ))
        }
    };
    if canon_path.is_dir() {
        return dir_listing(orig_path.as_str(), &canon_path).await;
    } else if canon_path.is_file() {
        return serve_file(&canon_path, &req, app_state).await;
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
                "<li><a href='/fs/{}:/'>{}:</a></li>\n",
                drive_letter, drive_letter
            ));
        }

        return HttpResponse::Ok()
            .content_type(ContentType::html())
            .body(html_wrapper(&format!(
                "<h1>Drives</h1>
<ul>
{}
</ul>
",
                drive_html
            )));
    }

    // List each file/directory with appropriate links.
    let mut unwrapped_read_dir = match fs::read_dir(dir_path).await {
        Ok(p) => p,
        Err(err) => {
            return html_not_found(&format!(
                "<p>Unable to list the directory {}: {}/</p>",
                path_display(dir_path),
                err
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
                                "<p>Unable to determine the type of {}: {}.",
                                path_display(&dir_entry.path()),
                                err
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
                return html_not_found(&format!("<p>Unable to read file in directory: {}.", err))
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
                    "<p>Unable to decode directory name '{:?}' as UTF-8.",
                    err
                ))
            }
        };
        let encoded_dir = encode(&dir_name);
        dir_html += &format!(
            "<li><a href='/fs/{}{}{}'>{}</a></li>\n",
            web_path,
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
            },
            encoded_dir,
            dir_name
        );
    }

    // List files second.
    let mut file_html = String::new();
    for file in files {
        let file_name = match file.file_name().into_string() {
            Ok(v) => v,
            Err(err) => {
                return html_not_found(&format!(
                    "<p>Unable to decode file name {:?} as UTF-8.",
                    err
                ))
            }
        };
        let encoded_file = encode(&file_name);
        file_html += &format!(
            "<li><a href=\"/fs/{}/{}\" target=\"_blank\">{}</a></li>\n",
            web_path, encoded_file, file_name
        );
    }
    let body = format!(
        "<h1>Directory {}</h1>
<h2>Subdirectories</h2>
<ul>
{}
</ul>
<h2>Files</h2>
<ul>
{}
</ul>
",
        path_display(dir_path),
        dir_html,
        file_html
    );

    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(html_wrapper(&body))
}

// ### Serve a CodeChat Editor Client webpage
async fn serve_file(
    file_path: &Path,
    req: &HttpRequest,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    // Create `JointEditor` queues.
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

    // Read the file.
    let mut file_contents = String::new();
    let read_ret = match File::open(file_path).await {
        Ok(fc) => fc,
        Err(err) => {
            return html_not_found(&format!(
                "<p>Error opening file {}: {}.",
                path_display(file_path),
                err
            ))
        }
    }
    .read_to_string(&mut file_contents)
    .await;

    // If this is a binary file (meaning we can't read the contents as UTF-8),
    // just serve it raw; assume this is an image/video/etc.
    if let Err(_err) = read_ret {
        // TODO: make a better decision, don't duplicate code. The file type is
        // unknown. Serve it raw, assuming it's an image/video/etc.
        match actix_files::NamedFile::open_async(file_path).await {
            Ok(v) => {
                let res = v
                    .set_content_disposition(ContentDisposition {
                        disposition: header::DispositionType::Inline,
                        parameters: vec![],
                    })
                    .into_response(req);
                return res;
            }
            Err(err) => {
                return html_not_found(&format!(
                    "<p>Error opening file {}: {}.",
                    path_display(file_path),
                    err
                ))
            }
        }
    }

    let (translation_results_string, path_to_toc) =
        source_to_codechat_for_web_string(file_contents, file_path, is_toc, &app_state.lexers);
    let is_project = path_to_toc.is_some();
    let codechat_for_web_string_raw = match translation_results_string {
        // The file type is unknown. Serve it raw.
        TranslationResultsString::Unknown => {
            match actix_files::NamedFile::open_async(file_path).await {
                Ok(v) => {
                    let res = v.into_response(req);
                    return res;
                }
                Err(err) => {
                    return html_not_found(&format!(
                        "<p>Error opening file {}: {}.",
                        path_display(file_path),
                        err
                    ))
                }
            }
        }
        // Report a lexer error.
        TranslationResultsString::Err(err_string) => return html_not_found(&err_string),
        // This is a CodeChat file. The following code wraps the CodeChat for
        // web results in a CodeChat Editor Client webpage.
        TranslationResultsString::CodeChat(codechat_for_web) => codechat_for_web,
    };

    if is_toc {
        // The TOC is a simplified web page which requires no additional processing.
        // The script ensures that all hyperlinks target the enclosing page, not
        // just the iframe containing this page.
        return HttpResponse::Ok()
            .content_type(ContentType::html())
            .body(format!(
                r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{} - The CodeChat Editor</title>

<link rel="stylesheet" href="/static/css/CodeChatEditor.css">
<link rel="stylesheet" href="/static/css/CodeChatEditorSidebar.css">
<script>
    addEventListener("DOMContentLoaded", (event) => {{
        document.querySelectorAll("a").forEach((a_element) => {{
            a_element.target = "_parent"
        }});
    }});
</script>
</head>
<body>
{}
</body>
</html>
"#,
                name, codechat_for_web_string_raw
            ));
    }

    // This is a CodeChat Editor file. Start a FileWatcher IDE to handle it.
    let (ide_tx, mut ide_rx) = mpsc::channel(10);
    let (client_tx, client_rx) = mpsc::channel(10);
    app_state
        .joint_editors
        .lock()
        .unwrap()
        .push_front(JointEditor {
            ide_tx_queue: ide_tx,
            client_tx_queue: client_tx.clone(),
            client_rx_queue: client_rx,
        });

    // Handle `JointMessage` data from the CodeChat Editor Client for this file.
    let file_pathbuf = Rc::new(file_path.to_path_buf());
    actix_rt::spawn(async move {
        while let Some(m) = ide_rx.recv().await {
            match m {
                JointMessage::Opened(ide_type) => {
                    assert!(ide_type == IdeType::CodeChatEditorClient);
                    // Tell the CodeChat Editor Client the type of this IDE.
                    if let Err(err) = client_tx
                        .send(JointMessage::Opened(IdeType::FileWatcher))
                        .await
                    {
                        println!("Unable to enqueue: {}", err);
                        break;
                    }
                    // Provide it a file to open.
                    if let Err(err) = client_tx
                        .send(JointMessage::Update(UpdateMessageContents {
                            contents: Some("testing".to_string()),
                            cursor_position: Some(0),
                            path: Some(file_pathbuf.to_path_buf()),
                            scroll_position: Some(0.0),
                        }))
                        .await
                    {
                        println!("Unable to enqueue: {}", err);
                        break;
                    }
                }
                other => {
                    println!("Unhandled message {:?}", other);
                }
            }
        }
        ide_rx.close();
        // Drain any remaining messages after closing the queue.
        while let Some(m) = ide_rx.recv().await {
            println!("Dropped queued message {:?}", &m);
        }
    });

    // Look for any script tags and prevent these from causing problems.
    let codechat_for_web_string = codechat_for_web_string_raw.replace("</script>", "<\\/script>");

    // For project files, add in the sidebar. Convert this from a Windows path
    // to a Posix path if necessary.
    let (sidebar_iframe, sidebar_css) = if is_project {
        (
            format!(
                r##"<iframe src="{}?mode=toc" id="CodeChat-sidebar"></iframe>"##,
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
    HttpResponse::Ok().content_type(ContentType::html()).body(format!(
        r##"<!DOCTYPE html>
<html lang="en">
    <head>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1">
        <title>{} - The CodeChat Editor</title>

        <link rel="stylesheet" href="/static/bundled/CodeChatEditor.css">
        <script type="module">
            import {{ page_init, on_keydown, on_save }} from "/static/bundled/CodeChatEditor{}.js"
            // <p>Make these accessible on the onxxx handlers below. See <a
            //         href="https://stackoverflow.com/questions/44590393/es6-modules-undefined-onclick-function-after-import">SO</a>.
            // </p>
            window.CodeChatEditor = {{ on_keydown, on_save }};

            page_init(
{},
);
        </script>
        {}
        {}
    </head>
    <body onkeydown="CodeChatEditor.on_keydown(event);">
        {}
        <div id="CodeChat-contents">
            <div id="CodeChat-top">
                <div id="CodeChat-filename">
                    <p>
                        <button onclick="CodeChatEditor.on_save();" id="CodeChat-save-button">
                            <span class="CodeChat-hotkey">S</span>ave
                        </button>
                        - {} - {}
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
"##, name, if is_test_mode { "-test" } else { "" }, codechat_for_web_string, testing_src, sidebar_css, sidebar_iframe, name, dir
    ))
}

// ## Utilities
//
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
        {}
    </body>
</html>",
        body
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

/// ## Websockets
///
/// Each CodeChat Editor IDE instance pairs with a CodeChat Editor Client
/// through the CodeChat Editor Server. Together, these form a joint editor,
/// allowing the user to edit the plain text of the source code in the IDE, or
/// make GUI-enhanced edits of the source code rendered by the CodeChat Editor
/// Client.
///
/// The IDE and the CodeChat Editor Client communicate to each other through
/// websocket connections to this server. Typically, each IDE is paired with an
/// associated editor. For the editor to send and receive messages with the
/// client, the websocket server for the IDE needs a way to pass messages to the
/// websocket server for the client and vice versa. TODO: how do I do this? In
/// addition, the server needs a way to close all clients when it shuts down.
/// TODO: do I need to handle this, or is it done automatically?
///
/// Messages to exchange:
///
/// - Ready (once, the first message from both client and IDE)
/// - Update (provide new contents / path / cursor position / scroll position)
/// - Load (only the client may send this; requests the IDE to load another
///   file)
/// - Closing
///
/// Define a websocket handler for the CodeChat Client.
async fn client_ws(
    req: HttpRequest,
    body: web::Payload,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, body)?;

    // Find a `JointEditor` that needs a client and assign this one to it.
    let joint_editor_wrapped = app_state.joint_editors.lock().unwrap().pop_front();
    if joint_editor_wrapped.is_none() {
        // TODO: return an error and report it instead!
        println!("Error: no joint editor available.");
        return Ok(response);
    }
    let joint_editor = joint_editor_wrapped.unwrap();
    let ide_tx = joint_editor.ide_tx_queue;
    let client_tx = joint_editor.client_tx_queue;
    let mut client_rx = joint_editor.client_rx_queue;
    // Start a task to handle receiving `JointMessage` websocket data from the CodeChat Editor Client.
    actix_rt::spawn(async move {
        while let Some(Ok(msg)) = msg_stream.next().await {
            match msg {
                // Enqueue a ping to this thread's tx queue, to send a pong. Trying to send here means borrow errors, or resorting to a mutex/correctly locking and unlocking it.
                Message::Ping(bytes) => {
                    if let Result::Err(err) = client_tx.send(JointMessage::Ping(bytes)).await {
                        println!("Unable to enqueue: {}", err);
                        break;
                    };
                }

                // Decode text messages as JSON then dispatch.
                Message::Text(b) => {
                    // The CodeChat Editor Client should always send valid JSON.
                    match serde_json::from_str(&b) {
                        Err(err) => {
                            println!(
                                "Unable to decode JSON message from the CodeChat Editor client: {}",
                                err.to_string()
                            );
                            break;
                        }
                        // Send the `JointMessage` to the IDE for processing.
                        Ok(joint_message) => {
                            if let Result::Err(err) = ide_tx.send(joint_message).await {
                                println!("Unable to enqueue: {}", err);
                                break;
                            }
                        }
                    }
                }

                Message::Close(reason) => {
                    println!("Closing per client request: {:?}", reason);
                    break;
                }

                other => {
                    println!("Unexpected message {:?}", &other);
                    break;
                }
            }
        }
        // TODO: shut down rx task.
    });

    // Start a task to forward `JointMessage` data from the IDE to the CodeChat Editor Client.
    actix_rt::spawn(async move {
        while let Some(m) = client_rx.recv().await {
            match m {
                JointMessage::Ping(bytes) => {
                    if let Err(err) = session.pong(&bytes).await {
                        println!("Unable to send pong: {}", err.to_string());
                        return;
                    }
                }
                other => match serde_json::to_string(&other) {
                    Ok(s) => {
                        if let Err(err) = session.text(&s).await {
                            println!("Unable to send: {}", err.to_string());
                            break;
                        }
                    }
                    Err(err) => panic!("Encoding failure {}", err.to_string()),
                },
            }
        }
        // Shut down the session, to stop any incoming messages.
        if let Err(err) = session.close(None).await {
            println!("Unable to close session: {}", err);
        }

        client_rx.close();
        // Drain any remaining messages after closing the queue.
        while let Some(m) = client_rx.recv().await {
            println!("Dropped queued message {:?}", &m);
        }
    });

    Ok(response)
}

/// Provide queues which send data to the IDE and the CodeChat Editor Client.
struct JointEditor {
    /// Data to send to the IDE.
    ide_tx_queue: Sender<JointMessage>,
    /// Data to send to the CodeChat Editor Client.
    client_tx_queue: Sender<JointMessage>,
    client_rx_queue: Receiver<JointMessage>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum JointMessage {
    /// Pings sent by the underlying websocket protocol.
    Ping(Bytes),
    /// This is the first message sent when the IDE or client starts up.
    Opened(IdeType),
    /// This sends an update; any missing fields are unchanged.
    Update(UpdateMessageContents),
    /// Only the CodeChat Client editor may send this; it requests the IDE to
    /// load the provided file. The IDE should respond by sending an `Update`
    /// with the requested file.
    Load(PathBuf),
    /// Sent when the IDE or client are closing.
    Closing,
}

/// Specify the type of IDE that this client represents.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum IdeType {
    /// The CodeChat Editor Client always sends this.
    CodeChatEditorClient,
    // The IDE client sends one of these.
    FileWatcher,
    VSCode,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct UpdateMessageContents {
    /// An absolute path to the file currently in use.
    path: Option<PathBuf>,
    /// The contents of this file.
    contents: Option<String>,
    /// The current cursor position in the file, where 0 = before the first
    /// character in the file and contents.length() = after the last character
    /// in the file. TODO: Selections are not yet supported.
    cursor_position: Option<u32>,
    /// The normalized vertical scroll position in the file, where 0 = top and 1
    /// = bottom.
    scroll_position: Option<f32>,
}

// The client task receives a JointMessage, translates the code, then sends it
// to its paired IDE by calling this function. Likewise, the IDE task receives a
// ClientMessage, translates the code, then sends it to its paired client by
// calling this function.

// Define the [state](https://actix.rs/docs/application/#state) available to all
// endpoints.
struct AppState {
    lexers: LanguageLexersCompiled,
    joint_editors: Mutex<VecDeque<JointEditor>>,
}

// ## Webserver startup
#[actix_web::main]
pub async fn main() -> std::io::Result<()> {
    // See
    // [shared mutable state](https://actix.rs/docs/application#shared-mutable-state).
    let app_data = web::Data::new(AppState {
        lexers: compile_lexers(get_language_lexer_vec()),
        joint_editors: Mutex::new(VecDeque::new()),
    });
    HttpServer::new(move || {
        // Get the path to this executable. Assume that static files for the
        // webserver are located relative to it.
        let exe_path = env::current_exe().unwrap();
        let exe_dir = exe_path.parent().unwrap();
        let mut client_static_path = PathBuf::from(exe_dir);
        // When in debug, use the layout of the Git repo to find client files.
        // In release mode, we assume the static folder is a subdirectory of the
        // directory containing the executable.
        #[cfg(debug_assertions)]
        client_static_path.push("../../../client");
        client_static_path.push("static");
        client_static_path = client_static_path.canonicalize().unwrap();

        // Start the server.
        App::new()
            // Provide data to all endpoints -- the compiler lexers.
            .app_data(app_data.clone())
            // Serve static files per the
            // [docs](https://actix.rs/docs/static-files).
            .service(actix_files::Files::new(
                "/static",
                client_static_path.as_os_str(),
            ))
            // These endpoints serve the files to/from the filesystem.
            .service(serve_fs)
            .service(save_source)
            // Reroute to the filesystem for typical user-requested URLs.
            .route("/", web::get().to(_root_fs_redirect))
            .route("/fs", web::get().to(_root_fs_redirect))
            .route("/client_ws/", web::get().to(client_ws))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}

// ## Tests
//
// As mentioned in the lexer.rs tests, Rust
// [almost mandates](https://doc.rust-lang.org/book/ch11-03-test-organization.html)
// putting tests in the same file as the source. Here's some
// [good information](http://xion.io/post/code/rust-unit-test-placement.html) on
// how to put tests in another file, for future refactoring reference.
#[cfg(test)]

// ### TODO!
mod tests {}
