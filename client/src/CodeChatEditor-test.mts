// <details>
//     <summary>Copyright (C) 2022 Bryan A. Jones.</summary>
//     <p>This file is part of the CodeChat Editor.</p>
//     <p>The CodeChat Editor is free software: you can redistribute it and/or
//         modify it under the terms of the GNU General Public License as
//         published by the Free Software Foundation, either version 3 of the
//         License, or (at your option) any later version.</p>
//     <p>The CodeChat Editor is distributed in the hope that it will be useful,
//         but WITHOUT ANY WARRANTY; without even the implied warranty of
//         MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
//         General Public License for more details.</p>
//     <p>You should have received a copy of the GNU General Public License
//         along with the CodeChat Editor. If not, see <a
//             href="http://www.gnu.org/licenses/">http://www.gnu.org/licenses/</a>.
//     </p>
// </details>
// <h1><code>CodeChatEditor-test.mts</code> &mdash; Tests for the CodeChat
//     Editor client</h1>
// <p>To run tests, add a <code>?test</code> to any web page served by the
//     CodeChat Editor server.</p>
// <h2>Imports</h2>
// <p>I can't get Mocha to work with ESBuild, so I import it using a script tag.
// </p>
import chai from "chai";
import {
    exportedForTesting,
    code_or_doc_block,
    page_init,
    on_keydown,
    on_save,
} from "./CodeChatEditor.mjs";

// <p>Re-export everything that <a
//         href="CodeChatEditor.mts">CodeChatEditor.mts</a> exports. Otherwise,
//     including <a href="CodeChatEditor.mts">CodeChatEditor.mts</a> elsewhere
//     would double-define everything (producing complaints about two attempts
//     to define each web component).</p>
export { page_init, on_keydown, on_save };
// <p>Provide convenient access to all functions tested here.</p>
const { editor_to_code_doc_blocks, EditorMode, open_lp } = exportedForTesting;

// <h2>Tests</h2>
// <p><a id="CodeChatEditor_test"></a>Defining this global variable signals the
//     CodeChat Editor to <a href="CodeChatEditor.mts#CodeChatEditor_test">run
//         tests</a>.</p>
window.CodeChatEditor_test = () => {
    // <p>See the <a href="https://mochajs.org/#browser-configuration">Mocha
    //         docs</a>.</p>
    mocha.setup({
        ui: "tdd",
        // <p>This is required to use Mocha's global teardown from the browser,
        //     AFAIK.</p>
        /// @ts-ignore
        globalTeardown: [
            () => {
                // <p>On teardown, put the Mocha div at the beginning of the
                //     body. Testing causes body to be wiped, so don't do this
                //     until all tests are done.</p>
                const mocha_div = document.getElementById("mocha")!;
                const ccb = document.getElementById("CodeChat-body")!;
                ccb.insertBefore(mocha_div, ccb.firstChild);
            },
        ],
    });

    // <p>Define some tests. See the <a href="https://mochajs.org/#tdd">Mocha
    //         TDD docs</a> and the <a
    //         href="https://www.chaijs.com/api/assert/">Chai assert API</a>.
    // </p>
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
                // <p>In JavaScript, <code>[] != []</code> (???), so use a
                //     length comparison instead.</p>
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

    // <p>Avoid an infinite loop of tests calling this again.</p>
    delete window.CodeChatEditor_test;
    mocha.run();
};
