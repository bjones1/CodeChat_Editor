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
// <p>To run tests, add a <code>?test</code> to any web page server by the
//     CodeChat Editor server.</p>
// <h2>Imports</h2>
// <p>I can't get Mocha to work with ESBuild, so I import it using a script tag.
// </p>
import chai from "chai";

// <h2>Tests</h2>
window.CodeChatEditor.test = () => {
    // <p>Put a Mocha div at the beginning of the body. Other JS replaces
    //     everything in the body, then calls this when it's done.</p>
    let div = document.createElement("div");
    div.id = "mocha";
    const ccb = document.getElementById("CodeChat-body")!;
    ccb.insertBefore(div, ccb.firstChild);

    // <p>Define some tests. See the <a href="https://mochajs.org/#tdd">Mocha
    //         TDD docs</a> and the <a
    //         href="https://www.chaijs.com/api/assert/">Chai assert API</a>.
    // </p>
    mocha.setup("tdd");
    suite("Array", function () {
        suite("#indexOf()", function () {
            test("should return -1 when the value is not present", function () {
                chai.assert.deepEqual(
                    [1, 2, { three: 4 }],
                    [1, 2, { three: 4 }]
                );
            });
        });
    });

    mocha.run();
};
