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
// <h1><code>ace-webpack.ts</code> &mdash; imports the Ace editor from NPM packages using webpack</h1>
import { Ace, config, edit } from "ace-code";

// Export ace.edit globally.
(window as any).ace = {};
(window as any).ace.edit = edit;
(window as any).ace.config = config;

export { edit, config };

// Import the Ace theme to use.
import "ace-code/src/theme/textmate";

// Optionally,  import a keyboard binding. (The default binding is Ace, which is built in.) See the <a href="https://ace.c9.io/build/kitchen-sink.html">Ace kitchen sink</a> demo for the available options.

// Import any extensions. There's not a lot of docs on these; see the ``ace/ext`` directory.

// <h2>Dynamic imports</h2>
// This is the type of a callback for the Ace editor's import system.
type Callback = (err: string | null, module: any) => Promise<void>;

// The Ace type definitions omit this function, which we need to call to use a dynamic loader.
interface ConfigAll extends Ace.Config {
    setLoader(loader: (moduleName: string, callback: Callback) => void): void;
}

// Define a new loader which uses the webpack dynamic import system.
(config as ConfigAll).setLoader((moduleName: string, callback: Callback) => {
    const dynamicAceImports: { [moduleName: string]: () => Promise<void> } = {
        // Note: all these dynamic imports rely on typing.d.ts to fix the lack of types for these files.
        "./theme/textmate": () => import("ace-code/src/theme/textmate"),
        "ace/theme/textmate": () => import("ace-code/src/theme/textmate"),
        "ace/mode/javascript": () => import("ace-code/src/mode/javascript"),
    };

    // Look up the module name. If nothing is found, output a warning message.
    const dynamic_import = dynamicAceImports[moduleName];
    if (dynamic_import) {
        // Given the promised results of an import(), invoke a callback when the promise resolves or rejects.
        return dynamic_import().then(
            (module) => callback(null, module),
            (err) => callback(err, null)
        );
    } else {
        // Complain if we don't recognize this import.
        const err = `Unknown Ace dynamic import of ${moduleName}`;
        console.log(err);
        callback(err, null);
        return Promise.resolve(null);
    }
});
