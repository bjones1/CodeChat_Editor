// # `HashReader.mts` -- Read the output produced by esbuild to determine the location of the output files, which have hashes in their file names.

import fs from 'node:fs/promises';

// Copied from the [esbuild docs](https://esbuild.github.io/api/#metafile).
interface Metafile {
    inputs: {
        [path: string]: {
            bytes: number
            imports: {
                path: string
                kind: string
                external?: boolean
                original?: string
                with?: Record<string, string>
            }[]
            format?: string
            with?: Record<string, string>
        }
    }
    outputs: {
        [path: string]: {
            bytes: number
            inputs: {
                [path: string]: {
                    bytesInOutput: number
                }
            }
            imports: {
                path: string
                kind: string
                external?: boolean
            }[]
            exports: string[]
            entryPoint?: string
            cssBundle?: string
        }
    }
}

// Load the esbuild metafile.
const data = await fs.readFile('meta.json', { encoding: 'utf8' });

// Interpret it as JSON.
const metafile: Metafile = JSON.parse(data)

// Walk the file, looking for the output names of given entry points. Transform those into paths used to import these files.
let outputContents: Record<string, string> = {}
for (const output in metafile.outputs) {
    const outputInfo = metafile.outputs[output]
    switch (outputInfo.entryPoint) {
        case "src/CodeChatEditorFramework.mts":
            outputContents["CodeChatEditorFramework.js"] = output
            break

        case "src/CodeChatEditor.mts":
            outputContents["CodeChatEditor.js"] = output
            outputContents["CodeChatEditor.css"] = outputInfo.cssBundle!
            break

        case "src/CodeChatEditor-test.mts":
            outputContents["CodeChatEditor-test.js"] = output
            outputContents["CodeChatEditor-test.css"] = outputInfo.cssBundle!
    }
}

// Write this to disk.
await fs.writeFile('../server/hashLocations.json', JSON.stringify(outputContents));