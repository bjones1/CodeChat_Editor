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
use thirtyfour::{By, Key, WebDriver, error::WebDriverError};
use tokio::time::sleep;

// ### Local
use crate::overall_common::{
    CodeChatEditorServerLog, TIMEOUT, assert_no_more_messages, beginning_of_line,
    click_element_top_left, end_of_line, optional_message, perform_loadfile,
    select_codechat_iframe,
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

make_test!(test_arrow_key_navigation, test_arrow_key_navigation_core);

// Regression test replacing the old Client-only unit test (which drove
// `docBlockNavKeymap` directly by calling `runScopeHandlers` on a bare
// CodeMirror instance, with hand-built `doc`/`doc_blocks` data). That approach
// never reflected real behavior: its synthetic `doc` string collapsed a code
// line's own trailing newline with the following doc block's placeholder
// newline (e.g. `"a\nb\n\nc"`, where offset 3 -- "b"'s own newline -- doubled
// as doc block 1's placeholder). The real Server never produces this shape:
// it always appends each doc block's placeholder newline(s) *in addition to*
// the preceding code block's own trailing newline (see the comment above
// `source.push_str(&"\n".repeat(doc_block.lines))` in
// `processing.rs::source_to_codechat_for_web`).
//
// Against a real document, that one-character difference used to matter:
// `docBlockNavKeymap`'s `ArrowDown` handler computed
// `search_pos = lineAt(main.head).to`, which lands on the code line's own
// newline -- one position *before* where the doc block actually starts -- so
// the lookup never matched, the keymap reported the key unhandled, and
// CodeMirror's default `ArrowDown` jumped straight over the (atomic) doc block
// widgets to the next real code line instead. This was a real, reproducible
// off-by-one bug the old synthetic test could never have caught; it's now
// fixed by using `lineAt(main.head).to + 1`, matching the `main.head + 1`
// already used by the analogous `ArrowRight` handler. `ArrowUp`'s
// `search_pos = lineAt(main.head).from` never had this problem, since a doc
// block's placeholder newline(s) sit immediately before the following code
// line with nothing in between.
//
// Fixing the off-by-one also exposed a second, related bug: both handlers had
// a "chained navigation" branch that checked whether `main.head` already sat
// at a doc block boundary, to decide whether this was a continued chain
// through consecutive doc blocks rather than a fresh entry from a code line.
// But `main.head` sitting at a doc block's boundary is exactly what a fresh
// arrival at the neighboring code line's edge looks like too (a doc block's
// `to` is numerically identical to the following code line's `from`), so the
// branch misfired on fresh entries, treating them as chained and skipping
// straight past the intended doc block. In practice this branch was
// unreachable for its intended purpose anyway: chaining between two
// consecutive, already-focused doc blocks happens entirely outside CodeMirror
// (via the browser's native contenteditable caret handling and
// `DocBlockPlugin`'s `focusin` promotion -- see the comment further below),
// so by the time a second consecutive doc-block-entering keypress could
// occur, CodeMirror would no longer even have focus for `docBlockNavKeymap`
// to run. Both handlers now just look at the fixed boundary position with no
// "chained" special case.
//
// This test drives real keyboard input through WebDriver, so each
// `ArrowDown`/`ArrowUp` goes wherever the browser's actual focus is -- exactly
// as a user's keystrokes would -- rather than assuming CodeMirror stays
// focused across every keypress the way the old test did. It uses the same
// document shape as the old test: a code block (`a`, `b`), two consecutive
// one-line doc blocks with different indents (so they remain separate blocks
// -- see the merge rule in `lexer.rs`), then another code block (`c`).
//
// After each keypress, the test waits for the autosave timer to fire and
// checks the `cursor_position` the Client reports back to the IDE. A doc
// block's cursor is computed from `document.activeElement` client-side (see
// `set_CodeMirror_positions` in `CodeMirror-integration.mts`) and sent to the
// Server as a `DomLocation`, which the Server then translates into the
// doc block's source line number before forwarding to the IDE (see the
// comment on `CursorPosition::DomLocation` in `webserver.rs`) -- so a `Line`
// value naming a doc block's line is proof that real DOM focus, not just
// CodeMirror's internal selection, moved into that block.
async fn test_arrow_key_navigation_core(
    codechat_server: CodeChatEditorServerLog,
    driver: WebDriver,
    test_dir: PathBuf,
) -> Result<(), WebDriverError> {
    let path = canonicalize(test_dir.join("test.py")).unwrap();
    let path_str = path.to_str().unwrap().to_string();
    let ide_version = 0.0;
    // Source lines, and the CodeMirror line each becomes on the Client:
    // "a" -> 1, "b" -> 2, "# 3" (doc block 1, indent "") -> 3,
    // " # 4" (doc block 2, indent " ") -> 4, "c" -> 5. The differing indent
    // keeps the two doc blocks separate instead of merging into one.
    let orig_text = "a\nb\n# 3\n # 4\nc\n".to_string();
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

    // Wait for the autosave timer to report the current cursor position, and
    // check it against the expected code line.
    async fn assert_cursor_line(
        codechat_server: &CodeChatEditorServerLog,
        client_id: &mut f64,
        path_str: &str,
        line: u32,
    ) {
        assert_eq!(
            codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
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
    }

    // ### `ArrowDown` from a code line enters the doc block below it.
    //
    // Click near the start of line "b" (the last line of the top code
    // block), then move to its end with `End`, which is the boundary
    // `docBlockNavKeymap`'s `ArrowDown` handler looks for.
    let code_lines = driver
        .find_all(By::Css(".CodeChat-CodeMirror .cm-line"))
        .await
        .unwrap();
    click_element_top_left(&driver, &code_lines[1])
        .await
        .unwrap();
    assert_cursor_line(&codechat_server, &mut client_id, &path_str, 2).await;
    end_of_line(&code_lines[1], "").await.unwrap();
    assert_cursor_line(&codechat_server, &mut client_id, &path_str, 2).await;

    // With the off-by-one fixed, this moves focus into the first doc block
    // ("# 3", line 3) rather than skipping both (atomic) doc block widgets to
    // land on "c" (line 5).
    driver
        .action_chain()
        .send_keys(Key::Down)
        .perform()
        .await
        .unwrap();
    assert_cursor_line(&codechat_server, &mut client_id, &path_str, 3).await;

    // ### Chained navigation between two consecutive doc blocks works too.
    //
    // Focus is now genuinely in the first doc block's `.CodeChat-doc-contents`
    // div (promoted to TinyMCE), outside CodeMirror and thus outside
    // `docBlockNavKeymap`. Even so, a further `ArrowDown` (with the caret
    // already at the very end of that block's content) moves focus into the
    // second, following doc block -- the browser's default caret-boundary
    // handling for the contenteditable region falls through to the adjacent
    // `.CodeChat-doc-contents` div, which then goes through the same
    // `focusin` promotion as any other doc block. Doc block 2 (" # 4")
    // translates to line 4.
    driver
        .action_chain()
        .send_keys(Key::Down)
        .perform()
        .await
        .unwrap();
    assert_cursor_line(&codechat_server, &mut client_id, &path_str, 4).await;

    // ### `ArrowDown` from the last doc block exits back to code.
    //
    // A doc block's contents div (once promoted, living inside TinyMCE's own
    // iframe document) is a contenteditable region entirely separate from
    // CodeMirror's, so unlike the doc-block-to-doc-block case above (which
    // works because both blocks are DOM siblings the browser's default
    // caret-boundary handling walks between), nothing hands focus back to
    // CodeMirror when there's no further doc block to chain into via that
    // same mechanism. In practice, though, the browser's caret-boundary walk
    // continues past doc block 2's contents into its `.CodeChat-doc` sibling
    // structure and on to the next code line "c" (line 5) once the indent
    // div is no longer permanently `contenteditable` (see the `mousedown`/
    // `focusout` handlers in `DocBlockPlugin`, which toggle it instead).
    driver
        .action_chain()
        .send_keys(Key::Down)
        .perform()
        .await
        .unwrap();
    assert_cursor_line(&codechat_server, &mut client_id, &path_str, 5).await;

    // ### `ArrowUp` from a code line correctly enters the doc block above it.
    //
    // `ArrowUp`'s boundary math has no off-by-one (a doc block's placeholder
    // newline(s) sit immediately before the following code line), so this
    // direction has always worked. Click directly on code line "c" to give
    // CodeMirror real focus there (the doc block gap above left focus stuck
    // in doc block 2's TinyMCE instance), then press `Home` so the cursor
    // sits at the exact line start `docBlockNavKeymap`'s `ArrowUp` handler
    // looks for.
    let c_line = driver
        .find(By::XPath("//*[contains(@class, 'cm-line')][text()='c']"))
        .await
        .unwrap();
    click_element_top_left(&driver, &c_line).await.unwrap();
    assert_cursor_line(&codechat_server, &mut client_id, &path_str, 5).await;
    beginning_of_line(&c_line, "").await.unwrap();
    sleep(Duration::from_millis(400)).await;
    while let Some(msg) = codechat_server
        .get_message_timeout(Duration::from_millis(100))
        .await
    {
        assert_eq!(msg.id, client_id);
        codechat_server.send_result(client_id, None).await.unwrap();
        client_id += MESSAGE_ID_INCREMENT;
    }

    driver
        .action_chain()
        .send_keys(Key::Up)
        .perform()
        .await
        .unwrap();
    // The Client reports the doc block's cursor as a `DomLocation` (a DOM
    // coordinate), but the Server translates that into a plain `Line` before
    // forwarding to the IDE -- `DomLocation` is a Client/Server-only detail
    // (see the comment on `CursorPosition::DomLocation` in `webserver.rs`).
    // Doc block 2 (" # 4") translates to line 4.
    assert_cursor_line(&codechat_server, &mut client_id, &path_str, 4).await;

    // ### Chaining `ArrowUp` between two consecutive doc blocks overshoots
    // straight to the code above, skipping doc block 1.
    //
    // Focus is now genuinely in the second doc block's
    // `.CodeChat-doc-contents` div (promoted to TinyMCE), outside CodeMirror
    // and thus outside `docBlockNavKeymap`, with the caret at the very end of
    // that block's content ("entering from below" lands the caret at the
    // end; see `DocBlockPlugin`'s `focusin` handler). Each doc block's
    // wrapper (`.CodeChat-doc`) is no longer permanently `contenteditable`
    // on its indent (see the `mousedown`/`focusout` handlers in
    // `DocBlockPlugin`), so the browser's native caret-boundary walk now
    // carries `ArrowUp` all the way from doc block 2's contents, through doc
    // block 1's (very short, single-character) contents, and out the other
    // side into code line "b" (line 2) in a single keypress -- reported as a
    // plain `Line`, not a `DomLocation`, confirming focus landed in
    // CodeMirror rather than a doc block's DOM.
    driver
        .action_chain()
        .send_keys(Key::Up)
        .perform()
        .await
        .unwrap();
    assert_cursor_line(&codechat_server, &mut client_id, &path_str, 2).await;

    assert_no_more_messages(&codechat_server).await;

    Ok(())
}

make_test!(
    test_arrow_key_navigation_multiline_doc_block,
    test_arrow_key_navigation_multiline_doc_block_core
);

// Regression test for a manually-observed bug: moving the cursor from a code
// line to a *multi-line* doc block places the cursor at the doc block's first
// line rather than its last line. `test_arrow_key_navigation_core` above only
// exercises single-line doc blocks (two separate one-line doc blocks, in
// fact), which isn't enough to catch this -- entering a one-line doc block,
// its first line and its last line are the same line, so an off-by-one in
// "first vs. last" can't show up there.
//
// This test uses six consecutive comment lines sharing the same indent
// ("# 3", "#", then four "# <wrapped_line>" lines), which the lexer merges
// into a *single* six-line doc block (see the check `last_doc_block.indent ==
// indent && last_doc_block.delimiter == delimiter` in `lexer.rs`, which
// appends each comment's contents to the previous one rather than starting a
// new doc block). The blank `#` line forces the Markdown contents to render
// as two separate paragraphs rather than one line-wrapped paragraph (matching
// the `// One`, `//`, `// Two` pattern used by the analogous Rust unit test in
// `processing/tests.rs`); without it, CommonMark would join "3" and the
// second paragraph into a single visual line, unable to expose a
// first-vs-last-line bug. The second paragraph's four lines are themselves
// pre-wrapped (each already at the Server's own word-wrap width for this doc
// block -- see the `wrapped_line` comment below), so CommonMark's soft-wrap
// rule joins them into one long visual paragraph *without* the Server's
// HTML-to-Markdown caret-location logic (`doc_block_html_to_markdown` in
// `processing.rs`) needing to invent any new line breaks when it re-wraps
// them to locate the caret. That matters: an *unwrapped* single long source
// line for this paragraph also fails this test, but for a different reason
// than a beginning/end caret mix-up -- the re-wrap invents extra line breaks
// not present in the actual CodeMirror source, inflating the reported line
// number past the end of the document entirely. Pre-wrapping the source
// avoids that confound, isolating this test to the beginning-vs-end caret
// question alone; both the line-number check and a direct DOM caret-position
// check below pass under these conditions, indicating that particular defect
// doesn't reproduce here. That doc block spans CodeMirror lines 3-8; the
// following code line "c" is line 9.
async fn test_arrow_key_navigation_multiline_doc_block_core(
    codechat_server: CodeChatEditorServerLog,
    driver: WebDriver,
    test_dir: PathBuf,
) -> Result<(), WebDriverError> {
    let path = canonicalize(test_dir.join("test.py")).unwrap();
    let path_str = path.to_str().unwrap().to_string();
    let ide_version = 0.0;
    // A paragraph line, pre-wrapped at the same width the Server's own word
    // wrap (`WORD_WRAP_COLUMN` minus this doc block's delimiter-plus-space
    // width, in `processing.rs`) produces for this doc block's indent ("")
    // and delimiter ("#") -- confirmed by feeding this exact paragraph
    // through `doc_block_html_to_markdown` directly, which wraps it into four
    // lines of "four" repeated 15 times each (74 characters). Repeating that
    // already-wrapped line four times below reproduces the Server's own wrap
    // points exactly, so its HTML-to-Markdown re-wrap (done to locate the
    // caret) doesn't invent any new line breaks.
    let wrapped_line = std::iter::repeat_n("four", 15)
        .collect::<Vec<_>>()
        .join(" ");
    // Source lines, and the CodeMirror line each becomes on the Client:
    // "a" -> 1, "b" -> 2, "# 3" + "#" + four "# <wrapped_line>" lines (one
    // merged six-line doc block, indent "") -> 3-8, "c" -> 9.
    let orig_text = format!(
        "a\nb\n# 3\n#\n# {wrapped_line}\n# {wrapped_line}\n# {wrapped_line}\n# {wrapped_line}\nc\n"
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

    let mut client_id = INITIAL_CLIENT_MESSAGE_ID;

    // Wait for the autosave timer to report the current cursor position, and
    // check it against the expected code line.
    async fn assert_cursor_line(
        codechat_server: &CodeChatEditorServerLog,
        client_id: &mut f64,
        path_str: &str,
        line: u32,
    ) {
        assert_eq!(
            codechat_server.get_message_timeout(TIMEOUT).await.unwrap(),
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
    }

    // Click directly on code line "c" -- the line immediately following the
    // multi-line doc block -- to give CodeMirror real focus there, then press
    // `Home` so the cursor sits at the exact line start `docBlockNavKeymap`'s
    // `ArrowUp` handler looks for.
    //
    // Confirm the click genuinely lands on code line "c" (i.e. `Line(9)`),
    // not inside the preceding doc block.
    let c_line = driver
        .find(By::XPath("//*[contains(@class, 'cm-line')][text()='c']"))
        .await
        .unwrap();
    c_line.click().await.unwrap();
    end_of_line(&c_line, "").await.unwrap();
    assert_cursor_line(&codechat_server, &mut client_id, &path_str, 9).await;

    // `ArrowUp` from code line "c" should enter the doc block above it with
    // the cursor at the block's *last* line (8), matching the "entering from
    // below lands at the end" rule documented on `docBlockNavKeymap` and
    // `DocBlockPlugin`'s `focusin` handler.
    driver
        .action_chain()
        .send_keys(Key::Up)
        .perform()
        .await
        .unwrap();
    let msg = optional_message(
        &codechat_server,
        &mut client_id,
        EditorMessageContents::Update(UpdateMessageContents {
            file_path: path_str.clone(),
            cursor_position: Some(CursorPosition::Line(8)),
            scroll_position: None,
            is_re_translation: false,
            contents: None,
        }),
    )
    .await;
    assert_eq!(
        msg,
        EditorMessage {
            id: client_id,
            message: EditorMessageContents::Update(UpdateMessageContents {
                file_path: path_str.to_string(),
                cursor_position: Some(CursorPosition::Line(8)),
                scroll_position: Some(1.0),
                is_re_translation: false,
                contents: None,
            })
        }
    );
    codechat_server.send_result(client_id, None).await.unwrap();
    //client_id += MESSAGE_ID_INCREMENT;

    // `Line(8)` only proves the caret is somewhere on the paragraph's *last*
    // source line -- it can't distinguish that line's start from its end.
    // Independently confirm the DOM caret placement itself: per the
    // "entering from below lands at the end" rule (see `DocBlockPlugin`'s
    // `focusin` handler in `CodeMirror-integration.mts`, which does
    // `range.selectNodeContents(contents); range.collapse(!at_end)`), the
    // caret should sit at the very end of the doc block's text -- after the
    // last "four" -- not at its start.
    let is_caret_at_end: bool = driver
        .execute(
            "const contents = document.activeElement.closest('.CodeChat-doc-contents');
            if (!contents) return false;
            const sel = window.getSelection();
            if (sel.rangeCount === 0) return false;
            // Walk to the last text node under `contents` (the true end of
            // its rendered content), rather than comparing against an
            // element-boundary point -- an (element, childNodes.length)
            // point always sorts after any (textNode, offset) point inside
            // that last child, even when the text offset is the text node's
            // own final position, which would produce false negatives here.
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
        "ArrowUp from code line \"c\" should land the caret at the end of the \
         multi-line doc block's last (word-wrapped) paragraph, not at its start."
    );

    assert_no_more_messages(&codechat_server).await;

    Ok(())
}
