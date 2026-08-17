// Copyright (C) 2026 Bryan A. Jones.
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
//! `overall_5.rs` - test the overall system
//! ========================================
//!
//! These are functional tests of the overall system, performed by attaching a
//! testing IDE to generate commands then observe results, along with a browser
//! tester.
//!
//! To run this test, execute `cargo test --test overall <optional_test_name>`
//! in the `server/` directory.
// Imports
// -------
//
// ### Standard library
use std::{fmt::Write, path::PathBuf, time::Duration};

// ### Third-party
use dunce::canonicalize;
use pretty_assertions::assert_eq;
use thirtyfour::{
    By, Key, WebDriver,
    error::WebDriverError,
    prelude::{ElementQueryable, ElementWaitable},
};

// ### Local
use crate::common::{
    CodeChatEditorServerLog, DOC_BLOCK_CSS, TIMEOUT, assert_no_more_messages, beginning_of_line,
    click_element_top_left, end_of_line, get_version, optional_message, perform_loadfile,
    select_codechat_iframe,
};
use crate::make_test;
use code_chat_editor::{
    lexer::supported_languages::MARKDOWN_MODE,
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

// Regression/stress test for the scroll-preservation logic when editing long
// doc blocks. This test builds a large document -- a big (100-paragraph) doc
// block, followed by 100 alternating one-character code/doc block pairs --
// edits a paragraph in the middle of the big doc block, and checks that both
// the reported cursor and scroll position are unchanged afterward.
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
    // A 100-paragraph doc block (rustdoc-style `///` comments). Since the
    // delimiter (`///`) differs from the preceding block's (`//`), these lines
    // don't merge into the first doc block. This also produces an empty line at
    // the end of these paragraphs, which will cause a server re-translation
    // when it cleans this up.
    for i in 0..100 {
        let _ = write!(orig_text, "/// P{i}\n///\n");
    }
    // 100 instances of a one-line code block, then a one-line doc block.
    for i in 0..100 {
        let _ = write!(orig_text, "{i}\n// {i}\n");
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
    let tinymce_contents = driver
        .query(By::Css(
            ".CodeChat-CodeMirror #TinyMCE-inst:not(.CodeChat-doc-hidden)",
        ))
        .first()
        .await
        .unwrap();
    tinymce_contents.wait_until().clickable().await.unwrap();
    tinymce_contents.send_keys("x").await.unwrap();

    // A cursor-only update (carrying no `contents`) may precede the text update
    // carrying the edit; accept and skip any of those, then inspect the first
    // update that does carry contents.
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
    // use std::time::Duration; use tokio::time::sleep;
    // sleep(Duration::from\_hours(1)).await;

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
                            // The server removes the empty line after the last
                            // paragraph in the big doc block. This also causes
                            // a re-translation.
                            StringDiff {
                                from: 1210,
                                to: Some(1214),
                                insert: String::new()
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

    // Editing a doc block prompts the Server to send the Client a re-translated
    // version of the document; the Client's acknowledgement comes back here as
    // a `Result(Ok)` carrying the Server's ID.
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: server_id,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );

    // The re-translation settles the cursor, producing a final cursor/scroll
    // update from the Client. Verify neither the cursor nor the scroll position
    // moved as a result of the edit.
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
// source lines, to catch a manually-observed bug (currently failing) where that
// second left-arrow press lands the cursor at the doc block's *beginning*
// instead of its *end*. Also checks (currently failing) that pressing Home at
// the end of the code block keeps the cursor on that line rather than moving up
// into the preceding doc block, and that pressing Home a second time (a no-op,
// since the cursor is already at the start of the line) also keeps the cursor
// on the current line.
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
    // and reports the cursor at line 3. The click is in the middle of the
    // element, which places the cursor at the end of the line (given that the
    // width of the screen is much larger than the width of a two-character
    // line.)
    let code_line = driver
        .query(By::XPath("//*[contains(@class, 'cm-line')][text()='cc']"))
        .first()
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
    // simply move the cursor to the middle of the two "c"s, staying on the
    // current line rather than jumping into the preceding doc block.
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

    // The cursor is at middle the two "c"s. The next `Left` press should move
    // the cursor to the beginning of the two "c"s, staying on the current line
    // rather than jumping into the preceding doc block.
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
    // should enter the preceding two-line doc block, with the caret landing at
    // the block's *end* (per the "entering from below lands at the end" rule
    // documented on `docBlockNavKeymap`'s `ArrowLeft` handler), not its start.
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
    // Independently confirm the DOM caret placement itself, mirroring the check
    // in `test_arrow_key_navigation_multiline_doc_block_core` (`overall_4.rs`):
    // the caret should sit at the very end of the doc block's text -- after "b"
    // -- not at its start.
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

    // Press `Home` a second time. The cursor is already at the beginning of the
    // line, so this should be a no-op that keeps the cursor on the current line
    // -- not a jump up into the preceding doc block.
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

// Regression test: a nested list created inside an existing list must survive
// the autosave which immediately follows.
//
// Pressing `Enter` then `Tab` at the end of a list item is the standard way to
// begin a sub-list; TinyMCE responds by nesting a new, still-empty list item
// inside the current one (`<li>Item one<ul><li><br></li></ul></li>`). The
// autosave that follows sends that HTML to the Server, which translates it to
// Markdown, then re-translates the result back to the Client. Before the empty
// blocks `empty_block_needs_placeholder` lists (see
// [processing.rs](../../src/processing.rs)) were given a placeholder, the empty
// nested item survived neither leg: the Markdown became `* Item one *`, so the
// re-translation replaced the nested list with a stray `*` appended to the
// parent item's text -- wiping out the sub-list the user just created, before
// they could type anything into it.
//
// This test drives that sequence and checks the document the user is left with;
// it deliberately doesn't pin down the exact Markdown produced, since more than
// one encoding of an empty nested item is reasonable.
//
// The other empty blocks TinyMCE can create -- empty items elsewhere in a list,
// empty block quotes, table cells, headings, and paragraphs -- are covered
// without a browser by `test_empty_block_round_trip` in
// [processing/tests.rs](../../src/processing/tests.rs).
make_test!(test_nested_list_creation, test_nested_list_creation_core);

async fn test_nested_list_creation_core(
    codechat_server: CodeChatEditorServerLog,
    driver: WebDriver,
    test_dir: PathBuf,
) -> Result<(), WebDriverError> {
    let path = canonicalize(test_dir.join("test.md")).unwrap();
    let path_str = path.to_str().unwrap().to_string();
    let version = 0.0;
    let orig_text = "*   Item one\n*   Item two\n".to_string();
    let server_id = perform_loadfile(
        &codechat_server,
        &test_dir,
        "test.md",
        Some((orig_text, version)),
        false,
        6.0,
    )
    .await;

    // Target the iframe containing the Client.
    select_codechat_iframe(&driver).await;

    // Click into the list, which places the caret at the start of the first
    // item and switches the doc block to a TinyMCE editor.
    let body_content = driver.query(By::Css(DOC_BLOCK_CSS)).first().await.unwrap();
    click_element_top_left(&driver, &body_content)
        .await
        .unwrap();
    let client_id = INITIAL_CLIENT_MESSAGE_ID;
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                cursor_position: Some(CursorPosition::Line(1)),
                scroll_position: None,
                is_re_translation: false,
                contents: None,
            })
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();
    // The remaining messages are acknowledged by ID in the drain loop below,
    // rather than by tracking the expected ID here.
    //client_id += MESSAGE_ID_INCREMENT;

    // Refind the editable contents, since the click switched them to a TinyMCE
    // editor, then create a sub-list under the first item: `End` to reach the
    // end of "Item one", `Enter` for a new item, `Tab` to indent it. Send them
    // as one `send_keys` call, so this produces a single autosave rather than
    // one per key.
    let body_content = driver.query(By::Css(DOC_BLOCK_CSS)).first().await.unwrap();
    body_content
        .send_keys(Key::End + Key::Enter + Key::Tab)
        .await
        .unwrap();

    // The premise of this test: TinyMCE nests a new list inside the first item.
    // This runs before the autosave round trip completes, so it sees the
    // document as TinyMCE built it. If a TinyMCE upgrade changes how `Tab`
    // indents a list item, this assertion fails first, distinguishing that from
    // the round-trip bug the assertions below check for.
    assert!(
        has_nested_list(&driver).await,
        "Expected `Enter` then `Tab` to nest a new list inside the first item: {}",
        doc_block_html(&driver).await
    );

    // Acknowledge messages until the Client goes quiet. Both the number of
    // messages and their order vary here (a cursor-only update can precede or
    // follow the update carrying the edit, and the Server's re-translation adds
    // an acknowledgement of its own), and this test's assertions are about the
    // document that results, not about the message sequence -- so accept
    // whatever arrives, keeping the last update which carried contents.
    let mut last_contents_update: Option<EditorMessage> = None;
    // Whether the Client acknowledged the Server's re-translation.
    let mut re_translation_acknowledged = false;
    let mut timeout = TIMEOUT;
    while let Some(msg) = codechat_server.get_message_timeout(timeout).await {
        match &msg.message {
            EditorMessageContents::Update(update) => {
                let has_contents = update.contents.is_some();
                codechat_server.send_result(msg.id, None).await.unwrap();
                if has_contents {
                    last_contents_update = Some(msg);
                }
            }
            // The Client's acknowledgement of the Server's re-translation,
            // which carries the Server's ID rather than the Client's; it needs
            // no reply. Any re-translation will do, so compare against the first
            // ID the Server can use rather than requiring exactly one.
            EditorMessageContents::Result(Ok(ResultOkTypes::Void)) => {
                assert!(
                    msg.id >= server_id,
                    "Expected the acknowledgement of a re-translation from the Server."
                );
                re_translation_acknowledged = true;
            }
            other => panic!("Unexpected message: {other:#?}"),
        }
        // Only the first message is worth a full wait; after that, a gap this
        // long means the round trip has settled.
        timeout = QUIESCENT_TIMEOUT;
    }

    // Both legs of the round trip must have actually happened before the
    // document is worth checking. Without these two assertions, a test in which
    // the edit never reached the Server -- or the Server's re-translation never
    // reached the Client -- would inspect the document TinyMCE built and pass no
    // matter what the Server does with an empty nested item.
    let last_update = last_contents_update.unwrap_or_else(|| {
        panic!("The Client sent no update carrying contents, so the edit never reached the Server.")
    });
    assert!(
        re_translation_acknowledged,
        "The Client never acknowledged a re-translation from the Server, so the document below \
         is the one TinyMCE built rather than the round trip's result.\nLast update carrying \
         contents: {last_update:#?}"
    );

    // The sub-list must still be there after the round trip. Failing this is
    // the bug: the Server's re-translation replaced it with a literal `*` in
    // the first item's text.
    assert!(
        has_nested_list(&driver).await,
        "The nested list was removed by the round trip through the Server.\n\
         Document: {}\nLast update carrying contents: {last_update:#?}",
        doc_block_html(&driver).await
    );

    // The Markdown sent to the IDE is what a save writes to the file: creating
    // an empty nested item must not append a list marker to the item above it.
    // (A fix which doesn't save the empty nested item at all is acceptable,
    // hence checking only the Markdown that was actually sent. The document
    // itself needs no equivalent check, since it's built from this Markdown.)
    let EditorMessageContents::Update(update) = &last_update.message else {
        unreachable!("Only an update is stored above.");
    };
    let source_text: String = match &update.contents.as_ref().unwrap().source {
        CodeMirrorDiffable::Diff(diff) => diff.doc.iter().map(|d| d.insert.as_str()).collect(),
        CodeMirrorDiffable::Plain(plain) => plain.doc.clone(),
    };
    assert!(
        !source_text.contains("Item one *"),
        "The Markdown sent to the IDE appends the nested item's list marker to the \
         item above it: {source_text:?}"
    );

    Ok(())
}

// Support for `test_nested_list_creation`
// ---------------------------------------
//
// A list nested inside the first item of the doc block's list.
const NESTED_LIST_CSS: &str = "#CodeChat-body .CodeChat-doc-contents > ul > li > ul";

// How long to wait for a further message once the Client has started
// responding: long enough to cover the gap between the messages one edit
// produces, short enough to keep the test quick once they stop.
const QUIESCENT_TIMEOUT: Duration = Duration::from_secs(2);

// Whether the document currently contains a nested list.
async fn has_nested_list(driver: &WebDriver) -> bool {
    !driver
        .find_all(By::Css(NESTED_LIST_CSS))
        .await
        .unwrap()
        .is_empty()
}

// The doc block's HTML, for use in assertion failure messages.
async fn doc_block_html(driver: &WebDriver) -> String {
    driver
        .query(By::Css(DOC_BLOCK_CSS))
        .first()
        .await
        .unwrap()
        .inner_html()
        .await
        .unwrap()
}

make_test!(
    test_named_anchor_round_trip,
    test_named_anchor_round_trip_core
);

// Regression test: an empty named anchor (`<a id="notes"></a>`, the pattern the
// manual uses to give a section a stable link target) must survive an edit
// unchanged. TinyMCE's anchor plugin marks every such anchor
// `contenteditable="false"` when it parses a document, and removes that mark
// only in its serializer -- which the Client bypasses by saving in TinyMCE's raw
// format. Without the Server dropping the attribute during dehydration (see
// `remove_tinymce_data` in [processing.rs](../../src/processing.rs)), editing
// the document writes it into the source file.
async fn test_named_anchor_round_trip_core(
    codechat_server: CodeChatEditorServerLog,
    driver: WebDriver,
    test_dir: PathBuf,
) -> Result<(), WebDriverError> {
    let path = canonicalize(test_dir.join("test.md")).unwrap();
    let path_str = path.to_str().unwrap().to_string();
    let version = 0.0;
    let orig_text = "<a id=\"notes\"></a>Notes\n-----------------------\n".to_string();
    perform_loadfile(
        &codechat_server,
        &test_dir,
        "test.md",
        Some((orig_text, version)),
        false,
        6.0,
    )
    .await;

    // Target the iframe containing the Client.
    select_codechat_iframe(&driver).await;

    // The premise of this test: TinyMCE marks the anchor non-editable in the
    // rendered document. If a TinyMCE upgrade drops that behavior, this
    // assertion fails first, and the Server-side workaround it forces can be
    // revisited.
    let body_content = driver.query(By::Css(DOC_BLOCK_CSS)).first().await.unwrap();
    let rendered = body_content.inner_html().await.unwrap();
    assert!(
        rendered.contains("contenteditable"),
        "Expected TinyMCE to mark the named anchor non-editable: {rendered}"
    );

    // Click into the heading, then type a character there. The Client converts
    // the edited HTML back to source and sends it to the IDE as an `Update`;
    // since the heading's text changed, the diff it carries spans the line the
    // anchor is on.
    click_element_top_left(&driver, &body_content)
        .await
        .unwrap();
    let mut client_id = INITIAL_CLIENT_MESSAGE_ID;
    assert_eq!(
        codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                cursor_position: Some(CursorPosition::Line(1)),
                scroll_position: None,
                is_re_translation: false,
                contents: None,
            })
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();
    client_id += MESSAGE_ID_INCREMENT;

    // Refind the editable contents, since the click switched them to a TinyMCE
    // editor.
    let body_content = driver.query(By::Css(DOC_BLOCK_CSS)).first().await.unwrap();
    body_content.send_keys("z").await.unwrap();

    // A cursor-only update may precede the text update; accept it, then inspect
    // the text update.
    let msg = optional_message(
        &codechat_server,
        &mut client_id,
        EditorMessageContents::Update(UpdateMessageContents {
            file_path: path_str.clone(),
            cursor_position: Some(CursorPosition::Line(1)),
            scroll_position: None,
            is_re_translation: false,
            contents: None,
        }),
    )
    .await;
    let client_version = get_version(&msg);
    // The click places the caret at the start of the heading, so the typed
    // character precedes the anchor. It lengthens the heading, so the underline
    // beneath it grows by one character as well. Critically, the anchor itself
    // is unchanged: it carries no `contenteditable` attribute, even though the
    // HTML the Client sent (see the `mce-item-anchor` class in this test's log)
    // does.
    assert_eq!(
        msg,
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.clone(),
                cursor_position: Some(CursorPosition::Line(1)),
                scroll_position: None,
                is_re_translation: false,
                contents: Some(CodeChatForWeb {
                    metadata: SourceFileMetadata {
                        mode: MARKDOWN_MODE.to_string(),
                    },
                    source: CodeMirrorDiffable::Diff(CodeMirrorDiff {
                        doc: vec![StringDiff {
                            from: 0,
                            to: Some(48),
                            insert: "z<a id=\"notes\"></a>Notes\n------------------------\n"
                                .to_string(),
                        }],
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
