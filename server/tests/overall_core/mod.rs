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
/// ============================================================================
///
/// These are functional tests of the overall system, performed by attaching a
/// testing IDE to generate commands then observe results, along with a browser
/// tester.
///
// Imports
// -----------------------------------------------------------------------------
//
// ### Standard library
use std::{
    env,
    error::Error,
    panic::AssertUnwindSafe,
    path::{Path, PathBuf},
    time::Duration,
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
use crate::{
    make_test,
    overall_common::{
        ExpectedMessages, TIMEOUT, assert_no_more_messages, get_empty_client_update, get_version,
        goto_line, perform_loadfile, select_codechat_iframe,
    },
};
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

// Tests
// -----------------------------------------------------------------------------
//
// ### Server-side test
//
// Perform most functions a user would: open/switch documents (Markdown,
// CodeChat, plain, PDF), use hyperlinks, perform edits on code and doc blocks.
make_test!(test_server, test_server_core);

// Some of the thirtyfour calls are marked as deprecated, though they aren't
// marked that way in the Selenium docs.
#[allow(deprecated)]
async fn test_server_core(
    codechat_server: CodeChatEditorServer,
    driver_ref: &WebDriver,
    test_dir: &Path,
) -> Result<(), WebDriverError> {
    let mut expected_messages = ExpectedMessages::new();
    let path = canonicalize(test_dir.join("test.py")).unwrap();
    let path_str = path.to_str().unwrap().to_string();
    let mut version = 1.0;
    let mut server_id = perform_loadfile(
        &codechat_server,
        test_dir,
        "test.py",
        Some(("# Test\ncode()".to_string(), version)),
        true,
        6.0,
    )
    .await;

    // ### Tests on source code
    //
    // #### Doc block tests
    //
    // Verify the first doc block.
    let codechat_iframe = select_codechat_iframe(driver_ref).await;
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
    // The click produces an updated cursor/scroll location after an autosave
    // delay.
    let mut client_id = INITIAL_CLIENT_MESSAGE_ID;
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
    client_id += MESSAGE_ID_INCREMENT;

    // Refind it, since it's now switched with a TinyMCE editor.
    let tinymce_contents = driver_ref.find(By::Id("TinyMCE-inst")).await.unwrap();
    // Make an edit.
    tinymce_contents.send_keys("foo").await.unwrap();

    // Verify the updated text.
    //
    // Update the version from the value provided by the client, which varies
    // randomly.
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
    client_id += MESSAGE_ID_INCREMENT;

    // Edit the indent. It should only allow spaces and tabs, rejecting other
    // edits.
    doc_block_indent.send_keys("  123").await.unwrap();
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
    client_id += MESSAGE_ID_INCREMENT;

    // #### Code block tests
    //
    // Verify the first line of code.
    let code_line_css = ".CodeChat-CodeMirror .cm-line";
    let code_line = driver_ref.find(By::Css(code_line_css)).await.unwrap();
    assert_eq!(code_line.inner_html().await.unwrap(), "code()");

    // A click will update the current position and focus the code block.
    code_line.click().await.unwrap();
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
    client_id += MESSAGE_ID_INCREMENT;

    // Moving left should move us back to the doc block.
    code_line
        .send_keys("" + Key::Home + Key::Left)
        .await
        .unwrap();
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
    client_id += MESSAGE_ID_INCREMENT;

    // Make an edit to the code. This should also produce a client diff.
    code_line.send_keys("bar").await.unwrap();

    // Verify the updated text.
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
    client_id += MESSAGE_ID_INCREMENT;

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
        "<p>Testfood</p>"
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
        "<p>food</p>"
    );
    let code_line = driver_ref.find(By::Css(code_line_css)).await.unwrap();
    assert_eq!(code_line.inner_html().await.unwrap(), "bark");

    // ### Document-only tests
    let toc_path = canonicalize(test_dir.join("toc.md")).unwrap();
    server_id = perform_loadfile(
        &codechat_server,
        test_dir,
        "test.md",
        Some(("A **markdown** file.".to_string(), version)),
        true,
        server_id,
    )
    .await;

    // Check the content.
    let body_css = "#CodeChat-body .CodeChat-doc-contents";
    let body_content = driver_ref.find(By::Css(body_css)).await.unwrap();
    assert_eq!(
        body_content.inner_html().await.unwrap(),
        "<p>A <strong>markdown</strong> file.</p>"
    );

    // Perform edits.
    body_content.send_keys("foo ").await.unwrap();
    let md_path = canonicalize(test_dir.join("test.md")).unwrap();
    let md_path_str = md_path.to_str().unwrap().to_string();
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
    client_id += MESSAGE_ID_INCREMENT;

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
    server_id += MESSAGE_ID_INCREMENT;
    // There's a second request for this file, made by the iframe, plus a
    // request for the TOC. The ordering isn't fixed; accommodate this.
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
    server_id += MESSAGE_ID_INCREMENT;

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
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::CurrentFile(pdf_path_str, Some(true))
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();
    //client_id += MESSAGE_ID_INCREMENT;
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
    //server_id += MESSAGE_ID_INCREMENT;

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

    assert_no_more_messages(&codechat_server).await;

    Ok(())
}

// ### Client tests
//
// This simply runs client-side tests written in TypeScript, verifying that they
// all pass.
make_test!(test_client, test_client_core);

// Some of the thirtyfour calls are marked as deprecated, though they aren't
// marked that way in the Selenium docs.
#[allow(deprecated)]
async fn test_client_core(
    codechat_server: CodeChatEditorServer,
    driver_ref: &WebDriver,
    test_dir: &Path,
) -> Result<(), WebDriverError> {
    let mut server_id =
        perform_loadfile(&codechat_server, test_dir, "test.py", None, true, 6.0).await;
    let path = canonicalize(test_dir.join("test.py")).unwrap();
    let path_str = path.to_str().unwrap().to_string();

    // Target the iframe containing the Client.
    let codechat_iframe = select_codechat_iframe(driver_ref).await;

    // Click on the link for the PDF to test.
    let toc_iframe = driver_ref.find(By::Css("#CodeChat-sidebar")).await.unwrap();
    driver_ref
        .switch_to()
        .frame_element(&toc_iframe)
        .await
        .unwrap();
    let test_py = driver_ref.find(By::LinkText("test.py")).await.unwrap();
    test_py.click().await.unwrap();

    // Respond to the current file, then load requests for the PDF and the TOC.
    let client_id = INITIAL_CLIENT_MESSAGE_ID;
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::CurrentFile(path_str, Some(true))
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();
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
    let toc_path = canonicalize(test_dir.join("toc.md")).unwrap();
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

    server_id -= MESSAGE_ID_INCREMENT;
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: server_id,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );
    //server_id += 2.0 * MESSAGE_ID_INCREMENT;

    assert_no_more_messages(&codechat_server).await;

    Ok(())
}

make_test!(test_client_updates, test_client_updates_core);

async fn test_client_updates_core(
    codechat_server: CodeChatEditorServer,
    driver_ref: &WebDriver,
    test_dir: &Path,
) -> Result<(), WebDriverError> {
    let mut ide_version = 0.0;
    let orig_text = indoc!(
        "
        # Test updates in the client that modify the client after appending to a line.
        def foo():
            A comment
            print()
        "
    )
    .to_string();
    let mut server_id = perform_loadfile(
        &codechat_server,
        test_dir,
        "test.py",
        Some((orig_text.clone(), ide_version)),
        true,
        6.0,
    )
    .await;

    let path = canonicalize(test_dir.join("test.py")).unwrap();
    let path_str = path.to_str().unwrap().to_string();

    // Target the iframe containing the Client.
    select_codechat_iframe(driver_ref).await;

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
    client_id += MESSAGE_ID_INCREMENT;

    // The Server sends the Client a wrapped version of the text; the Client
    // replies with a Result(Ok).
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: server_id,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );
    server_id += MESSAGE_ID_INCREMENT;

    goto_line(&codechat_server, driver_ref, &mut client_id, &path_str, 4)
        .await
        .unwrap();

    // Add an indented comment.
    let code_line_css = ".CodeChat-CodeMirror .cm-line";
    let code_line = driver_ref.find(By::Css(code_line_css)).await.unwrap();
    code_line.send_keys(Key::Home + "# ").await.unwrap();
    // This should edit the (new) third line of the file after word wrap: `def
    // foo():`.
    let msg = codechat_server.get_message_timeout(TIMEOUT).await.unwrap();
    let new_client_version = get_version(&msg);
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
                            from: 100,
                            to: Some(114),
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
    client_id += MESSAGE_ID_INCREMENT;

    // The Server sends the Client a re-translated version of the text with the new doc block; the Client
    // replies with a Result(Ok).
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: server_id,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );
    //server_id += MESSAGE_ID_INCREMENT;

    // Send the original text back, to ensure the re-translation correctly updated the Client.
    ide_version = 1.0;
    let ide_id = codechat_server
        .send_message_update_plain(
            path_str.clone(),
            Some((orig_text, ide_version)),
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

    // Trigger a client edit to send the Client contents back.
    let code_line = driver_ref.find(By::Css(code_line_css)).await.unwrap();
    code_line.send_keys(" ").await.unwrap();

    let msg = codechat_server.get_message_timeout(TIMEOUT).await.unwrap();
    let new_client_version = get_version(&msg);
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
                            to: Some(90),
                            insert: "def foo(): \n".to_string()
                        }],
                        doc_blocks: vec![],
                        version: ide_version,
                    }),
                    version: new_client_version,
                }),
                cursor_position: Some(2),
                scroll_position: Some(1.0)
            })
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();

    assert_no_more_messages(&codechat_server).await;

    Ok(())
}

make_test!(test_4, test_4_core);

async fn test_4_core(
    codechat_server: CodeChatEditorServer,
    driver_ref: &WebDriver,
    test_dir: &Path,
) -> Result<(), WebDriverError> {
    let path = canonicalize(test_dir.join("test.py")).unwrap();
    let path_str = path.to_str().unwrap().to_string();
    let ide_version = 0.0;
    perform_loadfile(
        &codechat_server,
        test_dir,
        "test.py",
        Some((
            indoc!(
                "
                    # 1
                    2
                    # 3
                    4
                    # 5
                    "
            )
            .to_string(),
            ide_version,
        )),
        false,
        6.0,
    )
    .await;

    // Target the iframe containing the Client.
    select_codechat_iframe(driver_ref).await;

    // Switch from one doc block to another. It should produce an update with only cursor/scroll info (no contents).
    let mut client_id = INITIAL_CLIENT_MESSAGE_ID;
    let doc_blocks = driver_ref.find_all(By::Css(".CodeChat-doc")).await.unwrap();
    doc_blocks[0].click().await.unwrap();
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
    client_id += MESSAGE_ID_INCREMENT;

    doc_blocks[1].click().await.unwrap();
    let mut client_version = 0.0;
    get_empty_client_update(
        &codechat_server,
        &path_str,
        &mut client_id,
        &mut client_version,
        "python",
        3,
        1.0,
    )
    .await;

    doc_blocks[2].click().await.unwrap();
    get_empty_client_update(
        &codechat_server,
        &path_str,
        &mut client_id,
        &mut client_version,
        "python",
        5,
        1.0,
    )
    .await;

    assert_no_more_messages(&codechat_server).await;

    Ok(())
}

make_test!(test_5, test_5_core);

// Verify that newlines in Mermaid and Graphviz diagrams aren't removed.
async fn test_5_core(
    codechat_server: CodeChatEditorServer,
    driver_ref: &WebDriver,
    test_dir: &Path,
) -> Result<(), WebDriverError> {
    let path = canonicalize(test_dir.join("test.py")).unwrap();
    let path_str = path.to_str().unwrap().to_string();
    let version = 0.0;
    let orig_text = indoc!(
        "
        # Test.
        #
        # ```graphviz
        # digraph g {
        #   A -> B
        # }
        # ```
        #
        # ```mermaid
        # graph TD
        #   A --> B
        # ```
        "
    )
    .to_string();
    let mut server_id = perform_loadfile(
        &codechat_server,
        test_dir,
        "test.py",
        Some((orig_text.clone(), version)),
        false,
        6.0,
    )
    .await;

    // Target the iframe containing the Client.
    select_codechat_iframe(driver_ref).await;

    // Focus it.
    let contents_css = ".CodeChat-CodeMirror .CodeChat-doc-contents";
    let doc_block_contents = driver_ref.find(By::Css(contents_css)).await.unwrap();
    doc_block_contents.click().await.unwrap();
    // The click produces an updated cursor/scroll location after an autosave
    // delay.
    let mut client_id = INITIAL_CLIENT_MESSAGE_ID;
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
    client_id += MESSAGE_ID_INCREMENT;

    // Refind it, since it's now switched with a TinyMCE editor.
    let tinymce_contents = driver_ref.find(By::Id("TinyMCE-inst")).await.unwrap();
    // Make an edit.
    tinymce_contents.send_keys("foo").await.unwrap();

    // Verify the updated text.
    //
    // Update the version from the value provided by the client, which varies
    // randomly.
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
                            to: Some(8),
                            insert: "# fooTest.\n".to_string(),
                        },],
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
    let version = client_version;
    codechat_server.send_result(client_id, None).await.unwrap();
    client_id += MESSAGE_ID_INCREMENT;

    // The Server sends the Client a wrapped version of the text; the Client
    // replies with a Result(Ok).
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: server_id,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );
    server_id += MESSAGE_ID_INCREMENT;

    // Send new text, which turns into a diff.
    let ide_id = codechat_server
        .send_message_update_plain(path_str.clone(), Some((orig_text, version)), Some(1), None)
        .await
        .unwrap();
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: ide_id,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );

    // Make another edit, to push any corrupted text back.
    tinymce_contents.send_keys("bar").await.unwrap();
    // Verify the updated text.
    //
    // Update the version from the value provided by the client, which varies
    // randomly.
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
                            to: Some(8),
                            insert: "# Tesbart.\n".to_string(),
                        },],
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
    //let version = client_version;
    codechat_server.send_result(client_id, None).await.unwrap();
    //client_id += MESSAGE_ID_INCREMENT;

    // The Server sends the Client a wrapped version of the text; the Client
    // replies with a Result(Ok).
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: server_id,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );
    //server_id += MESSAGE_ID_INCREMENT;

    assert_no_more_messages(&codechat_server).await;

    Ok(())
}

make_test!(test_6, test_6_core);

// Verify that edits in document-only mode don't result in data corruption.
async fn test_6_core(
    codechat_server: CodeChatEditorServer,
    driver_ref: &WebDriver,
    test_dir: &Path,
) -> Result<(), WebDriverError> {
    let path = canonicalize(test_dir.join("test.md")).unwrap();
    let path_str = path.to_str().unwrap().to_string();
    let version = 0.0;
    let orig_text = indoc!(
        "
        * a

        b
        "
    )
    .to_string();
    perform_loadfile(
        &codechat_server,
        test_dir,
        "test.md",
        Some((orig_text.clone(), version)),
        false,
        6.0,
    )
    .await;

    // Target the iframe containing the Client.
    select_codechat_iframe(driver_ref).await;

    // Check the content.
    let body_css = "#CodeChat-body .CodeChat-doc-contents";
    let body_content = driver_ref.find(By::Css(body_css)).await.unwrap();

    // Perform edits.
    body_content.send_keys("a").await.unwrap();
    let client_id = INITIAL_CLIENT_MESSAGE_ID;
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
                        mode: "markdown".to_string(),
                    },
                    source: CodeMirrorDiffable::Diff(CodeMirrorDiff {
                        doc: vec![StringDiff {
                            from: 0,
                            to: Some(4),
                            insert: "* aa\n".to_string(),
                        },],
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
    let version = client_version;
    codechat_server.send_result(client_id, None).await.unwrap();
    //client_id += MESSAGE_ID_INCREMENT;

    // Send new text, which turns into a diff.
    let ide_id = codechat_server
        .send_message_update_plain(
            path_str.clone(),
            Some((
                indoc!(
                    "
                    * aaa

                    b
                    "
                )
                .to_string(),
                version,
            )),
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
    //ide_id += MESSAGE_ID_INCREMENT;

    // Verify the updated text.
    assert_eq!(
        body_content.inner_html().await.unwrap(),
        "<ul><li>aaa</li></ul><p>b</p>"
    );

    assert_no_more_messages(&codechat_server).await;

    Ok(())
}
