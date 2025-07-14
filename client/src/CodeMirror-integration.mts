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
// `CodeMirror-integration.mts` -- integrate CodeMirror into the CodeChat Editor
// =============================================================================
//
// This file assumes the server has parsed the source. For example given the
// following original Python source code:
//
// ```
//  # This is a multi-line
//  # doc block.
//  print("Some code.")
// ```
//
// this is transformed to (note that `\n` represents an empty line):
//
// ```
//  \n                      <= Replace from character 0..
//  \n                      <= ..to character 1 with a doc block: indent = "", delimiter = "#",
//  print("Some code.")        contents = "This is a multi-line\ndoc block."
// ```
//
// To accomplish this:
//
// 1.  Create a single CodeMirror instance, which holds the parsed source.
//     Create a single TinyMCE instance, for editing doc block contents.
// 2.  Define a replacement decoration for each doc block, which replaces the
//     newlines in the parsed source with editable doc blocks.
// 3.  Define a StateField to store the doc block decorations.
// 4.  Define a ViewPlugin to route events to doc blocks; when doc block
//     contents are focused, apply the TinyMCE instance to those contents.
// 5.  Define a set of StateEffects to add/update/etc. doc blocks.
//
// Imports
// -------
//
// ### Third-party
import { basicSetup } from "codemirror";
import {
    EditorView,
    Decoration,
    DecorationSet,
    ViewUpdate,
    ViewPlugin,
    WidgetType,
} from "@codemirror/view";
import {
    ChangeDesc,
    EditorState,
    Extension,
    StateField,
    StateEffect,
    EditorSelection,
    Transaction,
    TransactionSpec,
    Annotation,
} from "@codemirror/state";
import { cpp } from "@codemirror/lang-cpp";
import { css } from "@codemirror/lang-css";
import { go } from "@codemirror/lang-go";
import { html } from "@codemirror/lang-html";
import { java } from "@codemirror/lang-java";
import { javascript } from "@codemirror/lang-javascript";
import { json } from "@codemirror/lang-json";
import { python } from "@codemirror/lang-python";
import { rust } from "@codemirror/lang-rust";
import { Editor, init, tinymce } from "./tinymce-config.mjs";

// ### Local
import { set_is_dirty, startAutosaveTimer } from "./CodeChatEditor.mjs";
import {
    CodeChatForWeb,
    CodeMirror,
    CodeMirrorDiffable,
    CodeMirrorDocBlockJson,
    StringDiff,
} from "./shared_types.mjs";
import { assert } from "./assert.mjs";

// Globals
// -------
let current_view: EditorView;
let tinymce_singleton: Editor | undefined;
// When true, don't update on the next call to `on_dirty`. See that function for
// more info.
let ignore_next_dirty = false;

// Options used when creating a `Decoration`.
const decorationOptions = {
    block: true,
    inclusiveEnd: false,
};

declare global {
    interface Window {
        MathJax: any;
    }
}

const docBlockFreezeAnnotation = Annotation.define<boolean>();

// Doc blocks in CodeMirror
// ------------------------
//
// The goal: given a [Range](https://codemirror.net/docs/ref/#state.Range) of
// lines containing a doc block (a delimiter, indent, and contents) residing at
// these lines,
// [replace](https://codemirror.net/docs/ref/#view.Decoration^replace) them with
// a widget which allows editing of the doc block.
//
// First, define a
// [StateField](https://codemirror.net/docs/ref/#state.StateField^define): the
// state required to store all doc blocks. Conveniently, a
// [DecorationSet](https://codemirror.net/docs/ref/#view.DecorationSet) is a
// [RangeSet](https://codemirror.net/docs/ref/#state.RangeSet)<[Decoration](https://codemirror.net/docs/ref/#view.Decoration)\>,
// each element of which contains the required range and the needed HTML in the
// Decoration -- all the required state. Making it a DecorationSet provides an
// easy way to store all doc blocks.
export const docBlockField = StateField.define<DecorationSet>({
    // [Create](https://codemirror.net/docs/ref/#state.StateField^define^config.create)
    // the initial value for the field, which is an empty set (no doc blocks).
    // Therefore, simply return an empty DecorationSet (oddly, the type of
    // [Decoration.none](https://codemirror.net/docs/ref/#view.Decoration^none)).
    create(state: EditorState) {
        return Decoration.none;
    },

    // [Update](https://codemirror.net/docs/ref/#state.StateField^define^config.update)
    // computes a new value for this field from the field's previous value and
    // the provided transaction.
    update(doc_blocks: DecorationSet, tr: Transaction) {
        // If there's a freeze annotation, then ignore the mapping update.
        if (tr.annotation(docBlockFreezeAnnotation) === undefined) {
            // [Map](https://codemirror.net/docs/ref/#state.RangeSet.map) these
            // changes through the provided transaction, which updates the offsets
            // of the range so the doc blocks is still anchored to the same location
            // in the document after this transaction completes.
            doc_blocks = doc_blocks.map(tr.changes);
        }
        // See [is](https://codemirror.net/docs/ref/#state.StateEffect.is). Add
        // a doc block, as requested by this effect.
        for (let effect of tr.effects)
            if (effect.is(addDocBlock)) {
                // Perform an
                // [update](https://codemirror.net/docs/ref/#state.RangeSet.update)
                // by adding the requested doc block.
                doc_blocks = doc_blocks.update({
                    // See
                    // [updateSpec](https://codemirror.net/docs/ref/#state.RangeSet.update^updateSpec)
                    add: [
                        // [Replace](https://codemirror.net/docs/ref/#view.Decoration^replace)
                        // the code (empty lines saving space for the doc block)
                        // with the doc block contents, rendered using a GUI
                        // editor.
                        Decoration.replace({
                            widget: new DocBlockWidget(
                                effect.value.indent,
                                effect.value.delimiter,
                                effect.value.content,
                                null,
                            ),
                            ...decorationOptions,
                        }).range(effect.value.from, effect.value.to),
                    ],
                });
            }

            // Perform an update to a doc block.
            else if (effect.is(updateDocBlock)) {
                // Remove the old doc block and create a new one to replace it.
                // (Recall that this is the functional approach required by
                // CodeMirror -- state is immutable.)
                //
                // Look for existing data in this effect's range. There should be one and only one result. The value for `to` may not be provided, so don't use it.
                let prev: Decoration | undefined;
                let to: number | undefined;
                doc_blocks.between(
                    effect.value.from,
                    effect.value.from,
                    (from, to_found, value) => {
                        // For the given `from`, there should be exactly one doc block.
                        assert(prev === undefined);
                        assert(
                            effect.value.from === from,
                            `${effect.value.from} !== ${from}`,
                        );
                        prev = value;
                        to = to_found;
                    },
                );
                assert(prev !== undefined);
                doc_blocks = doc_blocks.update({
                    // Remove the old doc block. We assume there's only one
                    // block in the provided from/to range.
                    filter: (from, to, value) => false,
                    filterFrom: effect.value.from,
                    filterTo: effect.value.from,
                    // This adds the replacement doc block with updated
                    // indent/delimiter/content.
                    add: [
                        Decoration.replace({
                            widget: new DocBlockWidget(
                                effect.value.indent ?? prev.spec.widget.indent,
                                effect.value.delimiter,
                                typeof effect.value.contents === "string"
                                    ? effect.value.contents
                                    : apply_diff_str(
                                        prev.spec.widget.contents,
                                        effect.value.contents,
                                    ),
                                effect.value.dom ?? prev.spec.widget.dom,
                            ),
                            ...decorationOptions,
                        }).range(
                            effect.value.from_new ?? effect.value.from,
                            effect.value.to ?? to,
                        ),
                    ],
                });
            } else if (effect.is(deleteDocBlock)) {
                doc_blocks = doc_blocks.update({
                    filter: (from, to, value) => false,
                    filterFrom: effect.value.from,
                    filterTo: effect.value.to,
                });
            }
        return doc_blocks;
    },

    // [Provide](https://codemirror.net/docs/ref/#state.StateField^define^config.provide)
    // extensions based on this field. See also
    // [EditorView.decorations](https://codemirror.net/docs/ref/#view.EditorView^decorations)
    // and [from](https://codemirror.net/docs/ref/#state.Facet.from). TODO: I
    // don't understand what this does, but removing it breaks the extension.
    provide: (field: StateField<DecorationSet>) =>
        EditorView.decorations.from(field),

    // Define a way to serialize this field; see
    // [toJSON](https://codemirror.net/docs/ref/#state.StateField^define^config.toJSON).
    // This provides a straightforward path to transform the entire editor's
    // contents (including these doc blocks) to JSON, which can then be sent
    // back to the server for reassembly into a source file.
    toJSON: (value: DecorationSet, state: EditorState) => {
        let json = [];
        for (const iter = value.iter(); iter.value !== null; iter.next()) {
            const w = iter.value.spec.widget;
            json.push([iter.from, iter.to, w.indent, w.delimiter, w.contents]);
        }
        return json;
    },

    // For loading a file from the server back into the editor, use
    // [fromJSON](https://codemirror.net/docs/ref/#state.StateField^define^config.fromJSON).
    fromJSON: (json: any, state: EditorState) =>
        Decoration.set(
            json.map(
                ([
                    from,
                    to,
                    indent,
                    delimiter,
                    contents,
                ]: CodeMirrorDocBlockJson) =>
                    Decoration.replace({
                        widget: new DocBlockWidget(
                            indent,
                            delimiter,
                            contents,
                            null,
                        ),
                        ...decorationOptions,
                    }).range(from, to),
            ),
        ),
});

export const CodeMirror_JSON_fields = { doc_blocks: docBlockField };

// Per the [docs](https://codemirror.net/docs/ref/#state.StateEffect^define),
// "State effects can be used to represent additional effects associated with a
// transaction. They are often useful to model changes to custom state fields,
// when those changes aren't implicit in document or selection changes." In this
// case, provide a way to add a doc block.
export const addDocBlock = StateEffect.define<{
    from: number;
    to: number;
    indent: string;
    delimiter: string;
    content: string;
}>({
    map: ({ from, to, indent, delimiter, content }, change: ChangeDesc) => ({
        // Update the location (from/to) of this effect due to the
        // transaction's changes. See this [thread](https://discuss.codemirror.net/t/mapping-ranges-in-a-decoration/9307/3).
        from: change.mapPos(from),
        to: change.mapPos(to),
        indent,
        delimiter,
        content,
    }),
});

type updateDocBlockType = {
    from: number;
    from_new?: number;
    to?: number;
    indent?: string;
    delimiter: string;
    contents: string | StringDiff[];
    dom?: HTMLDivElement;
};

// Define an update.
export const updateDocBlock = StateEffect.define<updateDocBlockType>({
    map: (
        { from, from_new: fromNew, to, indent, delimiter, contents, dom },
        change: ChangeDesc,
    ) => {
        const ret: updateDocBlockType = {
            // Update the position of this doc block due to the transaction's
            // changes.
            from: change.mapPos(from),
            indent,
            delimiter,
            contents,
            dom,
        };
        if (to !== undefined) {
            ret.to = change.mapPos(to);
        }
        if (fromNew !== undefined) {
            ret.from_new = change.mapPos(fromNew);
        }
        return ret;
    },
});

// Delete a doc block.
export const deleteDocBlock = StateEffect.define<{ from: number; to: number }>({
    // Returning undefined deletes the block per the [docs](https://codemirror.net/docs/ref/#state.StateEffect^define^spec.map).
    map: ({ from, to }, change: ChangeDesc) => ({
        from: change.mapPos(from),
        to: change.mapPos(to),
    }),
});

// Create a [widget](https://codemirror.net/docs/ref/#view.WidgetType) which
// contains a doc block.
class DocBlockWidget extends WidgetType {
    constructor(
        readonly indent: string,
        readonly delimiter: string,
        readonly contents: string,
        // Only used in an update to avoid changing an already-modified doc
        // block.
        readonly dom: null | HTMLDivElement,
    ) {
        // TODO: I don't understand why I don't need to store the provided
        // parameters in the object: `this.indent = indent;`, etc.
        super();
    }

    eq(other: DocBlockWidget) {
        return (
            other.indent == this.indent &&
            other.delimiter == this.delimiter &&
            other.contents == this.contents
        );
    }

    // See [toDom](https://codemirror.net/docs/ref/#view.WidgetType.toDOM).
    toDOM() {
        // Wrap this in an enclosing div.
        let wrap = document.createElement("div");
        wrap.className = "CodeChat-doc";
        wrap.innerHTML =
            // This doc block's indent. TODO: allow paste, but must only allow
            // pasting whitespace.
            `<div class="CodeChat-doc-indent" contenteditable onpaste="return false" data-delimiter=${JSON.stringify(
                this.delimiter,
            )}>${this.indent}</div>` +
            // The contents of this doc block.
            `<div class="CodeChat-doc-contents" contenteditable>` +
            this.contents +
            "</div>";
        mathJaxTypeset(wrap);
        return wrap;
    }

    // Per the
    // [docs](https://codemirror.net/docs/ref/#view.WidgetType.updateDOM),
    // "Update a DOM element created by a widget of the same type (but
    // different, non-eq content) to reflect this widget."
    updateDOM(dom: HTMLElement, view: EditorView): boolean {
        // If this update has already been made to the provided DOM, then we're
        // done. TODO: does this actually improve performance?
        if (this.dom === dom) {
            return true;
        }

        (dom.childNodes[0] as HTMLDivElement).innerHTML = this.indent;

        // The contents div could be a TinyMCE instance, or just a plain div.
        // Handle both cases.
        const [contents_div, is_tinymce] = get_contents(dom);
        if (is_tinymce) {
            ignore_next_dirty = true;
            tinymce_singleton!.setContent(this.contents);
            tinymce_singleton!.save();
        } else {
            contents_div.innerHTML = this.contents;
            mathJaxTypeset(contents_div);
        }

        // Indicate the update was successful.
        return true;
    }

    ignoreEvent(event: Event) {
        // Avoid handling other events, since this causes [weird problems with
        // event
        // routing](https://discuss.codemirror.net/t/how-to-get-focusin-events-on-a-custom-widget-decoration/6792).
        if (event.type === "focusin" || event.type === "input") {
            return false;
        } else {
            return true;
        }
    }

    // Per the [docs](https://codemirror.net/docs/ref/#view.WidgetType.destroy),
    // "This is called when the an instance of the widget is removed from the
    // editor view."
    destroy(dom: HTMLElement): void {
        // If this is the TinyMCE editor, save it.
        const [contents_div, is_tinymce] = get_contents(dom);
        // Forget about any typeset math in this node.
        window.MathJax.typesetClear([contents_div]);
        if (is_tinymce) {
            const codechat_body = document.getElementById("CodeChat-body")!;
            const tinymce_div = document.getElementById("TinyMCE-inst")!;
            codechat_body.insertBefore(tinymce_div, null);
        }
    }
}

// Typeset the provided node; taken from the [MathJax
// docs](https://docs.mathjax.org/en/latest/web/typeset.html#handling-asynchronous-typesetting).
export const mathJaxTypeset = async (
    // The node to typeset.
    node: HTMLElement,
    // An optional function to run when the typeset finishes.
    afterTypesetFunc: () => void = () => { },
) => {
    try {
        await window.MathJax.typesetPromise([node]);
        afterTypesetFunc();
    } catch (err: any) {
        console.log("Typeset failed: " + err.message);
    }
};

// Transform a typeset node back to the original (untypeset) text.
export const mathJaxUnTypeset = (node: HTMLElement) => {
    window.MathJax.startup.document
        .getMathItemsWithin(node)
        .forEach((item: any) => {
            item.removeFromDocument(true);
        });
};

// Given a doc block div element, return the contents div and if TinyMCE is
// attached to that div.
const get_contents = (element: HTMLElement): [HTMLDivElement, boolean] => {
    const contents_div = element.childNodes[1] as HTMLDivElement;
    const tinymce_inst = tinymce.get(contents_div.id);
    return [contents_div, tinymce_inst !== null];
};

// Determine if the element which generated the provided event was in a doc
// block or not. If not, return false; if so, return the doc block div.
const element_is_in_doc_block = (
    target: EventTarget | null,
): boolean | HTMLDivElement => {
    if (target === null) {
        return false;
    }
    // Look for either a CodeMirror ancestor or a CodeChat doc block ancestor.
    const ancestor = (target as HTMLElement).closest(".cm-line, .CodeChat-doc");
    // If it's a doc block, then tell Code Mirror not to handle this event.
    if (ancestor?.classList.contains("CodeChat-doc")) {
        return ancestor as HTMLDivElement;
    }
    return false;
};

// Called when a doc block is dirty...
//
// ...but it's more complicated than that. TinyMCE keeps track of a [dirty
// flag](https://www.tiny.cloud/docs/tinymce/latest/apis/tinymce.editor/#isDirty),
// but some dirty events it reports shouldn't be saved:
//
// 1.  When the existing TinyMCE instance is updated with new text on a redraw,
//     the resulting dirty flag should be ignored.
// 2.  When the existing TinyMCE instance is focused, existing math should be
//     untypeset, then the dirty ignored.
// 3.  When MathJax typesets math on a TinyMCE focus out event, the dirty flag
//     gets set. This should be ignored. However, typesetting is an async
//     operation, so we assume it's OK to await the typeset completion, then
//     clear the `ignore_next_dirty flag`. This will lead to nasty bugs at some
//     point.
// 4.  When an HTML doc block is assigned to the TinyMCE instance for editing,
//     the dirty flag is set. This must be ignored.
const on_dirty = (
    // The div that's dirty. It must be a child of the doc block div.
    event_target: HTMLElement,
) => {
    if (ignore_next_dirty) {
        ignore_next_dirty = false;
        return;
    }
    // Find the doc block parent div.
    const target = (event_target as HTMLDivElement).closest(
        ".CodeChat-doc",
    )! as HTMLDivElement;

    // We can only get the position (the `from` value) for the doc block. Use this to find the `to` value for the doc block.
    const from = current_view.posAtDOM(target);
    // Send an update to the state field associated with this DOM element.
    const indent_div = target.childNodes[0] as HTMLDivElement;
    const indent = indent_div.innerHTML;
    const delimiter = indent_div.getAttribute("data-delimiter")!;
    const [contents_div, is_tinymce] = get_contents(target);
    tinymce_singleton!.save();
    const contents = is_tinymce
        ? tinymce_singleton!.getContent()
        : contents_div.innerHTML;
    let effects: StateEffect<updateDocBlockType>[] = [
        updateDocBlock.of({
            from,
            indent,
            delimiter,
            contents,
            dom: target,
        }),
    ];

    current_view.dispatch({ effects });

    return false;
};

export const DocBlockPlugin = ViewPlugin.fromClass(
    class {
        constructor(view: EditorView) { }
        update(update: ViewUpdate) { }
    },
    {
        eventHandlers: {
            // When a doc block receives focus, turn it into a TinyMCE instance
            // so it can be edited. A simpler alternative is to do this in the
            // update() method above, but this is VERY slow, since update is
            // called frequently.
            focusin: (event: FocusEvent, view: EditorView) => {
                const target_or_false = element_is_in_doc_block(event.target);
                if (!target_or_false) {
                    return false;
                }
                // Set up for editing the indent of doc blocks.
                const target = target_or_false as HTMLDivElement;
                const indent_div = target.childNodes[0] as HTMLDivElement;
                // Use the
                // [beforeinput](https://developer.mozilla.org/en-US/docs/Web/API/HTMLElement/beforeinput_event)
                // event to allow only whitespace in the indent. Note that
                // [addEventListener](https://developer.mozilla.org/en-US/docs/Web/API/EventTarget/addEventListener)
                // states "If the function or object is already in the list of
                // event listeners for this target, the function or object is
                // not added a second time." So, we can just add it here without
                // needing to check if it's already present.
                indent_div.addEventListener(
                    "beforeinput",
                    doc_block_indent_on_before_input,
                );

                // If the target is in the indent, not the contents, then the
                // following code isn't needed.
                if (
                    (event.target as HTMLElement).closest(
                        ".CodeChat-doc-contents",
                    ) === null
                ) {
                    return false;
                }
                const [contents_div, is_tinymce] = get_contents(target);

                // See if this is already a TinyMCE instance; if not, move it
                // here.
                if (is_tinymce) {
                    ignore_next_dirty = true;
                    mathJaxUnTypeset(contents_div);
                    // If there was no math to untypeset, then `on_dirty` wasn't
                    // called, but we should no longer ignore the next dirty
                    // flag.
                    ignore_next_dirty = false;
                } else {
                    // Wait until the focus event completes; this causes the
                    // cursor position (the selection) to be set in the
                    // contenteditable div. Then, save that location.
                    setTimeout(() => {
                        // The code which moves TinyMCE into this div disturbs
                        // all the nodes, which causes it to loose a selection
                        // tied to a specific node. So, instead store the
                        // selection as an array of indices in the childNodes
                        // array of each element: for example, a given selection
                        // is element 10 of the root TinyMCE div's children
                        // (selecting an ol tag), element 5 of the ol's children
                        // (selecting the last li tag), element 0 of the li's
                        // children (a text node where the actual click landed;
                        // the offset in this node is placed in
                        // `selection_offset`.)
                        const sel = window.getSelection();
                        let selection_path = [];
                        const selection_offset = sel?.anchorOffset;
                        if (sel?.anchorNode) {
                            // Find a path from the selection back to the
                            // containing div.
                            for (
                                let current_node = sel.anchorNode,
                                is_first = true;
                                // Continue until we find the div which contains
                                // the doc block contents: either it's not an
                                // element (such as a div), ...
                                current_node.nodeType !== Node.ELEMENT_NODE ||
                                // or it's not the doc block contents div.
                                !(current_node as Element).classList.contains(
                                    "CodeChat-doc-contents",
                                );
                                current_node = current_node.parentNode!,
                                is_first = false
                            ) {
                                // Store the index of this node in its' parent
                                // list of child nodes/children. Use
                                // `childNodes` on the first iteration, since
                                // the selection is often in a text node, which
                                // isn't in the `parents` list. However, using
                                // `childNodes` all the time causes trouble when
                                // reversing the selection -- sometimes, the
                                // `childNodes` change based on whether text
                                // nodes (such as a newline) are included are
                                // not after tinyMCE parses the content.
                                let p = current_node.parentNode!;
                                selection_path.unshift(
                                    Array.prototype.indexOf.call(
                                        is_first ? p.childNodes : p.children,
                                        current_node,
                                    ),
                                );
                            }
                        }

                        // With the selection saved, it's safe to replace the
                        // contenteditable div with the TinyMCE instance (which
                        // would otherwise wipe the selection).
                        const tinymce_div =
                            document.getElementById("TinyMCE-inst")!;
                        // Copy the current TinyMCE instance contents into a
                        // contenteditable div.
                        const old_contents_div = document.createElement("div")!;
                        old_contents_div.className = "CodeChat-doc-contents";
                        old_contents_div.contentEditable = "true";
                        old_contents_div.replaceChildren(
                            ...tinymce_singleton!.getContentAreaContainer()
                                .childNodes,
                        );
                        tinymce_div.parentNode!.insertBefore(
                            old_contents_div,
                            null,
                        );
                        // Move TinyMCE to the new location, then remove the old
                        // div it will replace.
                        target.insertBefore(tinymce_div, null);
                        // TinyMCE edits booger MathJax. Also, the math is
                        // uneditable. So, translate it back to its untypeset
                        // form. When editing is done, it will be re-rendered.
                        mathJaxUnTypeset(contents_div);

                        // Setting the content makes TinyMCE consider it dirty
                        // -- ignore this "dirty" event.
                        ignore_next_dirty = true;
                        tinymce_singleton!.setContent(contents_div.innerHTML);
                        tinymce_singleton!.save();
                        contents_div.remove();

                        // This process causes TinyMCE to lose focus. Restore
                        // that. However, this causes TinyMCE to lose the
                        // selection, which the next bit of code then restores.
                        tinymce_singleton!.focus(false);

                        // Copy the selection over to TinyMCE by indexing the
                        // selection path to find the selected node.
                        if (
                            selection_path.length &&
                            typeof selection_offset === "number"
                        ) {
                            let selection_node =
                                tinymce_singleton!.getContentAreaContainer();
                            for (
                                ;
                                selection_path.length;
                                selection_node =
                                // As before, use the more-consistent
                                // `children` except for the last element,
                                // where we might be selecting a `text`
                                // node.
                                (
                                    selection_path.length > 1
                                        ? selection_node.children
                                        : selection_node.childNodes
                                )[selection_path.shift()!]! as HTMLElement
                            );
                            // Use that to set the selection.
                            tinymce_singleton!.selection.setCursorLocation(
                                selection_node,
                                selection_offset,
                            );
                        }
                    }, 0);
                }
                return false;
            },
        },
    },
);

// UI
// --
//
// Allow only spaces and delete/backspaces when editing the indent of a doc
// block.
const doc_block_indent_on_before_input = (event_: Event) => {
    // Declaring this as an InputEvent causes TypeScript to complain about an
    // incorrect type, so fix it here.
    const event = event_ as InputEvent;
    // Only modify the behavior of inserts.
    if (event.data) {
        // Block any insert that's not an insert of spaces. TODO: need to
        // support tabs.
        if (event.data !== " ".repeat(event.data.length)) {
            event.preventDefault();
        }
    }
    // Signal that this indent is dirty.
    event.target && on_dirty(event.target as HTMLElement);
};

// There doesn't seem to be any tracking of a dirty/clean flag built into
// CodeMirror v6 (although [v5
// does](https://codemirror.net/5/doc/manual.html#isClean)). The best I've found
// is a [forum
// post](https://discuss.codemirror.net/t/codemirror-6-proper-way-to-listen-for-changes/2395/11)
// showing code to do this, which I use below.
//
// How this works: the
// [EditorView.updateListener](https://codemirror.net/docs/ref/#codemirror) is a
// [Facet](https://codemirror.net/docs/ref/#state.Facet) with an [of
// function](https://codemirror.net/docs/ref/#state.Facet.of) that creates a
// CodeMirror extension.
const autosaveExtension = EditorView.updateListener.of(
    // CodeMirror passes this function a
    // [ViewUpdate](https://codemirror.net/docs/ref/#view.ViewUpdate) which
    // describes a change being made to the document.
    (v: ViewUpdate) => {
        // The
        // [docChanged](https://codemirror.net/docs/ref/#view.ViewUpdate.docChanged)
        // flag is the relevant part of this change description. However, this
        // only describes changes to the code blocks (the document, from
        // CodeMirror's perspective).
        let isChanged = v.docChanged;
        // Look for changes to doc blocks as well; skip if a change was already
        // detected for efficiency.
        if (!v.docChanged && v.transactions?.length) {
            // Check each effect of each transaction.
            outer: for (let tr of v.transactions) {
                for (let effect of tr.effects) {
                    // Look for a change to a doc block.
                    if (effect.is(addDocBlock) || effect.is(updateDocBlock)) {
                        isChanged = true;
                        break outer;
                    }
                }
            }
        }
        if (isChanged) {
            set_is_dirty();
            startAutosaveTimer();
        }
    },
);

// Given source code in a CodeMirror-friendly JSON format, load it into the
// provided div.
export const CodeMirror_load = async (
    // The div to place the loaded document in.
    codechat_body: HTMLDivElement,
    // The document to load.
    source: CodeChatForWeb["source"],
    // The name of the lexer to use.
    lexer_name: string,
    // Additional extensions.
    extensions: Array<Extension>,
) => {
    if ("Plain" in source) {
        // Although the [docs](https://codemirror.net/docs/ref/#state.EditorState^fromJSON) specify a [EditorStateConfig](https://codemirror.net/docs/ref/#state.EditorStateConfig) which contains `doc` and `selection`, the implementation requires these to be present in the `json` (first) argument. Therefore:
        const editor_state_json = {
            doc: source.Plain.doc,
            selection: EditorSelection.single(0).toJSON(),
            doc_blocks: source.Plain.doc_blocks,
        };
        // Save the current scroll position, to prevent the view from scrolling back
        // to the top after an update/reload.
        let scrollSnapshot;
        if (current_view !== undefined) {
            scrollSnapshot = current_view.scrollSnapshot();
            // For reloads, we need to remove previous instances; otherwise, Bad
            // Things happen.
            tinymce.remove();
        }

        codechat_body.innerHTML =
            '<div class="CodeChat-CodeMirror"></div><div id="TinyMCE-inst" class="CodeChat-doc-contents" spellcheck="true"></div>';
        let parser;
        // TODO: dynamically load the parser.
        switch (lexer_name) {
            // Languages with a parser
            case "sh":
                parser = cpp();
                break;
            case "c_cpp":
                parser = cpp();
                break;
            case "csharp":
                parser = javascript();
                break;
            case "css":
                parser = css();
                break;
            case "golang":
                parser = go();
                break;
            case "html":
                parser = html();
                break;
            case "java":
                parser = java();
                break;
            case "javascript":
                parser = javascript();
                break;
            case "python":
                parser = python();
                break;
            case "rust":
                parser = rust();
                break;
            case "typescript":
                parser = javascript({ typescript: true });
                break;

            // Languages without a parser.
            case "json5":
                parser = json();
                break;
            case "matlab":
                parser = python();
                break;
            case "sql":
                parser = python();
                break;
            case "swift":
                parser = python();
                break;
            case "toml":
                parser = json();
                break;
            case "vhdl":
                parser = cpp();
                break;
            case "verilog":
                parser = cpp();
                break;
            case "v":
                parser = javascript();
                break;

            default:
                parser = javascript();
                console.log(`Unknown lexer name ${lexer_name}`);
                break;
        }
        const state = EditorState.fromJSON(
            editor_state_json,
            {
                extensions: [
                    DocBlockPlugin,
                    parser,
                    basicSetup,
                    EditorView.lineWrapping,
                    autosaveExtension,
                    ...extensions,
                ],
            },
            CodeMirror_JSON_fields,
        );
        current_view = new EditorView({
            parent: codechat_body.childNodes[0] as HTMLDivElement,
            state,
            scrollTo: scrollSnapshot,
        });
        tinymce_singleton = (
            await init({
                selector: "#TinyMCE-inst",
                setup: (editor: Editor) => {
                    editor.on("Dirty", (event: any) => {
                        // Get the div TinyMCE stores edits in. TODO: find
                        // documentation for this.
                        const target_or_false = event.target?.bodyElement;
                        if (target_or_false == null) {
                            return false;
                        }
                        on_dirty(target_or_false);
                    });
                    // When leaving a TinyMCE block, retypeset the math. (It's
                    // untypeset when entering the block, to avoid editing
                    // problems.)
                    editor.on("focusout", (event: any) => {
                        const target_or_false = event.target;
                        if (target_or_false == null) {
                            return false;
                        }
                        // If the editor is dirty, save it first before we possibly
                        // modify it.
                        if (tinymce_singleton!.isDirty()) {
                            tinymce_singleton!.save();
                        }
                        // When switching from one doc block to another, the MathJax
                        // typeset finishes after the new doc block has been
                        // updated. To prevent saving the "dirty" content from
                        // typesetting, wait until this finishes to clear the
                        // `ignore_next_dirty` flag.
                        ignore_next_dirty = true;
                        mathJaxTypeset(target_or_false, () => {
                            tinymce_singleton!.save();
                            ignore_next_dirty = false;
                        });
                    });
                },
            })
        )[0];
    } else {
        // This contains a diff, instead of plain text. Apply the text diff.
        //
        // First, apply just the text edits. Use an annotation so that the doc blocks aren't changed; without this, the diff won't work (since from/to values of doc blocks are changed by unfrozen text edits).
        current_view.dispatch({
            changes: source.Diff.doc,
            annotations: docBlockFreezeAnnotation.of(true),
        });
        // Now, apply the diff in a separate transaction. Applying them in the same transaction causes the text edits to modify from/to values in the doc block effects, even when changes to the doc block state is frozen.
        const stateEffects: StateEffect<any>[] = [];
        for (const transaction of source.Diff.doc_blocks) {
            if ("Add" in transaction) {
                const add = transaction.Add;
                stateEffects.push(
                    addDocBlock.of({
                        from: add[0],
                        to: add[1],
                        indent: add[2],
                        delimiter: add[3],
                        content: add[4],
                    }),
                );
            } else if ("Update" in transaction) {
                stateEffects.push(updateDocBlock.of(transaction.Update));
            } else if ("Delete" in transaction) {
                stateEffects.push(deleteDocBlock.of(transaction.Delete));
            } else {
                assert(false, `Unknown transaction ${transaction}.`);
            }
        }
        // Update the view with these changes to the state.
        current_view.dispatch({ effects: stateEffects });
    }
};

// Appply a `StringDiff` to the before string to produce the after string.
const apply_diff_str = (before: string, diffs: StringDiff[]) => {
    // Walk from the last diff to the first. JavaScript doesn't have reverse iteration AFAIK.
    let after = before;
    for (let index = diffs.length - 1; index >= 0; --index) {
        const { from, to, insert } = diffs[index];
        if (to === undefined) {
            // This is an insert.
            after = after.slice(0, from) + insert + after.slice(from);
        } else {
            // This is a replace.
            after = after.slice(0, from) + insert + after.slice(to);
        }
    }
    return after;
};

// Return the JSON data to save from the current CodeMirror-based document.
export const CodeMirror_save = (): CodeMirrorDiffable => {
    // This is the data to write — the source code. First, transform the HTML
    // back into code and doc blocks.
    const code_mirror: CodeMirror = current_view.state.toJSON(
        CodeMirror_JSON_fields,
    );
    delete code_mirror.selection;

    return { Plain: code_mirror };
};
