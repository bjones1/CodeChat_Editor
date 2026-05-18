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
/// A second challenge revolves around the lack of an async `Drop` trait: the
/// web driver server should be started before any test, left running during all
/// tests, then terminated as the test program exits. The web driver must be
/// initialized before a test then stopped at the end of that test. Both are
/// ideal for this missing Drop trait. As a workaround:
///
/// * The web driver server relies on the C `atexit` call to stop the server.
///   However, when tests fail, this doesn't get called, leaving the server
///   running. This causes the server to fail to start on the next test run,
///   since it's still running. Therefore, errors when starting the web driver
///   server are ignored by design.
/// * Tests are run in an async block, and any panics produced inside it are
///   caught using `catch_unwind()`. The driver is shut down before returning an
///   error due to the panic.
// Imports
// -------
//
// ### Standard library
use std::{
    collections::HashMap,
    env,
    error::Error,
    panic::AssertUnwindSafe,
    path::{Path, PathBuf},
    time::Duration,
};

use assert_fs::TempDir;
// ### Third-party
use dunce::canonicalize;
use futures::FutureExt;
use pretty_assertions::assert_eq;
use thirtyfour::{
    By, ChromiumLikeCapabilities, DesiredCapabilities, Key, WebDriver, WebElement,
    error::WebDriverError,
};
use tracing::debug;
use tracing_log::LogTracer;
use tracing_subscriber::EnvFilter;

// ### Local
use code_chat_editor::{
    ide::CodeChatEditorServer,
    processing::{CodeChatForWeb, CodeMirrorDiff, CodeMirrorDiffable, SourceFileMetadata},
    webserver::{
        CursorPosition, EditorMessage, EditorMessageContents, MESSAGE_ID_INCREMENT, ResultOkTypes,
        UpdateMessageContents, set_root_path,
    },
};
use test_utils::cast;

// Utilities
// ---------
//
// Not all messages produced by the server are ordered. To accommodate
// out-of-order messages, this class provides a way to `insert` expected
// messages, then wait until they're all be received (`assert_all_messages`).
pub struct ExpectedMessages(HashMap<i64, (EditorMessageContents, bool)>);

impl ExpectedMessages {
    pub fn new() -> ExpectedMessages {
        ExpectedMessages(HashMap::new())
    }

    pub fn insert(
        &mut self,
        editor_message: EditorMessage,
        // For this message, copy the version from the received
        // EditorMessage.contents.version to the same field in the message to
        // check against.
        is_dynamic: bool,
    ) {
        assert!(
            self.0
                .insert(
                    editor_message.id as i64,
                    (editor_message.message, is_dynamic)
                )
                .is_none()
        );
    }

    pub fn check(&mut self, editor_message: EditorMessage) {
        if let Some((ref mut editor_message_contents, is_dynamic)) =
            self.0.remove(&(editor_message.id as i64))
        {
            if is_dynamic
                && let EditorMessageContents::Update(emc) = editor_message_contents
                && let Some(contents) = &mut emc.contents
            {
                let version = get_version(&editor_message);
                contents.version = version;
            }
            // Special case:
            assert_eq!(&editor_message.message, editor_message_contents);
        } else {
            panic!(
                "Message not found: looked for \n{:#?}\nin:\n{:#?}",
                self.0, editor_message,
            );
        }
    }

    async fn _assert_message(&mut self, codechat_server: &CodeChatEditorServer, timeout: Duration) {
        self.check(codechat_server.get_message_timeout(timeout).await.unwrap());
    }

    pub async fn assert_all_messages(
        &mut self,
        codechat_server: &CodeChatEditorServer,
        timeout: Duration,
    ) {
        while !self.0.is_empty() {
            if let Some(editor_message) = codechat_server.get_message_timeout(timeout).await {
                self.check(editor_message);
            } else {
                panic!(
                    "No matching messages found. Unmatched messages:\n{:#?}",
                    self.0
                );
            }
        }
    }
}

// Time to wait for browser/WebDriver-backed client-server messages. This
// matches the client-side response window and gives CI enough room for autosave
// and loadfile acknowledgements under matrix load.
pub const TIMEOUT: Duration = Duration::from_millis(15000);

// Browser-backed tests share a single WebDriver endpoint. Safari on macOS CI is
// unreliable with overlapping sessions, so serialize the harness.
pub(crate) static WEB_DRIVER_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

// ### Test harness
//
// A test harness. It runs the webdriver, the Server, opens the Client, then
// runs provided tests. After testing finishes, it cleans up (handling panics
// properly).
pub async fn harness<
    F: FnOnce(CodeChatEditorServer, WebDriver, PathBuf) -> Fut,
    Fut: Future<Output = Result<(), WebDriverError>>,
>(
    f: F,
    // The output from calling `prep_test_dir!()`.
    prep_test_dir: (TempDir, PathBuf),
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let _webdriver_test_lock = WEB_DRIVER_TEST_LOCK.lock().await;
    // Send log events to the tracing subscriber, since the code currently uses
    // a log-based framework. As below, ignore re-initialization errors.
    let _ = LogTracer::init();
    let filter = EnvFilter::new("debug")
        .add_directive("html5ever=off".parse().unwrap())
        .add_directive("thirtyfour::session=off".parse().unwrap())
        .add_directive("hyper_util=off".parse().unwrap());
    // Construct a subscriber that prints formatted traces to stdout.
    let subscriber = tracing_subscriber::fmt().with_env_filter(filter).finish();
    // Use that subscriber to process traces emitted after this point. Ignore
    // errors, since other threads may initialize this first, causing an
    // re-initialization error.
    let _ = tracing::subscriber::set_global_default(subscriber);
    let (temp_dir, test_dir) = prep_test_dir;
    let mut caps = DesiredCapabilities::chrome();
    // Ensure the screen is wide enough for an 80-character line, used to word
    // wrapping test in `test_client_updates`. Otherwise, this test send the End
    // key to go to the end of the line...but it's not the end of the full line
    // on a narrow screen.
    caps.add_arg("--window-size=1920,768")?;
    //caps.add_arg("--auto-open-devtools-for-tabs")?;
    caps.add_arg("--headless")?;
    // On Ubuntu CI, avoid failures, probably due to running Chrome as root.
    #[cfg(target_os = "linux")]
    if env::var("CI") == Ok("true".to_string()) {
        caps.add_arg("--disable-gpu")?;
        caps.add_arg("--no-sandbox")?;
    }
    // Start the webdriver.
    let driver = WebDriver::managed(caps).await?;
    let driver_clone = driver.clone();

    // Run the test inside an async, so we can shut down the driver before
    // returning an error. Mark the function as unwind safe. though I'm not
    // certain this is correct. Hopefully, it's good enough for testing.
    let ret = AssertUnwindSafe(async move {
        // ### Setup
        let p = env::current_exe().unwrap().parent().unwrap().join("../..");
        set_root_path(Some(&p)).unwrap();
        let codechat_server = CodeChatEditorServer::new().unwrap();

        // Get the resulting web page text.
        let opened_id = codechat_server.send_message_opened(true).await.unwrap();
        assert_eq!(
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
        driver_clone.goto(address).await.unwrap();
        f(codechat_server, driver_clone, test_dir).await?;

        Ok(())
    })
    // Catch any panics/assertions, again to ensure the driver shuts down
    // cleanly.
    .catch_unwind()
    .await;

    // Always explicitly close the browser.
    driver.quit().await?;
    // Report any errors produced when removing the temporary directory.
    temp_dir.close()?;

    ret.unwrap_or_else(
        // Convert a panic to an error.
        |err| Err::<(), Box<dyn Error + Send + Sync>>(Box::from(format!("{err:#?}"))),
    )
}

#[macro_export]
macro_rules! make_test {
    // The name of the test function to call inside the harness.
    ($test_name: ident, $test_core_name: ident) => {
        #[tokio::test]
        #[tracing::instrument]
        async fn $test_name() -> Result<(), Box<dyn Error + Send + Sync>> {
            $crate::overall_common::harness($test_core_name, prep_test_dir!()).await
        }
    };
}

// Given an `Update` message with contents, get the version.
pub fn get_version(msg: &EditorMessage) -> f64 {
    let ccfw = cast!(&msg.message, EditorMessageContents::Update)
        .contents
        .as_ref();
    ccfw.unwrap_or_else(|| panic!("No contents in message:\n{:#?}", msg))
        .version
}

// Used in one of the common tests, but not in the other...so we get a clippy
// lint.
#[allow(dead_code)]
#[tracing::instrument(skip_all)]
pub async fn goto_line(
    codechat_server: &CodeChatEditorServer,
    driver_ref: &WebDriver,
    client_id: &mut f64,
    path_str: &str,
    line: u32,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let code_line_css = ".CodeChat-CodeMirror .cm-line";
    let code_line = driver_ref.find(By::Css(code_line_css)).await.unwrap();
    code_line
        .send_keys(
            Key::Alt
                + if cfg!(target_os = "macos") {
                    Key::Command
                } else {
                    Key::Control
                }
                + "g",
        )
        .await
        .unwrap();
    // Enter a line in the dialog that pops up.
    driver_ref
        .find(By::Css("input.cm-textfield"))
        .await
        .unwrap()
        .send_keys(line.to_string() + Key::Enter)
        .await
        .unwrap();
    // The cursor movement produces a cursor/scroll position update after an
    // autosave delay. Sometimes, we get an update just before the movement;
    // ignore that.
    let mut msg = codechat_server.get_message_timeout(TIMEOUT).await.unwrap();
    if msg.id == *client_id
        && let EditorMessageContents::Update(update) = &msg.message
        && update.file_path == path_str
        && update.cursor_position != Some(CursorPosition::Line(line))
        && update.scroll_position == Some(1.0)
        && !update.is_re_translation
        && update.contents.is_none()
    {
        debug!("Accepted optional cursor update message for {path_str}.");
        codechat_server.send_result(*client_id, None).await.unwrap();
        *client_id += MESSAGE_ID_INCREMENT;
        msg = codechat_server.get_message_timeout(TIMEOUT).await.unwrap();
    }
    assert_eq!(
        msg,
        EditorMessage {
            id: *client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.to_string(),
                cursor_position: Some(CursorPosition::Line(line)),
                scroll_position: Some(1.0),
                is_re_translation: false,
                contents: None,
            })
        }
    );
    codechat_server.send_result(*client_id, None).await.unwrap();
    *client_id += MESSAGE_ID_INCREMENT;

    Ok(())
}

pub async fn perform_loadfile(
    codechat_server: &CodeChatEditorServer,
    test_dir: &Path,
    file_name: &str,
    file_contents: Option<(String, f64)>,
    has_toc: bool,
    server_id: f64,
) -> f64 {
    let mut expected_messages = ExpectedMessages::new();
    let path = canonicalize(test_dir.join(file_name)).unwrap();
    let path_str = path.to_str().unwrap().to_string();
    let current_file_id = codechat_server
        .send_message_current_file(path_str.clone())
        .await
        .unwrap();
    // The ordering of these messages isn't fixed -- one can come first, or the
    // other.
    expected_messages.insert(
        EditorMessage {
            id: current_file_id,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void)),
        },
        false,
    );
    expected_messages.insert(
        EditorMessage {
            id: server_id,
            message: EditorMessageContents::LoadFile(path.clone(), true),
        },
        false,
    );
    expected_messages
        .assert_all_messages(codechat_server, TIMEOUT)
        .await;

    // Respond to the load request.
    codechat_server
        .send_result_loadfile(server_id, file_contents)
        .await
        .unwrap();
    let mut server_id = server_id + MESSAGE_ID_INCREMENT;

    if has_toc {
        // Respond to the load request for the TOC.
        let toc_path = canonicalize(test_dir.join("toc.md")).unwrap();
        server_id += MESSAGE_ID_INCREMENT;
        assert_eq!(
            codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
            EditorMessage {
                id: server_id,
                message: EditorMessageContents::LoadFile(toc_path, false),
            }
        );
        codechat_server
            .send_result_loadfile(server_id, None)
            .await
            .unwrap();
        server_id -= MESSAGE_ID_INCREMENT;
    }

    // The loadfile produces a message to the client, which comes back here. We
    // don't need to acknowledge it.
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: server_id,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );
    server_id += MESSAGE_ID_INCREMENT;

    if has_toc {
        server_id + MESSAGE_ID_INCREMENT
    } else {
        server_id
    }
}

#[allow(deprecated)]
pub async fn select_codechat_iframe(driver_ref: &WebDriver) -> WebElement {
    // Target the iframe containing the Client.
    let codechat_iframe = driver_ref.find(By::Css("#CodeChat-iframe")).await.unwrap();
    codechat_iframe.clone().enter_frame().await.unwrap();

    codechat_iframe
}

// Used in one of the common tests, but not in the other...so we get a clippy
// lint.
#[allow(dead_code)]
pub async fn get_empty_client_update(
    codechat_server: &CodeChatEditorServer,
    path_str: &str,
    client_id: &mut f64,
    client_version: &mut f64,
    mode: &str,
    cursor_position: Option<CursorPosition>,
    scroll_position: Option<f32>,
) {
    let msg = codechat_server.get_message_timeout(TIMEOUT).await.unwrap();
    let version = *client_version;
    *client_version = get_version(&msg);
    assert_eq!(
        msg,
        EditorMessage {
            id: *client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.to_owned(),
                cursor_position,
                scroll_position,
                is_re_translation: false,
                contents: Some(CodeChatForWeb {
                    metadata: SourceFileMetadata {
                        mode: mode.to_string()
                    },
                    source: CodeMirrorDiffable::Diff(CodeMirrorDiff {
                        doc: vec![],
                        doc_blocks: vec![],
                        version,
                    }),
                    version: *client_version
                }),
            })
        }
    );
    codechat_server.send_result(*client_id, None).await.unwrap();
    *client_id += MESSAGE_ID_INCREMENT;
}

pub async fn assert_no_more_messages(codechat_server: &CodeChatEditorServer) {
    if let Some(msg) = codechat_server
        .get_message_timeout(Duration::from_millis(500))
        .await
    {
        panic!("Unprocessed messages: {:#?}", msg);
    }
}

/// Wait for a message. If it matches the provided optional message, acknowledge
/// it and update the client ID, then wait for another message. Return the most
/// recently received message.
#[tracing::instrument(skip_all)]
pub async fn optional_message(
    codechat_server: &CodeChatEditorServer,
    client_id: &mut f64,
    optional_message: EditorMessageContents,
) -> EditorMessage {
    let msg = codechat_server.get_message_timeout(TIMEOUT).await.unwrap();
    if msg
        == (EditorMessage {
            id: *client_id,
            message: optional_message,
        })
    {
        debug!("Accepted optional update message.");
        codechat_server.send_result(*client_id, None).await.unwrap();
        *client_id += MESSAGE_ID_INCREMENT;
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap()
    } else {
        msg
    }
}
