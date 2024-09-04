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
use std::{
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

// ### Third-party
use actix_files;
use actix_web::{
    error::Error,
    get,
    http::header::{self, ContentDisposition},
    web, HttpRequest, HttpResponse,
};
use async_trait::async_trait;
use log::{error, info, warn};
use notify_debouncer_full::{
    new_debouncer,
    notify::{EventKind, RecursiveMode, Watcher},
    DebounceEventResult,
};
use tokio::{
    fs::{self, File},
    io::AsyncReadExt,
    select,
    sync::mpsc,
};

// ### Local
use super::{
    client_websocket, html_not_found, path_display, send_response, serve_file, AppState,
    EditorMessage, EditorMessageContents, ProcessingTask, UpdateMessageContents, WebsocketQueues,
};
use crate::processing::TranslationResultsString;
use crate::processing::{
    codechat_for_web_to_source, source_to_codechat_for_web_string, CodeChatForWeb,
};
use crate::queue_send;

// ### Serve file
/// This could be a plain text file (for example, one not recognized as source
/// code that this program supports), a binary file (image/video/etc.), a
/// CodeChat Editor file, or a non-existent file. Determine which type this file
/// is then serve it. Serve a CodeChat Editor Client webpage using the
/// FileWatcher "IDE".
pub async fn serve_filewatcher(
    file_path: &Path,
    req: &HttpRequest,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let file_contents = match smart_read(file_path, req).await {
        Ok(fc) => fc,
        Err(err) => return err,
    };

    serve_file(
        file_path,
        &file_contents,
        "fw/ws",
        req,
        app_state,
        FilewatcherTask,
    )
    .await
}

/// Smart file reader. This returns an HTTP response if the provided file should
/// be served directly (including an error if necessary), or a string containing
/// the text of the file when it's Unicode.
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

/// This is an awkward workaround to create an async function pointer. See
/// `serve_file`.
struct FilewatcherTask;

#[async_trait]
impl ProcessingTask for FilewatcherTask {
    async fn processing_task(
        &self,
        file_path: &Path,
        app_state: web::Data<AppState>,
        codechat_for_web: CodeChatForWeb,
    ) -> u32 {
        // #### Filewatcher IDE
        //
        // This is a CodeChat Editor file. Start up the Filewatcher IDE tasks:
        //
        // 1.  A task to watch for changes to the file, notifying the CodeChat
        //     Editor Client when the file should be reloaded.
        // 2.  A task to receive and respond to messages from the CodeChat
        //     Editor Client.
        //
        // First, allocate variables needed by these two tasks.
        //
        // The path to the currently open CodeChat Editor file.
        let mut current_filepath = file_path.to_path_buf();
        // Access this way, to avoid borrow checker problems.
        let connection_id = {
            let mut connection_id = app_state.connection_id.lock().unwrap();
            *connection_id += 1;
            *connection_id
        };
        // #### The filewatcher task.
        actix_rt::spawn(async move {
            'task: {
                // Use a channel to send from the watcher (which runs in another
                // thread) into this async (task) context.
                let (watcher_tx, mut watcher_rx) = mpsc::channel(10);
                // Watch this file. Use the debouncer, to avoid multiple
                // notifications for the same file. This approach returns a
                // result of either a working debouncer or any errors that
                // occurred. The debouncer's scope needs live as long as this
                // connection does; dropping it early means losing file change
                // notifications.
                let Ok(mut debounced_watcher) = new_debouncer(
                    Duration::from_secs(2),
                    None,
                    // Note that this runs in a separate thread created by the
                    // watcher, not in an async context. Therefore, use a
                    // blocking send.
                    move |result: DebounceEventResult| {
                        if let Err(err) = watcher_tx.blocking_send(result) {
                            // Note: we can't break here, since this runs in a
                            // separate thread. We have no way to shut down the
                            // task (which would be the best action to take.)
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
                queue_send!(to_websocket_tx.send(EditorMessage {
                    id: 0,
                    message: EditorMessageContents::Update(UpdateMessageContents {
                        contents: Some(codechat_for_web.clone()),
                        cursor_position: Some(0),
                        path: Some(current_filepath.to_path_buf()),
                        scroll_position: Some(0.0),
                    }),
                }), 'task);

                // Process results produced by the file watcher.
                loop {
                    select! {
                        Some(result) = watcher_rx.recv() => {
                            match result {
                                Err(err_vec) => {
                                    for err in err_vec {
                                        // Report errors locally and to the
                                        // CodeChat Editor.
                                        let msg = format!("Watcher error: {err}");
                                        error!("{msg}");
                                        // Send using ID 0 to indicate this isn't a
                                        // response to a message received from the
                                        // client.
                                        send_response(&to_websocket_tx, 0, &msg).await;
                                    }
                                }

                                Ok(debounced_event_vec) => {
                                    for debounced_event in debounced_event_vec {
                                        match debounced_event.event.kind {
                                            EventKind::Modify(_modify_kind) => {
                                                // On Windows, the `_modify_kind` is `Any`;
                                                // therefore; ignore it rather than trying
                                                // to look at only content modifications.
                                                // As long as the parent of both files is
                                                // identical, we can update the contents.
                                                // Otherwise, we need to load in the new
                                                // URL.
                                                if debounced_event.event.paths.len() == 1 && debounced_event.event.paths[0].parent() == current_filepath.parent() {
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
                                                    source_to_codechat_for_web_string(&file_contents, &current_filepath, false, &app_state.lexers);
                                                    if let TranslationResultsString::CodeChat(cc) = translation_results_string {
                                                        // Send the new contents
                                                        queue_send!(to_websocket_tx.send(EditorMessage {
                                                                id: 0,
                                                                message: EditorMessageContents::Update(UpdateMessageContents {
                                                                    contents: Some(cc),
                                                                    cursor_position: None,
                                                                    path: Some(debounced_event.event.paths[0].to_path_buf()),
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
                                                    warn!("TODO: Modification to different parent.")
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

                        Some(m) = from_websocket_rx.recv() => {
                            match m.message {
                                EditorMessageContents::Update(update_message_contents) => {
                                    let result = 'process: {
                                        // With code or a path, there's nothing to
                                        // save. TODO: this should store and
                                        // remember the path, instead of needing it
                                        // repeated each time.
                                        let codechat_for_web1 = match update_message_contents.contents {
                                            None => break 'process "".to_string(),
                                            Some(cwf) => cwf,
                                        };
                                        if update_message_contents.path.is_none() {
                                            break 'process "".to_string();
                                        }

                                        // Translate from the CodeChatForWeb format
                                        // to the contents of a source file.
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

                                        if let Err(err) = debounced_watcher.watcher().unwatch(&current_filepath) {
                                            let msg = format!(
                                                "Unable to unwatch file '{}': {err}.",
                                                current_filepath.to_string_lossy()
                                            );
                                            break 'process msg;
                                        }
                                        // Save this string to a file. Add a
                                        // leading slash for Linux/OS X: this
                                        // changes from `foo/bar.c` to
                                        // `/foo/bar.c`. Windows paths already
                                        // start with a drive letter, such as
                                        // `C:\foo\bar.c`, so no changes are
                                        // needed.
                                        current_filepath = if cfg!(windows) {
                                            PathBuf::from_str("")
                                        } else {
                                            PathBuf::from_str("/")
                                        }
                                        .unwrap();
                                        current_filepath.push(update_message_contents.path.unwrap());
                                        if let Err(err) = fs::write(current_filepath.as_path(), file_contents).await {
                                            let msg = format!(
                                                "Unable to save file '{}': {err}.",
                                                current_filepath.to_string_lossy()
                                            );
                                            break 'process msg;
                                        }
                                        if let Err(err) = debounced_watcher.watcher().watch(&current_filepath, RecursiveMode::NonRecursive) {
                                            let msg = format!(
                                                "Unable to watch file '{}': {err}.",
                                                current_filepath.to_string_lossy()
                                            );
                                            break 'process msg;
                                        }
                                        current_filepath = current_filepath.into();
                                        "".to_string()
                                    };
                                    send_response(&to_websocket_tx, m.id, &result).await;
                                }

                                // Process a result, the respond to a message we
                                // sent.
                                EditorMessageContents::Result(err) => {
                                    // Report errors to the log.
                                    if !err.is_empty() {
                                        error!("Error in message {}: {err}.", m.id);
                                    }
                                }

                                EditorMessageContents::Closed => {
                                    info!("Filewatcher closing");
                                    break;
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

        connection_id
    }
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
    use std::fs;
    use std::path::PathBuf;
    use std::time::Duration;

    use actix_web::{test, web, App};
    use assertables::{assert_starts_with, assert_starts_with_as_result};
    use tokio::select;
    use tokio::sync::mpsc::{Receiver, Sender};
    use tokio::time::sleep;

    use super::super::{configure_app, make_app_data, WebsocketQueues};
    use super::{AppState, EditorMessage, EditorMessageContents, UpdateMessageContents};
    use crate::lexer::{compile_lexers, supported_languages::get_language_lexer_vec};
    use crate::processing::{
        source_to_codechat_for_web, CodeChatForWeb, CodeMirror, SourceFileMetadata,
        TranslationResults,
    };
    use crate::test_utils::{check_logger_errors, configure_testing_logger};
    use crate::{cast, prep_test_dir};

    async fn get_websocket_queues(
        // A path to the temporary directory where the source file is located.
        test_dir: &PathBuf,
    ) -> WebsocketQueues {
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

    #[actix_web::test]
    async fn test_websocket_opened_1() {
        configure_testing_logger();
        let (temp_dir, test_dir) = prep_test_dir!();
        let je = get_websocket_queues(&test_dir).await;
        let ide_tx_queue = je.from_websocket_tx;
        let mut client_rx = je.to_websocket_rx;

        // 2.  We should get the initial contents.
        let umc = get_message_as!(client_rx, EditorMessageContents::Update);
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
        check_logger_errors();
        temp_dir.close().unwrap();
    }

    #[actix_web::test]
    async fn test_websocket_update_1() {
        configure_testing_logger();
        let (temp_dir, test_dir) = prep_test_dir!();
        let je = get_websocket_queues(&test_dir).await;
        let ide_tx_queue = je.from_websocket_tx;
        let mut client_rx = je.to_websocket_rx;
        // Configure the logger here; otherwise, the glob used to copy files
        // outputs some debug-level logs.

        // We should get the initial contents.
        get_message_as!(client_rx, EditorMessageContents::Update);
        send_response(1, &ide_tx_queue, "").await;

        // 1.  Send an update message with no contents.
        ide_tx_queue
            .send(EditorMessage {
                id: 0,
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
        assert_eq!(
            get_message_as!(client_rx, EditorMessageContents::Result),
            ""
        );

        // 2.  Send an update message with no path.
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
        assert_eq!(
            get_message_as!(client_rx, EditorMessageContents::Result),
            ""
        );

        // 3.  Send an update message with unknown source language.
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
                    path: Some(PathBuf::new()),
                    cursor_position: None,
                    scroll_position: None,
                }),
            })
            .await
            .unwrap();

        // Check that it produces an error.
        assert_eq!(
            get_message_as!(client_rx, EditorMessageContents::Result),
            "Unable to translate to source: Invalid mode"
        );

        // 4.  Send an update message with an invalid path.
        ide_tx_queue
            .send(EditorMessage {
                id: 0,
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
            get_message_as!(client_rx, EditorMessageContents::Result),
            "Unable to save file '':"
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
                    path: Some(file_path.clone()),
                    cursor_position: None,
                    scroll_position: None,
                }),
            })
            .await
            .unwrap();
        assert_eq!(
            get_message_as!(client_rx, EditorMessageContents::Result),
            ""
        );

        // Check that the requested file is written.
        let mut s = fs::read_to_string(&file_path).unwrap();
        assert_eq!(s, "testing()");
        // Wait for the filewatcher to debounce this file write.
        sleep(Duration::from_secs(1)).await;

        // Change this file and verify that this produces an update.
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
                path: Some(file_path.clone()),
                cursor_position: None,
                scroll_position: None,
            }
        );
        // Acknowledge this message.
        send_response(3, &ide_tx_queue, "").await;

        // Rename it and check for an close (the file watcher can't detect the
        // destination file, so it's treated as the file is deleted).
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

        // Report any errors produced when removing the temporary directory.
        check_logger_errors();
        temp_dir.close().unwrap();
    }
}
