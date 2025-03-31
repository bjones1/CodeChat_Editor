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
/// `filewatcher.rs` -- Implement the File Watcher "IDE"
/// ====================================================
// Imports
// -------
//
// ### Standard library
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

// ### Third-party
use actix_web::{
    HttpRequest, HttpResponse, Responder,
    error::Error,
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
use super::{
    AppState, EditorMessage, EditorMessageContents, UpdateMessageContents, WebsocketQueues,
    client_websocket, escape_html, get_client_framework, get_connection_id, html_not_found,
    html_wrapper, path_display, send_response,
};
use crate::{
    oneshot_send,
    processing::{
        TranslationResultsString, codechat_for_web_to_source, source_to_codechat_for_web_string,
    },
    queue_send,
    webserver::{
        ResultOkTypes, filesystem_endpoint, get_test_mode, make_simple_http_response, path_to_url,
        url_to_path,
    },
};

// Globals
// -------
lazy_static! {
    /// Matches a bare drive letter. Only needed on Windows.
    static ref DRIVE_LETTER_REGEX: Regex = Regex::new("^[a-zA-Z]:$").unwrap();
}

pub const FILEWATCHER_PATH_PREFIX: &[&str] = &["fw", "fsc"];

/// File browser endpoints
/// ----------------------
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
    app_state: web::Data<AppState>,
    orig_path: web::Path<String>,
) -> impl Responder {
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
            ));
        }
    };
    if canon_path.is_dir() {
        return dir_listing(orig_path.as_str(), &canon_path).await;
    } else if canon_path.is_file() {
        // Get an ID for this connection.
        let connection_id = get_connection_id(&app_state);
        actix_rt::spawn(async move {
            processing_task(&canon_path, app_state, connection_id).await;
        });
        return match get_client_framework(get_test_mode(&req), "fw/ws", &connection_id.to_string())
        {
            Ok(s) => HttpResponse::Ok().content_type(ContentType::html()).body(s),
            Err(err) => html_not_found(&format!("<p>{}</p>", escape_html(&err))),
        };
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
            "<li><a href='/fw/fsb/{web_path}{}{encoded_dir}'>{dir_name}</a></li>\n",
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
                return html_not_found(
                    &format!("<p>Unable to decode file name {err:?} as UTF-8.",),
                );
            }
        };
        let encoded_file = urlencoding::encode(&file_name);
        file_html += &formatdoc!(
            r#"
            <li><a href="/fw/fsb/{web_path}/{encoded_file}" target="_blank">{file_name}</a></li>
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

/// `fsc` stands for "FileSystem Client", and provides the Client contents from
/// the filesystem.
#[get("/fw/fsc/{connection_id}/{file_path:.*}")]
async fn filewatcher_client_endpoint(
    request_path: web::Path<(String, String)>,
    req: HttpRequest,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    filesystem_endpoint(request_path, &req, &app_state).await
}

async fn processing_task(file_path: &Path, app_state: web::Data<AppState>, connection_id: u32) {
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
    // The path to the currently open CodeChat Editor file.
    let Ok(current_filepath) = file_path.to_path_buf().canonicalize() else {
        error!("Unable to canonicalize path {file_path:?}.");
        return;
    };
    let mut current_filepath = Some(PathBuf::from(simplified(&current_filepath)));
    // #### The filewatcher task.
    actix_rt::spawn(async move {
        'task: {
            // Use a channel to send from the watcher (which runs in another
            // thread) into this async (task) context.
            let (watcher_tx, mut watcher_rx) = mpsc::channel(10);
            // Watch this file. Use the debouncer, to avoid multiple
            // notifications for the same file. This approach returns a result
            // of either a working debouncer or any errors that occurred. The
            // debouncer's scope needs live as long as this connection does;
            // dropping it early means losing file change notifications.
            let Ok(mut debounced_watcher) = new_debouncer(
                Duration::from_secs(2),
                None,
                // Note that this runs in a separate thread created by the
                // watcher, not in an async context. Therefore, use a blocking
                // send.
                move |result: DebounceEventResult| {
                    if let Err(err) = watcher_tx.blocking_send(result) {
                        // Note: we can't break here, since this runs in a
                        // separate thread. We have no way to shut down the task
                        // (which would be the best action to take.)
                        error!("Unable to send: {err}");
                    }
                },
            ) else {
                error!("Unable to create debouncer.");
                break 'task;
            };
            if let Some(ref cfp) = current_filepath {
                if let Err(err) = debounced_watcher.watch(cfp, RecursiveMode::NonRecursive) {
                    error!("Unable to watch file: {err}");
                    break 'task;
                };
            }

            // Create the queues for the websocket connection to communicate
            // with this task.
            let (from_websocket_tx, mut from_websocket_rx) = mpsc::channel(10);
            let (to_websocket_tx, to_websocket_rx) = mpsc::channel(10);
            app_state.filewatcher_client_queues.lock().unwrap().insert(
                connection_id.to_string(),
                WebsocketQueues {
                    from_websocket_tx,
                    to_websocket_rx,
                },
            );

            // Provide it a file to open.
            let mut id: f64 = 0.0;
            if let Some(cfp) = &current_filepath {
                let url_pathbuf = path_to_url("/fw/fsc", &connection_id.to_string(), cfp);
                queue_send!(to_websocket_tx.send(EditorMessage {
                    id,
                    message: EditorMessageContents::CurrentFile(url_pathbuf, None)
                }), 'task);
                // Note: it's OK to postpone the increment to here; if the
                // `queue_send` exits before this runs, the message didn't get
                // sent, so the ID wasn't used.
                id += 1.0;
            };

            // Create a queue for HTTP requests fo communicate with this task.
            let (from_http_tx, mut from_http_rx) = mpsc::channel(10);
            app_state
                .processing_task_queue_tx
                .lock()
                .unwrap()
                .insert(connection_id.to_string(), from_http_tx);

            loop {
                select! {
                    // Process results produced by the file watcher.
                    Some(result) = watcher_rx.recv() => {
                        match result {
                            Err(err_vec) => {
                                for err in err_vec {
                                    // Report errors locally and to the CodeChat
                                    // Editor.
                                    let msg = format!("Watcher error: {err}");
                                    error!("{msg}");
                                    // Send using ID 0 to indicate this isn't a
                                    // response to a message received from the
                                    // client.
                                    send_response(&to_websocket_tx, 0.0, Err(msg)).await;
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
                                            info!("Watcher event: {debounced_event:?}.");
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
                                            let result = 'process: {
                                                // Since the parents are identical, send an
                                                // update. First, read the modified file.
                                                let mut file_contents = String::new();
                                                let read_ret = match File::open(&cfp).await {
                                                    Ok(fc) => fc,
                                                    Err(_err) => {
                                                        // We can't open the file -- it's been
                                                        // moved or deleted. Close the file.
                                                        break 'process Err(());
                                                    }
                                                }
                                                .read_to_string(&mut file_contents)
                                                .await;

                                                // Close the file if it can't be read as
                                                // Unicode text.
                                                if read_ret.is_err() {
                                                    error!("Unable to read '{}': {}", cfp.to_string_lossy(), read_ret.unwrap_err());
                                                    break 'process Err(());
                                                }

                                                // Translate the file.
                                                let (translation_results_string, _path_to_toc) =
                                                source_to_codechat_for_web_string(&file_contents, cfp, false);
                                                if let TranslationResultsString::CodeChat(cc) = translation_results_string {
                                                    let Some(current_filepath_str) = cfp.to_str() else {
                                                        error!("Unable to convert path {cfp:?} to string.");
                                                        break 'process Err(());
                                                    };
                                                    // Send the new contents.
                                                    Ok(EditorMessage {
                                                            id,
                                                            message: EditorMessageContents::Update(UpdateMessageContents {
                                                                file_path: current_filepath_str.to_string(),
                                                                contents: Some(cc),
                                                                cursor_position: None,
                                                                scroll_position: None,
                                                            }),
                                                        })
                                                } else {
                                                    break 'process Err(());
                                                }
                                            };
                                            if let Ok(editor_message) = result {
                                                queue_send!(to_websocket_tx.send(editor_message));
                                                id += 1.0;
                                            } else {
                                                // We can't open the file -- it's been
                                                // moved or deleted. Close the file.
                                                queue_send!(to_websocket_tx.send(EditorMessage {
                                                    id,
                                                    message: EditorMessageContents::Closed
                                                }));
                                                id += 1.0;

                                                // Unwatch it.
                                                if let Err(err) = debounced_watcher.unwatch(cfp) {
                                                    error!(
                                                        "Unable to unwatch file '{}': {err}.",
                                                        cfp.to_string_lossy()
                                                    );
                                                }
                                                current_filepath = None;
                                                continue;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    Some(http_request) = from_http_rx.recv() => {
                        // If there's no current file, replace it with an empty
                        // file, which will still produce an error.
                        let empty_path = PathBuf::new();
                        let cfp = current_filepath.as_ref().unwrap_or(&empty_path);
                        let (simple_http_response, option_update) = make_simple_http_response(&http_request, cfp, false).await;
                        if let Some(update) = option_update {
                            // Send the update to the client.
                            queue_send!(to_websocket_tx.send(EditorMessage { id, message: update }));
                            id += 1.0;
                        }
                        oneshot_send!(http_request.response_queue.send(simple_http_response));
                    }

                    Some(m) = from_websocket_rx.recv() => {
                        match m.message {
                            EditorMessageContents::Update(update_message_contents) => {
                                let result = 'process: {
                                    // Check that the file path matches the
                                    // current file. If `canonicalize` fails,
                                    // then the files don't match.
                                    if Some(Path::new(&update_message_contents.file_path).to_path_buf()) != current_filepath {
                                        break 'process Err(format!(
                                            "Update for file '{}' doesn't match current file '{current_filepath:?}'.",
                                            update_message_contents.file_path
                                        ));
                                    }
                                    // With code or a path, there's nothing to
                                    // save.
                                    let codechat_for_web = match update_message_contents.contents {
                                        None => break 'process Ok(ResultOkTypes::Void),
                                        Some(cfw) => cfw,
                                    };

                                    // Translate from the CodeChatForWeb format
                                    // to the contents of a source file.
                                    let file_contents = match codechat_for_web_to_source(
                                        &codechat_for_web,
                                    ) {
                                        Ok(r) => r,
                                        Err(message) => {
                                            break 'process Err(format!(
                                                "Unable to translate to source: {message}"
                                            ));
                                        }
                                    };

                                    let cfp = current_filepath.as_ref().unwrap();
                                    // Unwrap the file, write to it, then
                                    // rewatch it, in order to avoid a watch
                                    // notification from this write.
                                    if let Err(err) = debounced_watcher.unwatch(cfp) {
                                        let msg = format!(
                                            "Unable to unwatch file '{}': {err}.",
                                            cfp.to_string_lossy()
                                        );
                                        break 'process Err(msg);
                                    }
                                    // Save this string to a file.
                                    if let Err(err) = fs::write(cfp.as_path(), file_contents).await {
                                        let msg = format!(
                                            "Unable to save file '{}': {err}.",
                                            cfp.to_string_lossy()
                                        );
                                        break 'process Err(msg);
                                    }
                                    if let Err(err) = debounced_watcher.watch(cfp, RecursiveMode::NonRecursive) {
                                        let msg = format!(
                                            "Unable to watch file '{}': {err}.",
                                            cfp.to_string_lossy()
                                        );
                                        break 'process Err(msg);
                                    }
                                    Ok(ResultOkTypes::Void)
                                };
                                send_response(&to_websocket_tx, m.id, result).await;
                            }

                            EditorMessageContents::CurrentFile(url_string, _is_text) => {
                                let result = match url_to_path(&url_string, FILEWATCHER_PATH_PREFIX) {
                                    Err(err) => Err(err),
                                    Ok(ref file_path) => 'err_exit: {
                                        // We finally have the desired path! First,
                                        // unwatch the old path.
                                        if let Some(cfp) = &current_filepath {
                                            if let Err(err) = debounced_watcher.unwatch(cfp) {
                                                break 'err_exit Err(format!(
                                                    "Unable to unwatch file '{}': {err}.",
                                                    cfp.to_string_lossy()
                                                ));
                                            }
                                        };
                                        // Update to the new path.
                                        current_filepath = Some(file_path.to_path_buf());

                                        // Watch the new file.
                                        if let Err(err) = debounced_watcher.watch(file_path, RecursiveMode::NonRecursive) {
                                            break 'err_exit Err(format!(
                                                "Unable to watch file '{}': {err}.",
                                                file_path.to_string_lossy()
                                            ));
                                        }

                                        // Indicate there was no error in the
                                        // `Result` message.
                                        Ok(ResultOkTypes::Void)
                                    }
                                };
                                send_response(&to_websocket_tx, m.id, result).await;
                            },

                            // Process a result, the respond to a message we
                            // sent.
                            EditorMessageContents::Result(message_result) => {
                                // Report errors to the log.
                                if let Err(err) = message_result {
                                    error!("Error in message {}: {err}", m.id);
                                }
                            }

                            EditorMessageContents::Closed => {
                                info!("Filewatcher closing");
                                break;
                            }

                            EditorMessageContents::Opened(_) |
                            EditorMessageContents::OpenUrl(_) |
                            EditorMessageContents::LoadFile(_) |
                            EditorMessageContents::ClientHtml(_) |
                            EditorMessageContents::RequestClose => {
                                let msg = format!("Client sent unsupported message type {m:?}");
                                error!("{msg}");
                                send_response(&to_websocket_tx, m.id, Err(msg)).await;
                            }
                        }
                    }

                    else => break
                }
            }

            from_websocket_rx.close();
            if app_state
                .processing_task_queue_tx
                .lock()
                .unwrap()
                .remove(&connection_id.to_string())
                .is_none()
            {
                error!(
                    "Unable to remove connection ID {connection_id} from processing task queues."
                );
            }
            // Drain any remaining messages after closing the queue.
            while let Some(m) = from_websocket_rx.recv().await {
                warn!("Dropped queued message {m:?}");
            }
        }

        info!("Watcher closed.");
    });
}

/// Define a websocket handler for the CodeChat Editor Client.
#[get("/fw/ws/{connection_id}")]
pub async fn filewatcher_websocket(
    connection_id: web::Path<String>,
    req: HttpRequest,
    body: web::Payload,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
    client_websocket(
        connection_id,
        req,
        body,
        app_state.filewatcher_client_queues.clone(),
    )
    .await
}

// Tests
// -----
#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        str::FromStr,
        time::Duration,
    };

    use actix_http::Request;
    use actix_web::{
        App,
        body::BoxBody,
        dev::{Service, ServiceResponse},
        test, web,
    };
    use assertables::assert_starts_with;
    use dunce::simplified;
    use path_slash::PathExt;
    use tokio::{select, sync::mpsc::Receiver, time::sleep};
    use url::Url;

    use super::{
        super::{WebsocketQueues, configure_app, make_app_data},
        AppState, EditorMessage, EditorMessageContents, UpdateMessageContents, send_response,
    };
    use crate::{
        cast, prep_test_dir,
        processing::{
            CodeChatForWeb, CodeMirror, SourceFileMetadata, TranslationResults,
            source_to_codechat_for_web,
        },
        test_utils::{check_logger_errors, configure_testing_logger},
        webserver::{IdeType, ResultOkTypes, drop_leading_slash, tests::IP_PORT},
    };

    async fn get_websocket_queues(
        // A path to the temporary directory where the source file is located.
        test_dir: &Path,
    ) -> (
        WebsocketQueues,
        impl Service<Request, Response = ServiceResponse<BoxBody>, Error = actix_web::Error> + use<>,
    ) {
        let app_data = make_app_data(IP_PORT);
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
        let app_state = resp.request().app_data::<web::Data<AppState>>().unwrap();
        let mut joint_editors = app_state.filewatcher_client_queues.lock().unwrap();
        let connection_id = *app_state.connection_id.lock().unwrap();
        assert_eq!(joint_editors.len(), 1);
        (
            joint_editors.remove(&connection_id.to_string()).unwrap(),
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
            _ = sleep(Duration::from_secs(3)) => panic!("Timeout waiting for message")
        }
    }

    macro_rules! get_message_as {
        ($client_rx: expr_2021, $cast_type: ty) => {{
            let m = get_message(&mut $client_rx).await;
            (m.id, cast!(m.message, $cast_type))
        }};
        ($client_rx: expr_2021, $cast_type: ty, $( $tup: ident),*) => {{
            let m = get_message(&mut $client_rx).await;
            (m.id, cast!(m.message, $cast_type, $($tup),*))
        }};
    }

    #[actix_web::test]
    async fn test_websocket_opened_1() {
        configure_testing_logger();
        let (temp_dir, test_dir) = prep_test_dir!();
        let (je, app) = get_websocket_queues(&test_dir).await;
        let ide_tx_queue = je.from_websocket_tx;
        let mut client_rx = je.to_websocket_rx;

        // The initial web request for the Client framework produces a
        // `CurrentFile`.
        let (id, (url_string, is_text)) = get_message_as!(
            client_rx,
            EditorMessageContents::CurrentFile,
            file_name,
            is_text
        );
        assert_eq!(id, 0.0);
        assert_eq!(is_text, None);

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
        send_response(&ide_tx_queue, id, Ok(ResultOkTypes::Void)).await;

        // 2.  After fetching the file, we should get an update.
        let uri = format!(
            "/fw/fsc/1/{}/test.py",
            drop_leading_slash(&test_dir.to_slash().unwrap())
        );
        let req = test::TestRequest::get().uri(&uri).to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let (id, umc) = get_message_as!(client_rx, EditorMessageContents::Update);
        assert_eq!(id, 1.0);
        send_response(&ide_tx_queue, id, Ok(ResultOkTypes::Void)).await;

        // Check the contents.
        let translation_results = source_to_codechat_for_web("", &"py".to_string(), false, false);
        let codechat_for_web = cast!(translation_results, TranslationResults::CodeChat);
        assert_eq!(umc.contents, Some(codechat_for_web));

        // Report any errors produced when removing the temporary directory.
        check_logger_errors(0);
        temp_dir.close().unwrap();
    }

    #[actix_web::test]
    async fn test_websocket_update_1() {
        configure_testing_logger();
        let (temp_dir, test_dir) = prep_test_dir!();
        let (je, app) = get_websocket_queues(&test_dir).await;
        let ide_tx_queue = je.from_websocket_tx;
        let mut client_rx = je.to_websocket_rx;

        // The initial web request for the Client framework produces a
        // `CurrentFile`.
        let (id, (..)) = get_message_as!(
            client_rx,
            EditorMessageContents::CurrentFile,
            file_name,
            is_text
        );
        assert_eq!(id, 0.0);
        send_response(&ide_tx_queue, 0.0, Ok(ResultOkTypes::Void)).await;

        // The follow-up web request for the file produces an `Update`.
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
        let (id, _) = get_message_as!(client_rx, EditorMessageContents::Update);
        assert_eq!(id, 1.0);
        send_response(&ide_tx_queue, 1.0, Ok(ResultOkTypes::Void)).await;

        // 1.  Send an update message with no contents.
        ide_tx_queue
            .send(EditorMessage {
                id: 0.0,
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
            get_message_as!(client_rx, EditorMessageContents::Result),
            (0.0, Ok(ResultOkTypes::Void))
        );

        // 2.  Send invalid messages.
        for (id, msg) in [
            (1.0, EditorMessageContents::Opened(IdeType::VSCode(true))),
            (2.0, EditorMessageContents::ClientHtml("".to_string())),
            (3.0, EditorMessageContents::RequestClose),
        ] {
            ide_tx_queue
                .send(EditorMessage { id, message: msg })
                .await
                .unwrap();
            let (id_rx, msg_rx) = get_message_as!(client_rx, EditorMessageContents::Result);
            assert_eq!(id, id_rx);
            assert_starts_with!(cast!(&msg_rx, Err), "Client sent unsupported message type");
        }

        // 3.  Send an update message with no path.
        ide_tx_queue
            .send(EditorMessage {
                id: 4.0,
                message: EditorMessageContents::Update(UpdateMessageContents {
                    file_path: "".to_string(),
                    contents: Some(CodeChatForWeb {
                        metadata: SourceFileMetadata {
                            mode: "".to_string(),
                        },
                        source: CodeMirror {
                            doc: "".to_string(),
                            doc_blocks: vec![],
                        },
                    }),
                    cursor_position: None,
                    scroll_position: None,
                }),
            })
            .await
            .unwrap();

        // Check that it produces an error.
        let (id, err_msg) = get_message_as!(client_rx, EditorMessageContents::Result);
        assert_eq!(id, 4.0);
        assert_starts_with!(
            cast!(err_msg, Err),
            "Update for file '' doesn't match current file"
        );

        // 4.  Send an update message with unknown source language.
        ide_tx_queue
            .send(EditorMessage {
                id: 5.0,
                message: EditorMessageContents::Update(UpdateMessageContents {
                    file_path: file_path.clone(),
                    contents: Some(CodeChatForWeb {
                        metadata: SourceFileMetadata {
                            mode: "nope".to_string(),
                        },
                        source: CodeMirror {
                            doc: "testing".to_string(),
                            doc_blocks: vec![],
                        },
                    }),
                    cursor_position: None,
                    scroll_position: None,
                }),
            })
            .await
            .unwrap();

        // Check that it produces an error.
        assert_eq!(
            get_message_as!(client_rx, EditorMessageContents::Result),
            (
                5.0,
                Err("Unable to translate to source: Invalid mode".to_string())
            )
        );

        // 5.  Send a valid message.
        ide_tx_queue
            .send(EditorMessage {
                id: 6.0,
                message: EditorMessageContents::Update(UpdateMessageContents {
                    file_path: file_path.clone(),
                    contents: Some(CodeChatForWeb {
                        metadata: SourceFileMetadata {
                            mode: "python".to_string(),
                        },
                        source: CodeMirror {
                            doc: "testing()".to_string(),
                            doc_blocks: vec![],
                        },
                    }),
                    cursor_position: None,
                    scroll_position: None,
                }),
            })
            .await
            .unwrap();
        assert_eq!(
            get_message_as!(client_rx, EditorMessageContents::Result),
            (6.0, Ok(ResultOkTypes::Void))
        );

        // Check that the requested file is written.
        let mut s = fs::read_to_string(&file_path).unwrap();
        assert_eq!(s, "testing()");
        // Wait for the filewatcher to debounce this file write.
        sleep(Duration::from_secs(1)).await;

        // 6.  Change this file and verify that this produces an update.
        s.push_str("123");
        fs::write(&file_path, s).unwrap();
        assert_eq!(
            get_message_as!(client_rx, EditorMessageContents::Update),
            (
                2.0,
                UpdateMessageContents {
                    file_path: file_path.clone(),
                    contents: Some(CodeChatForWeb {
                        metadata: SourceFileMetadata {
                            mode: "python".to_string(),
                        },
                        source: CodeMirror {
                            doc: "testing()123".to_string(),
                            doc_blocks: vec![],
                        },
                    }),
                    cursor_position: None,
                    scroll_position: None,
                }
            )
        );
        // Acknowledge this message.
        send_response(&ide_tx_queue, 2.0, Ok(ResultOkTypes::Void)).await;

        // 7.  Rename it and check for an close (the file watcher can't detect
        //     the destination file, so it's treated as the file is deleted).
        let mut dest = PathBuf::from(&file_path).parent().unwrap().to_path_buf();
        dest.push("test2.py");
        fs::rename(file_path, dest.as_path()).unwrap();
        assert_eq!(
            client_rx.recv().await.unwrap(),
            EditorMessage {
                id: 3.0,
                message: EditorMessageContents::Closed
            }
        );
        send_response(&ide_tx_queue, 3.0, Ok(ResultOkTypes::Void)).await;

        // 8.  Load another file from the Client.
        let mut new_file_path = test_dir.clone();
        new_file_path.push("test1.py");
        let new_uri = format!(
            "http://localhost/fw/fsc/1/{}",
            drop_leading_slash(&urlencoding::encode(&new_file_path.to_slash().unwrap()))
        );
        ide_tx_queue
            .send(EditorMessage {
                id: 7.0,
                message: EditorMessageContents::CurrentFile(new_uri.clone(), None),
            })
            .await
            .unwrap();
        assert_eq!(
            get_message_as!(client_rx, EditorMessageContents::Result),
            (7.0, Ok(ResultOkTypes::Void))
        );

        // The follow-up web request for the file produces an `Update`.
        let new_req = test::TestRequest::get().uri(&new_uri).to_request();
        let new_resp = test::call_service(&app, new_req).await;
        assert!(new_resp.status().is_success());
        let (id, _) = get_message_as!(client_rx, EditorMessageContents::Update);
        assert_eq!(id, 4.0);
        send_response(&ide_tx_queue, 4.0, Ok(ResultOkTypes::Void)).await;

        // 9.  Writes to this file should produce an update.
        fs::write(&new_file_path, "testing 1").unwrap();
        get_message_as!(client_rx, EditorMessageContents::Update);

        // Each of the three invalid message types produces one error.
        check_logger_errors(3);
        // Report any errors produced when removing the temporary directory.
        temp_dir.close().unwrap();
    }
}
