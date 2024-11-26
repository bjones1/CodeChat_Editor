// Copyright (C) 2023 Bryan A. Jones.
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
// # `HashReader.mts` -- post-process esbuild output
//
// This script reads the output produced by esbuild to determine the location of
// the bundled files, which have hashes in their file names. It writes these
// results to a simple JSON file, which the CodeChat Editor Server reads.

import fs from "node:fs/promises";

// Copied from the [esbuild docs](https://esbuild.github.io/api/#metafile).
interface Metafile {
    inputs: {
        [path: string]: {
            bytes: number;
            imports: {
                path: string;
                kind: string;
                external?: boolean;
                original?: string;
                with?: Record<string, string>;
            }[];
            format?: string;
            with?: Record<string, string>;
        };
    };
    outputs: {
        [path: string]: {
            bytes: number;
            inputs: {
                [path: string]: {
                    bytesInOutput: number;
                };
            };
            imports: {
                path: string;
                kind: string;
                external?: boolean;
            }[];
            exports: string[];
            entryPoint?: string;
            cssBundle?: string;
        };
    };
}

// Load the esbuild metafile.
const data = await fs.readFile("meta.json", { encoding: "utf8" });

// Interpret it as JSON.
const metafile: Metafile = JSON.parse(data);

// Walk the file, looking for the names of specific entry points. Transform
// those into paths used to import these files.
let outputContents: Record<string, string> = {};
let num_found = 0;
for (const output in metafile.outputs) {
    const outputInfo = metafile.outputs[output];
    switch (outputInfo.entryPoint) {
        case "src/CodeChatEditorFramework.mts":
            outputContents["CodeChatEditorFramework.js"] = output;
            ++num_found;
            break;

        case "src/CodeChatEditor.mts":
            outputContents["CodeChatEditor.js"] = output;
            outputContents["CodeChatEditor.css"] = outputInfo.cssBundle!;
            ++num_found;
            break;

        case "src/CodeChatEditor-test.mts":
            outputContents["CodeChatEditor-test.js"] = output;
            outputContents["CodeChatEditor-test.css"] = outputInfo.cssBundle!;
            ++num_found;
            break;

        case "src/css/CodeChatEditorProject.css":
            outputContents["CodeChatEditorProject.css"] = output;
            ++num_found;
            break;
    }
}

console.assert(num_found === 4);

// Write this to disk.
await fs.writeFile(
    "../server/hashLocations.json",
    JSON.stringify(outputContents),
);

console.log("Wrote hashLocations.json");
