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
/// `overall/common/mod.rs` - test the overall system
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
use serde_json::Value;
use thirtyfour::{
    BrowserLogEntry, By, ChromiumLikeCapabilities, DesiredCapabilities, Key, LoggingPrefsLogLevel,
    TypingData, WebDriver, WebElement, error::WebDriverError, prelude::ElementQueryable,
};
use tracing::{debug, error, info, warn};
use tracing_log::LogTracer;
use tracing_subscriber::EnvFilter;

// ### Local
use code_chat_editor::{
    ide::CodeChatEditorServer,
    webserver::{
        CursorPosition, EditorMessage, EditorMessageContents, MESSAGE_ID_INCREMENT, ResultErrTypes,
        ResultOkTypes, UpdateMessageContents, set_root_path,
    },
};
use test_utils::cast;

// Console-log-polling server wrapper
// ----------------------------------
//
// The legacy `/log` "browser" buffer (where chromedriver collects page-side
// `console.*` output and uncaught JavaScript errors) is only drained when we
// ask for it. To interleave that output with the rest of the test's logging,
// this wrapper holds the `CodeChatEditorServer` together with a `WebDriver`
// handle and drains the buffer on every call the test framework makes. Each
// delegated method forwards to the inner server and, around that call, polls
// the browser log via \[`forward_browser_logs`\].
//
// The wrapper exposes the same method names as `CodeChatEditorServer`, so test
// bodies use it transparently.
pub struct CodeChatEditorServerLog {
    inner: CodeChatEditorServer,
    // A `WebDriver` handle used only to read the browser log. `WebDriver` is
    // cheap to clone (it's an `Arc` internally), so a clone of the harness's
    // driver is stored here.
    driver: WebDriver,
}

impl CodeChatEditorServerLog {
    pub fn new(inner: CodeChatEditorServer, driver: WebDriver) -> CodeChatEditorServerLog {
        CodeChatEditorServerLog { inner, driver }
    }

    // Drain and forward any console output the browser has produced so far,
    // returning the drained entries.
    pub async fn poll_log(&self) -> Vec<BrowserLogEntry> {
        forward_browser_logs(&self.driver).await
    }

    // The following methods mirror `CodeChatEditorServer`'s API. Each polls the
    // browser log so console output is emitted close to when it occurred, then
    // delegates to the inner server.
    pub async fn get_message_timeout(&self, timeout: Duration) -> Option<EditorMessage> {
        let msg = self.inner.get_message_timeout(timeout).await;
        // Waiting for a message is when the browser is most active, so poll
        // again after the wait to catch output produced during it.
        self.poll_log().await;
        msg
    }

    pub async fn send_message_opened(&self, hosted_in_ide: bool) -> std::io::Result<f64> {
        self.poll_log().await;
        self.inner.send_message_opened(hosted_in_ide).await
    }

    pub async fn send_message_current_file(&self, url: String) -> std::io::Result<f64> {
        self.poll_log().await;
        self.inner.send_message_current_file(url).await
    }

    // Used by some test targets but not others; each test binary compiles this
    // module separately, so it's dead code in the others.
    #[allow(dead_code)]
    pub async fn send_message_update_plain(
        &self,
        file_path: String,
        option_contents: Option<(String, f64)>,
        cursor_position: Option<u32>,
        scroll_position: Option<f64>,
    ) -> std::io::Result<f64> {
        self.poll_log().await;
        self.inner
            .send_message_update_plain(file_path, option_contents, cursor_position, scroll_position)
            .await
    }

    pub async fn send_result(
        &self,
        id: f64,
        message_result: Option<ResultErrTypes>,
    ) -> std::io::Result<()> {
        self.poll_log().await;
        self.inner.send_result(id, message_result).await
    }

    pub async fn send_result_loadfile(
        &self,
        id: f64,
        load_file: Option<(String, f64)>,
    ) -> std::io::Result<()> {
        self.poll_log().await;
        self.inner.send_result_loadfile(id, load_file).await
    }
}

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

    pub fn check(&mut self, editor_message: &EditorMessage) {
        if let Some((ref mut editor_message_contents, is_dynamic)) =
            self.0.remove(&(editor_message.id as i64))
        {
            if is_dynamic
                && let EditorMessageContents::Update(emc) = editor_message_contents
                && let Some(contents) = &mut emc.contents
            {
                let version = get_version(editor_message);
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

    async fn _assert_message(
        &mut self,
        codechat_server: &CodeChatEditorServerLog,
        timeout: Duration,
    ) {
        self.check(&codechat_server.get_message_timeout(timeout).await.unwrap());
    }

    pub async fn assert_all_messages(
        &mut self,
        codechat_server: &CodeChatEditorServerLog,
        timeout: Duration,
    ) {
        while !self.0.is_empty() {
            if let Some(editor_message) = codechat_server.get_message_timeout(timeout).await {
                self.check(&editor_message);
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
pub const TIMEOUT: Duration = Duration::from_secs(15);

// Browser-backed tests share a single WebDriver endpoint. Safari on macOS CI is
// unreliable with overlapping sessions, so serialize the harness.
pub(crate) static WEB_DRIVER_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

// ### Test harness
//
// A test harness. It runs the webdriver, the Server, opens the Client, then
// runs provided tests. After testing finishes, it cleans up (handling panics
// properly).
pub async fn harness<
    F: FnOnce(CodeChatEditorServerLog, WebDriver, PathBuf) -> Fut,
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
    // Tell chromedriver to capture page-side `console.*` output and uncaught
    // JavaScript errors in the `browser` log buffer, which we drain below and
    // forward to Rust logging. Without this capability the `/log` endpoint
    // returns nothing regardless of what the page does.
    caps.set_browser_log_level(LoggingPrefsLogLevel::All)?;

    // Debug support:
    //
    // Comment/uncomment these out to debug test failures.
    caps.add_arg("--headless")?;
    // See [SO](https://stackoverflow.com/questions/78996364/chrome-129-headless-shows-blank-window) -- this prevents a blank windows popping up for each test. Tested with Chrome version 150.0.7871.47 (Official Build) (64-bit).
    caps.add_arg("--window-position=-2400,-2400")?;
    //caps.add_arg("--auto-open-devtools-for-tabs")?;
    // Insert the code in a test to pause it for manual inspection.
    //use std::time::Duration;
    //use tokio::time::sleep;
    //sleep(Duration::from_hours(1)).await;

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
        // Wrap the server so every call the test framework makes also drains
        // the browser's JavaScript console log (see `CodeChatEditorServerLog`).
        let codechat_server = CodeChatEditorServerLog::new(
            CodeChatEditorServer::new().unwrap(),
            driver_clone.clone(),
        );

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
        let address_end = client_html[address_start..].find('"').unwrap() + address_start;
        let address = &client_html[address_start..address_end];

        // Open the Client and send it a file to load.
        driver_clone.goto(address).await.unwrap();
        let test_result = f(codechat_server, driver_clone.clone(), test_dir).await;

        // Drain any JavaScript console output captured during the test and
        // forward it to Rust logging, then propagate the test's result. Do this
        // even when the test failed, since the console output often explains
        // the failure.
        forward_browser_logs(&driver_clone).await;

        test_result?;
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

/// Decode a `BrowserLogEntry::message` produced by a `console.*` call.
/// chromedriver formats these as `<source-url> <line>:<column> <serialized
/// message>`, where the serialized message is the JSON encoding of each
/// argument passed to `console.*` (strings included, hence the embedded
/// quotes/escapes). Strip the `<source-url> <line>:<column>` prefix and decode
/// the remainder, falling back to the raw message if it doesn't match the
/// expected shape.
fn decode_console_message(message: &str) -> String {
    // Split off the `<source-url> <line>:<column>` prefix: the first two
    // whitespace-separated fields.
    let mut parts = message.splitn(3, ' ');
    let (Some(_source_url), Some(_line_col), Some(serialized)) =
        (parts.next(), parts.next(), parts.next())
    else {
        return message.to_string();
    };

    // The serialized message is a whitespace-separated sequence of JSON-encoded
    // values (one per `console.*` argument). Decode each, rendering strings
    // without their surrounding quotes and falling back to the raw text for
    // anything that isn't valid JSON.
    let mut decoded_parts = Vec::new();
    let mut deserializer = serde_json::Deserializer::from_str(serialized).into_iter::<Value>();
    for value in &mut deserializer {
        match value {
            Ok(Value::String(s)) => decoded_parts.push(s),
            Ok(other) => decoded_parts.push(other.to_string()),
            Err(_) => return message.to_string(),
        }
    }
    // If there's leftover, unparsed text, give up and return the raw message.
    if !serialized[deserializer.byte_offset()..].trim().is_empty() {
        return message.to_string();
    }

    decoded_parts.join(" ")
}

/// Drain the browser's `console.*` / uncaught-error log buffer, re-emit each
/// entry through the Rust `tracing` macros (mapping the Selenium log level to
/// the closest Rust log level), and return the drained entries so callers can
/// inspect them. Requires `set_browser_log_level(...)` to have been set on the
/// capabilities used to start the driver (see `harness`).
///
/// Errors fetching the log are ignored: `get_log("browser")` is a legacy,
/// non-W3C endpoint, and a failure to read it should never fail a test. In that
/// case an empty `Vec` is returned.
async fn forward_browser_logs(driver: &WebDriver) -> Vec<BrowserLogEntry> {
    let entries = match driver.get_log("browser").await {
        Ok(entries) => entries,
        Err(err) => {
            debug!("Unable to read browser console log: {err}");
            return Vec::new();
        }
    };
    for entry in &entries {
        // chromedriver emits SCREAMING levels (`SEVERE`, `WARNING`, `INFO`,
        // `DEBUG`, `FINE`, ...). Map them onto Rust log levels.
        let msg = format!(
            "JS console [{}]: {}",
            entry.level,
            decode_console_message(&entry.message)
        );
        match entry.level.as_str() {
            "SEVERE" => error!("{msg}"),
            "WARNING" => warn!("{msg}"),
            "INFO" | "CONFIG" => info!("{msg}"),
            // FINE/FINER/FINEST/DEBUG and anything else.
            _ => debug!("{msg}"),
        }
    }
    entries
}

#[macro_export]
macro_rules! make_test {
    // The name of the test function to call inside the harness.
    ($test_name: ident, $test_core_name: ident) => {
        #[tokio::test]
        #[tracing::instrument]
        async fn $test_name() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            $crate::common::harness($test_core_name, prep_test_dir!()).await
        }
    };

    // Same as above, but for a test that's currently known to fail (a
    // regression test pinning down an unfixed bug). `harness` converts a panic
    // into a `Result::Err` (see its `catch_unwind` use, needed to shut the
    // WebDriver down cleanly), so the failure never becomes a live unwind --
    // meaning `#[should_panic]` can't detect it. `#[ignore]` is the
    // alternative: the test is skipped by default (so the suite stays green)
    // but still compiles, and can be run explicitly with `cargo test --
    // --ignored` to check whether the bug has been fixed yet.
    ($test_name: ident, $test_core_name: ident, ignore = $reason: literal) => {
        #[tokio::test]
        #[tracing::instrument]
        #[ignore = $reason]
        async fn $test_name() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            $crate::common::harness($test_core_name, prep_test_dir!()).await
        }
    };
}

// Given an `Update` message with contents, get the version.
pub fn get_version(msg: &EditorMessage) -> f64 {
    let ccfw = cast!(&msg.message, EditorMessageContents::Update)
        .contents
        .as_ref();
    ccfw.unwrap_or_else(|| panic!("No contents in message:\n{msg:#?}"))
        .version
}

// Used in one of the common tests, but not in the other...so we get a clippy
// lint.
#[allow(dead_code)]
#[tracing::instrument(skip_all)]
pub async fn goto_line(
    codechat_server: &CodeChatEditorServerLog,
    driver_ref: &WebDriver,
    client_id: &mut f64,
    path_str: &str,
    line: u32,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let code_line_css = ".CodeChat-CodeMirror .cm-line";
    let code_line = driver_ref
        .query(By::Css(code_line_css))
        .first()
        .await
        .unwrap();
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
    // autosave delay. Sometimes, we get an update or two just before the
    // movement; ignore those (up to 2 of them).
    let mut msg = codechat_server.get_message_timeout(TIMEOUT).await.unwrap();
    let mut ignored = 0;
    while ignored < 2
        && msg.id == *client_id
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
        ignored += 1;
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

// Cursor-navigation helpers
// -------------------------
//
// On macOS, key combinations that are supposed to jump to the beginning/end of
// a document (`Ctrl+Home`, `Ctrl+End`, `Cmd+Up`, `Cmd+Down`, ...) are handled
// by macOS's native Cocoa/WebKit text-editing bridge rather than by pure
// JavaScript `keydown`/`keyup` DOM events. ChromeDriver's `send_keys` only
// injects DOM-level key events, so on macOS these shortcuts are silently
// swallowed before they ever produce the OS-level cursor jump. This is a
// longstanding WebDriver/Selenium limitation, not a bug in this project -- see
// [Command key modifier doesn't work on Mac OS](https://github.com/SeleniumHQ/selenium/issues/1290).
//
// `Ctrl+Home`/`Ctrl+End` do work via `send_keys` on Windows/Linux for a plain
// CodeMirror line, but empirically do *not* reliably reach the true
// beginning/end inside a TinyMCE `contenteditable` doc block (observed landing
// partway through the block instead of at the first/last paragraph, even on
// Windows) -- presumably TinyMCE's own keyboard-shortcut handling intercepts or
// only partially handles them. So rather than branch by OS,
// \[`beginning_of_document`\] and \[`end_of_document`\] always use the
// repeated-arrow-key approach below, which works uniformly on every platform
// and in both CodeMirror and TinyMCE elements.
//
// Each helper accepts `keys_after`, sent in the same `send_keys` call as the
// navigation keys (rather than a separate `send_keys` call afterward): each
// `send_keys` call can produce its own debounced cursor-update message, so
// splitting one logical action into two calls can surface an extra, unwanted
// intermediate update.

// Move the cursor to the beginning of the current line, then send `keys_after`
// (which may be empty). Plain `Home` (no modifier) is handled by the browser
// itself on every platform, so no OS-specific workaround is needed here.
#[allow(dead_code)]
#[tracing::instrument(skip(element))]
pub async fn beginning_of_line(
    element: &WebElement,
    keys_after: impl Into<TypingData> + std::fmt::Debug,
) -> Result<(), WebDriverError> {
    element.send_keys(Key::Home + keys_after).await
}

// Move the cursor to the end of the current line, then send `keys_after` (which
// may be empty). Plain `End` (no modifier) is handled by the browser itself on
// every platform, so no OS-specific workaround is needed here.
#[allow(dead_code)]
#[tracing::instrument(skip(element))]
pub async fn end_of_line(
    element: &WebElement,
    keys_after: impl Into<TypingData> + std::fmt::Debug,
) -> Result<(), WebDriverError> {
    element.send_keys(Key::End + keys_after).await
}

// Move the cursor to the beginning of the document, then send `keys_after`
// (which may be empty). See the module-level comment above for why this presses
// `Up` enough times to reach line 1 from anywhere within a test fixture's
// (small) document, rather than using `Ctrl+Home`/`Cmd+Up`.
#[allow(dead_code)]
#[tracing::instrument(skip(element))]
pub async fn beginning_of_document(
    element: &WebElement,
    keys_after: impl Into<TypingData> + std::fmt::Debug,
) -> Result<(), WebDriverError> {
    // Test fixtures used by this suite are well under this many lines;
    // repeating `Up` past the first line is a no-op, so an overshoot is
    // harmless. Sent as a single `send_keys` call, along with `keys_after`, so
    // this produces one cursor update rather than one per repeated key.
    let keys: TypingData = repeated_key(&Key::Up, MAX_TEST_DOCUMENT_LINES) + keys_after;
    element.send_keys(keys).await
}

// Move the cursor to the end of the document, then send `keys_after` (which may
// be empty). See the module-level comment above for why this presses `Down`
// enough times to reach the last line from anywhere within a test fixture's
// (small) document (then `End` to reach the end of that line), rather than
// using `Ctrl+End`/`Cmd+Down`.
#[allow(dead_code)]
#[tracing::instrument(skip(element))]
pub async fn end_of_document(
    element: &WebElement,
    keys_after: impl Into<TypingData> + std::fmt::Debug,
) -> Result<(), WebDriverError> {
    let keys: TypingData =
        repeated_key(&Key::Down, MAX_TEST_DOCUMENT_LINES) + Key::End + keys_after;
    element.send_keys(keys).await
}

// Build a `TypingData` consisting of `key` repeated `count` times, for the
// macOS repeated-arrow-key workaround in \[`beginning_of_document`\] and
// \[`end_of_document`\].
fn repeated_key(key: &Key, count: u32) -> TypingData {
    std::iter::repeat_n(key.value(), count as usize)
        .collect::<String>()
        .into()
}

// An upper bound on the number of lines in any document used by this test
// suite's fixtures, used by the macOS `Up`/`Down`-repeating workaround in
// \[`beginning_of_document`\] and \[`end_of_document`\] above.
const MAX_TEST_DOCUMENT_LINES: u32 = 100;

pub async fn perform_loadfile(
    codechat_server: &CodeChatEditorServerLog,
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

/// Click near the top-left corner of `element`. By default, `click()` selects
/// the middle of an element; we want to start at the first line, so use an
/// action chain to offset from the middle (the origin used by
/// `move_to_element_with_offset`) toward the top left.
///
/// Note that the offset must be computed from the element's `width`/`height`. A
/// few pixels of inset is also added so the click lands just inside the element
/// rather than on its border or in any surrounding padding.
#[allow(dead_code)]
pub async fn click_element_top_left(
    driver_ref: &WebDriver,
    element: &WebElement,
) -> Result<(), WebDriverError> {
    let element_size = element.rect().await?;
    driver_ref
        .action_chain()
        .move_to_element_with_offset(
            element,
            (-element_size.width / 2.0 + 8.0) as i64,
            (-element_size.height / 2.0 + 8.0) as i64,
        )
        .click()
        .perform()
        .await
}

#[allow(deprecated)]
pub async fn select_codechat_iframe(driver_ref: &WebDriver) -> WebElement {
    // Target the iframe containing the Client.
    let codechat_iframe = driver_ref
        .query(By::Css("#CodeChat-iframe"))
        .first()
        .await
        .unwrap();
    codechat_iframe.clone().enter_frame().await.unwrap();

    codechat_iframe
}

pub async fn assert_no_more_messages(codechat_server: &CodeChatEditorServerLog) {
    if let Some(msg) = codechat_server
        .get_message_timeout(Duration::from_millis(500))
        .await
    {
        panic!("Unprocessed messages: {msg:#?}");
    }
}

/// Wait for a message. If it matches the provided optional message, acknowledge
/// it and update the client ID, then wait for another message. Return the most
/// recently received message.
#[allow(dead_code)]
#[tracing::instrument(skip_all)]
pub async fn optional_message(
    codechat_server: &CodeChatEditorServerLog,
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
