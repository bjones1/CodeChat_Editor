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
/// # `filewatcher.rs` -- Implement the File Watcher "IDE"
///
/// ## Imports
///
/// ### Standard library
use std::{path::Path, time::Duration};

// ### Third-party
use actix_web::{
    error::Error,
    get,
    http::header::{self, ContentType},
    web, HttpRequest, HttpResponse, Responder,
};
use dunce::simplified;
use lazy_static::lazy_static;
use log::{error, info, warn};
use notify_debouncer_full::{
    new_debouncer,
    notify::{EventKind, RecursiveMode, Watcher},
    DebounceEventResult,
};
use path_slash::PathExt;
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
    client_websocket, get_client_framework, get_connection_id, html_not_found, html_wrapper,
    path_display, send_response, serve_file, AppState, EditorMessage, EditorMessageContents,
    SimpleHttpResponse, UpdateMessageContents, WebsocketQueues,
};
use crate::{
    oneshot_send,
    processing::{
        codechat_for_web_to_source, source_to_codechat_for_web_string, TranslationResultsString,
    },
    queue_send,
    webserver::{filesystem_endpoint, url_to_path},
};

// ## Globals
lazy_static! {
    /// Matches a bare drive letter. Only needed on Windows.
    static ref DRIVE_LETTER_REGEX: Regex = Regex::new("^[a-zA-Z]:$").unwrap();
}

/// ## File browser endpoints
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
        // Get an ID for this connection.
        let connection_id = get_connection_id(&app_state);
        actix_rt::spawn(async move {
            processing_task(&canon_path, app_state, connection_id).await;
        });
        return get_client_framework(&req, "fw/ws", connection_id);
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
                return html_not_found(&format!("<p>Unable to decode file name {err:?} as UTF-8.",))
            }
        };
        let encoded_file = urlencoding::encode(&file_name);
        file_html += &format!(
            r#"<li><a href="/fw/fsb/{web_path}/{encoded_file}" target="_blank">{file_name}</a></li>
"#
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

/// `fsc` stands for "FileSystem Client", and provides the Client contents from
/// the filesystem.
#[get("/fw/fsc/{connection_id}/{file_path:.*}")]
async fn filewatcher_client_endpoint(
    path: web::Path<(String, String)>,
    req: HttpRequest,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    filesystem_endpoint(path, &req, &app_state).await
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
    let mut current_filepath = file_path.to_path_buf().canonicalize().unwrap();
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
            if let Err(err) = debounced_watcher
                .watcher()
                .watch(&current_filepath, RecursiveMode::NonRecursive)
            {
                error!("Unable to watch file: {err}");
                break 'task;
            };

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
            let encoded_path =
                // First, convert the path to use forward slashes.
                &simplified(&current_filepath).to_slash().unwrap()
                // The convert each part of the path to a URL-encoded string.
                // (This avoids encoding the slashes.)
                .split("/").map(|s| urlencoding::encode(s))
                // Then put it all back together.
                .collect::<Vec<_>>().join("/");
            let url_pathbuf = format!("/fw/fsc/{connection_id}/{encoded_path}");
            queue_send!(to_websocket_tx.send(EditorMessage {
                id: 0,
                message: EditorMessageContents::CurrentFile(url_pathbuf)
            }), 'task);

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
                                    send_response(&to_websocket_tx, 0, Some(msg)).await;
                                }
                            }

                            Ok(debounced_event_vec) => {
                                for debounced_event in debounced_event_vec {
                                    match debounced_event.event.kind {
                                        EventKind::Modify(_modify_kind) => {
                                            // On Windows, the `_modify_kind` is `Any`;
                                            // therefore; ignore it rather than trying
                                            // to look at only content modifications.
                                            if debounced_event.event.paths.len() == 1 && debounced_event.event.paths[0] == current_filepath {
                                                // Since the parents are identical, send an
                                                // update. First, read the modified file.
                                                let mut file_contents = String::new();
                                                let read_ret = match File::open(&current_filepath).await {
                                                    Ok(fc) => fc,
                                                    Err(_err) => {
                                                        // We can't open the file -- it's been
                                                        // moved or deleted. Close the file.
                                                        queue_send!(to_websocket_tx.send(EditorMessage {
                                                            id: 0,
                                                            message: EditorMessageContents::Closed
                                                        }));
                                                        continue;
                                                    }
                                                }
                                                .read_to_string(&mut file_contents)
                                                .await;

                                                // Close the file if it can't be read as
                                                // Unicode text.
                                                if read_ret.is_err() {
                                                    queue_send!(to_websocket_tx.send(EditorMessage {
                                                        id: 0,
                                                        message: EditorMessageContents::Closed
                                                    }));
                                                }

                                                // Translate the file.
                                                let (translation_results_string, _path_to_toc) =
                                                source_to_codechat_for_web_string(&file_contents, &current_filepath, false);
                                                if let TranslationResultsString::CodeChat(cc) = translation_results_string {
                                                    // Send the new contents
                                                    queue_send!(to_websocket_tx.send(EditorMessage {
                                                            id: 0,
                                                            message: EditorMessageContents::Update(UpdateMessageContents {
                                                                contents: Some(cc),
                                                                cursor_position: None,
                                                                scroll_position: None,
                                                            }),
                                                        }));

                                                } else {
                                                    // Close the file -- it's not CodeChat
                                                    // anymore.
                                                    queue_send!(to_websocket_tx.send(EditorMessage {
                                                        id: 0,
                                                        message: EditorMessageContents::Closed
                                                    }));
                                                }

                                            } else {
                                                warn!("TODO: Modification to different file.")
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

                    Some(http_request) = from_http_rx.recv() => {
                        // Convert the provided URL back into a file name.
                        let file_path = Path::new(&http_request.request_path);

                        // Read the file
                        let simple_http_response = match File::open(file_path).await {
                            Err(err) => SimpleHttpResponse::Err(format!(
                                "<p>Error opening file {file_path:?}: {err}."
                            )),
                            Ok(mut fc) => {
                                let mut file_contents = String::new();
                                match fc.read_to_string(&mut file_contents).await {
                                    // If this is a binary file (meaning we can't read the contents as UTF-8),
                                    // just serve it raw; assume this is an image/video/etc.
                                    Err(_) => SimpleHttpResponse::Bin(http_request.request_path),
                                    Ok(_) => {
                                        let is_current = file_path.canonicalize().unwrap() == current_filepath;
                                        let (simple_http_response, option_codechat_for_web) = serve_file(file_path, &file_contents, http_request.is_toc, is_current, http_request.is_test_mode).await;
                                        // If this file is editable and is the main
                                        // file, send an `Update`. The
                                        // `simple_http_response` contains the Client.
                                        if let Some(codechat_for_web) = option_codechat_for_web {
                                            queue_send!(to_websocket_tx.send(EditorMessage {
                                                id: 0,
                                                message: EditorMessageContents::Update(UpdateMessageContents {
                                                    contents: Some(codechat_for_web),
                                                    cursor_position: None,
                                                    scroll_position: None
                                                })
                                            }));
                                        }
                                        simple_http_response
                                    },
                                }
                            },
                        };

                        oneshot_send!(http_request.response_queue.send(simple_http_response));
                    }

                    Some(m) = from_websocket_rx.recv() => {
                        match m.message {
                            EditorMessageContents::Update(update_message_contents) => {
                                let result = 'process: {
                                    // With code or a path, there's nothing to
                                    // save. TODO: this should store and
                                    // remember the path, instead of needing it
                                    // repeated each time.
                                    let codechat_for_web = match update_message_contents.contents {
                                        None => break 'process None,
                                        Some(cwf) => cwf,
                                    };

                                    // Translate from the CodeChatForWeb format
                                    // to the contents of a source file.
                                    let file_contents = match codechat_for_web_to_source(
                                        codechat_for_web,
                                    ) {
                                        Ok(r) => r,
                                        Err(message) => {
                                            break 'process Some(format!(
                                                "Unable to translate to source: {message}"
                                            ));
                                        }
                                    };

                                    if let Err(err) = debounced_watcher.watcher().unwatch(&current_filepath) {
                                        let msg = format!(
                                            "Unable to unwatch file '{}': {err}.",
                                            current_filepath.to_string_lossy()
                                        );
                                        break 'process Some(msg);
                                    }
                                    // Save this string to a file.
                                    if let Err(err) = fs::write(current_filepath.as_path(), file_contents).await {
                                        let msg = format!(
                                            "Unable to save file '{}': {err}.",
                                            current_filepath.to_string_lossy()
                                        );
                                        break 'process Some(msg);
                                    }
                                    if let Err(err) = debounced_watcher.watcher().watch(&current_filepath, RecursiveMode::NonRecursive) {
                                        let msg = format!(
                                            "Unable to watch file '{}': {err}.",
                                            current_filepath.to_string_lossy()
                                        );
                                        break 'process Some(msg);
                                    }
                                    None
                                };
                                send_response(&to_websocket_tx, m.id, result).await;
                            }

                            EditorMessageContents::CurrentFile(url_string) => {
                                let result = match url_to_path(url_string) {
                                    Err(err) => Some(err),
                                    Ok(file_path) => 'err_exit: {
                                        // We finally have the desired path! First,
                                        // unwatch the old path.
                                        if let Err(err) = debounced_watcher.watcher().unwatch(&current_filepath) {
                                            break 'err_exit Some(format!(
                                                "Unable to unwatch file '{}': {err}.",
                                                current_filepath.to_string_lossy()
                                            ));
                                        }
                                        // Update to the new path.
                                        current_filepath = file_path;
                                        // Watch the new file.
                                        if let Err(err) = debounced_watcher.watcher().watch(&current_filepath, RecursiveMode::NonRecursive) {
                                            break 'err_exit Some(format!(
                                                "Unable to watch file '{}': {err}.",
                                                current_filepath.to_string_lossy()
                                            ));
                                        }

                                        // Indicate there was no error in the
                                        // `Result` message.
                                        None
                                    }
                                };
                                send_response(&to_websocket_tx, m.id, result).await;
                            },

                            // Process a result, the respond to a message we
                            // sent.
                            EditorMessageContents::Result(err, _) => {
                                // Report errors to the log.
                                if let Some(err_msg) = err {
                                    error!("Error in message {}: {err_msg}.", m.id);
                                }
                            }

                            EditorMessageContents::Closed => {
                                info!("Filewatcher closing");
                                break;
                            }

                            EditorMessageContents::Opened(_) | EditorMessageContents::ClientHtml(_) | EditorMessageContents::RequestClose => {
                                let msg = format!("Client sent unsupported message type {m:?}");
                                error!("{msg}");
                                send_response(&to_websocket_tx, m.id, Some(msg)).await;
                            }

                            other => {
                                warn!("Unhandled message {other:?}");
                            }
                        }
                    }

                    else => break
                }
            }

            from_websocket_rx.close();
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

// ## Tests
#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, str::FromStr, time::Duration};

    use actix_http::Request;
    use actix_web::{
        body::BoxBody,
        dev::{Service, ServiceResponse},
        test, web, App,
    };
    use assertables::{assert_starts_with, assert_starts_with_as_result};
    use path_slash::PathExt;
    use tokio::{select, sync::mpsc::Receiver, time::sleep};
    use url::Url;

    use super::{
        super::{configure_app, make_app_data, WebsocketQueues},
        send_response, AppState, EditorMessage, EditorMessageContents, UpdateMessageContents,
    };
    use crate::{
        processing::{
            source_to_codechat_for_web, CodeChatForWeb, CodeMirror, SourceFileMetadata,
            TranslationResults,
        },
        test_utils::{check_logger_errors, configure_testing_logger},
        webserver::IdeType,
        {cast, cast2, prep_test_dir},
    };

    async fn get_websocket_queues(
        // A path to the temporary directory where the source file is located.
        test_dir: &PathBuf,
    ) -> (
        WebsocketQueues,
        impl Service<Request, Response = ServiceResponse<BoxBody>, Error = actix_web::Error>,
    ) {
        let app_data = make_app_data();
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
        return (
            joint_editors.remove(&connection_id.to_string()).unwrap(),
            app,
        );
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

    macro_rules! get_message_as {
        ($client_rx: expr, $cast_type: ty) => {
            cast!(get_message(&mut $client_rx).await, $cast_type)
        };
    }

    macro_rules! get_message_as2 {
        ($client_rx: expr, $cast_type: ty) => {
            cast2!(get_message(&mut $client_rx).await, $cast_type)
        };
    }

    #[actix_web::test]
    async fn test_websocket_opened_1() {
        configure_testing_logger();
        let (temp_dir, test_dir) = prep_test_dir!();
        let (je, app) = get_websocket_queues(&test_dir).await;
        let ide_tx_queue = je.from_websocket_tx;
        let mut client_rx = je.to_websocket_rx;

        // The initial web request for the Client framework produces a `CurrentFile`.
        let url_string = get_message_as!(client_rx, EditorMessageContents::CurrentFile);
        send_response(&ide_tx_queue, 0, None).await;

        // Check the path this message contains.
        let mut test_path = test_dir.clone();
        test_path.push("test.py");
        // The comparison below fails without this.
        let test_path = test_path.canonicalize().unwrap();
        let url = Url::parse(&format!(
            "http://localhost{}",
            urlencoding::decode(&url_string).unwrap()
        ))
        .unwrap();
        let url_segs: Vec<_> = url.path_segments().unwrap().collect();
        let url_path = PathBuf::from_str(&url_segs[3..].join("/"))
            .unwrap()
            .canonicalize()
            .unwrap();
        assert_eq!(url_path, test_path);

        // 2. After fetching the file, we should get an update.
        let uri = format!("/fw/fsc/1/{}/test.py", test_dir.to_string_lossy());
        let req = test::TestRequest::get().uri(&uri).to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let umc = get_message_as!(client_rx, EditorMessageContents::Update);
        send_response(&ide_tx_queue, 0, None).await;

        // Check the contents.
        let translation_results = source_to_codechat_for_web(&"".to_string(), "py", false, false);
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

        // The initial web request for the Client framework produces a `CurrentFile`.
        get_message_as!(client_rx, EditorMessageContents::CurrentFile);
        send_response(&ide_tx_queue, 0, None).await;

        // The follow-up web request for the file produces an `Update`.
        let uri = format!("/fw/fsc/1/{}/test.py", test_dir.to_string_lossy());
        let req = test::TestRequest::get().uri(&uri).to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        get_message_as!(client_rx, EditorMessageContents::Update);
        send_response(&ide_tx_queue, 0, None).await;

        // 1.  Send an update message with no contents.
        ide_tx_queue
            .send(EditorMessage {
                id: 0,
                message: EditorMessageContents::Update(UpdateMessageContents {
                    contents: None,
                    cursor_position: None,
                    scroll_position: None,
                }),
            })
            .await
            .unwrap();

        // Check that it produces no error.
        assert_eq!(
            get_message_as2!(client_rx, EditorMessageContents::Result),
            (None, None)
        );

        // 2.  Send invalid messages.
        for msg in [
            EditorMessageContents::Opened(IdeType::VSCode(true)),
            EditorMessageContents::ClientHtml("".to_string()),
            EditorMessageContents::RequestClose,
        ] {
            ide_tx_queue
                .send(EditorMessage {
                    id: 0,
                    message: msg,
                })
                .await
                .unwrap();
            assert_starts_with!(
                cast!(
                    get_message_as2!(client_rx, EditorMessageContents::Result).0,
                    Option::Some
                ),
                "Client sent unsupported message type"
            );
        }

        // 3.  Send an update message with no path.
        ide_tx_queue
            .send(EditorMessage {
                id: 0,
                message: EditorMessageContents::Update(UpdateMessageContents {
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

        // Check that it produces no error.
        assert_eq!(
            get_message_as2!(client_rx, EditorMessageContents::Result),
            (None, None)
        );

        // 4.  Send an update message with unknown source language.
        ide_tx_queue
            .send(EditorMessage {
                id: 0,
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
                    cursor_position: None,
                    scroll_position: None,
                }),
            })
            .await
            .unwrap();

        // Check that it produces an error.
        assert_eq!(
            get_message_as2!(client_rx, EditorMessageContents::Result),
            (
                Some("Unable to translate to source: Invalid mode".to_string()),
                None
            )
        );

        // 5.  Send a valid message.
        let mut file_path = test_dir.clone();
        file_path.push("test.py");
        ide_tx_queue
            .send(EditorMessage {
                id: 0,
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
                    cursor_position: None,
                    scroll_position: None,
                }),
            })
            .await
            .unwrap();
        assert_eq!(
            get_message_as2!(client_rx, EditorMessageContents::Result),
            (None, None)
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
                cursor_position: None,
                scroll_position: None,
            }
        );
        // Acknowledge this message.
        send_response(&ide_tx_queue, 0, None).await;

        // 7.  Rename it and check for an close (the file watcher can't detect
        //     the destination file, so it's treated as the file is deleted).
        let mut dest = file_path.clone().parent().unwrap().to_path_buf();
        dest.push("test2.py");
        fs::rename(file_path, dest.as_path()).unwrap();
        assert_eq!(
            client_rx.recv().await.unwrap(),
            EditorMessage {
                id: 0,
                message: EditorMessageContents::Closed
            }
        );

        // 8. Load another file from the Client.
        let mut new_file_path = test_dir.clone();
        new_file_path.push("test1.py");
        let new_uri = format!(
            "http://localhost/fw/fsc/1/{}",
            urlencoding::encode(&new_file_path.to_slash().unwrap())
        );
        ide_tx_queue
            .send(EditorMessage {
                id: 0,
                message: EditorMessageContents::CurrentFile(new_uri.clone()),
            })
            .await
            .unwrap();
        assert_eq!(
            get_message_as2!(client_rx, EditorMessageContents::Result),
            (None, None)
        );

        // The follow-up web request for the file produces an `Update`.
        let new_req = test::TestRequest::get().uri(&new_uri).to_request();
        let new_resp = test::call_service(&app, new_req).await;
        assert!(new_resp.status().is_success());
        get_message_as!(client_rx, EditorMessageContents::Update);
        send_response(&ide_tx_queue, 0, None).await;

        // 9. Writes to this file should produce an update.
        fs::write(&new_file_path, "testing 1").unwrap();
        get_message_as!(client_rx, EditorMessageContents::Update);

        // Each of the three invalid message types produces one error.
        check_logger_errors(3);
        // Report any errors produced when removing the temporary directory.
        temp_dir.close().unwrap();
    }
}
