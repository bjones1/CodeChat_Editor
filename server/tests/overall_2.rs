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
// Modules
// -----------------------------------------------------------------------------
mod overall_common;

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
    By, ChromiumLikeCapabilities, DesiredCapabilities, WebDriver, error::WebDriverError,
    start_webdriver_process,
};
use tokio::time::sleep;

// ### Local
use crate::overall_common::{
    TIMEOUT, assert_no_more_messages, get_empty_client_update, get_version, perform_loadfile,
    select_codechat_iframe,
};
use code_chat_editor::{
    ide::CodeChatEditorServer,
    processing::{
        CodeChatForWeb, CodeMirrorDiff, CodeMirrorDiffable, SourceFileMetadata, StringDiff,
    },
    webserver::{
        EditorMessage, EditorMessageContents, INITIAL_CLIENT_MESSAGE_ID, MESSAGE_ID_INCREMENT,
        ResultOkTypes, UpdateMessageContents, set_root_path,
    },
};
use test_utils::{cast, prep_test_dir};

make_test!(test_4, test_4_core);

// Tests
// -----------------------------------------------------------------------------
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
        Some(3),
        Some(1.0),
    )
    .await;

    doc_blocks[2].click().await.unwrap();
    get_empty_client_update(
        &codechat_server,
        &path_str,
        &mut client_id,
        &mut client_version,
        "python",
        Some(5),
        Some(1.0),
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
                            insert: "# foobarTest.\n".to_string(),
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
