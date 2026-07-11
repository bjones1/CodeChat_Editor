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
    CodeChatEditorServerLog, TIMEOUT, assert_no_more_messages, beginning_of_line, end_of_line,
    get_version, perform_loadfile, select_codechat_iframe,
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
make_test!(
    test_edit_preserves_cursor_scroll_in_large_doc_block,
    test_edit_preserves_cursor_scroll_in_large_doc_block_core
);

// Regression/stress test for the scroll-preservation logic when editing long doc blocks. This test builds a large document -- a big
// (100-paragraph) doc block, followed by 100 alternating one-character
// code/doc block pairs -- edits a paragraph in the middle of the big doc
// block, and checks that both the reported cursor and scroll position are
// unchanged afterward.
async fn test_edit_preserves_cursor_scroll_in_large_doc_block_core(
    codechat_server: CodeChatEditorServerLog,
    driver: WebDriver,
    test_dir: PathBuf,
) -> Result<(), WebDriverError> {
    let path = canonicalize(test_dir.join("test.rs")).unwrap();
    let path_str = path.to_str().unwrap().to_string();
    let ide_version = 0.0;
    let version = ide_version;

    // A one-line doc block (plain `//` comment).
    let mut orig_text = "// L1\n/// Pt\n/// ==\n///\n".to_string();
    // A 100-paragraph doc block (rustdoc-style `///` comments).
    // Since the delimiter (`///`) differs from the preceding block's (`//`),
    // these lines don't merge into the first doc block. This also produces an empty line at the end of these paragraphs, which will cause a server re-translation when it cleans this up.
    for i in 0..100 {
        orig_text += &format!("/// P{i}\n///\n");
    }
    // 100 instances of a one-line code block, then a one-line doc block.
    for i in 0..100 {
        orig_text += &format!("{i}\n// {i}\n");
    }

    let server_id = perform_loadfile(
        &codechat_server,
        &test_dir,
        "test.rs",
        Some((orig_text, ide_version)),
        false,
        6.0,
    )
    .await;

    // Target the iframe containing the Client.
    select_codechat_iframe(&driver).await;

    let mut client_id = INITIAL_CLIENT_MESSAGE_ID;

    // The 100-paragraph doc block is the second `.CodeChat-doc-contents` div
    // (the first is the single-line "L1" doc block).
    let doc_blocks = driver
        .find_all(By::Css(".CodeChat-CodeMirror .CodeChat-doc-contents"))
        .await
        .unwrap();
    let big_doc_block = &doc_blocks[1];

    // Click a paragraph in the middle of the big doc block, to focus it and
    // place the cursor there.
    let paragraphs = big_doc_block.find_all(By::Css("p")).await.unwrap();
    assert_eq!(
        paragraphs.len(),
        100,
        "Expected 100 paragraphs in the big doc block."
    );
    let middle_paragraph = &paragraphs[50];
    middle_paragraph.click().await.unwrap();

    // The click produces an updated cursor/scroll location after an autosave
    // delay. Record it as the baseline to compare against after the edit.
    let msg_before = codechat_server.get_message_timeout(TIMEOUT).await.unwrap();
    assert_eq!(msg_before.id, client_id);
    let UpdateMessageContents {
        cursor_position: cursor_position_before,
        scroll_position: scroll_position_before,
        ..
    } = match msg_before.message {
        EditorMessageContents::Update(contents) => contents,
        other => panic!("Expected an Update message, got: {other:#?}"),
    };
    codechat_server.send_result(client_id, None).await.unwrap();
    client_id += MESSAGE_ID_INCREMENT;

    // Make an edit in the middle of the big doc block: refind the paragraph,
    // since it's now switched to a TinyMCE editor, then type a character.
    let tinymce_contents = driver.find(By::Id("TinyMCE-inst")).await.unwrap();
    tinymce_contents.send_keys("x").await.unwrap();

    // A cursor-only update (carrying no `contents`) may precede the text
    // update carrying the edit; accept and skip any of those, then inspect
    // the first update that does carry contents.
    let msg = loop {
        let msg = codechat_server.get_message_timeout(TIMEOUT).await.unwrap();
        assert_eq!(msg.id, client_id);
        let is_cursor_only = matches!(
            &msg.message,
            EditorMessageContents::Update(UpdateMessageContents { contents: None, .. })
        );
        if !is_cursor_only {
            break msg;
        }
        codechat_server.send_result(client_id, None).await.unwrap();
        client_id += MESSAGE_ID_INCREMENT;
    };
    // use std::time::Duration;
    // use tokio::time::sleep;
    // sleep(Duration::from_hours(1)).await;

    let client_version = get_version(&msg);
    assert_eq!(
        msg,
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                cursor_position: Some(CursorPosition::Line(105)),
                scroll_position: Some(1.0),
                is_re_translation: false,
                contents: Some(CodeChatForWeb {
                    metadata: SourceFileMetadata {
                        mode: "rust".to_string(),
                    },
                    source: CodeMirrorDiffable::Diff(CodeMirrorDiff {
                        doc: vec![
                            StringDiff {
                                from: 614,
                                to: Some(622),
                                insert: "/// P50x\n".to_string()
                            },
                            // The server removes the empty line after the last paragraph in the big doc block. This also causes a re-translation.
                            StringDiff {
                                from: 1210,
                                to: Some(1214),
                                insert: "".to_string()
                            }
                        ],
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

    // Editing a doc block prompts the Server to send the Client a
    // re-translated version of the document; the Client's acknowledgement
    // comes back here as a `Result(Ok)` carrying the Server's ID.
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: server_id,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );

    // The re-translation settles the cursor, producing a final cursor/scroll
    // update from the Client. Verify neither the cursor nor the scroll
    // position moved as a result of the edit.
    let msg_after = codechat_server.get_message_timeout(TIMEOUT).await.unwrap();
    assert_eq!(msg_after.id, client_id);
    let UpdateMessageContents {
        cursor_position: cursor_position_after,
        scroll_position: scroll_position_after,
        ..
    } = match msg_after.message {
        EditorMessageContents::Update(contents) => contents,
        other => panic!("Expected an Update message, got: {other:#?}"),
    };
    assert_eq!(
        cursor_position_after, cursor_position_before,
        "Cursor position changed after editing the middle of the large doc block."
    );
    assert_eq!(
        scroll_position_after, scroll_position_before,
        "Scroll position changed after editing the middle of the large doc block."
    );
    codechat_server.send_result(client_id, None).await.unwrap();
    //client_id += MESSAGE_ID_INCREMENT;

    assert_no_more_messages(&codechat_server).await;

    Ok(())
}

// Regression test to ensure left arrow allows placing the cursor at the
// beginning of a code block, then moves back to the end of the preceding doc
// block on another left arrow press. The preceding doc block here spans two
// source lines, to catch a manually-observed bug (currently failing) where
// that second left-arrow press lands the cursor at the doc block's
// *beginning* instead of its *end*. Also checks (currently failing) that
// pressing Home at the end of the code block keeps the cursor on that line
// rather than moving up into the preceding doc block, and that pressing Home
// a second time (a no-op, since the cursor is already at the start of the
// line) also keeps the cursor on the current line.
make_test!(
    test_cursor_home_from_code_after_doc_block,
    test_cursor_home_from_code_after_doc_block_core
);

async fn test_cursor_home_from_code_after_doc_block_core(
    codechat_server: CodeChatEditorServerLog,
    driver: WebDriver,
    test_dir: PathBuf,
) -> Result<(), WebDriverError> {
    let path = canonicalize(test_dir.join("test.py")).unwrap();
    let path_str = path.to_str().unwrap().to_string();
    let ide_version = 0.0;
    let orig_text = "# a<br>\n# b\ncc\n".to_string();
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

    // Click on the two-character code block ("cc"), which focuses CodeMirror
    // and reports the cursor at line 3. The click is in the middle of the element, which places the cursor at the end of the line (given that the width of the screen is much larger than the width of a two-character line.)
    let code_line = driver
        .find(By::XPath("//*[contains(@class, 'cm-line')][text()='cc']"))
        .await
        .unwrap();
    code_line.click().await.unwrap();
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

    // The cursor is at the end of the two "c"s. The first `Left` press should
    // simply move the cursor to the middle of the two "c"s, staying on the current
    // line rather than jumping into the preceding doc block.
    code_line.send_keys(Key::Left).await.unwrap();
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

    // The cursor is at middle the two "c"s. The next `Left` press should
    // move the cursor to the beginning of the two "c"s, staying on the current
    // line rather than jumping into the preceding doc block.
    code_line.send_keys(Key::Left).await.unwrap();
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

    // The cursor is now at the start of code line "cc". A final `Left` press
    // should enter the preceding two-line doc block, with the caret landing
    // at the block's *end* (per the "entering from below lands at the end"
    // rule documented on `docBlockNavKeymap`'s `ArrowLeft` handler), not its
    // start.
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

    // `Line(2)` only proves the caret is somewhere on the doc block's last
    // source line -- it can't distinguish that line's start from its end.
    // Independently confirm the DOM caret placement itself, mirroring the
    // check in `test_arrow_key_navigation_multiline_doc_block_core`
    // (`overall_4.rs`): the caret should sit at the very end of the doc
    // block's text -- after "b" -- not at its start.
    let is_caret_at_end: bool = driver
        .execute(
            "const contents = document.activeElement.closest('.CodeChat-doc-contents');
            if (!contents) return false;
            const sel = window.getSelection();
            if (sel.rangeCount === 0) return false;
            let last_text_node = contents;
            while (last_text_node.lastChild) {
                last_text_node = last_text_node.lastChild;
            }
            return (
                sel.anchorNode === last_text_node &&
                sel.anchorOffset === last_text_node.textContent.length
            );",
            Vec::new(),
        )
        .await
        .unwrap()
        .convert()
        .unwrap();
    assert!(
        is_caret_at_end,
        "ArrowLeft from code line \"cc\" should land the caret at the end of the \
         two-line doc block, not at its start."
    );

    // Move back into the code block for the remaining `Home` checks below.
    code_line.click().await.unwrap();
    end_of_line(&code_line, "").await.unwrap();
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

    // The cursor is already at the end of the line (from `end_of_line` above,
    // line 214), so press `Home` via the `beginning_of_line` helper directly,
    // to check for a regression: the cursor should stay on the current line
    // rather than jumping up into the preceding doc block.
    beginning_of_line(&code_line, "").await.unwrap();
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

    // Press `Home` a second time. The cursor is already at the beginning of
    // the line, so this should be a no-op that keeps the cursor on the
    // current line -- not a jump up into the preceding doc block.
    beginning_of_line(&code_line, "").await.unwrap();
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
    //client_id += MESSAGE_ID_INCREMENT;

    assert_no_more_messages(&codechat_server).await;

    Ok(())
}
