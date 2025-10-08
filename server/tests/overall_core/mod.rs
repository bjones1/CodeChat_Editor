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
/// without that feature's crates enabled.
///
/// This is implemented here, then `use`d in `overall.rs`, so that a single
/// `#[cfg(feature = "int_tests")]` statement there gates everything in this
/// file. See the [test
/// docs](https://doc.rust-lang.org/book/ch11-03-test-organization.html#submodules-in-integration-tests)
/// for the correct file and directory names.
// Imports
// -------
//
// ### Standard library
use std::{env, error::Error, panic::AssertUnwindSafe, time::Duration};

// ### Third-party
use dunce::canonicalize;
use futures::FutureExt;
use pretty_assertions::assert_eq;
use thirtyfour::prelude::*;
use tokio::time::sleep;

// ### Local
use code_chat_editor::{
    cast,
    ide::CodeChatEditorServer,
    prep_test_dir,
    processing::{
        CodeChatForWeb, CodeMirrorDiff, CodeMirrorDiffable, SourceFileMetadata, StringDiff,
    },
    webserver::{
        EditorMessage, EditorMessageContents, INITIAL_CLIENT_MESSAGE_ID, MESSAGE_ID_INCREMENT,
        ResultOkTypes, UpdateMessageContents, set_root_path,
    },
};

// Tests
// -----
//
// Some of the thirtyfour calls are marked as deprecated, though they aren't
// marked that way in the Selenium docs.
#[allow(deprecated)]
#[tokio::test]
pub async fn thirtyfour() -> Result<(), Box<dyn Error + Send + Sync>> {
    let (temp_dir, test_dir) = prep_test_dir!();
    // The logger gets configured by (I think) `start_webdriver_process`, which
    // delegates to `selenium-manager`. Set logging level here.
    unsafe { env::set_var("RUST_LOG", "debug") };
    // Start the webdriver.
    let server_url = "http://localhost:4444";
    let mut caps = DesiredCapabilities::chrome();
    caps.add_arg("--headless")?;
    // On Ubuntu CI, avoid failures, probably due to running Chrome as root.
    #[cfg(target_os = "linux")]
    if env::var("CI") == Ok("true".to_string()) {
        caps.add_arg("--disable-gpu")?;
        caps.add_arg("--no-sandbox")?;
    }
    if let Err(err) = start_webdriver_process(server_url, &caps) {
        // Often, the "failure" is that the webdriver is already running.
        eprintln!("Failed to start the webdriver process: {err:#?}");
    }
    let driver = WebDriver::new(server_url, caps).await?;
    let driver_ref = &driver;

    // Run the test inside an async, so we can shut down the driver before
    // returning an error. Mark the function as unwind safe. though I'm not
    // certain this is correct. Hopefully, it's good enough for testing.
    let ret = AssertUnwindSafe(async move {
        let p = env::current_exe().unwrap().parent().unwrap().join("../..");
        set_root_path(Some(&p)).unwrap();
        let codechat_server = CodeChatEditorServer::new().unwrap();

        // Get the resulting web page text.
        let timeout = Duration::from_millis(2000);
        let opened_id = codechat_server.send_message_opened(true).await.unwrap();
        assert_eq!(
            codechat_server
                .get_message_timeout(timeout)
                .await
                .expect("Expected message."),
            EditorMessage {
                id: opened_id,
                message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
            }
        );
        let em_html = codechat_server
            .get_message_timeout(timeout)
            .await
            .expect("Expected message.");
        codechat_server.send_result(em_html.id, None).await.unwrap();

        // Parse out the address to use.
        let client_html = cast!(&em_html.message, EditorMessageContents::ClientHtml);
        let find_str = "<iframe src=\"";
        let address_start = client_html.find(find_str).unwrap() + find_str.len();
        let address_end = client_html[address_start..].find("\"").unwrap() + address_start;
        let address = &client_html[address_start..address_end];

        // Open the Client and send it a file to load.
        driver_ref.goto(address).await.unwrap();
        let path = canonicalize(test_dir.join("test.py")).unwrap();
        let path_str = path.to_str().unwrap().to_string();
        let current_file_id = codechat_server
            .send_message_current_file(path_str.clone())
            .await
            .unwrap();
        // These next two messages can come in either order. Work around this.
        let msg1 = codechat_server
            .get_message_timeout(timeout)
            .await
            .expect("Expected message.");
        let msg2 = codechat_server
            .get_message_timeout(timeout)
            .await
            .expect("Expected message.");
        let msg1_expected = EditorMessage {
            id: current_file_id,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void)),
        };
        let id = 6.0;
        let msg2_expected = EditorMessage {
            id,
            message: EditorMessageContents::LoadFile(path),
        };
        assert!(
            (msg1 == msg1_expected && msg2 == msg2_expected)
                || (msg1 == msg2_expected && msg2 == msg1_expected)
        );

        // Respond to the load request.
        codechat_server
            .send_result_loadfile(id, Some("# Test\ncode()".to_string()))
            .await
            .unwrap();
        // The loadfile produces a message to the client, which comes back here.
        // We don't need to acknowledge it.
        let id = id + MESSAGE_ID_INCREMENT;
        assert_eq!(
            codechat_server
                .get_message_timeout(timeout)
                .await
                .expect("Expected message."),
            EditorMessage {
                id,
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

        // Check the first doc block.
        let doc_block_indent = driver_ref
            .find(By::Css(".CodeChat-CodeMirror .CodeChat-doc-indent"))
            .await
            .unwrap();
        assert_eq!(doc_block_indent.inner_html().await.unwrap(), "");
        let doc_block_contents = driver_ref
            .find(By::Css(".CodeChat-CodeMirror .CodeChat-doc-contents"))
            .await
            .unwrap();
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

        // Check for updated text.
        let client_id = INITIAL_CLIENT_MESSAGE_ID;
        assert_eq!(
            codechat_server
                .get_message_timeout(timeout)
                .await
                .expect("Expected message."),
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
                            doc_blocks: vec![]
                        })
                    }),
                    cursor_position: Some(1),
                    scroll_position: Some(0.0)
                })
            }
        );
        codechat_server.send_result(client_id, None).await.unwrap();

        // Check the first line of code.
        let code_line = driver_ref
            .find(By::Css(".CodeChat-CodeMirror .cm-line"))
            .await
            .unwrap();
        assert_eq!(code_line.inner_html().await.unwrap(), "code()");

        // Make an edit to the code.
        code_line.send_keys("bar").await.unwrap();

        // Check for updated text.
        let client_id = client_id + MESSAGE_ID_INCREMENT;
        assert_eq!(
            codechat_server
                .get_message_timeout(timeout)
                .await
                .expect("Expected message."),
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
                                from: 10,
                                to: Some(16),
                                insert: "code()bar".to_string()
                            }],
                            doc_blocks: vec![]
                        })
                    }),
                    cursor_position: Some(2),
                    scroll_position: Some(0.0)
                })
            }
        );
        codechat_server.send_result(client_id, None).await.unwrap();

        // Make an edit to the doc block.

        // Click the search button.
        //elem_button.click().await?;

        // Look for header to implicitly wait for the page to load.
        /*driver_ref
            .query(By::ClassName("firstHeading"))
            .first()
            .await.unwrap();
        assert_eq!(driver_ref.title().await?, "Selenium - Wikipedia");*/

        sleep(std::time::Duration::from_secs(1)).await;

        Ok(())
    })
    // Also catch any panics/assertions, again to ensure the driver shuts down
    // cleanly.
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
