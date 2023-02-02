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
// <h1><code>ace-webpack.ts</code> &mdash; imports the Ace editor from NPM
//     packages using webpack</h1>
import ace, { Ace, config } from "ace-code";

export { ace };

// <p>Import the Ace theme to use.</p>
import "ace-code/src/theme/textmate";

// <p>Optionally, import a keyboard binding. (The default binding is Ace, which
//     is built in.) See the <a
//         href="https://ace.c9.io/build/kitchen-sink.html">Ace kitchen sink</a>
//     demo for the available options.</p>
// <p>Import any extensions. There's not a lot of docs on these; see the
//     ``ace/ext`` directory.</p>
// <h2>Dynamic imports</h2>
// <p>This is the type of a callback for the Ace editor's import system.</p>
type Callback = (err: string | null, module: any) => Promise<void>;

// <p>The Ace type definitions omit this function, which we need to call to use
//     a dynamic loader.</p>
interface ConfigAll extends Ace.Config {
    setLoader(loader: (moduleName: string, callback: Callback) => void): void;
}

// <p>Define a new loader which uses the esbuild dynamic import system.</p>
(config as ConfigAll).setLoader((moduleName: string, callback: Callback) => {
    const dynamicAceImports: { [moduleName: string]: () => Promise<any> } = {
        // <p>Note: all these dynamic imports rely on typing.d.ts to fix the
        //     lack of types for these files.</p>
        // <p>Themes</p>
        "./theme/textmate": () => import("ace-code/src/theme/textmate"),
        "ace/theme/textmate": () => import("ace-code/src/theme/textmate"),

        // <p>Modes</p>
        "ace/mode/c_cpp": () => import("ace-code/src/mode/c_cpp"),
        "ace/mode/csharp": () => import("ace-code/src/mode/csharp"),
        "ace/mode/css": () => import("ace-code/src/mode/css"),
        "ace/mode/golang": () => import("ace-code/src/mode/golang"),
        "ace/mode/java": () => import("ace-code/src/mode/java"),
        "ace/mode/javascript": () => import("ace-code/src/mode/javascript"),
        "ace/mode/json5": () => import("ace-code/src/mode/json5"),
        "ace/mode/html": () => import("ace-code/src/mode/html"),
        "ace/mode/python": () => import("ace-code/src/mode/python"),
        "ace/mode/rust": () => import("ace-code/src/mode/rust"),
        "ace/mode/sql": () => import("ace-code/src/mode/sql"),
        "ace/mode/toml": () => import("ace-code/src/mode/toml"),
        "ace/mode/typescript": () => import("ace-code/src/mode/typescript"),
        "ace/mode/verilog": () => import("ace-code/src/mode/verilog"),
        "ace/mode/yaml": () => import("ace-code/src/mode/yaml"),
    };

    // <p>Look up the module name. If nothing is found, output a warning
    //     message.</p>
    const dynamic_import = dynamicAceImports[moduleName];
    if (dynamic_import) {
        // <p>Given the promised results of an import(), invoke a callback when
        //     the promise resolves or rejects.</p>
        return dynamic_import().then(
            // <p>Using <code>module.default</code> here works around bug when
            //     esbuild's splitting is used -- it seems to pick the wrong
            //     type of import in this case. See the <a
            //         href="https://esbuild.github.io/content-types/#default-interop">ESBuild
            //         docs on the default export</a>. When not using the
            //     splitting feature, this should be just <code>module</code>.
            // </p>
            (module) => callback(null, module.default),
            (err) => callback(err, null)
        );
    } else {
        // <p>Complain if we don't recognize this import.</p>
        const err = `Unknown Ace dynamic import of ${moduleName}`;
        console.log(err);
        callback(err, null);
        return Promise.resolve(null);
    }
});
