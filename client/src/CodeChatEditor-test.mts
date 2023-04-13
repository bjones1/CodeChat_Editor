// Copyright (C) 2022 Bryan A. Jones.
//
// This file is part of the CodeChat Editor.
//
// The CodeChat Editor is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
//
// The CodeChat Editor is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with the CodeChat Editor. If not, see [http://www.gnu.org/licenses/](http://www.gnu.org/licenses/).
//
// `CodeChatEditor-test.mts` — Tests for the CodeChat Editor client
// ================================================================
//
// To run tests, add a `?test` to any web page served by the CodeChat Editor server.
//
// Imports
// -------
//
// I can't get Mocha to work with ESBuild, so I import it using a script tag.
import chai from "chai";
import {
    exportedForTesting,
    code_or_doc_block,
    page_init,
    on_keydown,
    on_save,
} from "./CodeChatEditor.mjs";

// Re-export everything that [CodeChatEditor.mts](CodeChatEditor.mts) exports. Otherwise, including [CodeChatEditor.mts](CodeChatEditor.mts) elsewhere would double-define everything (producing complaints about two attempts to define each web component).
export { page_init, on_keydown, on_save };
// Provide convenient access to all functions tested here.
const { editor_to_code_doc_blocks, EditorMode, open_lp } = exportedForTesting;

// Tests
// -----
//
// Defining this global variable signals the CodeChat Editor to [run tests](CodeChatEditor.mts#CodeChatEditor_test).
window.CodeChatEditor_test = () => {
    // See the [Mocha docs](https://mochajs.org/#browser-configuration).
    mocha.setup({
        ui: "tdd",
        // This is required to use Mocha's global teardown from the browser, AFAIK.
        /// @ts-ignore
        globalTeardown: [
            () => {
                // On teardown, put the Mocha div at the beginning of the body. Testing causes body to be wiped, so don't do this until all tests are done.
                const mocha_div = document.getElementById("mocha")!;
                const ccb = document.getElementById("CodeChat-body")!;
                ccb.insertBefore(mocha_div, ccb.firstChild);
            },
        ],
    });

    // Define some tests. See the [Mocha TDD docs](https://mochajs.org/#tdd) and the [Chai assert API](https://www.chaijs.com/api/assert/).
    suite("CodeChatEditor.mts", function () {
        suite("open_lp", function () {
            test("Load an empty file", async function () {
                await open_lp(
                    {
                        metadata: {
                            mode: "javascript",
                        },
                        code_doc_block_arr: [],
                    },
                    EditorMode.edit
                );
                // In JavaScript, `[] != []` (???), so use a length comparison instead.
                chai.assert.strictEqual(editor_to_code_doc_blocks().length, 0);
            });
            test("Load a code block", async function () {
                const cdb: code_or_doc_block[] = [["", "", "a = 1;\nb = 2;\n"]];
                await open_lp(
                    {
                        metadata: {
                            mode: "javascript",
                        },
                        code_doc_block_arr: cdb,
                    },
                    EditorMode.edit
                );
                chai.assert.deepEqual(editor_to_code_doc_blocks(), cdb);
            });
            test("Load a doc block", async function () {
                await open_lp(
                    {
                        metadata: {
                            mode: "javascript",
                        },
                        code_doc_block_arr: [["", "//", "This is a doc block"]],
                    },
                    EditorMode.edit
                );
                const actual_cdb = editor_to_code_doc_blocks();
                const expected_cdb: code_or_doc_block[] = [
                    ["", "//", "<p>This is a doc block</p>\n"],
                ];
                chai.assert.deepEqual(actual_cdb, expected_cdb);
            });
            test("Load a mixed code and doc block", async function () {
                const cdb: code_or_doc_block[] = [
                    ["", "", "a = 1;\nb = 2;\n"],
                    ["", "//", "This is a doc block"],
                ];
                await open_lp(
                    {
                        metadata: {
                            mode: "javascript",
                        },
                        code_doc_block_arr: cdb,
                    },
                    EditorMode.edit
                );
                const actual_cdb = editor_to_code_doc_blocks();
                chai.assert.deepEqual(actual_cdb, cdb);
            });
        });
    });

    // Avoid an infinite loop of tests calling this again.
    delete window.CodeChatEditor_test;
    mocha.run();
};
