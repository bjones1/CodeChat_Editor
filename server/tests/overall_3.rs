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
/// `overall_3.rs` - test the overall system
/// ========================================
///
/// These are functional tests of the overall system, performed by attaching a
/// testing IDE to generate commands then observe results, along with a browser
/// tester.
// Modules
// -------
mod overall_common;

// Imports
// -------
//
// ### Standard library
use std::path::PathBuf;

// ### Third-party
use dunce::canonicalize;
use indoc::indoc;
use pretty_assertions::assert_eq;
use thirtyfour::{By, Key, WebDriver, error::WebDriverError, extensions::query::ElementQueryable};

// ### Local
use crate::overall_common::{
    CodeChatEditorServerLog, TIMEOUT, assert_no_more_messages, beginning_of_document,
    beginning_of_line, click_element_top_left, get_version, optional_message, perform_loadfile,
    select_codechat_iframe,
};
use code_chat_editor::{
    processing::{
        CodeChatForWeb, CodeMirrorDiff, CodeMirrorDiffable, SourceFileMetadata, StringDiff,
    },
    webserver::{
        CursorPosition, EditorMessage, EditorMessageContents, INITIAL_CLIENT_MESSAGE_ID,
        MESSAGE_ID_INCREMENT, ResultOkTypes, UpdateMessageContents,
    },
};
use test_utils::prep_test_dir;

// Tests
// -----
make_test!(test_7, test_7_core);

// Test that Client to IDE cursor sync in doc blocks works.
async fn test_7_core(
    codechat_server: CodeChatEditorServerLog,
    driver: WebDriver,
    test_dir: PathBuf,
) -> Result<(), WebDriverError> {
    let path = canonicalize(test_dir.join("test.py")).unwrap();
    let path_str = path.to_str().unwrap().to_string();
    let ide_version = 0.0;
    perform_loadfile(
        &codechat_server,
        &test_dir,
        "test.py",
        Some((
            indoc!(
                "
                    # 1<br>
                    # 2
                    #
                    # 4
                    #
                    # 6
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
    select_codechat_iframe(&driver).await;

    // Focus the doc block. It should produce an update with only cursor/scroll
    // info (no contents). Click on the contents specifically (rather than the
    // whole doc block, which also contains the indent) so the click reliably
    // lands inside the text regardless of the indent's width, which varies
    // with its `contenteditable` state.
    let mut client_id = INITIAL_CLIENT_MESSAGE_ID;
    let doc_block = driver
        .find(By::Css(".CodeChat-doc-contents"))
        .await
        .unwrap();
    click_element_top_left(&driver, &doc_block).await.unwrap();
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                cursor_position: Some(CursorPosition::Line(1)),
                scroll_position: Some(1.0),
                is_re_translation: false,
                contents: None,
            })
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();
    client_id += MESSAGE_ID_INCREMENT;

    // Refind it, since it's now switched with a TinyMCE editor.
    let tinymce_contents = driver.find(By::Id("TinyMCE-inst")).await.unwrap();

    // Move to the next lines.
    for expected_line in [2, 4, 6] {
        tinymce_contents.send_keys(Key::Down).await.unwrap();

        assert_eq!(
            codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
            EditorMessage {
                id: client_id,
                message: EditorMessageContents::Update(UpdateMessageContents {
                    file_path: path_str.clone(),
                    cursor_position: Some(CursorPosition::Line(expected_line)),
                    scroll_position: Some(1.0),
                    is_re_translation: false,
                    contents: None,
                })
            }
        );
        codechat_server.send_result(client_id, None).await.unwrap();
        client_id += MESSAGE_ID_INCREMENT;
    }

    assert_no_more_messages(&codechat_server).await;

    Ok(())
}

make_test!(test_8, test_8_core);

// Test that Clients can insert a new paragraph; do so at the beginning of a doc block, in the middle of a doc block, and at the end of a doc block.
async fn test_8_core(
    codechat_server: CodeChatEditorServerLog,
    driver: WebDriver,
    test_dir: PathBuf,
) -> Result<(), WebDriverError> {
    let path = canonicalize(test_dir.join("test.py")).unwrap();
    let path_str = path.to_str().unwrap().to_string();
    let ide_version = 0.0;
    let mut server_id = perform_loadfile(
        &codechat_server,
        &test_dir,
        "test.py",
        Some((
            indoc!(
                "
                # 2
                #
                # 4
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
    select_codechat_iframe(&driver).await;

    // Focus the doc block. It should produce an update with only cursor/scroll
    // info (no contents). Click on the contents specifically (rather than the
    // whole doc block, which also contains the indent) so the click reliably
    // lands inside the text regardless of the indent's width, which varies
    // with its `contenteditable` state.
    let mut client_id = INITIAL_CLIENT_MESSAGE_ID;
    let doc_block = driver
        .find(By::Css(".CodeChat-doc-contents"))
        .await
        .unwrap();
    click_element_top_left(&driver, &doc_block).await.unwrap();

    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                cursor_position: Some(CursorPosition::Line(1)),
                scroll_position: Some(1.0),
                is_re_translation: false,
                contents: None,
            })
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();
    client_id += MESSAGE_ID_INCREMENT;

    // Refind it, since it's now switched with a TinyMCE editor.
    let tinymce_contents = driver.find(By::Id("TinyMCE-inst")).await.unwrap();

    beginning_of_line(&tinymce_contents, "").await.unwrap();
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                cursor_position: Some(CursorPosition::Line(1)),
                scroll_position: Some(1.0),
                is_re_translation: false,
                contents: None,
            })
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();
    client_id += MESSAGE_ID_INCREMENT;

    // Start a new paragraph. Wait for a re-translation as the line changes.
    // The click above lands the cursor right at the start of "2" (the first
    // paragraph), so this splits it into an empty paragraph followed by "2".
    tinymce_contents.send_keys(Key::Enter).await.unwrap();

    let msg = optional_message(
        &codechat_server,
        &mut client_id,
        EditorMessageContents::Update(UpdateMessageContents {
            file_path: path_str.clone(),
            cursor_position: Some(CursorPosition::Line(1)),
            scroll_position: Some(1.0),
            is_re_translation: false,
            contents: None,
        }),
    )
    .await;
    let mut version = 0.0;
    let client_version = get_version(&msg);
    assert_eq!(
        msg,
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                cursor_position: Some(CursorPosition::Line(3)),
                scroll_position: Some(1.0),
                is_re_translation: false,
                contents: Some(CodeChatForWeb {
                    metadata: SourceFileMetadata {
                        mode: "python".to_string(),
                    },
                    source: CodeMirrorDiffable::Diff(CodeMirrorDiff {
                        doc: vec![StringDiff {
                            from: 0,
                            to: None,
                            insert: "# \u{a0}\n#\n".to_string(),
                        },],
                        doc_blocks: vec![],
                        version,
                    }),
                    version: client_version,
                }),
            })
        }
    );
    version = client_version;
    codechat_server.send_result(client_id, None).await.unwrap();
    client_id += MESSAGE_ID_INCREMENT;

    // There's a re-translation sent to the client, whose response comes back to
    // the IDE.
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: server_id,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );
    server_id += MESSAGE_ID_INCREMENT;

    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                cursor_position: Some(CursorPosition::Line(3)),
                scroll_position: Some(1.0),
                is_re_translation: false,
                contents: None,
            })
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();
    client_id += MESSAGE_ID_INCREMENT;

    // ### Insert a newline between two existing paragraphs
    //
    // After the previous edit, the doc block contains three paragraphs (an
    // empty one, "2", and "4"). Move up to the first (empty) paragraph
    // (producing a cursor-only update), then start a new paragraph there.
    // Wait for a re-translation as the lines change.
    beginning_of_document(&tinymce_contents, "").await.unwrap();
    tinymce_contents.send_keys(Key::Enter).await.unwrap();

    // The cursor move produces an optional cursor-only update before the
    // re-translation arrives.
    let msg = optional_message(
        &codechat_server,
        &mut client_id,
        EditorMessageContents::Update(UpdateMessageContents {
            file_path: path_str.clone(),
            cursor_position: Some(CursorPosition::Line(1)),
            scroll_position: Some(1.0),
            is_re_translation: false,
            contents: None,
        }),
    )
    .await;
    let client_version = get_version(&msg);
    assert_eq!(
        msg,
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                cursor_position: Some(CursorPosition::Line(1)),
                scroll_position: Some(1.0),
                is_re_translation: false,
                contents: Some(CodeChatForWeb {
                    metadata: SourceFileMetadata {
                        mode: "python".to_string(),
                    },
                    source: CodeMirrorDiffable::Diff(CodeMirrorDiff {
                        doc: vec![StringDiff {
                            from: 6,
                            to: None,
                            insert: "# \u{a0}\n#\n".to_string(),
                        },],
                        doc_blocks: vec![],
                        version,
                    }),
                    version: client_version,
                }),
            })
        }
    );
    version = client_version;
    codechat_server.send_result(client_id, None).await.unwrap();
    client_id += MESSAGE_ID_INCREMENT;

    // There's a re-translation sent to the client, whose response comes back to
    // the IDE.
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: server_id,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );
    server_id += MESSAGE_ID_INCREMENT;

    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                cursor_position: Some(CursorPosition::Line(1)),
                scroll_position: Some(1.0),
                is_re_translation: false,
                contents: None,
            })
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();
    client_id += MESSAGE_ID_INCREMENT;

    // ### Insert a newline at the end of the document
    //
    // Move to the end of the last paragraph, then start a new paragraph there.
    // Wait for a re-translation as the lines change.
    tinymce_contents
        .send_keys(Key::Down + Key::Down + Key::Down + Key::Down + Key::Down + Key::End)
        .await
        .unwrap();
    tinymce_contents.send_keys(Key::Enter).await.unwrap();

    let msg = optional_message(
        &codechat_server,
        &mut client_id,
        EditorMessageContents::Update(UpdateMessageContents {
            file_path: path_str.clone(),
            cursor_position: Some(CursorPosition::Line(1)),
            scroll_position: Some(1.0),
            is_re_translation: false,
            contents: None,
        }),
    )
    .await;
    let client_version = get_version(&msg);
    assert_eq!(
        msg,
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                cursor_position: Some(CursorPosition::Line(1)),
                scroll_position: Some(1.0),
                is_re_translation: false,
                contents: Some(CodeChatForWeb {
                    metadata: SourceFileMetadata {
                        mode: "python".to_string(),
                    },
                    source: CodeMirrorDiffable::Diff(CodeMirrorDiff {
                        doc: vec![StringDiff {
                            from: 22,
                            to: None,
                            insert: "#\n# \u{a0}\n".to_string(),
                        },],
                        doc_blocks: vec![],
                        version,
                    }),
                    version: client_version,
                }),
            })
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();
    client_id += MESSAGE_ID_INCREMENT;

    // There's a re-translation sent to the client, whose response comes back to
    // the IDE.
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: server_id,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );

    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                cursor_position: Some(CursorPosition::Line(1)),
                scroll_position: Some(1.0),
                is_re_translation: false,
                contents: None,
            })
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();
    //client_id += MESSAGE_ID_INCREMENT;

    assert_no_more_messages(&codechat_server).await;

    Ok(())
}

make_test!(test_9, test_9_core);

// Test that Clients can insert a new paragraph.
async fn test_9_core(
    codechat_server: CodeChatEditorServerLog,
    driver: WebDriver,
    test_dir: PathBuf,
) -> Result<(), WebDriverError> {
    let path = canonicalize(test_dir.join("test.py")).unwrap();
    let path_str = path.to_str().unwrap().to_string();
    let ide_version = 0.0;
    perform_loadfile(
        &codechat_server,
        &test_dir,
        "test.py",
        Some((
            indoc!(
                "
                # 1
                 # 2
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
    select_codechat_iframe(&driver).await;

    // Focus the doc block. It should produce an update with only cursor/scroll
    // info (no contents). Click on the contents specifically (rather than the
    // whole doc block, which also contains the indent) so the click reliably
    // lands inside the text regardless of the indent's width, which varies
    // with its `contenteditable` state.
    let mut client_id = INITIAL_CLIENT_MESSAGE_ID;
    let doc_block = driver
        .query(By::Css(".CodeChat-doc-contents"))
        .first()
        .await
        .unwrap();
    click_element_top_left(&driver, &doc_block).await.unwrap();

    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                cursor_position: Some(CursorPosition::Line(1)),
                scroll_position: Some(1.0),
                is_re_translation: false,
                contents: None,
            })
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();
    client_id += MESSAGE_ID_INCREMENT;

    // Refind it, since it's now switched with a TinyMCE editor.
    let tinymce_contents = driver.find(By::Id("TinyMCE-inst")).await.unwrap();

    // The click above doesn't necessarily land exactly at the start of the
    // text (its position now depends on the contents div's layout, not the
    // indent's), so explicitly move to the beginning of the line before
    // editing.
    beginning_of_line(&tinymce_contents, "").await.unwrap();
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                cursor_position: Some(CursorPosition::Line(1)),
                scroll_position: Some(1.0),
                is_re_translation: false,
                contents: None,
            })
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();
    client_id += MESSAGE_ID_INCREMENT;

    // Perform an edit
    tinymce_contents.send_keys("a").await.unwrap();

    let msg = optional_message(
        &codechat_server,
        &mut client_id,
        EditorMessageContents::Update(UpdateMessageContents {
            file_path: path_str.clone(),
            cursor_position: Some(CursorPosition::Line(1)),
            scroll_position: Some(1.0),
            is_re_translation: false,
            contents: None,
        }),
    )
    .await;
    let mut version = 0.0;
    let client_version = get_version(&msg);
    assert_eq!(
        msg,
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                cursor_position: Some(CursorPosition::Line(1)),
                scroll_position: Some(1.0),
                is_re_translation: false,
                contents: Some(CodeChatForWeb {
                    metadata: SourceFileMetadata {
                        mode: "python".to_string(),
                    },
                    source: CodeMirrorDiffable::Diff(CodeMirrorDiff {
                        doc: vec![StringDiff {
                            from: 0,
                            to: Some(4),
                            insert: "# a1\n".to_string(),
                        },],
                        doc_blocks: vec![],
                        version,
                    }),
                    version: client_version,
                }),
            })
        }
    );
    version = client_version;
    codechat_server.send_result(client_id, None).await.unwrap();
    client_id += MESSAGE_ID_INCREMENT;

    // Focus on the code block. Both source lines are doc blocks (kept
    // separate by their differing indents), so the only real `.cm-line` is
    // the trailing blank line CodeMirror renders after them -- line 3.
    let cm_line = driver.query(By::Css(".cm-line")).first().await.unwrap();
    cm_line.click().await.unwrap();

    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                cursor_position: Some(CursorPosition::Line(3)),
                scroll_position: Some(1.0),
                is_re_translation: false,
                contents: None,
            })
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();
    client_id += MESSAGE_ID_INCREMENT;

    // Add a character.
    cm_line.send_keys("3").await.unwrap();
    let msg = codechat_server.get_message_timeout(TIMEOUT).await.unwrap();
    let client_version = get_version(&msg);
    assert_eq!(
        msg,
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                cursor_position: Some(CursorPosition::Line(3)),
                scroll_position: Some(1.0),
                is_re_translation: false,
                contents: Some(CodeChatForWeb {
                    metadata: SourceFileMetadata {
                        mode: "python".to_string(),
                    },
                    source: CodeMirrorDiffable::Diff(CodeMirrorDiff {
                        doc: vec![StringDiff {
                            from: 10,
                            to: None,
                            insert: "3".to_string(),
                        },],
                        doc_blocks: vec![],
                        version,
                    }),
                    version: client_version,
                }),
            })
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();
    //client_id += MESSAGE_ID_INCREMENT;

    assert_no_more_messages(&codechat_server).await;

    Ok(())
}
