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

// Load the esbuild metafiles.
const data_client_framework = await fs.readFile("metaClientFramework.json", {
    encoding: "utf8",
});
const data_client = await fs.readFile("metaClient.json", { encoding: "utf8" });

// Interpret it as JSON.
const metafile_client_framework: Metafile = JSON.parse(data_client_framework);
const metafile_client: Metafile = JSON.parse(data_client);

// Walk the file, looking for the names of specific entry points. Transform
// those into paths used to import these files.
let outputContents: Record<string, string> = {};
let num_found = 0;
for (const metafile of [metafile_client_framework, metafile_client]) {
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
                outputContents["CodeChatEditor-test.css"] =
                    outputInfo.cssBundle!;
                ++num_found;
        }
    }
}
console.assert(num_found === 3);

// Write this to disk.
await fs.writeFile(
    "../server/hashLocations.json",
    JSON.stringify(outputContents),
);

console.log("Wrote hashLocations.json");
