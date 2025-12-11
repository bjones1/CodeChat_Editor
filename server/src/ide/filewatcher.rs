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
/// `filewatcher.rs` -- Implement the File Watcher "IDE"
/// ============================================================================
// Imports
// -----------------------------------------------------------------------------
//
// ### Standard library
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

// ### Third-party
use actix_web::{
    HttpRequest, HttpResponse, Responder,
    error::{self, Error},
    get,
    http::header::{self, ContentType},
    web,
};
use dunce::simplified;
use indoc::formatdoc;
use lazy_static::lazy_static;
use log::{error, info, warn};
use notify_debouncer_full::{
    DebounceEventResult, new_debouncer,
    notify::{EventKind, RecursiveMode},
};
use rand::random;
use regex::Regex;
use tokio::{
    fs::DirEntry,
    fs::{self, File},
    io::AsyncReadExt,
    select,
    sync::mpsc,
};
use urlencoding;
#[cfg(target_os = "windows")]
use win_partitions::win_api::get_logical_drive;

// ### Local
use crate::{
    processing::CodeMirrorDiffable,
    queue_send,
    webserver::{
        INITIAL_IDE_MESSAGE_ID, MESSAGE_ID_INCREMENT, ResultErrTypes, ResultOkTypes, WebAppState,
        filesystem_endpoint, get_test_mode,
    },
};
use crate::{
    processing::{CodeChatForWeb, CodeMirror, SourceFileMetadata},
    translation::{create_translation_queues, translation_task},
    webserver::{
        EditorMessage, EditorMessageContents, RESERVED_MESSAGE_ID, UpdateMessageContents,
        client_websocket, get_client_framework, html_not_found, html_wrapper, path_display,
        send_response,
    },
};

// Globals
// -----------------------------------------------------------------------------
lazy_static! {
    /// Matches a bare drive letter. Only needed on Windows.
    static ref DRIVE_LETTER_REGEX: Regex = Regex::new("^[a-zA-Z]:$").unwrap();
}

pub const FILEWATCHER_PATH_PREFIX: &[&str] = &["fw", "fsc"];

/// File browser endpoints
/// ----------------------------------------------------------------------------
///
/// The file browser provides a very crude interface, allowing a user to select
/// a file from the local filesystem for editing. Long term, this should be
/// replaced by something better.
///
/// Redirect from the root of the filesystem to the actual root path on this OS.
pub async fn filewatcher_root_fs_redirect() -> impl Responder {
    HttpResponse::TemporaryRedirect()
        .insert_header((header::LOCATION, "/fw/fsb/"))
        .finish()
}

/// Dispatch to support functions which serve either a directory listing, a
/// CodeChat Editor file, or a normal file.
///
/// `fsb` stands for "FileSystem Browser" -- directories provide a simple
/// navigation GUI; files load the Client framework.
///
/// Omit code coverage -- this is a temporary interface, until IDE integration
/// replaces this.
#[cfg(not(tarpaulin_include))]
#[get("/fw/fsb/{path:.*}")]
async fn filewatcher_browser_endpoint(
    req: HttpRequest,
    app_state: WebAppState,
    orig_path: web::Path<String>,
) -> Result<HttpResponse, Error> {
    #[cfg(not(target_os = "windows"))]
    let fixed_path = orig_path.to_string();
    #[cfg(target_os = "windows")]
    let mut fixed_path = orig_path.to_string();
    #[cfg(target_os = "windows")]
    // On Windows, a path of `drive_letter:` needs a `/` appended.
    if DRIVE_LETTER_REGEX.is_match(&fixed_path) {
        fixed_path += "/";
    } else if fixed_path.is_empty() {
        // If there's no drive letter yet, we will always use `dir_listing` to
        // select a drive.
        return Ok(dir_listing("", Path::new("")).await);
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
            return Ok(html_not_found(&format!(
                "<p>The requested path <code>{fixed_path}</code> is not valid: {err}.</p>"
            )));
        }
    };
    if canon_path.is_dir() {
        return Ok(dir_listing(orig_path.as_str(), &canon_path).await);
    } else if canon_path.is_file() {
        // Get an ID for this connection.
        let connection_id_raw = get_connection_id_raw(&app_state);
        return processing_task(&canon_path, req, app_state, connection_id_raw).await;
    }

    // It's not a directory or a file...we give up. For simplicity, don't handle
    // symbolic links.
    Ok(html_not_found(&format!(
        "<p>The requested path <code>{}</code> is not a directory or a file.</p>",
        path_display(&canon_path)
    )))
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
            Err(err) => return html_not_found(&format!("Unable to list drive letters: {err}.")),
        };
        for drive_letter in logical_drives {
            drive_html.push_str(&format!(
                "<li><a href='/fw/fsb/{drive_letter}:/'>{drive_letter}:</a></li>\n"
            ));
        }

        return HttpResponse::Ok()
            .content_type(ContentType::html())
            .body(html_wrapper(&formatdoc!(
                "
                <h1>Drives</h1>
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
            ));
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
                            ));
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
                return html_not_found(&format!("<p>Unable to read file in directory: {err}."));
            }
        };
    }
    // Sort them -- case-insensitive on Windows, normally on Linux/OS X.
    #[cfg(target_os = "windows")]
    let file_name_key = |a: &DirEntry| {
        Ok::<String, std::ffi::OsString>(a.file_name().into_string()?.to_lowercase())
    };
    #[cfg(not(target_os = "windows"))]
    let file_name_key = |a: &DirEntry| a.file_name().into_string();
    files.sort_unstable_by_key(file_name_key);
    dirs.sort_unstable_by_key(file_name_key);

    // Put this on the resulting webpage. List directories first.
    let mut dir_html = String::new();
    // Add a separator if the web path doesn't end with it.
    let separator = if web_path.ends_with('/') || web_path.is_empty() {
        ""
    } else {
        "/"
    };
    for dir in dirs {
        let dir_name = match dir.file_name().into_string() {
            Ok(v) => v,
            Err(err) => {
                return html_not_found(&format!(
                    "<p>Unable to decode directory name '{err:?}' as UTF-8."
                ));
            }
        };
        let encoded_dir = urlencoding::encode(&dir_name);
        dir_html += &format!(
            "<li><a href='/fw/fsb/{web_path}{separator}{encoded_dir}'>{dir_name}</a></li>\n",
        );
    }

    // List files second.
    let mut file_html = String::new();
    for file in files {
        let file_name = match file.file_name().into_string() {
            Ok(v) => v,
            Err(err) => {
                return html_not_found(
                    &format!("<p>Unable to decode file name {err:?} as UTF-8.",),
                );
            }
        };
        let encoded_file = urlencoding::encode(&file_name);
        file_html += &formatdoc!(
            r#"
            <li><a href="/fw/fsb/{web_path}{separator}{encoded_file}" target="_blank">{file_name}</a></li>
            "#
        );
    }
    let body = formatdoc!(
        "
        <h1>Directory {}</h1>
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

const FW: &str = "fw-";

/// `fsc` stands for "FileSystem Client", and provides the Client contents from
/// the filesystem.
#[get("/fw/fsc/{connection_id_raw}/{file_path:.*}")]
async fn filewatcher_client_endpoint(
    request_path: web::Path<(String, String)>,
    req: HttpRequest,
    app_state: WebAppState,
) -> HttpResponse {
    let (connection_id_raw, file_path) = request_path.into_inner();
    filesystem_endpoint(
        format!("{FW}{connection_id_raw}"),
        file_path,
        &req,
        &app_state,
    )
    .await
}

async fn processing_task(
    file_path: &Path,
    req: HttpRequest,
    app_state: WebAppState,
    connection_id_raw: u32,
) -> Result<HttpResponse, Error> {
    // #### Filewatcher IDE
    //
    // This is a CodeChat Editor file. Start up the Filewatcher IDE tasks:
    //
    // 1. A task to watch for changes to the file, notifying the CodeChat Editor
    //    Client when the file should be reloaded.
    // 2. A task to receive and respond to messages from the CodeChat Editor
    //    Client.
    //
    // First, allocate variables needed by these two tasks.
    //
    // The path to the currently open CodeChat Editor file.
    let Ok(current_filepath) = file_path.to_path_buf().canonicalize() else {
        let msg = format!("Unable to canonicalize path {file_path:?}.");
        error!("{msg}");
        return Err(error::ErrorBadRequest(msg));
    };
    let mut current_filepath = Some(PathBuf::from(simplified(&current_filepath)));

    let connection_id_raw = connection_id_raw.to_string();
    let connection_id = format!("{FW}{connection_id_raw}");

    let created_translation_queues_result =
        create_translation_queues(connection_id.clone(), &app_state);
    let (from_ide_rx, to_ide_tx, from_client_rx, to_client_tx) =
        match created_translation_queues_result {
            Err(err) => {
                error!("{err}");
                return Err(error::ErrorBadRequest(err));
            }
            Ok(tqr) => (
                tqr.from_ide_rx,
                tqr.to_ide_tx,
                tqr.from_client_rx,
                tqr.to_client_tx,
            ),
        };

    // Transfer the queues from the global state to this task.
    let (from_ide_tx, mut to_ide_rx) =
        match app_state.ide_queues.lock().unwrap().remove(&connection_id) {
            Some(queues) => (queues.from_websocket_tx.clone(), queues.to_websocket_rx),
            None => {
                let err = "No websocket queues for connection id {connection_id}.";
                error!("{err}");
                return Err(error::ErrorBadRequest(err));
            }
        };

    // #### The filewatcher task.
    let connection_id_raw_task = connection_id_raw.clone();
    actix_rt::spawn(async move {
        let mut shutdown_only = true;
        let mut id: f64 = INITIAL_IDE_MESSAGE_ID;

        // Use a channel to send from the watcher (which runs in another thread)
        // into this async (task) context.
        let (watcher_tx, mut watcher_rx) = mpsc::channel(10);
        // Watch this file. Use the debouncer, to avoid multiple notifications
        // for the same file. This approach returns a result of either a working
        // debouncer or any errors that occurred. The debouncer's scope needs
        // live as long as this connection does; dropping it early means losing
        // file change notifications.
        let Ok(mut debounced_watcher) = new_debouncer(
            Duration::from_secs(2),
            None,
            // Note that this runs in a separate thread created by the watcher,
            // not in an async context. Therefore, use a blocking send.
            move |result: DebounceEventResult| {
                if let Err(err) = watcher_tx.blocking_send(result) {
                    // Note: we can't break here, since this runs in a separate
                    // thread. We have no way to shut down the task (which would
                    // be the best action to take.)
                    error!("Unable to send: {err}");
                }
            },
        ) else {
            error!("Unable to create debouncer.");
            return;
        };
        if let Some(ref cfp) = current_filepath
            && let Err(err) = debounced_watcher.watch(cfp, RecursiveMode::NonRecursive)
        {
            error!("Unable to watch file: {err}");
            return;
        }

        'task: {
            // Provide it a file to open.
            if let Some(cfp) = &current_filepath {
                let Some(cfp_str) = cfp.to_str() else {
                    let err = format!("Unable to convert file path {cfp:?} to string.");
                    error!("{err}");
                    break 'task;
                };
                queue_send!(from_ide_tx.send(EditorMessage {
                    id,
                    message: EditorMessageContents::CurrentFile(cfp_str.to_string(), None)
                }), 'task);
                // Note: it's OK to postpone the increment to here; if the
                // `queue_send` exits before this runs, the message didn't get
                // sent, so the ID wasn't used.
                id += MESSAGE_ID_INCREMENT;
            };

            shutdown_only = false;
        }

        // Now that the filewatcher is started, start the translation task then
        // proceed to the filewatcher main loop.
        actix_rt::spawn(translation_task(
            FW.to_string(),
            connection_id_raw_task,
            FILEWATCHER_PATH_PREFIX,
            app_state,
            shutdown_only,
            false,
            to_ide_tx,
            from_ide_rx,
            to_client_tx,
            from_client_rx,
        ));

        let mut is_closed = false;
        'task: loop {
            select! {
                // Process results produced by the file watcher.
                Some(result) = watcher_rx.recv() => {
                    match result {
                        Err(err_vec) => {
                            for err in err_vec {
                                // Report errors locally and to the CodeChat
                                // Editor.
                                let err = ResultErrTypes::FileWatchingError(err.to_string());
                                error!("{err:?}");
                                // Send using an ID which indicates this isn't a
                                // response to a message received from the
                                // client.
                                send_response(&from_ide_tx, RESERVED_MESSAGE_ID, Err(err)).await;
                            }
                        }

                        Ok(debounced_event_vec) => {
                            for debounced_event in debounced_event_vec {
                                let is_modify = match debounced_event.event.kind {
                                    // On OS X, we get a `Create` event when a
                                    // file is modified.
                                    EventKind::Create(_create_kind) => true,
                                    // On Windows, the `_modify_kind` is `Any`;
                                    // therefore; ignore it rather than trying
                                    // to look at only content modifications.
                                    EventKind::Modify(_modify_kind) => true,
                                    _ => {
                                        // TODO: handle delete.
                                        info!("Unhandled watcher event: {debounced_event:?}.");
                                        false
                                    }
                                };
                                if is_modify {
                                    if debounced_event.event.paths.len() != 1 ||
                                        current_filepath.as_ref().is_none_or(|cfp| cfp != &debounced_event.event.paths[0])
                                    {
                                        warn!("Modification to different file {}.", debounced_event.event.paths[0].to_string_lossy());
                                    } else {
                                        let cfp = current_filepath.as_ref().unwrap();
                                        let Some(current_filepath_str) = cfp.to_str() else {
                                            error!("Unable to convert path {cfp:?} to string.");
                                            break 'task;
                                        };

                                        // Since the parents are identical, send an
                                        // update. First, read the modified file.
                                        let mut file_contents = String::new();
                                        let read_ret = match File::open(&cfp).await {
                                            Ok(fc) => fc,
                                            Err(err) => {
                                                // We can't open the file -- it's been
                                                // moved or deleted. Close the file.
                                                error!("Unable to open file: {err}");
                                                break 'task;
                                            }
                                        }
                                        .read_to_string(&mut file_contents)
                                        .await;

                                        // Close the file if it can't be read as
                                        // Unicode text.
                                        if let Err(e) = read_ret {
                                            error!("Unable to read '{}': {e}", cfp.to_string_lossy());
                                            break 'task;
                                        }

                                        queue_send!(from_ide_tx.send(EditorMessage {
                                            id,
                                            message: EditorMessageContents::Update(UpdateMessageContents {
                                                file_path: current_filepath_str.to_string(),
                                                contents: Some(CodeChatForWeb {
                                                    metadata: SourceFileMetadata {
                                                        // The IDE doesn't need to provide this.
                                                        mode: "".to_string(),
                                                    },
                                                    source: crate::processing::CodeMirrorDiffable::Plain(CodeMirror {
                                                        doc: file_contents,
                                                        doc_blocks: vec![],
                                                    }),
                                                    // The filewatcher doesn't store a version,
                                                    // since it only accepts plain (non-diff)
                                                    // results. Provide a version so the Client
                                                    // stays in sync with any diffs. Produce a
                                                    // whole number to avoid encoding
                                                    // difference with fractional values.
                                                    version: random::<u64>() as f64,
                                                }),
                                                cursor_position: None,
                                                scroll_position: None,
                                            }),
                                        }));
                                        id += MESSAGE_ID_INCREMENT;
                                    }
                                }
                            }
                        }
                    }
                }

                Some(m) = to_ide_rx.recv() => {
                    match m.message {
                        EditorMessageContents::Update(update_message_contents) => {
                            let result = 'process: {
                                // Check that the file path matches the current
                                // file. If `canonicalize` fails, then the files
                                // don't match.
                                if Some(Path::new(&update_message_contents.file_path).to_path_buf()) != current_filepath {
                                    break 'process Err(ResultErrTypes::WrongFileUpdate(update_message_contents.file_path, current_filepath.clone()));
                                }
                                // With code or a path, there's nothing to save.
                                let codechat_for_web = match update_message_contents.contents {
                                    None => break 'process Ok(ResultOkTypes::Void),
                                    Some(cfw) => cfw,
                                };

                                // Translate from the CodeChatForWeb format to
                                // the contents of a source file.
                                let CodeMirrorDiffable::Plain(plain) = codechat_for_web.source else {
                                    error!("{}", "Diff not supported.");
                                    break 'task;
                                };
                                let cfp = current_filepath.as_ref().unwrap();
                                // Unwrap the file, write to it, then rewatch
                                // it, in order to avoid a watch notification
                                // from this write.
                                if let Err(err) = debounced_watcher.unwatch(cfp) {
                                    break 'process Err(ResultErrTypes::FileUnwatchError(cfp.to_path_buf(), err.to_string()));
                                }
                                // Save this string to a file.
                                if let Err(err) = fs::write(cfp.as_path(), plain.doc).await {
                                    break 'process Err(ResultErrTypes::SaveFileError(cfp.to_path_buf(), err.to_string()));
                                }
                                if let Err(err) = debounced_watcher.watch(cfp, RecursiveMode::NonRecursive) {
                                    break 'process Err(ResultErrTypes::FileWatchError(cfp.to_path_buf(), err.to_string()));
                                }
                                Ok(ResultOkTypes::Void)
                            };
                            send_response(&from_ide_tx, m.id, result).await;
                        }

                        EditorMessageContents::CurrentFile(file_path_str, _is_text) => {
                            let file_path = PathBuf::from(file_path_str.clone());
                            let result = 'err_exit: {
                                // Unwatch the old path.
                                if let Some(cfp) = &current_filepath
                                    && let Err(err) = debounced_watcher.unwatch(cfp)
                                {
                                    break 'err_exit Err(ResultErrTypes::FileUnwatchError(cfp.to_path_buf(), err.to_string()));
                                }
                                // Update to the new path.
                                current_filepath = Some(file_path.to_path_buf());

                                // Watch the new file.
                                if let Err(err) = debounced_watcher.watch(&file_path, RecursiveMode::NonRecursive) {
                                    break 'err_exit Err(ResultErrTypes::FileWatchError(file_path.to_path_buf(), err.to_string()));
                                }
                                // Indicate there was no error in the `Result`
                                // message.
                                Ok(ResultOkTypes::Void)
                            };
                            send_response(&from_ide_tx, m.id, result).await;
                        },

                        // Process a result, the respond to a message we sent.
                        EditorMessageContents::Result(message_result) => {
                            // Report errors to the log.
                            if let Err(err) = message_result {
                                error!("Error in message {}: {err}", m.id);
                            }
                        }

                        EditorMessageContents::Closed => {
                            info!("Filewatcher closing");
                            is_closed = true;
                            break;
                        }

                        EditorMessageContents::LoadFile(_)  => {
                            // We never have the requested file loaded in this
                            // "IDE". Intead, it's always on disk.
                            send_response(&from_ide_tx, m.id, Ok(ResultOkTypes::LoadFile(None))).await;
                        }

                        EditorMessageContents::Opened(_) |
                        EditorMessageContents::OpenUrl(_) |
                        EditorMessageContents::ClientHtml(_) |
                        EditorMessageContents::RequestClose => {
                            let err = ResultErrTypes::ClientIllegalMessage;
                            error!("{err:?}");
                            send_response(&from_ide_tx, m.id, Err(err)).await;
                        }
                    }
                }

                else => break
            }
        }

        #[allow(clippy::never_loop)]
        loop {
            if !is_closed {
                queue_send!(from_ide_tx.send(EditorMessage {
                    id,
                    message: EditorMessageContents::Closed
                }));
            }
            break;
        }
        info!("Watcher closed.");
    });

    match get_client_framework(get_test_mode(&req), "fw/ws", &connection_id_raw.to_string()) {
        Ok(s) => Ok(HttpResponse::Ok().content_type(ContentType::html()).body(s)),
        Err(err) => Err(error::ErrorBadRequest(err)),
    }
}

/// Define a websocket handler for the CodeChat Editor Client.
#[get("/fw/ws/{connection_id_raw}")]
pub async fn filewatcher_websocket(
    connection_id_raw: web::Path<String>,
    req: HttpRequest,
    body: web::Payload,
    app_state: WebAppState,
) -> Result<HttpResponse, Error> {
    client_websocket(
        format!("{FW}{connection_id_raw}"),
        req,
        body,
        app_state.client_queues.clone(),
    )
}

/// Return a unique ID for an IDE websocket connection.
pub fn get_connection_id_raw(app_state: &WebAppState) -> u32 {
    let mut connection_id_raw = app_state.filewatcher_next_connection_id.lock().unwrap();
    *connection_id_raw += 1;
    *connection_id_raw
}

// Tests
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use std::{
        backtrace::Backtrace,
        env, fs,
        path::{Path, PathBuf},
        str::FromStr,
        time::Duration,
    };

    use actix_http::Request;
    use actix_web::{
        App,
        body::BoxBody,
        dev::{Service, ServiceResponse},
        test,
    };
    use dunce::simplified;
    use path_slash::PathExt;
    use pretty_assertions::assert_eq;
    use tokio::{select, sync::mpsc::Receiver, time::sleep};
    use url::Url;

    use super::FW;
    use crate::{
        cast, prep_test_dir,
        processing::{
            CodeChatForWeb, CodeMirror, CodeMirrorDiffable, SourceFileMetadata, TranslationResults,
            source_to_codechat_for_web,
        },
        test_utils::{check_logger_errors, configure_testing_logger},
        webserver::{
            EditorMessage, EditorMessageContents, INITIAL_CLIENT_MESSAGE_ID,
            INITIAL_IDE_MESSAGE_ID, INITIAL_MESSAGE_ID, IdeType, MESSAGE_ID_INCREMENT,
            ResultErrTypes, ResultOkTypes, UpdateMessageContents, WebAppState, WebsocketQueues,
            configure_app, drop_leading_slash, make_app_data, send_response, set_root_path,
        },
    };

    async fn get_websocket_queues(
        // A path to the temporary directory where the source file is located.
        test_dir: &Path,
    ) -> (
        WebsocketQueues,
        impl Service<Request, Response = ServiceResponse<BoxBody>, Error = actix_web::Error> + use<>,
    ) {
        set_root_path(None).unwrap();
        let app_data = make_app_data(None);
        let app = test::init_service(configure_app(App::new(), &app_data)).await;

        // Load in a test source file to create a websocket.
        let uri = format!("/fw/fsb/{}/test.py", test_dir.to_string_lossy());
        let req = test::TestRequest::get().uri(&uri).to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        // Even after the webpage is served, the websocket task hasn't started.
        // Wait a bit for that.
        sleep(Duration::from_millis(10)).await;

        // The web page has been served; fake the connected websocket by getting
        // the appropriate tx/rx queues.
        let app_state = resp.request().app_data::<WebAppState>().unwrap();
        let mut client_queues = app_state.client_queues.lock().unwrap();
        let connection_id_raw = *app_state.filewatcher_next_connection_id.lock().unwrap();
        assert_eq!(client_queues.len(), 1);
        (
            client_queues
                .remove(&format!("{FW}{connection_id_raw}"))
                .unwrap(),
            app,
        )
    }

    async fn get_message(client_rx: &mut Receiver<EditorMessage>) -> EditorMessage {
        select! {
            data = client_rx.recv() => {
                let m = data.unwrap();
                // For debugging, print out each message.
                println!("{} - {:?}", m.id, m.message);
                m
            }
            _ = sleep(Duration::from_secs(3)) => {
                // The backtrace shows what message the code was waiting for; otherwise, it's an unhelpful error message.
                panic!("Timeout waiting for message:\n{}", Backtrace::force_capture());
            }
        }
    }

    macro_rules! get_message_as {
        ($client_rx: expr, $cast_type: ty) => {{
            let m = get_message(&mut $client_rx).await;
            (m.id, cast!(m.message, $cast_type))
        }};
        ($client_rx: expr, $cast_type: ty, $( $tup: ident),*) => {{
            let m = get_message(&mut $client_rx).await;
            (m.id, cast!(m.message, $cast_type, $($tup),*))
        }};
    }

    #[actix_web::test]
    async fn test_websocket_opened_1() {
        configure_testing_logger();
        let (temp_dir, test_dir) = prep_test_dir!();
        let (wq, app) = get_websocket_queues(&test_dir).await;
        let from_client_tx = wq.from_websocket_tx;
        let mut to_client_rx = wq.to_websocket_rx;

        // The initial web request for the Client framework produces a
        // `CurrentFile`.
        //
        // Message ids: IDE - 0->1, Server - 2, Client - 0.
        let (id, (url_string, is_text)) = get_message_as!(
            to_client_rx,
            EditorMessageContents::CurrentFile,
            file_name,
            is_text
        );
        assert_eq!(id, INITIAL_IDE_MESSAGE_ID);
        assert_eq!(is_text, Some(true));
        // Acknowledge it.
        send_response(&from_client_tx, id, Ok(ResultOkTypes::Void)).await;

        // Compute the path this message should contain.
        let mut test_path = test_dir.clone();
        test_path.push("test.py");
        // The comparison below fails without this.
        let test_path = test_path.canonicalize().unwrap();
        // The URL parser requires a valid origin.
        let url = Url::parse(&format!("http://foo.com{url_string}")).unwrap();
        let url_segs: Vec<_> = url
            .path_segments()
            .unwrap()
            .map(|s| urlencoding::decode(s).unwrap())
            .collect();
        let mut url_path = if cfg!(windows) {
            PathBuf::new()
        } else {
            PathBuf::from_str("/").unwrap()
        };
        url_path.push(PathBuf::from_str(&url_segs[3..].join("/")).unwrap());
        let url_path = url_path.canonicalize().unwrap();
        assert_eq!(url_path, test_path);

        // 2. After fetching the file, we should get an update.
        //
        // Message ids: IDE - 1, Server - 2->3, Client - 0.
        let uri = format!(
            "/fw/fsc/1/{}/test.py",
            drop_leading_slash(&test_dir.to_slash().unwrap())
        );
        let req = test::TestRequest::get().uri(&uri).to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let (id, umc) = get_message_as!(to_client_rx, EditorMessageContents::Update);
        assert_eq!(id, INITIAL_MESSAGE_ID + 2.0 * MESSAGE_ID_INCREMENT);
        send_response(&from_client_tx, id, Ok(ResultOkTypes::Void)).await;

        // Check the contents.
        let translation_results = source_to_codechat_for_web(
            "",
            &"py".to_string(),
            umc.contents.as_ref().unwrap().version,
            false,
            false,
        );
        let tr = cast!(translation_results, Ok);
        let codechat_for_web = cast!(tr, TranslationResults::CodeChat);
        assert_eq!(umc.contents, Some(codechat_for_web));

        // Report any errors produced when removing the temporary directory.
        check_logger_errors(0);
        temp_dir.close().unwrap();
    }

    #[actix_web::test]
    async fn test_websocket_update_1() {
        configure_testing_logger();
        let (temp_dir, test_dir) = prep_test_dir!();
        let (wq, app) = get_websocket_queues(&test_dir).await;
        let from_client_tx = wq.from_websocket_tx;
        let mut to_client_rx = wq.to_websocket_rx;

        // 1. The initial web request for the Client framework produces a
        //    `CurrentFile`.
        //
        // Message ids: IDE - 0->1, Server - 2, Client - 0.
        let (id, (..)) = get_message_as!(
            to_client_rx,
            EditorMessageContents::CurrentFile,
            file_name,
            is_text
        );
        assert_eq!(id, INITIAL_IDE_MESSAGE_ID);
        send_response(&from_client_tx, id, Ok(ResultOkTypes::Void)).await;

        // 2. After fetching the file, we should get an update. The Server sends
        //    a `LoadFile` to the IDE using message the next ID; therefore, this
        //    consumes two IDs.
        //
        // Message ids: IDE - 1, Server - 2->3, Client - 0.
        let mut file_path = test_dir.clone();
        file_path.push("test.py");
        let file_path = simplified(&file_path.canonicalize().unwrap())
            .to_str()
            .unwrap()
            .to_string();
        let uri = format!(
            "/fw/fsc/1/{}/test.py",
            drop_leading_slash(&test_dir.to_slash().unwrap())
        );
        let req = test::TestRequest::get().uri(&uri).to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let (id, _) = get_message_as!(to_client_rx, EditorMessageContents::Update);
        assert_eq!(id, INITIAL_MESSAGE_ID + 2.0 * MESSAGE_ID_INCREMENT);
        send_response(&from_client_tx, id, Ok(ResultOkTypes::Void)).await;

        // 3. Send an update message with no contents.
        //
        // Message ids: IDE - 1, Server - 3, Client - 0->1.
        from_client_tx
            .send(EditorMessage {
                id: INITIAL_CLIENT_MESSAGE_ID,
                message: EditorMessageContents::Update(UpdateMessageContents {
                    file_path: file_path.clone(),
                    contents: None,
                    cursor_position: None,
                    scroll_position: None,
                }),
            })
            .await
            .unwrap();

        // Check that it produces no error.
        assert_eq!(
            get_message_as!(to_client_rx, EditorMessageContents::Result),
            (INITIAL_CLIENT_MESSAGE_ID, Ok(ResultOkTypes::Void))
        );

        // 4. Send invalid messages.
        //
        // Message ids: IDE - 1, Server - 3, Client - 1->4.
        for (id, msg) in [
            (
                INITIAL_CLIENT_MESSAGE_ID + MESSAGE_ID_INCREMENT,
                EditorMessageContents::Opened(IdeType::VSCode(true)),
            ),
            (
                INITIAL_CLIENT_MESSAGE_ID + 2.0 * MESSAGE_ID_INCREMENT,
                EditorMessageContents::ClientHtml("".to_string()),
            ),
            (
                INITIAL_CLIENT_MESSAGE_ID + 3.0 * MESSAGE_ID_INCREMENT,
                EditorMessageContents::RequestClose,
            ),
        ] {
            from_client_tx
                .send(EditorMessage { id, message: msg })
                .await
                .unwrap();
            let (id_rx, msg_rx) = get_message_as!(to_client_rx, EditorMessageContents::Result);
            assert_eq!(id, id_rx);
            matches!(cast!(&msg_rx, Err), ResultErrTypes::ClientIllegalMessage);
        }

        // 5. Send an update message with no path.
        //
        // Message ids: IDE - 1, Server - 3, Client - 4->5.
        from_client_tx
            .send(EditorMessage {
                id: INITIAL_CLIENT_MESSAGE_ID + 4.0 * MESSAGE_ID_INCREMENT,
                message: EditorMessageContents::Update(UpdateMessageContents {
                    file_path: "".to_string(),
                    contents: Some(CodeChatForWeb {
                        metadata: SourceFileMetadata {
                            mode: "".to_string(),
                        },
                        source: CodeMirrorDiffable::Plain(CodeMirror {
                            doc: "".to_string(),
                            doc_blocks: vec![],
                        }),
                        version: 0.0,
                    }),
                    cursor_position: None,
                    scroll_position: None,
                }),
            })
            .await
            .unwrap();

        // Check that it produces an error.
        let (id, err_msg) = get_message_as!(to_client_rx, EditorMessageContents::Result);
        assert_eq!(id, INITIAL_CLIENT_MESSAGE_ID + 4.0 * MESSAGE_ID_INCREMENT);
        cast!(
            err_msg.as_ref().unwrap_err(),
            ResultErrTypes::WrongFileUpdate,
            _a,
            _b
        );

        // 6. Send an update message with unknown source language.
        //
        // Message ids: IDE - 1, Server - 3, Client - 5->6.
        from_client_tx
            .send(EditorMessage {
                id: INITIAL_CLIENT_MESSAGE_ID + 5.0 * MESSAGE_ID_INCREMENT,
                message: EditorMessageContents::Update(UpdateMessageContents {
                    file_path: file_path.clone(),
                    contents: Some(CodeChatForWeb {
                        metadata: SourceFileMetadata {
                            mode: "nope".to_string(),
                        },
                        source: CodeMirrorDiffable::Plain(CodeMirror {
                            doc: "testing".to_string(),
                            doc_blocks: vec![],
                        }),
                        version: 1.0,
                    }),
                    cursor_position: None,
                    scroll_position: None,
                }),
            })
            .await
            .unwrap();

        // Check that it produces an error.
        let (msg_id, msg) = get_message_as!(to_client_rx, EditorMessageContents::Result);
        assert_eq!(
            msg_id,
            INITIAL_CLIENT_MESSAGE_ID + 5.0 * MESSAGE_ID_INCREMENT
        );
        cast!(
            msg.as_ref().unwrap_err(),
            ResultErrTypes::CannotTranslateCodeChat
        );

        // 7. Send a valid message.
        //
        // Message ids: IDE - 1, Server - 3, Client - 6->7.
        from_client_tx
            .send(EditorMessage {
                id: INITIAL_CLIENT_MESSAGE_ID + 6.0 * MESSAGE_ID_INCREMENT,
                message: EditorMessageContents::Update(UpdateMessageContents {
                    file_path: file_path.clone(),
                    contents: Some(CodeChatForWeb {
                        metadata: SourceFileMetadata {
                            mode: "python".to_string(),
                        },
                        source: CodeMirrorDiffable::Plain(CodeMirror {
                            doc: "testing()".to_string(),
                            doc_blocks: vec![],
                        }),
                        version: 2.0,
                    }),
                    cursor_position: None,
                    scroll_position: None,
                }),
            })
            .await
            .unwrap();
        assert_eq!(
            get_message_as!(to_client_rx, EditorMessageContents::Result),
            (
                INITIAL_CLIENT_MESSAGE_ID + 6.0 * MESSAGE_ID_INCREMENT,
                Ok(ResultOkTypes::Void)
            )
        );

        // Check that the requested file was written.
        let mut s = fs::read_to_string(&file_path).unwrap();
        assert_eq!(s, "testing()");

        // 8. Change this file and verify that this produces an update.
        //
        // Message ids: IDE - 1->2, Server - 3, Client - 7.
        s.push_str("123");
        fs::write(&file_path, s).unwrap();
        // Wait for the filewatcher to debounce this file write.
        sleep(Duration::from_secs(
            // Mac in CI seems to need a long delay here.
            if cfg!(target_os = "macos") && env::var("CI") == Ok("true".to_string()) {
                5
            } else {
                2
            },
        ))
        .await;
        // The version is random; don't check it with a fixed value.
        let msg = get_message_as!(to_client_rx, EditorMessageContents::Update);
        assert_eq!(
            msg,
            (
                INITIAL_IDE_MESSAGE_ID + MESSAGE_ID_INCREMENT,
                UpdateMessageContents {
                    file_path: file_path.clone(),
                    contents: Some(CodeChatForWeb {
                        metadata: SourceFileMetadata {
                            mode: "python".to_string(),
                        },
                        source: CodeMirrorDiffable::Plain(CodeMirror {
                            doc: "testing()123".to_string(),
                            doc_blocks: vec![],
                        }),
                        version: msg.1.contents.as_ref().unwrap().version,
                    }),
                    cursor_position: None,
                    scroll_position: None,
                }
            )
        );
        // Acknowledge this message.
        send_response(
            &from_client_tx,
            INITIAL_IDE_MESSAGE_ID + MESSAGE_ID_INCREMENT,
            Ok(ResultOkTypes::Void),
        )
        .await;

        // 9. Rename it and check for an close (the file watcher can't detect
        //    the destination file, so it's treated as the file is deleted).
        //
        // Message ids: IDE - 2->3, Server - 3, Client - 7.
        let mut dest = PathBuf::from(&file_path).parent().unwrap().to_path_buf();
        dest.push("test2.py");
        fs::rename(file_path, dest.as_path()).unwrap();
        // Wait for the filewatcher to debounce this file write.
        sleep(Duration::from_secs(3)).await;
        let m = get_message(&mut to_client_rx).await;
        assert_eq!(m.id, INITIAL_IDE_MESSAGE_ID + 2.0 * MESSAGE_ID_INCREMENT);
        assert!(matches!(m.message, EditorMessageContents::Closed));
        send_response(
            &from_client_tx,
            INITIAL_IDE_MESSAGE_ID + 2.0 * MESSAGE_ID_INCREMENT,
            Ok(ResultOkTypes::Void),
        )
        .await;

        // Each of the three invalid message types produces one error.
        check_logger_errors(5);
        // Report any errors produced when removing the temporary directory.
        temp_dir.close().unwrap();
    }
}
