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
//
// `CodeChatEditor-test.mts` -- Tests for the CodeChat Editor client
// =================================================================
//
// To run tests, add a `?test` to any web page served by the CodeChat Editor
// server.
//
// Imports
// -------
import { assert } from "chai";
import "mocha/mocha.js";
import "mocha/mocha.css";
import { EditorView } from "@codemirror/view";
import { ChangeSpec, EditorState, EditorSelection } from "@codemirror/state";
import { CodeMirror, CodeMirrorDocBlockTuple } from "./shared_types.mjs";
import {
    DocBlockPlugin,
    CodeMirror_JSON_fields,
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

// Tests
// -----
//
// <a id="CodeChatEditor_test"></a>Defining this global variable signals the
// CodeChat Editor to [run tests](CodeChatEditor.mts#CodeChatEditor_test).
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
                const mocha_div = document.getElementById("mocha")!;
                const ccb = document.getElementById("CodeChat-body")!;
                ccb.insertBefore(mocha_div, ccb.firstChild);
            },
        ],
    });

    // Define some tests. See the [Mocha TDD docs](https://mochajs.org/#tdd) and
    // the [Chai assert API](https://www.chaijs.com/api/assert/).
    suite("CodeChatEditor.mts", function () {
        suite("CodeMirror checks", function () {
            test("insert/delete/replace expectations", function () {
                // Create a div to hold an editor.
                const codechat_body = document.getElementById(
                    "CodeChat-body",
                ) as HTMLDivElement;
                const testing_div = document.createElement("div");
                testing_div.id = "testing-div";
                codechat_body.insertBefore(
                    testing_div,
                    codechat_body.firstChild,
                );

                // Test insert at beginning of doc block.
                const after_state = run_CodeMirror_test(
                    "a\nbcd",
                    [[1, 2, "", "#", "test"]],
                    { from: 1, insert: "\n" },
                );
                assert.deepEqual(after_state, {
                    doc: "a\n\nbcd",
                    doc_blocks: [[1, 3, "", "#", "test"]],
                });
            });

            test("GraphViz, Mathjax, Mermaid", async function () {
                // Wait for the renderers to run.
                await sleep(1500);
                // Make sure GraphViz includes a `div` at the top of the shadow
                // root, with a `svg` inside it.
                const gv =
                    document.getElementsByTagName("graphviz-graph")[0]
                        .shadowRoot!.children[0];
                assert.equal(gv.tagName, "DIV");
                assert.equal(gv.children[0].tagName, "svg");

                // Mermaid graphs start with a div.
                const mer =
                    document.getElementsByTagName("wc-mermaid")[0].shadowRoot!
                        .children[0];
                assert.equal(mer.tagName, "DIV");
                assert.equal(mer.children[0].tagName, "svg");

                // MathJax has its own stuff.
                assert.equal(
                    document.getElementsByTagName("mjx-container").length,
                    2,
                );
            });
        });
    });

    // Avoid an infinite loop of tests calling this again.
    delete window.CodeChatEditor_test;
    mocha.run();
};

const run_CodeMirror_test = (
    doc: string,
    doc_blocks: [CodeMirrorDocBlockTuple],
    changes: ChangeSpec,
): CodeMirror => {
    // Create the CodeChat Editor for testing.
    const editor_state_json = {
        doc,
        selection: EditorSelection.single(0).toJSON(),
        doc_blocks,
    };
    const state = EditorState.fromJSON(
        editor_state_json,
        {
            extensions: [DocBlockPlugin],
        },
        CodeMirror_JSON_fields,
    );
    const view = new EditorView({
        parent: document.getElementById("testing-div")!,
        state,
    });

    // Run a transaction, then extract at the results.
    view.dispatch({ changes });
    const after_state = view.state.toJSON(CodeMirror_JSON_fields);
    delete after_state.selection;
    return after_state;
};
