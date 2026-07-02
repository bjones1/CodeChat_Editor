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
/// `overall_4.rs` - test the overall system
/// ========================================
///
/// These are functional tests of the overall system, performed by attaching a
/// testing IDE to generate commands then observe results, along with a browser
/// tester. This file focuses on security: it verifies that malicious HTML
/// supplied as a document's source is sanitized, so that embedded JavaScript
/// never executes and is removed from the source code the Client produces.
// Modules
// -------
mod overall_common;

// Imports
// -------
//
// ### Standard library
use std::{path::PathBuf, time::Duration};

// ### Third-party
use dunce::canonicalize;
use indoc::formatdoc;
use pretty_assertions::assert_eq;
use thirtyfour::{By, WebDriver, error::WebDriverError};
use tokio::time::sleep;

// ### Local
use crate::overall_common::{
    CodeChatEditorServerLog, TIMEOUT, assert_no_more_messages, click_element_top_left,
    optional_message, perform_loadfile, select_codechat_iframe,
};
use code_chat_editor::{
    processing::{CodeChatForWeb, CodeMirrorDiffable},
    webserver::{
        CursorPosition, EditorMessage, EditorMessageContents, INITIAL_CLIENT_MESSAGE_ID,
        MESSAGE_ID_INCREMENT, ResultOkTypes, UpdateMessageContents,
    },
};
use test_utils::prep_test_dir;

// Tests
// -----
make_test!(test_xss, test_xss_core);

// Send malicious HTML (an `<img>` tag carrying an `onerror` handler that runs
// JavaScript) as a document's source, then verify three things:
//
// 1. The JavaScript never runs. If it did, the `onerror` handler would call
//    `console.log("XSS")`, which chromedriver captures in the `browser` log
//    buffer. We read that buffer directly and assert the marker never appears.
// 2. The rendered DOM has had the `onerror` handler stripped, so the handler
//    can never fire.
// 3. Editing the document in the Client produces source code that no longer
//    contains the malicious handler -- i.e., the sanitized HTML is what gets
//    written back to disk.
async fn test_xss_core(
    codechat_server: CodeChatEditorServerLog,
    driver: WebDriver,
    test_dir: PathBuf,
) -> Result<(), WebDriverError> {
    let path = canonicalize(test_dir.join("test.md")).unwrap();
    let path_str = path.to_str().unwrap().to_string();
    let version = 0.0;
    // The malicious payload: an image whose `src` is guaranteed to fail
    // loading, firing the `onerror` handler. If the handler were allowed
    // through, it would log the `XSS` marker to the browser console.
    //
    // The `src` is an invalid `data:` URI so the failed load is resolved
    // entirely in the browser. (A relative `src` such as `x` would instead make
    // the browser issue an HTTP request to the Server for that resource,
    // injecting an unexpected `LoadFile` into the message stream.)
    let orig_text =
        "Before <img src='data:image/png;base64,!' onerror='console.log(\"XSS\")'> after."
            .to_string();
    let server_id = perform_loadfile(
        &codechat_server,
        &test_dir,
        "test.md",
        Some((orig_text.clone(), version)),
        false,
        6.0,
    )
    .await;

    // Target the iframe containing the Client.
    select_codechat_iframe(&driver).await;

    // Give the browser a moment to render the doc block; if the `onerror`
    // handler had survived sanitization, the failed image load would fire it
    // during this window.
    sleep(Duration::from_millis(500)).await;

    // ### 1\. The JavaScript must not have executed.
    //
    // Drain the browser console log. chromedriver records page-side `console.*`
    // output (and uncaught errors) in the `browser` buffer; if the `onerror`
    // handler had run, our `XSS` marker would appear here. Draining via the
    // wrapper (rather than `driver.get_log("browser")` directly) both forwards
    // each entry to Rust logging and hands the entries back for inspection. Do
    // this right after rendering and before any further server call, which
    // would otherwise drain the buffer first.
    let entries = codechat_server.poll_log().await;
    for entry in &entries {
        assert!(
            !entry.message.contains("XSS"),
            "Malicious JavaScript executed: found XSS marker in browser log entry: {}",
            entry.message
        );
    }

    // ### 2\. The rendered DOM must not contain the malicious handler.
    //
    // The doc block should render the image with its `onerror` attribute
    // stripped, leaving a harmless `<img>`.
    let body_css = "#CodeChat-body .CodeChat-doc-contents";
    let body_content = driver.find(By::Css(body_css)).await.unwrap();
    let rendered = body_content.inner_html().await.unwrap();
    assert!(
        !rendered.contains("onerror"),
        "Sanitized DOM still contains an onerror handler: {rendered}"
    );
    assert!(
        rendered.contains("<img"),
        "Expected a sanitized <img> tag in the rendered DOM: {rendered}"
    );

    // ### 3\. Editing the document must write back sanitized source.
    //
    // Click into the doc block, then type a character. The Client converts the
    // (already sanitized) rendered HTML back to source and sends it to the IDE
    // as an `Update`. That source must not contain the malicious handler.
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

    // Refind the editable contents and type a character to trigger an update.
    let body_content = driver.find(By::Css(body_css)).await.unwrap();
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

    // The update must carry contents; pull the source out of the diff and
    // verify the malicious handler is gone.
    let contents = match &msg.message {
        EditorMessageContents::Update(UpdateMessageContents {
            contents: Some(contents),
            ..
        }) => contents,
        other => panic!("Expected an Update with contents, got: {other:#?}"),
    };
    let CodeChatForWeb { source, .. } = contents;
    let doc = match source {
        CodeMirrorDiffable::Diff(diff) => &diff.doc,
        CodeMirrorDiffable::Plain(_) => panic!("Expected a diff, got plain contents."),
    };
    let inserted: String = doc.iter().map(|d| d.insert.as_str()).collect();
    assert!(
        !inserted.contains("onerror"),
        "Source written back to the IDE still contains an onerror handler: {inserted}"
    );
    assert!(
        !inserted.contains("XSS"),
        "Source written back to the IDE still contains the XSS payload: {inserted}"
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

    // The re-translation settles the cursor, producing a final cursor-only
    // update from the Client.
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
    //client_id += MESSAGE_ID_INCREMENT;

    assert_no_more_messages(&codechat_server).await;

    Ok(())
}

make_test!(
    test_horizontal_scroll_preserved,
    test_horizontal_scroll_preserved_core
);

// Regression test for
// [#113](https://github.com/bjones1/CodeChat_Editor/issues/113): when the IDE
// moves the cursor into a doc block containing a line too wide for the Client's
// viewport, the Client must scroll vertically to bring that line into view
// without disturbing the horizontal scroll position. Before the fix,
// CodeMirror's `scrollIntoView` pinned the horizontal scrollbar to its maximum.
//
// The test loads a doc block containing a few one-line paragraphs, a fenced
// code block with a very long, non-wrapping line, then more one-line
// paragraphs. It scrolls the CodeMirror scroller horizontally to a middle
// position, then simulates the IDE moving its cursor to each line in the doc
// block (as arrow-key presses in the IDE would), verifying after each move that
// the horizontal scroll position hasn't changed.
async fn test_horizontal_scroll_preserved_core(
    codechat_server: CodeChatEditorServerLog,
    driver: WebDriver,
    test_dir: PathBuf,
) -> Result<(), WebDriverError> {
    let path = canonicalize(test_dir.join("test.py")).unwrap();
    let path_str = path.to_str().unwrap().to_string();
    let ide_version = 0.0;
    // A long, non-wrapping line: fenced code blocks render as `<pre>`, which
    // doesn't wrap, so this forces horizontal scrolling.
    let long_line = "x".repeat(500);
    let orig_text = formatdoc!(
        "
        # 1
        #
        # 2
        #
        # ```
        # {long_line}
        # ```
        #
        # 8
        #
        # 9
        "
    );
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

    // Scroll the CodeMirror scroller horizontally to a middle position (not
    // fully left or fully right).
    let scroller_css = ".CodeChat-CodeMirror .cm-scroller";
    driver
        .execute(
            &format!("document.querySelector('{scroller_css}').scrollLeft = 200;"),
            Vec::new(),
        )
        .await
        .unwrap();
    let get_scroll_left = format!("return document.querySelector('{scroller_css}').scrollLeft;");
    let scroll_left_before: f64 = driver
        .execute(&get_scroll_left, Vec::new())
        .await
        .unwrap()
        .convert()
        .unwrap();
    assert!(
        scroll_left_before > 0.0,
        "Failed to scroll the CodeMirror scroller horizontally before the test began."
    );

    // Simulate the IDE moving its cursor to each line of the doc block, as
    // arrow-key presses in the IDE would produce. Check every line, including
    // the fenced-code-block lines.
    for line in 1..=11u32 {
        let ide_id = codechat_server
            .send_message_update_plain(path_str.clone(), None, Some(line), Some(line.into()))
            .await
            .unwrap();
        // The Client acknowledges the Update with a Result(Ok).
        assert_eq!(
            codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
            EditorMessage {
                id: ide_id,
                message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
            }
        );

        let scroll_left_after: f64 = driver
            .execute(&get_scroll_left, Vec::new())
            .await
            .unwrap()
            .convert()
            .unwrap();
        assert_eq!(
            scroll_left_after, scroll_left_before,
            "Horizontal scroll changed after moving the cursor to line {line}."
        );
    }

    assert_no_more_messages(&codechat_server).await;

    Ok(())
}
