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
/// `overall_core/mod.rs` - test the overall system
/// ===============================================
///
/// These are functional tests of the overall system, performed by attaching a
/// testing IDE to generate commands then observe results, along with a browser
/// tester.
///
/// Some subtleties of this approach: development dependencies aren't available
/// to integration tests. Therefore, this crate's `Cargo.toml` file includes the
/// `int_tests` feature, which enables crates needed only for integration
/// testing, while keeping these out of the final binary when compiling for
/// production. This means that the same crate appears both in
/// `dev-dependencies` and in `dependencies`, so it's available for both unit
/// tests and integration tests. In addition, any code used in integration tests
/// must be gated on the `int_tests` feature, since this code fails to compile
/// without that feature's crates enabled. Tests are implemented here, then
/// `use`d in `overall.rs`, so that a single `#[cfg(feature = "int_tests")]`
/// statement there gates everything in this file. See the [test
/// docs](https://doc.rust-lang.org/book/ch11-03-test-organization.html#submodules-in-integration-tests)
/// for the correct file and directory names.
///
/// A second challenge revolves around the lack of an async `Drop` trait: the
/// web driver server should be started before any test, left running during all
/// tests, then terminated as the test program exits. The web driver must be
/// initialized before a test then stopped at the end of that test. Both are
/// ideal for this missing Drop trait. As a workaround:
///
/// *   The web driver server relies on the C `atexit` call to stop the server.
///     However, when tests fail, this doesn't get called, leaving the server
///     running. This causes the server to fail to start on the next test run,
///     since it's still running. Therefore, errors when starting the web driver
///     server are ignored by design.
/// *   Tests are run in an async block, and any panics produced inside it are
///     caught using `catch_unwind()`. The driver is shut down before returning
///     an error due to the panic.
// Imports
// -------
//
// ### Standard library
use std::{
    collections::HashMap, env, error::Error, panic::AssertUnwindSafe, path::PathBuf, time::Duration,
};

// ### Third-party
use assert_fs::TempDir;
use dunce::canonicalize;
use futures::FutureExt;
use indoc::indoc;
use pretty_assertions::assert_eq;
use thirtyfour::{
    By, ChromiumLikeCapabilities, DesiredCapabilities, Key, WebDriver, error::WebDriverError,
    start_webdriver_process,
};
use tokio::time::sleep;

// ### Local
use code_chat_editor::{
    cast,
    ide::CodeChatEditorServer,
    lexer::supported_languages::MARKDOWN_MODE,
    prep_test_dir,
    processing::{
        CodeChatForWeb, CodeMirrorDiff, CodeMirrorDiffable, SourceFileMetadata, StringDiff,
    },
    webserver::{
        EditorMessage, EditorMessageContents, INITIAL_CLIENT_MESSAGE_ID, MESSAGE_ID_INCREMENT,
        ResultOkTypes, UpdateMessageContents, set_root_path,
    },
};

// Utilities
// ---------
//
// Not all messages produced by the server are ordered. To accommodate
// out-of-order messages, this class provides a way to `insert` expected
// messages, then wait until they're all be received (`assert_all_messages`).
struct ExpectedMessages(HashMap<i64, EditorMessageContents>);

impl ExpectedMessages {
    fn new() -> ExpectedMessages {
        ExpectedMessages(HashMap::new())
    }

    fn insert(&mut self, editor_message: EditorMessage) {
        assert!(
            self.0
                .insert(editor_message.id as i64, editor_message.message)
                .is_none()
        );
    }

    fn check(&mut self, editor_message: EditorMessage) {
        if let Some(editor_message_contents) = self.0.remove(&(editor_message.id as i64)) {
            assert_eq!(editor_message.message, editor_message_contents);
        } else {
            panic!(
                "Message not found: looked for \n{:#?}\nin:\n{:#?}",
                editor_message, self.0
            );
        }
    }

    async fn _assert_message(&mut self, codechat_server: &CodeChatEditorServer, timeout: Duration) {
        self.check(codechat_server.get_message_timeout(timeout).await.unwrap());
    }

    async fn assert_all_messages(
        &mut self,
        codechat_server: &CodeChatEditorServer,
        timeout: Duration,
    ) {
        while !self.0.is_empty() {
            self.check(codechat_server.get_message_timeout(timeout).await.unwrap());
        }
    }
}

// Time to wait for `ExpectedMessages`.
const TIMEOUT: Duration = Duration::from_millis(2000);

// ### Test harness
//
// A test harness. It runs the webdriver, the Server, opens the Client, then
// runs provided tests. After testing finishes, it cleans up (handling panics
// properly).
//
// The goal was to pass the harness a function which runs the tests. This
// currently doesn't work, due to problems with lifetimes (see comments). So,
// implement this as a macro instead (kludge!).
macro_rules! harness {
    // The name of the test function to call inside the harness.
    ($func: ident) => {
        pub async fn harness<
            'a,
            F: FnOnce(CodeChatEditorServer, &'a WebDriver, PathBuf) -> Fut,
            Fut: Future<Output = Result<(), WebDriverError>>,
        >(
            // The function which performs tests using thirtyfour. TODO: not
            // used.
            _f: F,
            // The output from calling `prep_test_dir!()`.
            prep_test_dir: (TempDir, PathBuf),
        ) -> Result<(), Box<dyn Error + Send + Sync>> {
            let (temp_dir, test_dir) = prep_test_dir;
            // The logger gets configured by (I think)
            // `start_webdriver_process`, which delegates to `selenium-manager`.
            // Set logging level here.
            unsafe { env::set_var("RUST_LOG", "debug") };
            // Start the webdriver.
            let server_url = "http://localhost:4444";
            let mut caps = DesiredCapabilities::chrome();
            caps.add_arg("--headless")?;
            // On Ubuntu CI, avoid failures, probably due to running Chrome as
            // root.
            #[cfg(target_os = "linux")]
            if env::var("CI") == Ok("true".to_string()) {
                caps.add_arg("--disable-gpu")?;
                caps.add_arg("--no-sandbox")?;
            }
            if let Err(err) = start_webdriver_process(server_url, &caps, true) {
                // Often, the "failure" is that the webdriver is already
                // running.
                eprintln!("Failed to start the webdriver process: {err:#?}");
            }
            let driver = WebDriver::new(server_url, caps).await?;
            let driver_clone = driver.clone();
            let driver_ref = &driver_clone;

            // Run the test inside an async, so we can shut down the driver
            // before returning an error. Mark the function as unwind safe.
            // though I'm not certain this is correct. Hopefully, it's good
            // enough for testing.
            let ret = AssertUnwindSafe(async move {
                // ### Setup
                let p = env::current_exe().unwrap().parent().unwrap().join("../..");
                set_root_path(Some(&p)).unwrap();
                let codechat_server = CodeChatEditorServer::new().unwrap();

                // Get the resulting web page text.
                let opened_id = codechat_server.send_message_opened(true).await.unwrap();
                pretty_assertions::assert_eq!(
                    codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
                    EditorMessage {
                        id: opened_id,
                        message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
                    }
                );
                let em_html = codechat_server.get_message_timeout(TIMEOUT).await.unwrap();
                codechat_server.send_result(em_html.id, None).await.unwrap();

                // Parse out the address to use.
                let client_html = cast!(&em_html.message, EditorMessageContents::ClientHtml);
                let find_str = "<iframe src=\"";
                let address_start = client_html.find(find_str).unwrap() + find_str.len();
                let address_end = client_html[address_start..].find("\"").unwrap() + address_start;
                let address = &client_html[address_start..address_end];

                // Open the Client and send it a file to load.
                driver_ref.goto(address).await.unwrap();
                // I'd like to call `f` here, but can't: Rust reports that
                // `driver_clone` doesn't live long enough. I don't know how to
                // fix this lifetime issue -- I want to specify that `f`'s
                // lifetime (which contains the state referring to
                // `driver_clone`) ends after the call to `f`, but don't know
                // how.
                $func(codechat_server, driver_ref, test_dir).await?;

                Ok(())
            })
            // Catch any panics/assertions, again to ensure the driver shuts
            // down cleanly.
            .catch_unwind()
            .await;

            // Always explicitly close the browser.
            driver.quit().await?;
            // Report any errors produced when removing the temporary directory.
            temp_dir.close()?;

            ret.unwrap_or_else(|err|
                    // Convert a panic to an error.
                    Err::<(), Box<dyn Error + Send + Sync>>(Box::from(format!(
                        "{err:#?}"
                    ))))
        }
    };
}

// Given an `Update` message with contents, get the version.
fn get_version(msg: &EditorMessage) -> f64 {
    cast!(&msg.message, EditorMessageContents::Update)
        .contents
        .as_ref()
        .unwrap()
        .version
}

// Tests
// -----
//
// ### Server-side test
//
// Perform most functions a user would: open/switch documents (Markdown,
// CodeChat, plain, PDF), use hyperlinks, perform edits on code and doc blocks.
mod test1 {
    use super::*;
    harness!(test_server_core);
}

#[tokio::test]
async fn test_server() -> Result<(), Box<dyn Error + Send + Sync>> {
    test1::harness(test_server_core, prep_test_dir!()).await
}

// Some of the thirtyfour calls are marked as deprecated, though they aren't
// marked that way in the Selenium docs.
#[allow(deprecated)]
async fn test_server_core(
    codechat_server: CodeChatEditorServer,
    driver_ref: &WebDriver,
    test_dir: PathBuf,
) -> Result<(), WebDriverError> {
    let mut expected_messages = ExpectedMessages::new();
    let path = canonicalize(test_dir.join("test.py")).unwrap();
    let path_str = path.to_str().unwrap().to_string();
    let current_file_id = codechat_server
        .send_message_current_file(path_str.clone())
        .await
        .unwrap();
    // The ordering of these messages isn't fixed -- one can come first, or the
    // other.
    expected_messages.insert(EditorMessage {
        id: current_file_id,
        message: EditorMessageContents::Result(Ok(ResultOkTypes::Void)),
    });
    let mut server_id = 6.0;
    expected_messages.insert(EditorMessage {
        id: server_id,
        message: EditorMessageContents::LoadFile(path),
    });
    expected_messages
        .assert_all_messages(&codechat_server, TIMEOUT)
        .await;

    // Respond to the load request.
    let mut version = 1.0;
    codechat_server
        .send_result_loadfile(server_id, Some(("# Test\ncode()".to_string(), version)))
        .await
        .unwrap();

    // Respond to the load request for the TOC.
    let toc_path = canonicalize(test_dir.join("toc.md")).unwrap();
    server_id += MESSAGE_ID_INCREMENT * 2.0;
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: server_id,
            message: EditorMessageContents::LoadFile(toc_path),
        }
    );
    codechat_server
        .send_result_loadfile(server_id, None)
        .await
        .unwrap();

    // The loadfile produces a message to the client, which comes back here. We
    // don't need to acknowledge it.
    server_id -= MESSAGE_ID_INCREMENT;
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: server_id,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );

    // Target the iframe containing the Client.
    let codechat_iframe = driver_ref.find(By::Css("#CodeChat-iframe")).await.unwrap();
    driver_ref
        .switch_to()
        .frame_element(&codechat_iframe)
        .await
        .unwrap();

    // ### Tests on source code
    //
    // #### Doc block tests
    //
    // Verify the first doc block.
    let indent_css = ".CodeChat-CodeMirror .CodeChat-doc-indent";
    let doc_block_indent = driver_ref.find(By::Css(indent_css)).await.unwrap();
    assert_eq!(doc_block_indent.inner_html().await.unwrap(), "");
    let contents_css = ".CodeChat-CodeMirror .CodeChat-doc-contents";
    let doc_block_contents = driver_ref.find(By::Css(contents_css)).await.unwrap();
    assert_eq!(
        doc_block_contents.inner_html().await.unwrap(),
        "<p>Test</p>\n"
    );

    // Focus it.
    doc_block_contents.click().await.unwrap();
    sleep(Duration::from_millis(100)).await;
    // Refind it, since it's now switched with a TinyMCE editor.
    let tinymce_contents = driver_ref.find(By::Id("TinyMCE-inst")).await.unwrap();
    // Make an edit.
    tinymce_contents.send_keys("foo").await.unwrap();

    // Verify the updated text.
    let mut client_id = INITIAL_CLIENT_MESSAGE_ID;
    // Update the version from the value provided by the client, which varies randomly.
    let msg = codechat_server.get_message_timeout(TIMEOUT).await.unwrap();
    let client_version = get_version(&msg);
    assert_eq!(
        msg,
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                contents: Some(CodeChatForWeb {
                    metadata: SourceFileMetadata {
                        mode: "python".to_string(),
                    },
                    source: CodeMirrorDiffable::Diff(CodeMirrorDiff {
                        doc: vec![StringDiff {
                            from: 0,
                            to: Some(7),
                            insert: "# Testfoo\n".to_string()
                        }],
                        doc_blocks: vec![],
                        version,
                    }),
                    version: client_version,
                }),
                cursor_position: Some(1),
                scroll_position: Some(1.0)
            })
        }
    );
    version = client_version;
    codechat_server.send_result(client_id, None).await.unwrap();

    // Edit the indent. It should only allow spaces and tabs, rejecting other
    // edits.
    doc_block_indent.send_keys("  123").await.unwrap();
    let msg = codechat_server.get_message_timeout(TIMEOUT).await.unwrap();
    let client_version = get_version(&msg);
    client_id += MESSAGE_ID_INCREMENT;
    assert_eq!(
        msg,
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                contents: Some(CodeChatForWeb {
                    metadata: SourceFileMetadata {
                        mode: "python".to_string(),
                    },
                    source: CodeMirrorDiffable::Diff(CodeMirrorDiff {
                        doc: vec![StringDiff {
                            from: 0,
                            to: Some(10),
                            insert: "  # Testfoo\n".to_string(),
                        }],
                        doc_blocks: vec![],
                        version,
                    }),
                    version: client_version,
                }),
                cursor_position: Some(1),
                scroll_position: Some(1.0),
            }),
        }
    );
    version = client_version;
    codechat_server.send_result(client_id, None).await.unwrap();

    // #### Code block tests
    //
    // Verify the first line of code.
    let code_line_css = ".CodeChat-CodeMirror .cm-line";
    let code_line = driver_ref.find(By::Css(code_line_css)).await.unwrap();
    assert_eq!(code_line.inner_html().await.unwrap(), "code()");

    // A click will update the current position and focus the code block.
    code_line.click().await.unwrap();
    client_id += MESSAGE_ID_INCREMENT;
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                contents: None,
                cursor_position: Some(2),
                scroll_position: Some(1.0)
            })
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();
    // Moving left should move us back to the doc block.
    code_line
        .send_keys("" + Key::Home + Key::Left)
        .await
        .unwrap();
    client_id += MESSAGE_ID_INCREMENT;
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                contents: None,
                cursor_position: Some(1),
                scroll_position: Some(1.0)
            })
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();

    // Make an edit to the code. This should also produce a client diff.
    code_line.send_keys("bar").await.unwrap();

    // Verify the updated text.
    client_id += MESSAGE_ID_INCREMENT;
    let msg = codechat_server.get_message_timeout(TIMEOUT).await.unwrap();
    let client_version = get_version(&msg);
    assert_eq!(
        msg,
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                contents: Some(CodeChatForWeb {
                    metadata: SourceFileMetadata {
                        mode: "python".to_string(),
                    },
                    source: CodeMirrorDiffable::Diff(CodeMirrorDiff {
                        doc: vec![StringDiff {
                            from: 12,
                            to: Some(18),
                            insert: "code()bar".to_string()
                        }],
                        doc_blocks: vec![],
                        version,
                    }),
                    version: client_version,
                }),
                cursor_position: Some(2),
                scroll_position: Some(1.0)
            })
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();

    // #### IDE edits
    //
    // Perform IDE edits.
    version = 2.0;
    let ide_id = codechat_server
        .send_message_update_plain(
            path_str.clone(),
            Some(("  # Testfood\ncode()bark".to_string(), version)),
            Some(1),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: ide_id,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );

    // Verify them.
    let doc_block_indent = driver_ref.find(By::Css(indent_css)).await.unwrap();
    assert_eq!(doc_block_indent.inner_html().await.unwrap(), "  ");
    let doc_block_contents = driver_ref.find(By::Css(contents_css)).await.unwrap();
    assert_eq!(
        doc_block_contents.inner_html().await.unwrap(),
        "<p>Testfood</p>\n"
    );
    let code_line = driver_ref.find(By::Css(code_line_css)).await.unwrap();
    assert_eq!(code_line.inner_html().await.unwrap(), "code()bark");

    // Perform a second edit and verification, to produce a diff sent to the
    // Client.
    version = 3.0;
    let ide_id = codechat_server
        .send_message_update_plain(
            path_str.clone(),
            Some((" # food\nbark".to_string(), version)),
            Some(1),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: ide_id,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );
    let doc_block_indent = driver_ref.find(By::Css(indent_css)).await.unwrap();
    assert_eq!(doc_block_indent.inner_html().await.unwrap(), " ");
    let doc_block_contents = driver_ref.find(By::Css(contents_css)).await.unwrap();
    assert_eq!(
        doc_block_contents.inner_html().await.unwrap(),
        "<p>food</p>\n"
    );
    let code_line = driver_ref.find(By::Css(code_line_css)).await.unwrap();
    assert_eq!(code_line.inner_html().await.unwrap(), "bark");

    // ### Document-only tests
    //
    // Load in a document.
    let md_path = canonicalize(test_dir.join("test.md")).unwrap();
    let md_path_str = md_path.to_str().unwrap().to_string();
    let current_file_id = codechat_server
        .send_message_current_file(md_path_str.clone())
        .await
        .unwrap();

    // Before changing files, the current file will be updated.
    client_id += MESSAGE_ID_INCREMENT;
    let msg = codechat_server.get_message_timeout(TIMEOUT).await.unwrap();
    let client_version = get_version(&msg);
    assert_eq!(
        msg,
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                contents: Some(CodeChatForWeb {
                    metadata: SourceFileMetadata {
                        mode: "python".to_string()
                    },
                    source: CodeMirrorDiffable::Plain(CodeMirror {
                        doc: " # food\nbark".to_string(),
                        doc_blocks: vec![]
                    }),
                    version: client_version,
                }),
                cursor_position: Some(1),
                scroll_position: Some(1.0),
            })
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();

    // These next two messages can come in either order. Work around this.
    expected_messages.insert(EditorMessage {
        id: current_file_id,
        message: EditorMessageContents::Result(Ok(ResultOkTypes::Void)),
    });
    server_id += MESSAGE_ID_INCREMENT * 2.0;
    expected_messages.insert(EditorMessage {
        id: server_id,
        message: EditorMessageContents::LoadFile(md_path),
    });
    expected_messages
        .assert_all_messages(&codechat_server, TIMEOUT)
        .await;

    // Provide the requested file contents.
    version = 4.0;
    codechat_server
        .send_result_loadfile(
            server_id,
            Some(("A **markdown** file.".to_string(), version)),
        )
        .await
        .unwrap();

    // Respond to the load request for the TOC.
    let toc_path = canonicalize(test_dir.join("toc.md")).unwrap();
    server_id += MESSAGE_ID_INCREMENT * 2.0;
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: server_id,
            message: EditorMessageContents::LoadFile(toc_path.clone()),
        }
    );
    codechat_server
        .send_result_loadfile(server_id, None)
        .await
        .unwrap();

    // Absorb the result produced by the Server's Update resulting from the
    // LoadFile.
    server_id -= MESSAGE_ID_INCREMENT;
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: server_id,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );

    // Check the content.
    let body_css = "#CodeChat-body .CodeChat-doc-contents";
    let body_content = driver_ref.find(By::Css(body_css)).await.unwrap();
    assert_eq!(
        body_content.inner_html().await.unwrap(),
        "<p>A <strong>markdown</strong> file.</p>"
    );

    // Perform edits.
    body_content.send_keys("foo ").await.unwrap();
    client_id += MESSAGE_ID_INCREMENT;
    let msg = codechat_server.get_message_timeout(TIMEOUT).await.unwrap();
    let client_version = get_version(&msg);
    assert_eq!(
        msg,
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: md_path_str.clone(),
                contents: Some(CodeChatForWeb {
                    metadata: SourceFileMetadata {
                        mode: MARKDOWN_MODE.to_string()
                    },
                    source: CodeMirrorDiffable::Diff(CodeMirrorDiff {
                        doc: vec![StringDiff {
                            from: 0,
                            to: Some(20),
                            insert: format!(
                                "foo A **markdown** file.{}",
                                if cfg!(windows) { "\r\n" } else { "\n" }
                            ),
                        }],
                        doc_blocks: vec![],
                        version,
                    }),
                    version: client_version,
                }),
                cursor_position: None,
                scroll_position: None
            })
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();

    // Perform an IDE edit.
    version = 5.0;
    let ide_id = codechat_server
        .send_message_update_plain(
            md_path_str.clone(),
            Some(("food A **markdown** file.".to_string(), version)),
            Some(1),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: ide_id,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );
    assert_eq!(
        body_content.inner_html().await.unwrap(),
        "<p>food A <strong>markdown</strong> file.</p>"
    );

    // ### Unsupported document
    let txt_path = canonicalize(test_dir.join("test.txt")).unwrap();
    let txt_path_str = txt_path.to_str().unwrap().to_string();
    let current_file_id = codechat_server
        .send_message_current_file(txt_path_str.clone())
        .await
        .unwrap();

    expected_messages.insert(EditorMessage {
        id: current_file_id,
        message: EditorMessageContents::Result(Ok(ResultOkTypes::Void)),
    });
    server_id += MESSAGE_ID_INCREMENT * 2.0;
    expected_messages.insert(EditorMessage {
        id: server_id,
        message: EditorMessageContents::LoadFile(txt_path.clone()),
    });
    expected_messages
        .assert_all_messages(&codechat_server, TIMEOUT)
        .await;
    codechat_server.send_result(client_id, None).await.unwrap();

    // Ask the server to load the file from disk.
    codechat_server
        .send_result_loadfile(server_id, None)
        .await
        .unwrap();
    // There's a second request for this file, made by the iframe, plus a
    // request for the TOC. The ordering isn't fixed; accommodate this.
    server_id += MESSAGE_ID_INCREMENT;
    let msg = codechat_server.get_message_timeout(TIMEOUT).await.unwrap();
    assert_eq!(msg.id, server_id);
    let msg_contents = cast!(msg.message, EditorMessageContents::LoadFile);
    let next_path = if msg_contents == toc_path.clone() {
        txt_path.clone()
    } else if msg_contents == txt_path.clone() {
        toc_path.clone()
    } else {
        panic!("Unexpected path {msg_contents:?}.");
    };
    codechat_server
        .send_result_loadfile(server_id, None)
        .await
        .unwrap();
    server_id += MESSAGE_ID_INCREMENT;
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: server_id,
            message: EditorMessageContents::LoadFile(next_path),
        }
    );
    codechat_server
        .send_result_loadfile(server_id, None)
        .await
        .unwrap();

    // Look at the content, which should be an iframe.
    let plain_content = driver_ref
        .find(By::Css("#CodeChat-contents"))
        .await
        .unwrap();
    assert!(
        plain_content
            .outer_html()
            .await
            .unwrap()
            .starts_with("<iframe src=\"test.txt?raw")
    );

    // TODO: This isn't editable in the Client. Only perform edits in the IDE.
    // However, this code needs revising, so testing it is skipped for now.

    // #### PDF viewer
    //
    // Click on the link for the PDF to test.
    let toc_iframe = driver_ref.find(By::Css("#CodeChat-sidebar")).await.unwrap();
    driver_ref
        .switch_to()
        .frame_element(&toc_iframe)
        .await
        .unwrap();
    let test_pdf = driver_ref.find(By::LinkText("test.pdf")).await.unwrap();
    test_pdf.click().await.unwrap();

    // Respond to the current file, then load requests for the PDf and the TOC.
    let pdf_path = canonicalize(test_dir.join("test.pdf")).unwrap();
    let pdf_path_str = pdf_path.to_str().unwrap().to_string();
    client_id += MESSAGE_ID_INCREMENT;
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::CurrentFile(pdf_path_str, Some(true))
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();
    server_id += MESSAGE_ID_INCREMENT;
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: server_id,
            message: EditorMessageContents::LoadFile(pdf_path.clone())
        }
    );
    codechat_server
        .send_result_loadfile(server_id, None)
        .await
        .unwrap();
    server_id += MESSAGE_ID_INCREMENT;
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: server_id,
            message: EditorMessageContents::LoadFile(toc_path)
        }
    );
    codechat_server
        .send_result_loadfile(server_id, None)
        .await
        .unwrap();
    server_id += MESSAGE_ID_INCREMENT;
    // Another load is sent for the actual PDF contents.
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: server_id,
            message: EditorMessageContents::LoadFile(pdf_path)
        }
    );
    codechat_server
        .send_result_loadfile(server_id, None)
        .await
        .unwrap();

    // Check that the PDF viewer was sent.
    //
    // Target the iframe containing the Client.
    driver_ref
        .switch_to()
        .frame_element(&codechat_iframe)
        .await
        .unwrap();
    let plain_content = driver_ref
        .find(By::Css("#CodeChat-contents"))
        .await
        .unwrap();
    assert!(
        plain_content
            .outer_html()
            .await
            .unwrap()
            .starts_with("<iframe src=\"/static/pdfjs-main.html?")
    );

    Ok(())
}

// ### Client tests
//
// This simply runs client-side tests written in TypeScript, verifying that they
// all pass.
mod test2 {
    use super::*;
    harness!(test_client_core);
}

#[tokio::test]
async fn test_client() -> Result<(), Box<dyn Error + Send + Sync>> {
    // If both thirtyfour tests start at the same time, both fail; perhaps
    // there's some confusion when two requests care made to the same webserver
    // from two clients within the same process? In order to avoid then, insert
    // a delay to hopefully start this test at a different time than
    // `test_server_core`.
    sleep(Duration::from_millis(100)).await;
    test2::harness(test_client_core, prep_test_dir!()).await
}

// Some of the thirtyfour calls are marked as deprecated, though they aren't
// marked that way in the Selenium docs.
#[allow(deprecated)]
async fn test_client_core(
    codechat_server: CodeChatEditorServer,
    driver_ref: &WebDriver,
    test_dir: PathBuf,
) -> Result<(), WebDriverError> {
    let mut expected_messages = ExpectedMessages::new();
    let path = canonicalize(test_dir.join("test.py")).unwrap();
    let path_str = path.to_str().unwrap().to_string();
    let current_file_id = codechat_server
        .send_message_current_file(path_str.clone())
        .await
        .unwrap();
    // The ordering of these messages isn't fixed -- one can come first, or the
    // other.
    expected_messages.insert(EditorMessage {
        id: current_file_id,
        message: EditorMessageContents::Result(Ok(ResultOkTypes::Void)),
    });
    let mut server_id = 6.0;
    expected_messages.insert(EditorMessage {
        id: server_id,
        message: EditorMessageContents::LoadFile(path.clone()),
    });
    expected_messages
        .assert_all_messages(&codechat_server, TIMEOUT)
        .await;

    // Respond to the load request.
    codechat_server
        .send_result_loadfile(server_id, None)
        .await
        .unwrap();

    // Respond to the load request for the TOC.
    let toc_path = canonicalize(test_dir.join("toc.md")).unwrap();
    server_id += MESSAGE_ID_INCREMENT * 2.0;
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: server_id,
            message: EditorMessageContents::LoadFile(toc_path.clone()),
        }
    );
    codechat_server
        .send_result_loadfile(server_id, None)
        .await
        .unwrap();

    // The loadfile produces a message to the client, which comes back here. We
    // don't need to acknowledge it.
    server_id -= MESSAGE_ID_INCREMENT;
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: server_id,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );

    // Target the iframe containing the Client.
    let codechat_iframe = driver_ref.find(By::Css("#CodeChat-iframe")).await.unwrap();
    driver_ref
        .switch_to()
        .frame_element(&codechat_iframe)
        .await
        .unwrap();

    // Click on the link for the PDF to test.
    let toc_iframe = driver_ref.find(By::Css("#CodeChat-sidebar")).await.unwrap();
    driver_ref
        .switch_to()
        .frame_element(&toc_iframe)
        .await
        .unwrap();
    let test_py = driver_ref.find(By::LinkText("test.py")).await.unwrap();
    test_py.click().await.unwrap();

    // Respond to the current file, then load requests for the PDf and the TOC.
    let client_id = INITIAL_CLIENT_MESSAGE_ID;
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::CurrentFile(path_str, Some(true))
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();
    server_id += MESSAGE_ID_INCREMENT * 2.0;
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: server_id,
            message: EditorMessageContents::LoadFile(path.clone())
        }
    );
    codechat_server
        .send_result_loadfile(server_id, None)
        .await
        .unwrap();
    server_id += MESSAGE_ID_INCREMENT * 2.0;
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: server_id,
            message: EditorMessageContents::LoadFile(toc_path)
        }
    );
    codechat_server
        .send_result_loadfile(server_id, None)
        .await
        .unwrap();

    // Wait for the tests to run.
    sleep(Duration::from_millis(3000)).await;

    // Look for the test results.
    driver_ref
        .switch_to()
        .frame_element(&codechat_iframe)
        .await
        .unwrap();
    let mocha_results = driver_ref
        .find(By::Css("#mocha-stats .result"))
        .await
        .unwrap();
    assert_eq!(mocha_results.inner_html().await.unwrap(), "✓");

    Ok(())
}

mod test3 {
    use super::*;
    harness!(test_client_updates_core);
}

#[tokio::test]
async fn test_client_updates() -> Result<(), Box<dyn Error + Send + Sync>> {
    // If both thirtyfour tests start at the same time, both fail; perhaps
    // there's some confusion when two requests care made to the same webserver
    // from two clients within the same process? In order to avoid then, insert
    // a delay to hopefully start this test at a different time than
    // `test_server_core`.
    sleep(Duration::from_millis(100)).await;
    test3::harness(test_client_updates_core, prep_test_dir!()).await
}

// Some of the thirtyfour calls are marked as deprecated, though they aren't
// marked that way in the Selenium docs.
#[allow(deprecated)]
async fn test_client_updates_core(
    codechat_server: CodeChatEditorServer,
    driver_ref: &WebDriver,
    test_dir: PathBuf,
) -> Result<(), WebDriverError> {
    let mut expected_messages = ExpectedMessages::new();
    let path = canonicalize(test_dir.join("test.py")).unwrap();
    let path_str = path.to_str().unwrap().to_string();
    let current_file_id = codechat_server
        .send_message_current_file(path_str.clone())
        .await
        .unwrap();
    // The ordering of these messages isn't fixed -- one can come first, or the
    // other.
    expected_messages.insert(EditorMessage {
        id: current_file_id,
        message: EditorMessageContents::Result(Ok(ResultOkTypes::Void)),
    });
    let mut server_id = 6.0;
    expected_messages.insert(EditorMessage {
        id: server_id,
        message: EditorMessageContents::LoadFile(path.clone()),
    });
    expected_messages
        .assert_all_messages(&codechat_server, TIMEOUT)
        .await;

    // Respond to the load request.
    let ide_version = 0.0;
    codechat_server
        .send_result_loadfile(
            server_id,
            Some((
                indoc!(
                    "
                    # Test updates in the client that modify the client after appending to a line.
                    def foo():
                        A comment
                        print()
                    "
                )
                .to_string(),
                ide_version,
            )),
        )
        .await
        .unwrap();

    // Respond to the load request for the TOC.
    let toc_path = canonicalize(test_dir.join("toc.md")).unwrap();
    server_id += MESSAGE_ID_INCREMENT * 2.0;
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: server_id,
            message: EditorMessageContents::LoadFile(toc_path.clone()),
        }
    );
    codechat_server
        .send_result_loadfile(server_id, None)
        .await
        .unwrap();

    // The loadfile produces a message to the client, which comes back here. We
    // don't need to acknowledge it.
    server_id -= MESSAGE_ID_INCREMENT;
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: server_id,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );

    // Target the iframe containing the Client.
    let codechat_iframe = driver_ref.find(By::Css("#CodeChat-iframe")).await.unwrap();
    driver_ref
        .switch_to()
        .frame_element(&codechat_iframe)
        .await
        .unwrap();

    // Select the doc block and add to the line, causing a word wrap.
    let contents_css = ".CodeChat-CodeMirror .CodeChat-doc-contents";
    let doc_block_contents = driver_ref.find(By::Css(contents_css)).await.unwrap();
    doc_block_contents
        .send_keys("" + Key::End + " testing")
        .await
        .unwrap();

    // Verify the updated text.
    let msg = codechat_server.get_message_timeout(TIMEOUT).await.unwrap();
    let client_version = get_version(&msg);
    let mut client_id = INITIAL_CLIENT_MESSAGE_ID;
    assert_eq!(
        msg,
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                contents: Some(CodeChatForWeb {
                    metadata: SourceFileMetadata {
                        mode: "python".to_string(),
                    },
                    source: CodeMirrorDiffable::Diff(CodeMirrorDiff {
                        doc: vec![StringDiff {
                            from: 79,
                            to: None,
                            insert: "# testing\n".to_string()
                        }],
                        doc_blocks: vec![],
                        version: ide_version,
                    }),
                    version: client_version,
                }),
                cursor_position: Some(1),
                scroll_position: Some(1.0)
            })
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();

    // Insert a character to check the insertion point.
    let code_line_css = ".CodeChat-CodeMirror .cm-line";
    let code_line = driver_ref.find(By::Css(code_line_css)).await.unwrap();
    code_line
        .send_keys(Key::Alt + Key::Control + "g")
        .await
        .unwrap();
    // Enter a line in the dialog that pops up.
    driver_ref
        .find(By::Css("input.cm-textfield"))
        .await
        .unwrap()
        .send_keys("4" + Key::Enter)
        .await
        .unwrap();
    // Add an indented comment.
    code_line.send_keys(Key::Home + "# ").await.unwrap();
    // This should edit the (new) third line of the file after word wrap: `def
    // foo():`.
    let msg = codechat_server.get_message_timeout(TIMEOUT).await.unwrap();
    let new_client_version = get_version(&msg);
    client_id += MESSAGE_ID_INCREMENT;
    assert_eq!(
        msg,
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                contents: Some(CodeChatForWeb {
                    metadata: SourceFileMetadata {
                        mode: "python".to_string(),
                    },
                    source: CodeMirrorDiffable::Diff(CodeMirrorDiff {
                        doc: vec![StringDiff {
                            from: 115,
                            to: Some(131),
                            insert: "    # A comment\n".to_string()
                        }],
                        doc_blocks: vec![],
                        version: client_version,
                    }),
                    version: new_client_version,
                }),
                cursor_position: Some(4),
                scroll_position: Some(1.0)
            })
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();

    Ok(())
}
