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
use std::{error::Error, path::PathBuf};

// ### Third-party
use dunce::canonicalize;
use indoc::indoc;
use pretty_assertions::assert_eq;
use thirtyfour::{By, Key, WebDriver, error::WebDriverError, prelude::ElementQueryable};

// ### Local
use crate::overall_common::{
    TIMEOUT, assert_no_more_messages, perform_loadfile, select_codechat_iframe,
};
use code_chat_editor::{
    ide::CodeChatEditorServer,
    webserver::{
        CursorPosition, EditorMessage, EditorMessageContents, INITIAL_CLIENT_MESSAGE_ID,
        MESSAGE_ID_INCREMENT, UpdateMessageContents,
    },
};
use test_utils::prep_test_dir;

// Tests
// -----
make_test!(test_7, test_7_core);

// Test that Client to IDE cursor sync in doc blocks works.
async fn test_7_core(
    codechat_server: CodeChatEditorServer,
    driver: WebDriver,
    test_dir: PathBuf,
) -> Result<(), WebDriverError> {
    let path = canonicalize(test_dir.join("test.py"))?;
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

    // Switch from one doc block to another. It should produce an update with
    // only cursor/scroll info (no contents).
    let mut client_id = INITIAL_CLIENT_MESSAGE_ID;
    let doc_block = driver.query(By::Css(".CodeChat-doc")).first().await?;
    let doc_block_size = doc_block.rect().await?;
    // By default, `click()` selects the middle of an element. We want to start at the first line, so use an action chain to offset from the middle to the top left.
    driver
        .action_chain()
        .move_to_element_with_offset(
            &doc_block,
            (-doc_block_size.x / 2.0 - 2.0) as i64,
            (-doc_block_size.y / 2.0 - 2.0) as i64,
        )
        .click()
        .perform()
        .await?;
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
    codechat_server.send_result(client_id, None).await?;
    client_id += MESSAGE_ID_INCREMENT;

    // Refind it, since it's now switched with a TinyMCE editor.
    let tinymce_contents = driver.find(By::Id("TinyMCE-inst")).await?;

    // Move to the next lines.
    for expeted_line in [2, 4, 6] {
        tinymce_contents.send_keys(Key::Down).await?;

        assert_eq!(
            codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
            EditorMessage {
                id: client_id,
                message: EditorMessageContents::Update(UpdateMessageContents {
                    file_path: path_str.clone(),
                    cursor_position: Some(CursorPosition::Line(expeted_line)),
                    scroll_position: Some(1.0),
                    is_re_translation: false,
                    contents: None,
                })
            }
        );
        codechat_server.send_result(client_id, None).await?;
        client_id += MESSAGE_ID_INCREMENT;
    }

    assert_no_more_messages(&codechat_server).await;

    Ok(())
}
