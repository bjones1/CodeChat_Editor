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
// <h1><code>CodeChat-editor.mts</code> &mdash; TypeScript which implements part
//     of the client-side portion of the CodeChat Editor</h1>
// <p>The overall process of load a file is:</p>
// <ol>
//     <li>The user browses to a file on the local machine, using the very
//         simple file browser webpage provided by the CodeChat Server. Clicking
//         on this file starts the process of loading a file into the CodeChat
//         editor.</li>
//     <li>The server sees a request for a file supported by the CodeChat
//         Editor. It lexes the files into code and doc blocks, then wraps these
//         in a webpage which contains this program (the CodeChat Editor).</li>
//     <li>On load, this program (the CodeChat Editor) transforms these code and
//         doc blocks into HTML. Specifically, code blocks are placed in <a
//             href="https://ace.c9.io/">ACE editor</a> instances, while doc
//         blocks are placed in <a href="https://www.tiny.cloud/">TinyMCE</a>
//         instances.</li>
// </ol>
// <p>The user then uses the editing capabilities of ACE/TinyMCE to edit their
//     program. When the user saves a file:</p>
// <ol>
//     <li>This program looks through the HTML, converting ACE editor/TinyMCE
//         instances back into code blocks and doc blocks.</li>
//     <li>It sends these code/doc blocks back to the server.</li>
//     <li>The server then transforms these code/doc blocks into source code,
//         then writes this code to the disk.</li>
// </ol>
// <h2>Imports</h2>
// <h3>JavaScript/TypeScript</h3>
import "./EditorComponents.mjs";
import "./graphviz-webcomponent-setup.mts";
import "./graphviz-webcomponent/index.min.mjs";
import { html_beautify } from "js-beautify";
import { tinymce, tinymce_init } from "./tinymce-webpack.mjs";

import { javascript } from "@codemirror/lang-javascript";
import { basicSetup } from "codemirror";
import {
    EditorView,
    Decoration,
    DecorationSet,
    DOMEventMap,
    ViewUpdate,
    ViewPlugin,
    keymap,
    WidgetType,
} from "@codemirror/view";
import {
    ChangeDesc,
    EditorState,
    StateField,
    StateEffect,
    EditorSelection,
    Transaction,
} from "@codemirror/state";
import { syntaxTree } from "@codemirror/language";

// <h3>CSS</h3>
import "./../static/css/CodeChatEditor.css";

// <h2>Initialization</h2>
// <p>Load code when the DOM is ready.</p>
export const page_init = (all_source: any) => {
    // <p>Use <a
    //         href="https://developer.mozilla.org/en-US/docs/Web/API/URLSearchParams">URLSearchParams</a>
    //     to parse out the search parameters of this window's URL.</p>
    const urlParams = new URLSearchParams(window.location.search);
    // <p>Get the mode from the page's query parameters. Default to edit using
    //     the <a
    //         href="https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Operators/Nullish_coalescing_operator">nullish
    //         coalescing operator</a>. This works, but TypeScript marks it as
    //     an error. Ignore this error by including the <a
    //         href="https://www.typescriptlang.org/docs/handbook/intro-to-js-ts.html#ts-check">@ts-ignore
    //         directive</a>.</p>
    /// @ts-ignore
    const editorMode = EditorMode[urlParams.get("mode") ?? "edit"];
    on_dom_content_loaded(() => open_lp(all_source, editorMode));
};

// <p>This is copied from <a
//         href="https://developer.mozilla.org/en-US/docs/Web/API/Document/DOMContentLoaded_event#checking_whether_loading_is_already_complete">MDN</a>.
// </p>
const on_dom_content_loaded = (on_load_func: () => void) => {
    if (document.readyState === "loading") {
        // <p>Loading hasn't finished yet.</p>
        document.addEventListener("DOMContentLoaded", on_load_func);
    } else {
        // <p><code>DOMContentLoaded</code> has already fired.</p>
        on_load_func();
    }
};

// <p><a id="EditorMode"></a>Define all possible editor modes; these are passed
//     as a <a href="https://en.wikipedia.org/wiki/Query_string">query
//         string</a> (<code>http://path/to/foo.py?mode=toc</code>, for example)
//     to the page's URL.</p>
enum EditorMode {
    // <p>Display the source code using CodeChat, but disallow editing.</p>
    view,
    // <p>For this source, the same a view; the server uses this to avoid
    //     recursive iframes of the table of contents.</p>
    toc,
    // <p>The full CodeChat editor.</p>
    edit,
    // <p>Show only raw source code; ignore doc blocks, treating them also as
    //     code.</p>
    raw,
}

// <p>This function is called on page load to "load" a file. Before this point,
//     the server has already lexed the source file into code and doc blocks;
//     this function transforms the code and doc blocks into HTML and updates
//     the current web page with the results.</p>
const open_lp = (
    // <p>A data structure provided by the server, containing the source and
    //     associated metadata. See <a
    //         href="#AllSource"><code>AllSource</code></a>.</p>
    all_source: LexedSourceFile,
    // <p>See <code><a href="#EditorMode">EditorMode</a></code>.</p>
    editorMode: EditorMode
) => {
    // <p>Get the <code><a href="#current_metadata">current_metadata</a></code>
    //     from the provided <code>all_source</code> struct and store it as a
    //     global variable.</p>
    current_metadata = all_source["metadata"];
    const source = all_source["source"];
    const codechat_body = document.getElementById("CodeChat-body")!;
    if (is_doc_only()) {
        // <p>Special case: a CodeChat Editor document's HTML doesn't need
        //     lexing; it only contains HTML. Instead, its structure is always:
        //     <code>[["", "", HTML]]</code>. Therefore, the HTML is at item
        //     [0][2].</p>
        codechat_body.innerHTML = `<div class="CodeChat-TinyMCE">${source.doc}</div>`;
    } else {
        codechat_body.innerHTML = '<div class="CodeChat-CodeMirror"></div>';
        source.selection = EditorSelection.single(0).toJSON();
        const initialState = EditorState.fromJSON(
            source,
            {
                extensions: [
                    javascript(),
                    basicSetup,
                    underlineKeymap,
                    EditorView.lineWrapping,
                    DocBlockPlugin,
                ],
            },
            { doc_blocks: docBlockField }
        );
        const view = new EditorView({
            parent: codechat_body,
            state: initialState,
        });
    }
};

// <p>This defines a single code or doc block entry:</p>
export type code_or_doc_block = [
    // <p>The indent for a doc bloc; empty for a code block.</p>
    string,
    // <p>The opening comment delimiter for a doc block;</p>
    string | null,
    string
];

// <p>The server passes this to the client to load a file. See <a
//         href="../../server/src/webserver.rs#LexedSourceFile">LexedSourceFile</a>.
// </p>
type LexedSourceFile = {
    metadata: { mode: string };
    source: {
        doc: string;
        doc_blocks: [DocBlockJSON];
        selection: any;
    };
};

// How a doc block is stored using CodeMirror.
type DocBlockJSON = [number, number, string, string, string];

// <p>Store the lexer info for the currently-loaded language.</p>
// <p><a id="current_metadata"></a>This mirrors the data provided by the server
//     -- see <a
//         href="../../server/src/webserver.rs#SourceFileMetadata">SourceFileMetadata</a>.
// </p>
let current_metadata: {
    mode: string;
};

// <p>Tell TypeScript about the global namespace this program defines.</p>
declare global {
    interface Window {
        CodeChatEditor_test: any;
    }
}

// <h2>Editors</h2>
// <p>This code instantiates editors/viewers for code and doc blocks.</p>
const make_editors = async (
    // <p>A instance of the <code>EditorMode</code> enum.</p>
    editorMode: EditorMode
) => {
    return new Promise((accept) => {
        setTimeout(async () => {
            // <p>Set up for editing the indent of doc blocks.</p>
            for (const td of document.querySelectorAll(
                ".CodeChat-doc-indent"
            )) {
                // <p>While this follows the <a
                //         href="https://developer.mozilla.org/en-US/docs/Web/API/HTMLElement/beforeinput_event">MDN
                //         docs</a> and also works, TypeScript still reports an
                //     error. Suppress it.</p>
                /// @ts-ignore
                td.addEventListener(
                    "beforeinput",
                    doc_block_indent_on_before_input
                );
            }

            // <p><a id="CodeChatEditor_test"></a>If tests should be run, then
            //     the <a
            //         href="CodeChatEditor-test.mts#CodeChatEditor_test">following
            //         global variable</a> is function that runs them.</p>
            if (typeof window.CodeChatEditor_test === "function") {
                window.CodeChatEditor_test();
            }

            accept("");
        });
    });
};

// <p>Instantiate a doc block editor (TinyMCE).</p>
const make_doc_block_editor = (
    // <p>CSS selector to specify which HTML elements should be editable using
    //     TinyMCE.</p>
    selector: string
) => {
    return tinymce_init({
        // <p>Enable the <a
        //         href="https://www.tiny.cloud/docs/tinymce/6/spelling/#browser_spellcheck">browser-supplied
        //         spellchecker</a>, since TinyMCE's spellchecker is a premium
        //     feature.</p>
        browser_spellcheck: true,
        // <p>Put more buttons on the <a
        //         href="https://www.tiny.cloud/docs/tinymce/6/quickbars/">quick
        //         toolbar</a> that appears when text is selected. TODO: add a
        //     button for code format (can't find this one -- it's only on the
        //     <a
        //         href="https://www.tiny.cloud/docs/tinymce/6/available-menu-items/#the-core-menu-items">list
        //         of menu items</a> as <code>codeformat</code>).</p>
        quickbars_selection_toolbar:
            "align | bold italic underline | quicklink h2 h3 blockquote",
        // <p>Place the Tiny MCE menu bar at the top of the screen; otherwise,
        //     it floats in front of text, sometimes obscuring what the user
        //     wants to edit. See the <a
        //         href="https://www.tiny.cloud/docs/configure/editor-appearance/#fixed_toolbar_container">docs</a>.
        // </p>
        fixed_toolbar_container: "#CodeChat-menu",
        inline: true,
        // <p>When true, this still prevents hyperlinks to anchors on the
        //     current page from working correctly. There's an onClick handler
        //     that prevents links in the current page from working -- need to
        //     look into this. See also <a
        //         href="https://github.com/tinymce/tinymce/issues/3836">a
        //         related GitHub issue</a>.</p>
        //readonly: true  // Per the comment above, this is commented out.
        // <p>TODO: Notes on this setting.</p>
        relative_urls: true,
        selector: selector,
        // <p>This combines the <a
        //         href="https://www.tiny.cloud/blog/tinymce-toolbar/">default
        //         TinyMCE toolbar buttons</a> with a few more from plugins. I
        //     like the default, so this is currently disabled.</p>
        //toolbar: 'undo redo | styleselect | bold italic | alignleft aligncenter alignright alignjustify | outdent indent | numlist bullist | ltr rtl | help',

        // <p>Settings for plugins</p>
        // <p><a
        //         href="https://www.tiny.cloud/docs/plugins/opensource/image/">Image</a>
        // </p>
        image_caption: true,
        image_advtab: true,
        image_title: true,
        // <p>Needed to allow custom elements.</p>
        extended_valid_elements:
            "graphviz-graph[graph|scale],graphviz-script-editor[value|tab],graphviz-combined[graph|scale]",
        custom_elements:
            "graphviz-graph,graphviz-script-editor,graphviz-combined",
    });
};

// <h2>Doc blocks in CodeMirror</h2>
// <p>The goal: given a <a href="https://codemirror.net/docs/ref/#state.Range">Range</a> of lines containing a doc block (a delimiter, indent, and contents) residing at these lines, <a href="https://codemirror.net/docs/ref/#view.Decoration^replace">replace</a> them with a widget which allows editing of the doc block.</p>
// <p>First, define a <a href="https://codemirror.net/docs/ref/#state.StateField^define">StateField</a>: the state required to store all doc blocks. Conveniently, a <a href="https://codemirror.net/docs/ref/#view.DecorationSet">DecorationSet</a> is a <a href="https://codemirror.net/docs/ref/#state.RangeSet">RangeSet</a>&lt;<a href="https://codemirror.net/docs/ref/#view.Decoration">Decoration</a>&gt;, each element of which contains the required range and the needed HTML in the Decoration -- all the required state. Making it a DecorationSet provides an easy way to store all doc blocks.
const docBlockField = StateField.define<DecorationSet>({
    // <a href="https://codemirror.net/docs/ref/#state.StateField^define^config.create">Create</a> the initial value for the field, which is an empty set (no doc blocks). Therefore, simply return an empty DecorationSet (oddly, the type of <a href="https://codemirror.net/docs/ref/#view.Decoration^none">Decoration.none</a>).
    create(state: EditorState) {
        return Decoration.none;
    },

    // <a href="https://codemirror.net/docs/ref/#state.StateField^define^config.update">Update</a> computes a new value for this field from the field's previous value and the provided transaction.
    update(doc_blocks: DecorationSet, tr: Transaction) {
        // <a href="https://codemirror.net/docs/ref/#state.RangeSet.map">Map</a> these changes through the provided transaction, which updates the offsets of the range so the doc blocks is still anchored to the same location in the document after this transaction completes.
        doc_blocks = doc_blocks.map(tr.changes);
        // See <a href="https://codemirror.net/docs/ref/#state.StateEffect.is">is</a>.
        // Add a doc block, as requested by this effect. TODO: add cases to handle combining two adjacent doc blocks, deleting a doc block, etc.
        for (let effect of tr.effects)
            if (effect.is(addDocBlock)) {
                // Perform an <a href="https://codemirror.net/docs/ref/#state.RangeSet.update">update</a> by adding the requested doc block.
                doc_blocks = doc_blocks.update({
                    // See <a href="https://codemirror.net/docs/ref/#state.RangeSet.update^updateSpec">updateSpec</a>
                    add: [
                        // <a href="https://codemirror.net/docs/ref/#view.Decoration^replace">Replace</a> the code (empty lines saving space for the doc block) with the doc block contents, rendered using a GUI editor.
                        Decoration.replace({
                            widget: new DocBlockWidget(
                                effect.value.indent,
                                effect.value.delimiter,
                                effect.value.content,
                                null
                            ),
                            block: true,
                        }).range(effect.value.from, effect.value.to),
                    ],
                });
            }

            // Perform an update to a doc block.
            else if (effect.is(updateDocBlock)) {
                // The view provides only the to value (the position); use this to find the from value for the doc block to update.
                const from = effect.value.pos;
                let to;
                doc_blocks.between(
                    from,
                    from,
                    (_from: number, _to: number, doc_block: Decoration) => {
                        to = _to;
                        // Assume that there's only one doc block for this range: stop looking for any others.
                        return false;
                    }
                );
                // Remove the old doc block and create a new one to replace it. (Recall that this is the functional approach required by CodeMirror -- state is immutable.)
                doc_blocks = doc_blocks.update({
                    // Remove the old doc block. We assume there's only one block in the provided from/to range.
                    filter: (from, to, doc_block) => false,
                    filterFrom: from,
                    filterTo: to,
                    // This adds the replacement doc block with updated indent/delimiter/content.
                    add: [
                        Decoration.replace({
                            widget: new DocBlockWidget(
                                effect.value.indent,
                                effect.value.delimiter,
                                effect.value.content,
                                effect.value.dom
                            ),
                            block: true,
                        }).range(
                            from,
                            // We know that the to value will always be found; make TypeScript happy.
                            to as unknown as number
                        ),
                    ],
                });
            }
        return doc_blocks;
    },

    // <a href="https://codemirror.net/docs/ref/#state.StateField^define^config.provide">Provide</a> extensions based on this field. See also <a href="https://codemirror.net/docs/ref/#view.EditorView^decorations">EditorView.decorations</a> and <a href="unknown">from</a>. TODO: I don't understand what this does, but removing it breaks the extension.
    provide: (field: StateField<DecorationSet>) =>
        EditorView.decorations.from(field),

    // Define a way to serialize this field; see <a href="https://codemirror.net/docs/ref/#state.StateField^define^config.toJSON">toJSON</a>. This provides a straightforward path to transform the entire editor's contents (including these doc blocks) to JSON, which can then be sent back to the server for reassembly into a source file.
    toJSON: (value: DecorationSet, state: EditorState) => {
        let json = [];
        for (const iter = value.iter(); iter.value !== null; iter.next()) {
            const w = iter.value.spec.widget;
            json.push([iter.from, iter.to, w.indent, w.delimiter, w.contents]);
        }
        return json;
    },

    // For loading a file from the server back into the editor, use <a href="https://codemirror.net/docs/ref/#state.StateField^define^config.fromJSON">fromJSON</a>.
    fromJSON: (json: any, state: EditorState) =>
        Decoration.set(
            json.map(([from, to, indent, delimiter, contents]: DocBlockJSON) =>
                Decoration.replace({
                    widget: new DocBlockWidget(
                        indent,
                        delimiter,
                        contents,
                        null
                    ),
                    block: true,
                }).range(from, to)
            )
        ),
});

// <p>Per the <a href="https://codemirror.net/docs/ref/#state.StateEffect^define">docs</a>, "State effects can be used to represent additional effects associated with a transaction. They are often useful to model changes to custom state fields, when those changes aren't implicit in document or selection changes." In this case, provide a way to add a doc block.</p>
const addDocBlock = StateEffect.define<{
    from: number;
    to: number;
    indent: string;
    delimiter: string;
    content: string;
}>({
    map: ({ from, to, indent, delimiter, content }, change: ChangeDesc) => ({
        // Update the location (from/to) of this doc block due to the transaction's changes.
        from: change.mapPos(from),
        to: change.mapPos(to),
        indent,
        delimiter,
        content,
    }),
});

// Define an update. Note that we have only a position (the only data a view can gather), rather than a from/to.
const updateDocBlock = StateEffect.define<{
    pos: number;
    indent: string;
    delimiter: string;
    content: string;
    dom: HTMLDivElement;
}>({
    map: ({ pos, indent, delimiter, content, dom }, change: ChangeDesc) => ({
        // Update the position of this doc block due to the transaction's changes.
        pos: change.mapPos(pos),
        indent,
        delimiter,
        content,
        dom,
    }),
});

// Create a <a href="https://codemirror.net/docs/ref/#view.WidgetType">widget</a> which contains a doc block.
class DocBlockWidget extends WidgetType {
    constructor(
        readonly indent: string,
        readonly delimiter: string,
        readonly contents: string,
        // Only used in an update to avoid changing an already-modified doc block.
        readonly dom: null | HTMLDivElement
    ) {
        // TODO: I don't understand why I don't need to store the provided parameters in the object: <code>this.indent = indent;</code>, etc.
        super();
    }

    eq(other: DocBlockWidget) {
        return (
            other.indent == this.indent &&
            other.delimiter == this.delimiter &&
            other.contents == this.contents
        );
    }

    // See <a href="https://codemirror.net/docs/ref/#view.WidgetType.toDOM">toDom</a>.
    toDOM() {
        // Wrap this in an enclosing div.
        let wrap = document.createElement("div");
        wrap.className = "CodeChat-doc";
        wrap.innerHTML =
            // <p>This doc block's indent. TODO: allow paste, but must
            //     only allow pasting whitespace.</p>
            `<div class="CodeChat-doc-indent" contenteditable onpaste="return false">${this.indent}</div>` +
            // <p>The contents of this doc block.</p>
            `<div class="CodeChat-TinyMCE" contenteditable>` +
            this.contents +
            "</div>";
        return wrap;
    }

    // Per the <a href="https://codemirror.net/docs/ref/#view.WidgetType.updateDOM">docs</a>, "Update a DOM element created by a widget of the same type (but different, non-eq content) to reflect this widget."
    updateDOM(dom: HTMLElement, view: EditorView): boolean {
        // If this update has already been made to the provided DOM, then we're done. TODO: does this actually improve performance?
        if (this.dom === dom) {
            return true;
        }

        (dom.childNodes[0] as HTMLDivElement).innerHTML = this.indent;

        // The contents div could be a TinyMCE instance, or just a plain div. Handle both cases.
        const [contents_div, tinymce_inst] = get_contents(dom);
        if (tinymce_inst === null) {
            contents_div.innerHTML = this.contents;
        } else {
            tinymce_inst.setContent(this.contents);
        }

        // Indicate the update was successful.
        return true;
    }

    ignoreEvent() {
        // Don't ignore events that happen in this widget, so the view can handle them correctly.
        return false;
    }

    destroy(dom: HTMLElement): void {
        const [contents_div, tinymce_inst] = get_contents(dom);
        if (tinymce_inst !== null) {
            tinymce_inst.remove();
        }
    }
}

// Given a doc block div element, return the TinyMCE instance and the div it's rooted in.
const get_contents = (element: HTMLElement) => {
    const contents_div = element.childNodes[1] as HTMLDivElement;
    const tinymce_inst = tinymce.get(contents_div.id);
    return [contents_div, tinymce_inst];
};

const DocBlockPlugin = ViewPlugin.fromClass(
    class {
        constructor(view: EditorView) {}

        update(update: ViewUpdate) {
            // TODO: make this much less expensive. It's called very frequently.
            make_doc_block_editor(".CodeChat-TinyMCE");
        }
    },
    {
        eventHandlers: {
            mousedown: (event: MouseEvent, view: EditorView) => {
                if (event_is_in_doc_block(event)) {
                    return null;
                } else {
                    return false;
                }
            },

            input: (event: Event, view: EditorView) => {
                const target_or_false = event_is_in_doc_block(event);
                if (target_or_false) {
                    // Send an update to the state field associated with this DOM element.
                    const target = target_or_false as HTMLDivElement;
                    const pos = view.posAtDOM(target);
                    const indent = (target.childNodes[0] as HTMLDivElement)
                        .innerHTML;
                    const [contents_div, tinymce_inst] = get_contents(target);
                    const content =
                        tinymce_inst === null
                            ? contents_div.innerHTML
                            : tinymce_inst.getContent();
                    let effects: StateEffect<unknown>[] = [
                        updateDocBlock.of({
                            pos,
                            indent,
                            delimiter: "",
                            content: content,
                            dom: target,
                        }),
                    ];

                    view.dispatch({ effects });
                    return null;
                } else {
                    return false;
                }
            },
        },
    }
);

// Determine if the element which generated the provided event was in a doc block or not.
const event_is_in_doc_block = (event: Event): boolean | HTMLDivElement => {
    let target = event.target as HTMLElement;
    while (target.parentElement) {
        // If we find any CodeMirror element, this isn't a doc block.
        if (target.className.includes("cm-")) {
            return false;
        }
        // If it's a doc block, then tell Code Mirror not to handle this event.
        if (target.classList.contains("CodeChat-doc")) {
            return target as HTMLDivElement;
        }
        // Keep searching higher in the DOM,
        target = target.parentElement;
    }
    // We shouldn't reach here; if so, it's definitely not a doc block.
    return false;
};

function underlineSelection(view: EditorView) {
    const doc = view.state.doc;
    console.log(view.state.toJSON({ doc_blocks: docBlockField }));
    return true;
    let effects: StateEffect<unknown>[] = [
        addDocBlock.of({
            from: doc.line(1).from,
            to: doc.line(2).to,
            indent: "",
            delimiter: "",
            content: "Trying",
        }),
    ];

    view.dispatch({ effects });
    return true;
}

const underlineKeymap = keymap.of([
    {
        key: "Mod-h",
        preventDefault: true,
        run: underlineSelection,
    },
]);

// <h2>UI</h2>
// <p>Allow only spaces and delete/backspaces when editing the indent of a doc
//     block.</p>
const doc_block_indent_on_before_input = (event: InputEvent) => {
    // <p>Only modify the behavior of inserts.</p>
    if (event.data) {
        // <p>Block any insert that's not an insert of spaces. TODO: need to
        //     support tabs.</p>
        if (event.data !== " ".repeat(event.data.length)) {
            event.preventDefault();
        }
    }
};

// <p>Provide a shortcut of ctrl-s (or command-s) to save the current file.</p>
export const on_keydown = (event: KeyboardEvent) => {
    if (
        event.key === "s" &&
        ((event.ctrlKey && !os_is_osx) || (event.metaKey && os_is_osx)) &&
        !event.altKey
    ) {
        on_save();
        event.preventDefault();
    }
};

// <p>Save CodeChat Editor contents.</p>
export const on_save = async () => {
    // <p>This is the data to write &mdash; the source code. First, transform
    //     the HTML back into code and doc blocks.</p>
    const source_code = editor_to_code_doc_blocks();
    // <p>Then, wrap these in a <a
    //         href="../server/src/webserver.rs#ClientSourceFile">struct the
    //         server expects</a> and send it.</p>
    await save({
        metadata: current_metadata,
        code_doc_block_arr: source_code,
    });
};

// <p><a id="save"></a>Save the provided contents back to the filesystem, by
//     sending a <code>PUT</code> request to the server. See the <a
//         href="CodeChatEditorServer.v.html#save_file">save_file endpoint</a>.
// </p>
const save = async (contents: LexedSourceFile) => {
    let response;
    try {
        response = await window.fetch(window.location.href, {
            method: "PUT",
            headers: {
                "Content-Type": "application/json",
            },
            body: JSON.stringify(contents),
        });
    } catch (error) {
        window.alert(`Save failed -- ${error}.`);
        return;
    }
    if (response.ok) {
        const response_body = await response.json();
        if (response_body.success !== true) {
            window.alert("Save failed.");
        }
        return;
    }
    window.alert(
        `Save failed -- server returned ${response.status}, ${response.statusText}.`
    );
};

// <h2>Helper functions</h2>
// <p>Given text, escape it so it formats correctly as HTML. Because the
//     solution at <a href="https://stackoverflow.com/a/48054293">SO</a>
//     transforms newlines in odd ways (see <a
//         href="https://developer.mozilla.org/en-US/docs/Web/API/HTMLElement/innerText">innerText</a>),
//     it's not usable with code. Instead, this is a translation of Python's
//     <code>html.escape</code> function.</p>
const escapeHTML = (unsafeText: string): string => {
    // <p>Must be done first!</p>
    unsafeText = unsafeText.replaceAll("&", "&amp;");
    unsafeText = unsafeText.replaceAll("<", "&lt;");
    unsafeText = unsafeText.replaceAll(">", "&gt;");
    return unsafeText;
};

// <p>This handles only three HTML entities, but no others!</p>
const unescapeHTML = (html: string): string => {
    let text = html.replaceAll("&gt;", ">");
    text = text.replaceAll("&lt;", "<");
    text = text.replaceAll("&amp;", "&");
    return text;
};

// <p>True if this is a CodeChat Editor document (not a source file).</p>
const is_doc_only = () => {
    return current_metadata["mode"] === "codechat-html";
};

// <p>Per <a
//         href="https://developer.mozilla.org/en-US/docs/Web/API/Navigator/platform#examples">MDN</a>,
//     here's the least bad way to choose between the control key and the
//     command key.</p>
const os_is_osx =
    navigator.platform.indexOf("Mac") === 0 || navigator.platform === "iPhone"
        ? true
        : false;

// <p>A great and simple idea taken from <a
//         href="https://stackoverflow.com/a/54116079">SO</a>: wrap all testing
//     exports in a single variable. This avoids namespace pollution, since only
//     one name is exported, and it's clearly marked for testing only. Test code
//     still gets access to everything it needs.</p>
export const exportedForTesting = {
    EditorMode,
    open_lp,
};
