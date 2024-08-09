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
/// ## Imports
///
/// ### Standard library
use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
    rc::Rc,
    str::FromStr,
    sync::Mutex,
    time::Duration,
};

// ### Third-party
use actix_files;
use actix_web::{
    dev::{ServiceFactory, ServiceRequest},
    error::{Error, ErrorMisdirectedRequest},
    get,
    http::header::{self, ContentDisposition, ContentType},
    web, App, HttpRequest, HttpResponse, HttpServer, Responder,
};
use actix_ws::AggregatedMessage;
use bytes::Bytes;
use futures_util::StreamExt;
use lazy_static::lazy_static;
use log::{error, info, warn};
use log4rs;
use notify_debouncer_full::{
    new_debouncer,
    notify::{EventKind, RecursiveMode, Watcher},
    DebounceEventResult,
};
use path_slash::PathBufExt;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json;
use tokio::{
    fs::{self, DirEntry, File},
    io::AsyncReadExt,
    select,
    sync::mpsc::{self, Receiver, Sender},
    task::JoinHandle,
    time::sleep,
};
use urlencoding::{self, encode};
#[cfg(target_os = "windows")]
use win_partitions::win_api::get_logical_drive;

// ### Local
use crate::lexer::LanguageLexersCompiled;
use crate::lexer::{compile_lexers, supported_languages::get_language_lexer_vec};
use crate::processing::TranslationResultsString;
use crate::processing::{
    codechat_for_web_to_source, source_to_codechat_for_web_string, CodeChatForWeb,
};

// ## Macros
/// Create a macro to report an error when enqueueing an item.
macro_rules! queue_send {
    ($tx: expr) => {{
        if let Err(err) = $tx.await {
            error!("Unable to enqueue: {}", err);
            break;
        }
    }};
}

/// ## Data structures
///
/// ### Data structures supporting a websocket connection between the IDE, this server, and the CodeChat Editor Client
///
/// Provide queues which send data to the IDE and the CodeChat Editor Client.
#[derive(Debug)]
struct ClientQueues {
    /// Data from the CodeChat Editor Client.
    from_client_tx: Sender<EditorMessage>,
    /// Data to the CodeChat Editor Client.
    to_client_rx: Receiver<EditorMessage>,
}

struct IdeQueues {
    from_ide_tx: Sender<EditorMessage>,
    to_ide_rx: Receiver<EditorMessage>,
}

struct ProcessingQueues {
    to_ide_tx: Sender<EditorMessage>,
    from_ide_rx: Receiver<EditorMessage>,
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
    /// This is the first message sent when the IDE or client starts up or
    /// reconnects.
    Opened(IdeType),
    /// This sends an update; any missing fields are unchanged.
    Update(UpdateMessageContents),
    /// Only the CodeChat Client editor may send this; it requests the IDE to
    /// load the provided file. The IDE should respond by sending an `Update`
    /// with the requested file.
    LoadFile(PathBuf),
    /// Only the server may send this to the IDE. It contains HTML to display in its built-in browser.
    LoadHtml(String),
    /// Sent when the IDE or client are closing.
    Closing,
    /// Sent as a response to any of the above messages,
    /// reporting success/error. An empty string indicates success; otherwise,
    /// the string contains the error message.
    Result(String),
}

/// Specify the type of IDE that this client represents.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum IdeType {
    FileWatcher,
    /// True if the CodeChat Editor will be hosted inside VSCode; false means it should be hosted in an external browser.
    VSCode(bool),
}

/// Contents of the `Update` message.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct UpdateMessageContents {
    /// An absolute path to the file currently in use.
    path: Option<PathBuf>,
    /// The contents of this file.
    contents: Option<CodeChatForWeb>,
    /// The current cursor position in the file, where 0 = before the first
    /// character in the file and contents.length() = after the last character
    /// in the file. TODO: Selections are not yet supported.
    cursor_position: Option<u32>,
    /// The normalized vertical scroll position in the file, where 0 = top and 1
    /// = bottom.
    scroll_position: Option<f32>,
}

/// ### Data structures used by the webserver
///
/// Define the [state](https://actix.rs/docs/application/#state) available to
/// all endpoints.
struct AppState {
    lexers: LanguageLexersCompiled,
    connection_id: Mutex<u32>,
    filewatcher_client_queues: Mutex<HashMap<String, ClientQueues>>,
    vscode_ide_queues: Mutex<HashMap<u32, IdeQueues>>,
    vscode_processing_queues: Mutex<HashMap<u32, ProcessingQueues>>,
    vscode_client_queues: Mutex<HashMap<u32, ClientQueues>>,
    pending_messages: Mutex<HashMap<u32, JoinHandle<()>>>,
}

// ## Globals
lazy_static! {
    /// Matches a bare drive letter. Only needed on Windows.
    static ref DRIVE_LETTER_REGEX: Regex = Regex::new("^[a-zA-Z]:$").unwrap();
}

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

// ### Serve file
//
// This could be a plain text file (for example, one not recognized as source
// code that this program supports), a binary file (image/video/etc.), a
// CodeChat Editor file, or a non-existent file. Determine which type this file
// is then serve it. Serve a CodeChat Editor Client webpage using the
// FileWatcher "IDE".
async fn serve_filewatcher(
    file_path: &Path,
    req: &HttpRequest,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let file_contents = match smart_read(file_path, req).await {
        Ok(fc) => fc,
        Err(err) => return err,
    };
    serve_file(file_path, &file_contents, "fw", req, app_state).await
}

// Smart file reader. This returns an HTTP response if the provided file should
// be served directly (including an error if necessary), or a string containing
// the text of the file when it's Unicode.
async fn smart_read(file_path: &Path, req: &HttpRequest) -> Result<String, HttpResponse> {
    let mut file_contents = String::new();
    let read_ret = match File::open(file_path).await {
        Ok(fc) => fc,
        Err(err) => {
            return Err(html_not_found(&format!(
                "<p>Error opening file {}: {err}.",
                path_display(file_path)
            )))
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
                // This isn't an error per se, but it does indicate that the
                // caller should return with this value immediately, rather than
                // continue processing.
                return Err(res);
            }
            Err(err) => {
                return Err(html_not_found(&format!(
                    "<p>Error opening file {}: {err}.",
                    path_display(file_path)
                )))
            }
        }
    }

    Ok(file_contents)
}

async fn serve_file(
    file_path: &Path,
    file_contents: &str,
    ide_path: &str,
    req: &HttpRequest,
    app_state: web::Data<AppState>,
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

    // #### Filewatcher IDE
    //
    // This is a CodeChat Editor file. Start up the Filewatcher IDE tasks:
    //
    // 1.  A task to watch for changes to the file, notifying the CodeChat
    //     Editor Client when the file should be reloaded.
    // 2.  A task to receive and respond to messages from the CodeChat Editor
    //     Client.
    //
    // First, allocate variables needed by these two tasks.
    //
    // The path to the CodeChat Editor file to operate on.
    let file_pathbuf = Rc::new(file_path.to_path_buf());
    // Access this way, to avoid borrow checker problems.
    let connection_id = {
        let mut connection_id = app_state.connection_id.lock().unwrap();
        *connection_id += 1;
        *connection_id
    };
    // #### The filewatcher task.
    actix_rt::spawn(async move {
        // Handle `JointMessage` data from the CodeChat Editor Client for this
        // file.
        let (from_client_tx, mut from_client_rx) = mpsc::channel(10);
        let (to_client_tx, to_client_rx) = mpsc::channel(10);
        app_state.filewatcher_client_queues.lock().unwrap().insert(
            connection_id.to_string(),
            ClientQueues {
                from_client_tx,
                to_client_rx,
            },
        );

        // Provide a unique ID for each message sent to the CodeChat Editor
        // Client.
        let mut id: u32 = 0;
        // Use a channel to send from the watcher (which runs in another thread)
        // into this async (task) context.
        let (watcher_tx, mut watcher_rx) = mpsc::channel(10);
        // Watch this file. Use the debouncer, to avoid multiple notifications
        // for the same file. This approach returns a result of either a working
        // debouncer or any errors that occurred. The debouncer's scope needs
        // live as long as this connection does; dropping it early means losing
        // file change notifications.
        let debounced_watcher = match new_debouncer(
            Duration::from_secs(2),
            None,
            // Note that this runs in a separate thread created by the watcher,
            // not in an async context. Therefore, use a blocking send.
            move |result: DebounceEventResult| {
                if let Err(err) = watcher_tx.blocking_send(result) {
                    error!("Unable to send: {err}");
                }
            },
        ) {
            Ok(mut debounced_watcher) => {
                match debounced_watcher
                    .watcher()
                    .watch(&file_pathbuf, RecursiveMode::NonRecursive)
                {
                    Ok(()) => Ok(debounced_watcher),
                    Err(err) => Err(err),
                }
            }
            Err(err) => Err(err),
        };

        if let Err(err) = debounced_watcher {
            error!("Debouncer error: {}", err);
        }

        // Process results produced by the file watcher.
        let mut ignore_file_modify = false;
        let mut just_opened = true;
        loop {
            // On first opening the websocket, send the Editor an `Opened` then an `Update` message.
            if just_opened {
                queue_send!(to_client_tx.send(EditorMessage {
                    id,
                    message: EditorMessageContents::Opened(IdeType::FileWatcher),
                }));
                create_timeout(&app_state, id);

                // Provide it a file to open.
                id += 1;
                queue_send!(to_client_tx.send(EditorMessage {
                        id,
                        message: EditorMessageContents::Update(UpdateMessageContents {
                            contents: Some(codechat_for_web.clone()),
                            cursor_position: Some(0),
                            path: Some(file_pathbuf.to_path_buf()),
                            scroll_position: Some(0.0),
                        }),
                }));
                create_timeout(&app_state, id);
                id += 1;
                just_opened = false;
            }

            select! {
                Some(result) = watcher_rx.recv() => {
                    match result {
                        Err(err_vec) => {
                            for err in err_vec {
                                // Report errors locally and to the CodeChat
                                // Editor.
                                let msg = format!("Watcher error: {err}");
                                error!("{}", msg);
                                // Send using ID 0 to indicate this isn't a
                                // response to a message received from the
                                // client.
                                send_response(&to_client_tx, 0, &msg).await;
                            }
                        }

                        Ok(debounced_event_vec) => {
                            for debounced_event in debounced_event_vec {
                                match debounced_event.event.kind {
                                    EventKind::Modify(_modify_kind) => {
                                        if ignore_file_modify {
                                            ignore_file_modify = false;
                                        } else {
                                            // On Windows, the `_modify_kind` is `Any`;
                                            // therefore; ignore it rather than trying
                                            // to look at only content modifications.
                                            // As long as the parent of both files is
                                            // identical, we can update the contents.
                                            // Otherwise, we need to load in the new
                                            // URL.
                                            if debounced_event.event.paths.len() == 1 && debounced_event.event.paths[0].parent() == file_pathbuf.parent() {
                                                // Since the parents are identical, send an
                                                // update. First, read the modified file.
                                                let mut file_contents = String::new();
                                                let read_ret = match File::open(file_pathbuf.as_ref()).await {
                                                    Ok(fc) => fc,
                                                    Err(_err) => {
                                                        id += 1;
                                                        // We can't open the file -- it's been
                                                        // moved or deleted. Close the file.
                                                        queue_send!(to_client_tx.send(EditorMessage {
                                                            id,
                                                            message: EditorMessageContents::Closing
                                                        }));
                                                        continue;
                                                    }
                                                }
                                                .read_to_string(&mut file_contents)
                                                .await;

                                                // Close the file if it can't be read as
                                                // Unicode text.
                                                if read_ret.is_err() {
                                                    id +=1 ;
                                                    queue_send!(to_client_tx.send(EditorMessage {
                                                        id,
                                                        message: EditorMessageContents::Closing
                                                    }));
                                                    create_timeout(&app_state, id);
                                                }

                                                // Translate the file.
                                                let (translation_results_string, _path_to_toc) =
                                                source_to_codechat_for_web_string(&file_contents, &file_pathbuf, false, &app_state.lexers);
                                                if let TranslationResultsString::CodeChat(cc) = translation_results_string {
                                                    // Send the new contents
                                                    id += 1;
                                                    queue_send!(to_client_tx.send(EditorMessage {
                                                            id,
                                                            message: EditorMessageContents::Update(UpdateMessageContents {
                                                                contents: Some(cc),
                                                                cursor_position: None,
                                                                path: Some(debounced_event.event.paths[0].to_path_buf()),
                                                                scroll_position: None,
                                                            }),
                                                        }));
                                                    create_timeout(&app_state, id);

                                                } else {
                                                    // Close the file -- it's not CodeChat
                                                    // anymore.
                                                    id +=1 ;
                                                    queue_send!(to_client_tx.send(EditorMessage {
                                                        id,
                                                        message: EditorMessageContents::Closing
                                                    }));
                                                    create_timeout(&app_state, id);
                                                }

                                            } else {
                                                warn!("TODO: Modification to different parent.")
                                            }
                                        }
                                    }
                                    _ => {
                                        // TODO: handle delete.
                                        info!("Watcher event: {debounced_event:?}.");
                                    }
                                }
                            }
                        }
                    }
                }

                Some(m) = from_client_rx.recv() => {
                    match m.message {
                        EditorMessageContents::Update(update_message_contents) => {
                            let result = 'process: {
                                // With code or a path, there's nothing to save.
                                // TODO: this should store and remember the
                                // path, instead of needing it repeated each
                                // time.
                                let codechat_for_web1 = match update_message_contents.contents {
                                    None => break 'process "".to_string(),
                                    Some(cwf) => cwf,
                                };
                                if update_message_contents.path.is_none() {
                                    break 'process "".to_string();
                                }

                                // Translate from the CodeChatForWeb format to
                                // the contents of a source file.
                                let language_lexers_compiled = &app_state.lexers;
                                let file_contents = match codechat_for_web_to_source(
                                    codechat_for_web1,
                                    language_lexers_compiled,
                                ) {
                                    Ok(r) => r,
                                    Err(message) => {
                                        break 'process format!(
                                            "Unable to translate to source: {message}"
                                        );
                                    }
                                };

                                // Save this string to a file. Add a leading
                                // slash for Linux/OS X: this changes from
                                // `foo/bar.c` to `/foo/bar.c`. Windows paths
                                // already start with a drive letter, such as
                                // `C:\foo\bar.c`, so no changes are needed.
                                let mut save_file_path = if cfg!(windows) {
                                    PathBuf::from_str("")
                                } else {
                                    PathBuf::from_str("/")
                                }
                                .unwrap();
                                save_file_path.push(update_message_contents.path.unwrap());
                                if let Err(err) = fs::write(save_file_path.as_path(), file_contents).await {
                                    let msg = format!(
                                        "Unable to save file '{}': {err}.",
                                        save_file_path.to_string_lossy()
                                    );
                                    break 'process msg;
                                }
                                ignore_file_modify = true;
                                "".to_string()
                            };
                            send_response(&to_client_tx, m.id, &result).await;
                        }

                        // Process a result, the respond to a message we sent.
                        EditorMessageContents::Result(err) => {
                            // Cancel the timeout for this result.
                            let mut pm = app_state.pending_messages.lock().unwrap();
                            if let Some(task) = pm.remove(&m.id) {
                                task.abort();
                            }

                            // Report errors to the log.
                            if !err.is_empty() {
                                error!("Error in message {}: {err}.", m.id);
                            }
                        }

                        EditorMessageContents::Closing => {
                            info!("Filewatcher closing");
                            break;
                        }

                        other => {
                            warn!("Unhandled message {:?}", other);
                        }
                    }
                }

                else => break
            }
        }

        from_client_rx.close();
        // Drain any remaining messages after closing the queue.
        while let Some(m) = from_client_rx.recv().await {
            warn!("Dropped queued message {:?}", &m);
        }

        info!("Watcher closed.");
    });

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
    // Quote the string using JSON to handle any necessary escapes.
    let ws_url = match serde_json::to_string(&format!(
        "ws://localhost:8080/{ide_path}/ws/{connection_id}"
    )) {
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

// Start a timeout task in case a message isn't delivered.
fn create_timeout(
    // The global state, which contains the hashmap of pending messages to
    // modify.
    app_state: &AppState,
    // The id of the message just sent.
    id: u32,
) {
    let mut pm = app_state.pending_messages.lock().unwrap();
    let waiting_task = actix_rt::spawn(async move {
        sleep(REPLY_TIMEOUT).await;
        error!("Timeout: message id {id} unacknowledged.");
    });
    pm.insert(id, waiting_task);
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
        error!("Unable to enqueue: {}", err);
    }
}

/// ## Websockets
///
/// Each CodeChat Editor IDE instance pairs with a CodeChat Editor Client
/// through the CodeChat Editor Server. Together, these form a joint editor,
/// allowing the user to edit the plain text of the source code in the IDE, or
/// make GUI-enhanced edits of the source code rendered by the CodeChat Editor
/// Client.
///
/// Define a websocket handler for the CodeChat Editor Client.
#[get("/fw/ws/{connection_id}")]
async fn filewatcher_websocket(
    connection_id: web::Path<String>,
    req: HttpRequest,
    body: web::Payload,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
    // Find a `JointEditor` that needs a client and assign this one to it.
    let joint_editor_wrapped = app_state
        .filewatcher_client_queues
        .lock()
        .unwrap()
        .remove(&connection_id.to_string());
    if joint_editor_wrapped.is_none() {
        error!("Error: no joint editor available.");
        return Err(ErrorMisdirectedRequest("No joint editor available."));
    }
    let joint_editor = joint_editor_wrapped.unwrap();

    client_websocket(
        connection_id,
        req,
        body,
        app_state,
        joint_editor.from_client_tx,
        joint_editor.to_client_rx,
    )
    .await
}

async fn client_websocket(
    connection_id: web::Path<String>,
    req: HttpRequest,
    body: web::Payload,
    app_state: web::Data<AppState>,
    from_websocket_tx: Sender<EditorMessage>,
    mut to_websocket_rx: Receiver<EditorMessage>,
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

        // True when the client requests the websocket to close; otherwise,
        // closing represents an interruption (such as the computer going to
        // sleep).
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
                        error!("Unable to send ping: {}", err);
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
                                        error!("Unable to send pong: {}", err);
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
                                    match serde_json::from_str(&b) {
                                        Err(err) => {
                                            error!(
                                        "Unable to decode JSON message from the CodeChat Editor client: {}",
                                        err
                                    );
                                            break;
                                        }
                                        // Send the `JointMessage` to the IDE for
                                        // processing.
                                        Ok(joint_message) => {
                                            queue_send!(from_websocket_tx.send(joint_message));
                                        }
                                    }
                                }

                                // Forward a close message from the client to
                                // the IDE, so that both this websocket
                                // connection and the IDE connection will both
                                // be closed.
                                AggregatedMessage::Close(reason) => {
                                    info!("Closing per client request: {:?}", reason);
                                    is_closing = true;
                                    queue_send!(from_websocket_tx.send(EditorMessage { id: 0, message: EditorMessageContents::Closing }));
                                    break;
                                }

                                other => {
                                    warn!("Unexpected message {:?}", &other);
                                    break;
                                }
                            }
                        }
                        Err(err) => {
                            error!("websocket receive error {:?}", err);
                        }
                    }
                }

                // Forward a message from the IDE to the client.
                Some(m) = to_websocket_rx.recv() => {
                    match serde_json::to_string(&m) {
                        Ok(s) => {
                            if let Err(err) = session.text(&*s).await {
                                error!("Unable to send: {}", err);
                                break;
                            }
                        }
                        Err(err) => {
                            error!("Encoding failure {}", err);
                        }
                    }
                }

                else => break,
            }
        }

        // Shut down the session, to stop any incoming messages.
        if let Err(err) = session.close(None).await {
            error!("Unable to close session: {}", err);
        }

        // Re-enqueue this unless the client requested the websocket to close.
        if is_closing {
            info!("Websocket closed.");
            to_websocket_rx.close();
            // Drain any remaining messages after closing the queue.
            while let Some(m) = to_websocket_rx.recv().await {
                warn!("Dropped queued message {:?}", &m);
            }
        } else {
            info!("Websocket re-enqueued.");
            app_state.filewatcher_client_queues.lock().unwrap().insert(
                connection_id.to_string(),
                ClientQueues {
                    from_client_tx: from_websocket_tx,
                    to_client_rx: to_websocket_rx,
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
    let app_data = make_app_data();
    HttpServer::new(move || configure_app(App::new(), &app_data))
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
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
        filewatcher_client_queues: Mutex::new(HashMap::new()),
        vscode_ide_queues: Mutex::new(HashMap::new()),
        vscode_processing_queues: Mutex::new(HashMap::new()),
        vscode_client_queues: Mutex::new(HashMap::new()),
        pending_messages: Mutex::new(HashMap::new()),
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
        // These endpoints serve the files from the filesystem.
        .service(serve_fs)
        .service(filewatcher_websocket)
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

// ## Tests
#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::Duration;

    use actix_web::{test, web, App};
    use assertables::{assert_starts_with, assert_starts_with_as_result};
    use log::Level;
    use tokio::select;
    use tokio::sync::mpsc::{Receiver, Sender};
    use tokio::time::sleep;

    use super::REPLY_TIMEOUT;
    use super::{configure_app, make_app_data};
    use super::{
        AppState, ClientQueues, IdeType, EditorMessage, EditorMessageContents, UpdateMessageContents,
    };
    use crate::lexer::{compile_lexers, supported_languages::get_language_lexer_vec};
    use crate::processing::{
        source_to_codechat_for_web, CodeChatForWeb, CodeMirror, SourceFileMetadata,
        TranslationResults,
    };
    use crate::testing_logger;
    use crate::{cast, prep_test_dir};

    async fn get_websocket_queues(
        // A path to the temporary directory where the source file is located.
        test_dir: &PathBuf,
    ) -> ClientQueues {
        let app_data = make_app_data();
        let app = test::init_service(configure_app(App::new(), &app_data)).await;

        // Load in a test source file to create a websocket.
        let uri = format!("/fw/fs/{}/test.py", test_dir.to_string_lossy());
        let req = test::TestRequest::get().uri(&uri).to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        // Even after the webpage is served, the websocket task hasn't started.
        // Wait a bit for that.
        sleep(Duration::from_millis(10)).await;

        // The web page has been served; fake the connected websocket by getting
        // the appropriate tx/rx queues.
        let app_state = resp.request().app_data::<web::Data<AppState>>().unwrap();
        let mut joint_editors = app_state.filewatcher_client_queues.lock().unwrap();
        let connection_id = *app_state.connection_id.lock().unwrap();
        assert_eq!(joint_editors.len(), 1);
        return joint_editors.remove(&connection_id.to_string()).unwrap();
    }

    async fn send_response(id: u32, ide_tx_queue: &Sender<EditorMessage>, result: &str) {
        ide_tx_queue
            .send(EditorMessage {
                id,
                message: EditorMessageContents::Result(result.to_string()),
            })
            .await
            .unwrap();
    }

    // Testing with logs is subtle. If logs won't be examined by unit tests,
    // this is straightforward. However, to sometimes simply log data and at
    // other times examine logs requires care:
    //
    // 1.  The global logger can only be configured once. Configuring it for one
    //     test for the production logger and for another test using the testing
    //     logger doesn't work.
    // 2.  Since tests are run by default in multiple threads, the logger used
    //     should keep each thread's logs separate.
    // 3.  The logger needs to be initialized for all tests and for production,
    //     preferably without adding code to each test.
    //
    // The modified `testing_logger` takes care of items 2 and 3. For item 3, I
    // don't have a way to auto-initialize the logger for all tests easily;
    // [test-log](https://crates.io/crates/test-log) seems like a possibility,
    // but it works only for `env_logger`. While `rstest` offers fixtures, this
    // seems like a bit of overkill to call one function for each test.
    fn configure_logger() {
        testing_logger::setup();
    }

    async fn get_message(client_rx: &mut Receiver<EditorMessage>) -> EditorMessageContents {
        select! {
            data = client_rx.recv() => {
                let m = data.unwrap().message;
                // For debugging, print out each message.
                println!("{:?}", m);
                m
            }
            _ = sleep(Duration::from_secs(3)) => panic!("Timeout waiting for message")
        }
    }

    macro_rules! get_message {
        ($client_rx: expr, $cast_type: ty) => {
            cast!(get_message(&mut $client_rx).await, $cast_type)
        };
    }

    #[actix_web::test]
    async fn test_websocket_opened_1() {
        let (temp_dir, test_dir) = prep_test_dir!();
        let je = get_websocket_queues(&test_dir).await;
        let ide_tx_queue = je.from_client_tx;
        let mut client_rx = je.to_client_rx;
        configure_logger();

        // 1.  We should get a message specifying the IDE client type.
        assert_eq!(
            get_message!(client_rx, EditorMessageContents::Opened),
            IdeType::FileWatcher
        );
        send_response(0, &ide_tx_queue, "").await;

        // 2.  We should get the initial contents.
        let umc = get_message!(client_rx, EditorMessageContents::Update);
        assert_eq!(umc.cursor_position, Some(0));
        assert_eq!(umc.scroll_position, Some(0.0));

        // Check the path.
        let mut test_path = test_dir.clone();
        test_path.push("test.py");
        // The comparison below fails without this.
        let test_path = test_path.canonicalize().unwrap();
        assert_eq!(umc.path, Some(test_path));

        // Check the contents.
        let llc = compile_lexers(get_language_lexer_vec());
        let translation_results =
            source_to_codechat_for_web(&"".to_string(), "py", false, false, &llc);
        let codechat_for_web = cast!(translation_results, TranslationResults::CodeChat);
        assert_eq!(umc.contents, Some(codechat_for_web));
        send_response(1, &ide_tx_queue, "").await;

        // Report any errors produced when removing the temporary directory.
        temp_dir.close().unwrap();
    }

    #[actix_web::test]
    async fn test_websocket_timeout() {
        let (temp_dir, test_dir) = prep_test_dir!();
        let je = get_websocket_queues(&test_dir).await;
        let mut client_rx = je.to_client_rx;
        // Configure the logger here; otherwise, the glob used to copy files
        // outputs some debug-level logs.
        configure_logger();

        // We should get a message specifying the IDE client type.
        assert_eq!(
            get_message!(client_rx, EditorMessageContents::Opened),
            IdeType::FileWatcher
        );

        // We should get the initial contents.
        get_message!(client_rx, EditorMessageContents::Update);

        // Don't send any acknowledgements to these message and see if we get
        // errors. The stderr redirection covers only this block.
        sleep(REPLY_TIMEOUT).await;
        sleep(REPLY_TIMEOUT).await;

        // We should get two errors.
        testing_logger::validate(|captured_logs| {
            assert_eq!(captured_logs.len(), 2);
            assert_eq!(captured_logs[0].target, "code_chat_editor::webserver");
            assert_eq!(
                captured_logs[0].body,
                "Timeout: message id 0 unacknowledged."
            );
            assert_eq!(captured_logs[0].level, Level::Error);

            assert_eq!(captured_logs[0].target, "code_chat_editor::webserver");
            assert_eq!(
                captured_logs[1].body,
                "Timeout: message id 1 unacknowledged."
            );
            assert_eq!(captured_logs[1].level, Level::Error);
        });

        // Report any errors produced when removing the temporary directory.
        temp_dir.close().unwrap();
    }

    #[actix_web::test]
    async fn test_websocket_update_1() {
        let (temp_dir, test_dir) = prep_test_dir!();
        let je = get_websocket_queues(&test_dir).await;
        let ide_tx_queue = je.from_client_tx;
        let mut client_rx = je.to_client_rx;
        // Configure the logger here; otherwise, the glob used to copy files
        // outputs some debug-level logs.
        configure_logger();

        // We should get a message specifying the IDE client type.
        assert_eq!(
            get_message!(client_rx, EditorMessageContents::Opened),
            IdeType::FileWatcher
        );

        // We should get the initial contents.
        get_message!(client_rx, EditorMessageContents::Update);

        // 1.  Send an update message with no contents.
        ide_tx_queue
            .send(EditorMessage {
                id: 1,
                message: EditorMessageContents::Update(UpdateMessageContents {
                    contents: None,
                    path: Some(PathBuf::new()),
                    cursor_position: None,
                    scroll_position: None,
                }),
            })
            .await
            .unwrap();

        // Check that it produces no error.
        assert_eq!(get_message!(client_rx, EditorMessageContents::Result), "");

        // 2.  Send an update message with no path.
        ide_tx_queue
            .send(EditorMessage {
                id: 2,
                message: EditorMessageContents::Update(UpdateMessageContents {
                    contents: Some(CodeChatForWeb {
                        metadata: SourceFileMetadata {
                            mode: "".to_string(),
                        },
                        source: CodeMirror {
                            doc: "".to_string(),
                            doc_blocks: vec![(
                                0,
                                0,
                                "".to_string(),
                                "".to_string(),
                                "".to_string(),
                            )],
                        },
                    }),
                    path: None,
                    cursor_position: None,
                    scroll_position: None,
                }),
            })
            .await
            .unwrap();

        // Check that it produces no error.
        assert_eq!(get_message!(client_rx, EditorMessageContents::Result), "");

        // 3.  Send an update message with unknown source language.
        ide_tx_queue
            .send(EditorMessage {
                id: 3,
                message: EditorMessageContents::Update(UpdateMessageContents {
                    contents: Some(CodeChatForWeb {
                        metadata: SourceFileMetadata {
                            mode: "nope".to_string(),
                        },
                        source: CodeMirror {
                            doc: "testing".to_string(),
                            doc_blocks: vec![],
                        },
                    }),
                    path: Some(PathBuf::new()),
                    cursor_position: None,
                    scroll_position: None,
                }),
            })
            .await
            .unwrap();

        // Check that it produces an error.
        assert_eq!(
            get_message!(client_rx, EditorMessageContents::Result),
            "Unable to translate to source: Invalid mode"
        );

        // 4.  Send an update message with an invalid path.
        ide_tx_queue
            .send(EditorMessage {
                id: 3,
                message: EditorMessageContents::Update(UpdateMessageContents {
                    contents: Some(CodeChatForWeb {
                        metadata: SourceFileMetadata {
                            mode: "python".to_string(),
                        },
                        source: CodeMirror {
                            doc: "".to_string(),
                            doc_blocks: vec![],
                        },
                    }),
                    path: Some(PathBuf::new()),
                    cursor_position: None,
                    scroll_position: None,
                }),
            })
            .await
            .unwrap();

        // Check that it produces an error.
        assert_starts_with!(
            get_message!(client_rx, EditorMessageContents::Result),
            "Unable to save file '':"
        );

        // 5.  Send a valid message.
        let mut file_path = test_dir.clone();
        file_path.push("test.py");
        ide_tx_queue
            .send(EditorMessage {
                id: 3,
                message: EditorMessageContents::Update(UpdateMessageContents {
                    contents: Some(CodeChatForWeb {
                        metadata: SourceFileMetadata {
                            mode: "python".to_string(),
                        },
                        source: CodeMirror {
                            doc: "testing()".to_string(),
                            doc_blocks: vec![],
                        },
                    }),
                    path: Some(file_path.clone()),
                    cursor_position: None,
                    scroll_position: None,
                }),
            })
            .await
            .unwrap();
        assert_eq!(get_message!(client_rx, EditorMessageContents::Result), "");

        // Check that the requested file is written.
        let mut s = fs::read_to_string(&file_path).unwrap();
        assert_eq!(s, "testing()");
        // Wait for the filewatcher to debounce this file write.
        sleep(Duration::from_secs(1)).await;

        // Change this file and verify that this produces an update.
        s.push_str("123");
        fs::write(&file_path, s).unwrap();
        assert_eq!(
            get_message!(client_rx, EditorMessageContents::Update),
            UpdateMessageContents {
                contents: Some(CodeChatForWeb {
                    metadata: SourceFileMetadata {
                        mode: "python".to_string(),
                    },
                    source: CodeMirror {
                        doc: "testing()123".to_string(),
                        doc_blocks: vec![],
                    },
                }),
                path: Some(file_path.clone().canonicalize().unwrap()),
                cursor_position: None,
                scroll_position: None,
            }
        );
        // Acknowledge this message.
        send_response(1, &ide_tx_queue, "");

        // Rename it and check for an close (the file watcher can't detect the
        // destination file, so it's treated as the file is deleted).
        let mut dest = file_path.clone().parent().unwrap().to_path_buf();
        dest.push("test2.py");
        fs::rename(file_path, dest.as_path()).unwrap();
        assert_eq!(
            client_rx.recv().await.unwrap(),
            EditorMessage {
                id: 4,
                message: EditorMessageContents::Closing
            }
        );

        // Report any errors produced when removing the temporary directory.
        temp_dir.close().unwrap();
    }
}
