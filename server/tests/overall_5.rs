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
/// `overall_5.rs` - test the overall system
/// ========================================
///
/// These are functional tests of the overall system, performed by attaching a
/// testing IDE to generate commands then observe results, along with a browser
/// tester.
///
/// To run this test, execute `cargo test --test overall_5 <optional_test_name>`
/// in the `server/` directory.
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
use pretty_assertions::assert_eq;
use thirtyfour::{By, Key, WebDriver, error::WebDriverError};

// ### Local
use crate::overall_common::{
    CodeChatEditorServerLog, TIMEOUT, assert_no_more_messages, beginning_of_line,
    click_element_top_left, end_of_line, perform_loadfile, select_codechat_iframe,
};
use code_chat_editor::webserver::{
    CursorPosition, EditorMessage, EditorMessageContents, INITIAL_CLIENT_MESSAGE_ID,
    MESSAGE_ID_INCREMENT, UpdateMessageContents,
};
use test_utils::prep_test_dir;

// Tests
// -----
make_test!(
    test_cursor_home_from_code_after_doc_block,
    test_cursor_home_from_code_after_doc_block_core
);

// Regression test to ensure left arrow allows placing the cursor at the
// beginning of a code block, then moves back to the end of the preceding doc
// block on another left arrow press. Also checks (currently failing) that
// pressing Home at the end of the code block keeps the cursor on that line
// rather than moving up into the preceding doc block.
async fn test_cursor_home_from_code_after_doc_block_core(
    codechat_server: CodeChatEditorServerLog,
    driver: WebDriver,
    test_dir: PathBuf,
) -> Result<(), WebDriverError> {
    let path = canonicalize(test_dir.join("test.py")).unwrap();
    let path_str = path.to_str().unwrap().to_string();
    let ide_version = 0.0;
    // Source lines, and the CodeMirror line each becomes on the Client: "# a"
    // (doc block, one-character contents "a") -> 1, "b" (code block,
    // one-character contents) -> 2.
    let orig_text = "# a\nb\n".to_string();
    perform_loadfile(
        &codechat_server,
        &test_dir,
        "test.py",
        Some((orig_text, ide_version)),
        false,
        6.0,
    )
    .await;

    // Target the iframe containing the Client.
    select_codechat_iframe(&driver).await;

    let mut client_id = INITIAL_CLIENT_MESSAGE_ID;

    // Click on the one-character code block ("b"), which focuses CodeMirror and
    // reports the cursor at line 2.
    let code_line = driver
        .find(By::XPath("//*[contains(@class, 'cm-line')][text()='b']"))
        .await
        .unwrap();
    click_element_top_left(&driver, &code_line).await.unwrap();
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                cursor_position: Some(CursorPosition::Line(2)),
                scroll_position: Some(1.0),
                is_re_translation: false,
                contents: None,
            })
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();
    client_id += MESSAGE_ID_INCREMENT;

    // Move to the end of the line first, so the following `Home` press is a
    // real cursor movement rather than a no-op (the click above may already
    // land at the line's start for a one-character line).
    end_of_line(&code_line, "").await.unwrap();
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                cursor_position: Some(CursorPosition::Line(2)),
                scroll_position: Some(1.0),
                is_re_translation: false,
                contents: None,
            })
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();
    client_id += MESSAGE_ID_INCREMENT;

    // For now, send a left arrow instead of the home key.
    code_line.send_keys(Key::Left).await.unwrap();

    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                cursor_position: Some(CursorPosition::Line(2)),
                scroll_position: Some(1.0),
                is_re_translation: false,
                contents: None,
            })
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();
    client_id += MESSAGE_ID_INCREMENT;

    // Move to the end of the line again, then press `Home` via the
    // `beginning_of_line` helper, to check for a regression: the cursor
    // should stay on the current line rather than jumping up into the
    // preceding doc block.
    end_of_line(&code_line, "").await.unwrap();
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                cursor_position: Some(CursorPosition::Line(2)),
                scroll_position: Some(1.0),
                is_re_translation: false,
                contents: None,
            })
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();
    client_id += MESSAGE_ID_INCREMENT;

    beginning_of_line(&code_line, "").await.unwrap();
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                cursor_position: Some(CursorPosition::Line(2)),
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
