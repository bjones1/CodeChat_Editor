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
//
// `CodeChatEditor-test.mts` -- Tests for the CodeChat Editor client
// =================================================================
//
// To run tests, change to the `server/` directory then run `./bt client-build`
// to build any changes to the Client, followed by `cargo test --test overall
// test_client`. Or, add a `?test` parameter to any web page served by the
// CodeChat Editor Server.
//
// Imports
// -------
import { assert } from "chai";
import "mocha/mocha.js";
import "mocha/mocha.css";
import { EditorView } from "@codemirror/view";
import { ChangeSpec, EditorState, EditorSelection } from "@codemirror/state";
import { CodeMirror, CodeMirrorDocBlockTuple } from "./shared.mjs";
import {
    DocBlockPlugin,
    codeMirrorJsonFields,
} from "./CodeMirror-integration.mjs";

// Re-export everything that [CodeChatEditor.mts](CodeChatEditor.mts) exports.
// Otherwise, including [CodeChatEditor.mts](CodeChatEditor.mts) elsewhere would
// double-define everything (producing complaints about two attempts to define
// each web component).
//
// Nothing needed at present.
//
// From [SO](https://stackoverflow.com/a/39914235).
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const RENDER_TIMEOUT_MS = 10000;
const MOCHA_TEST_TIMEOUT_MS = RENDER_TIMEOUT_MS + 5000;

const waitFor = async (
    description: string,
    predicate: () => boolean,
    timeoutMs = RENDER_TIMEOUT_MS,
) => {
    const start = Date.now();
    while (Date.now() - start < timeoutMs) {
        if (predicate()) {
            return;
        }
        await sleep(100);
    }
    assert.fail(`Timed out waiting for ${description}.`);
};

// Tests
// -----
//
// Defining this global variable signals the CodeChat Editor to
// [run tests](CodeChatEditor.mts#CodeChatEditor_test).
window.CodeChatEditor_test = () => {
    // See the [Mocha docs](https://mochajs.org/#browser-configuration).
    mocha.setup({
        ui: "tdd",
        // This is required to use Mocha's global teardown from the browser,
        // AFAIK.
        /// @ts-expect-error("See above.")
        globalTeardown: [
            () => {
                // On teardown, put the Mocha div at the beginning of the body.
                // Testing causes body to be wiped, so don't do this until all
                // tests are done.
                const mochaDiv = document.getElementById("mocha")!;
                const ccb = document.getElementById("CodeChat-body")!;
                ccb.insertBefore(mochaDiv, ccb.firstChild);
            },
        ],
    });

    // Define some tests. See the [Mocha TDD docs](https://mochajs.org/#tdd) and
    // the [Chai assert API](https://www.chaijs.com/api/assert/).
    suite("CodeChatEditor.mts", function () {
        suite("CodeMirror checks", function () {
            test("insert/delete/replace expectations", function () {
                // Create a div to hold an editor.
                const codechatBody = document.getElementById(
                    "CodeChat-body",
                ) as HTMLDivElement;
                const testingDiv = document.createElement("div");
                testingDiv.id = "testing-div";
                codechatBody.insertBefore(testingDiv, codechatBody.firstChild);

                // Test insert at beginning of doc block.
                const afterState = runCodeMirrorTest(
                    "a\nbcd",
                    [[1, 2, "", "#", "test"]],
                    { from: 1, insert: "\n" },
                );
                assert.deepEqual(afterState, {
                    doc: "a\n\nbcd",
                    doc_blocks: [[1, 3, "", "#", "test"]],
                });
            });

            test("GraphViz, Mathjax, Mermaid", async function (this: Mocha.Context) {
                this.timeout(MOCHA_TEST_TIMEOUT_MS);

                // Make sure GraphViz includes a `div` at the top of the shadow
                // root, with a `svg` inside it.
                const getGraphVizRoot = () =>
                    document.getElementsByTagName("graphviz-graph")[0]
                        ?.shadowRoot?.children[0];
                await waitFor(
                    "GraphViz SVG",
                    () => getGraphVizRoot()?.children[0]?.tagName === "svg",
                );
                const gv =
                    document.getElementsByTagName("graphviz-graph")[0]
                        .shadowRoot!.children[0];
                assert.equal(gv.tagName, "DIV");
                assert.equal(gv.children[0].tagName, "svg");

                // Mermaid graphs start with a div.
                const getMermaidRoot = () =>
                    document.getElementsByTagName("wc-mermaid")[0]?.shadowRoot
                        ?.children[0];
                await waitFor(
                    "Mermaid SVG",
                    () => getMermaidRoot()?.children[0]?.tagName === "svg",
                );
                const mer =
                    document.getElementsByTagName("wc-mermaid")[0].shadowRoot!
                        .children[0];
                assert.equal(mer.tagName, "DIV");
                assert.equal(mer.children[0].tagName, "svg");

                // MathJax has its own stuff.
                await waitFor(
                    "MathJax containers",
                    () =>
                        document.getElementsByTagName("mjx-container")
                            .length === 2,
                );
                assert.equal(
                    document.getElementsByTagName("mjx-container").length,
                    2,
                );
            });
        });

        // These check the layout the Server's `render_fragment_content`
        // (`processing.rs`) and the `cc-*` styles in
        // [CodeChatEditor.css](css/CodeChatEditor.css) produce together, which
        // no test of either alone can see: a fragment gathered into a list must
        // reproduce the layout of the source it came from. The fragment
        // gathered in `test.py` is the case that matters -- a doc block and the
        // line of code below it are both indented four spaces there.
        suite("Gathered fragment layout", function () {
            // The gather list is drawn by CodeMirror as the doc block holding
            // the gather element scrolls into view.
            const gatherItems = async () => {
                await waitFor(
                    "gathered fragment",
                    () =>
                        document.querySelector(
                            ".cc-gather-items .cc-fragment-code",
                        ) !== null,
                );
                return document.querySelector(".cc-gather-items")!;
            };

            // The vertical position of each line number in the rendered code
            // block, which says whether its lines are stacked as they are in
            // the source or run together onto one line. Search from the copy of
            // the gather list under test rather than from the document:
            // focusing a doc block promotes it to a TinyMCE instance which
            // holds its own copy of the list, and a search spanning both would
            // report the two copies' positions interleaved.
            const lineNumberTops = (gatherItems: ParentNode) =>
                Array.from(
                    gatherItems.querySelectorAll(
                        ".cc-fragment-code .cc-line-number",
                    ),
                ).map((lineNumber) => lineNumber.getBoundingClientRect().top);

            test("aligns doc block indents with code", async function (this: Mocha.Context) {
                this.timeout(MOCHA_TEST_TIMEOUT_MS);

                const items = await gatherItems();
                const indent = items.querySelector(".cc-fragment-indent")!;
                const code = items.querySelector(".cc-fragment-code")!;

                // An indent aligns with the code below it only if the two are
                // measured in the same character width.
                const indentStyle = getComputedStyle(indent);
                const codeStyle = getComputedStyle(code);
                assert.equal(indentStyle.fontFamily, codeStyle.fontFamily);
                assert.equal(indentStyle.fontSize, codeStyle.fontSize);

                // The doc block and the code are each preceded by their line
                // numbers in `test.py`.
                const docLineNumber = indent.querySelector(".cc-line-number")!;
                assert.equal(docLineNumber.textContent, "7");
                const codeLineNumber = code.querySelector(".cc-line-number")!;
                assert.equal(codeLineNumber.textContent, "9");

                // The two gutters must be the same width, since each holds the
                // column its side's content begins in.
                assert.closeTo(
                    docLineNumber.getBoundingClientRect().width,
                    codeLineNumber.getBoundingClientRect().width,
                    1,
                );

                // The doc block's contents begin where its indent ends...
                const docStart = items
                    .querySelector(".cc-fragment-doc-contents")!
                    .getBoundingClientRect().left;
                // ...and the code on the next line must begin in that same
                // column: each line number fills a gutter of the same width, and
                // the four spaces which follow the code's match the four the doc
                // block is indented by.
                const codeText = Array.from(code.childNodes).find(
                    (node) => node.nodeType === Node.TEXT_NODE,
                ) as Text;
                assert.equal(codeText.data.slice(0, 4), "    ");
                const codeIndent = document.createRange();
                codeIndent.setStart(codeText, 0);
                codeIndent.setEnd(codeText, 4);
                assert.closeTo(
                    codeIndent.getBoundingClientRect().right,
                    docStart,
                    1,
                );
            });

            // The source `test.py` gathers a doc block with the code block
            // directly beneath it, so the rendering must place them the same
            // way: the paragraph the doc block becomes carries a top and bottom
            // margin which would otherwise open a gap the source doesn't have.
            // The `remove-space` rules in
            // [CodeChatEditor.css](css/CodeChatEditor.css) trim it.
            test("puts a doc block directly against the code below it", async function (this: Mocha.Context) {
                this.timeout(MOCHA_TEST_TIMEOUT_MS);

                const items = await gatherItems();
                const docContents = items.querySelector(
                    ".cc-fragment-doc-contents",
                )!;
                const paragraph = docContents.firstElementChild!;
                assert.equal(paragraph.tagName, "P");

                // The doc block is exactly as tall as its text...
                const paragraphBox = paragraph.getBoundingClientRect();
                const docContentsBox = docContents.getBoundingClientRect();
                assert.closeTo(paragraphBox.top, docContentsBox.top, 1);
                assert.closeTo(paragraphBox.bottom, docContentsBox.bottom, 1);

                // ...and the code block begins where the doc block ends.
                const docBox = items
                    .querySelector(".cc-fragment-doc")!
                    .getBoundingClientRect();
                const codeBox = items
                    .querySelector(".cc-fragment-code")!
                    .getBoundingClientRect();
                assert.closeTo(docBox.bottom, codeBox.top, 1);
            });

            test("puts each line of a code block on its own line", async function (this: Mocha.Context) {
                this.timeout(MOCHA_TEST_TIMEOUT_MS);

                const items = await gatherItems();
                // The newlines separating a code block's lines are part of the
                // code; losing them runs the whole block onto one line.
                assert.include(
                    items.querySelector(".cc-fragment-code")!.textContent!,
                    "\n",
                );
                const asRendered = lineNumberTops(items);
                assert.lengthOf(asRendered, 2);
                assert.isAbove(asRendered[1], asRendered[0]);

                // Focusing a doc block promotes it to a TinyMCE instance, which
                // reparses the block's HTML. TinyMCE collapses the whitespace
                // in every element it doesn't consider whitespace-sensitive,
                // which is why a rendered fragment's code and indents are
                // `<pre>`s; see `render_fragment_content` in
                // [processing.rs](../../server/src/processing.rs).
                (
                    items.closest(".CodeChat-doc-contents") as HTMLDivElement
                ).focus();
                await waitFor(
                    "the doc block to become editable",
                    () =>
                        document.querySelector(
                            "#TinyMCE-inst .cc-gather-items .cc-fragment-code",
                        ) !== null,
                );
                const editedItems = document.querySelector(
                    "#TinyMCE-inst .cc-gather-items",
                )!;
                const afterEditing = lineNumberTops(editedItems);
                assert.lengthOf(afterEditing, 2);
                assert.isAbove(afterEditing[1], afterEditing[0]);
            });
        });
    });

    // Avoid an infinite loop of tests calling this again.
    delete window.CodeChatEditor_test;
    mocha.run();
};

const runCodeMirrorTest = (
    doc: string,
    docBlocks: [CodeMirrorDocBlockTuple],
    changes: ChangeSpec,
): CodeMirror => {
    // Create the CodeChat Editor for testing.
    const editorStateJson = {
        doc,
        selection: EditorSelection.single(0).toJSON(),
        doc_blocks: docBlocks,
    };
    const state = EditorState.fromJSON(
        editorStateJson,
        {
            extensions: [DocBlockPlugin],
        },
        codeMirrorJsonFields,
    );
    const view = new EditorView({
        parent: document.getElementById("testing-div")!,
        state,
    });

    // Run a transaction, then extract at the results.
    view.dispatch({ changes });
    const afterState = view.state.toJSON(codeMirrorJsonFields);
    delete afterState.selection;
    return afterState;
};
