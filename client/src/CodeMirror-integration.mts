// Copyright (C) 2025 Bryan A. Jones.
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
// 1. Create a single CodeMirror instance, which holds the parsed source. Create
//    a single TinyMCE instance, for editing doc block contents.
// 2. Define a replacement decoration for each doc block, which replaces the
//    newlines in the parsed source with editable doc blocks.
// 3. Define a StateField to store the doc block decorations.
// 4. Define a ViewPlugin to route events to doc blocks; when doc block contents
//    are focused, apply the TinyMCE instance to those contents.
// 5. Define a set of StateEffects to add/update/etc. doc blocks.
//
// Imports
// -------
//
// ### Third-party
import { basicSetup } from "codemirror";
import { indentWithTab } from "@codemirror/commands";
import {
    EditorView,
    Decoration,
    DecorationSet,
    keymap,
    ViewUpdate,
    ViewPlugin,
    WidgetType,
} from "@codemirror/view";
import {
    ChangeDesc,
    EditorState,
    Extension,
    Prec,
    StateField,
    StateEffect,
    EditorSelection,
    Transaction,
    Annotation,
    TransactionSpec,
} from "@codemirror/state";

import type { StreamParser } from "@codemirror/language";
import type { Editor, EditorEvent, Events } from "tinymce";

// ### Local
import {
    set_is_dirty,
    startAutoUpdateTimer,
    saveSelection,
    restoreSelection,
    tinymce_instance,
    tinymce,
    init,
} from "./CodeChatEditor.mjs";
import {
    CodeChatForWeb,
    CodeMirror,
    CodeMirrorDiffable,
    CodeMirrorDocBlockTuple,
    StringDiff,
    UpdateMessageContents,
} from "./shared.mjs";
import { assert } from "./assert.mjs";
import { show_toast } from "./show_toast.mjs";
import { CursorPosition } from "./rust-types/CursorPosition";

// Globals
// -------
let current_view: EditorView;
// This indicates that a call to `on_dirty` is scheduled, but hasn't run yet.
let on_dirty_scheduled = false;
// This set when an `input` event occurs, which usually produces a duplicate
// `Dirty` event which should be ignored.
let ignoreTinyMceDirty = false;

// Options used when creating a `Decoration`.
const decorationOptions = {
    block: true,
    inclusiveEnd: false,
};

declare global {
    interface Window {
        // The `@types/MathJax` definitions are out of date and I can't figure
        // out how to import the v4 Typescript definitions.
        /*eslint-disable-next-line @typescript-eslint/no-explicit-any */
        MathJax: any;
    }
}

// When this is included in a transaction, don't update from/to of doc blocks.
const docBlockFreezeAnnotation = Annotation.define<boolean>();

// When this is included in a transaction, don't send autosave scroll/cursor
// location updates.
const noAutosaveAnnotation = Annotation.define<boolean>();

// When this is included in a transaction, `DocBlockPlugin.update` won't capture
// focus into a doc block even though the resulting selection touches its
// boundary. Used by `docBlockNavKeymap`'s `ArrowLeft` handler when it
// deliberately stops the cursor at a code block's start (a doc block's `to` is
// numerically identical to the following code line's `from`, so without this
// annotation that stop would be indistinguishable from -- and incorrectly
// treated as -- entry into the preceding doc block).
const stayInCodeBlockAnnotation = Annotation.define<boolean>();

// Define a facet called when extensions produce an error.
const exceptionSink = EditorView.exceptionSink.of((exception) => {
    show_toast(`Error: ${exception}`);
    console.error(exception);
});

const TINYMCE_INST = "TinyMCE-inst";
const CODECHAT_DOC_HIDDEN = "CodeChat-doc-hidden";

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
    create(_state: EditorState) {
        return Decoration.none;
    },

    // [Update](https://codemirror.net/docs/ref/#state.StateField^define^config.update)
    // computes a new value for this field from the field's previous value and
    // the provided transaction.
    update(doc_blocks: DecorationSet, tr: Transaction) {
        // If there's a freeze annotation, then ignore the mapping update.
        if (tr.annotation(docBlockFreezeAnnotation) === undefined) {
            // [Map](https://codemirror.net/docs/ref/#state.RangeSet.map) these
            // changes through the provided transaction, which updates the
            // offsets of the range so the doc blocks is still anchored to the
            // same location in the document after this transaction completes.
            doc_blocks = doc_blocks.map(tr.changes);
        }
        // See [is](https://codemirror.net/docs/ref/#state.StateEffect.is). Add
        // a doc block, as requested by this effect.
        for (const effect of tr.effects)
            if (effect.is(addDocBlock)) {
                // Check that we're not overwriting text.
                const newlines = tr.newDoc
                    .slice(effect.value.from, effect.value.to)
                    .toString();
                if (newlines !== "\n".repeat(newlines.length)) {
                    halt_on_error(`Attempt to overwrite text: "${newlines}".`);
                }
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
                                effect.value.contents,
                                false,
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
                // Look for existing data in this effect's range. There should
                // be one and only one result. The value for `to` may not be
                // provided, so don't use it.
                let prev: Decoration | undefined;
                let to: number | undefined;
                doc_blocks.between(
                    effect.value.from,
                    effect.value.from,
                    (from, to_found, value) => {
                        // Only look for blocks whose from is as specified.
                        // `between` will also return blocks whose to matches --
                        // for example, given from = 1, one doc block of \[0,
                        // 1\], and another of \[1, 2\], *both* will be found;
                        // we want only the \[1, 2\] doc block.
                        if (effect.value.from === from) {
                            // For the given `from`, there should be exactly one
                            // doc block.
                            if (prev !== undefined) {
                                console.error({ doc_blocks, effect });
                                halt_on_error(
                                    "More than one doc block at one location found.",
                                );
                            }
                            prev = value;
                            to = to_found;

                            // We could return `false` here to stop the search
                            // for efficiency. However, we let it continue in
                            // case there are two doc blocks with the same
                            // `from` value, so we can at least flag this error.
                        }
                    },
                );
                if (prev === undefined) {
                    console.error({ doc_blocks, effect });
                    halt_on_error("No doc block found.");
                    assert(false);
                }
                // Determine the final from/to values.
                to = effect.value.to ?? to;
                const from = effect.value.from_new ?? effect.value.from;
                // Check that we're not overwriting text.
                const newlines = tr.newDoc.slice(from, to).toString();
                if (newlines !== "\n".repeat(newlines.length)) {
                    halt_on_error(`Attempt to overwrite text: "${newlines}".`);
                }
                const prev_widget = prev.spec.widget;
                assert(prev_widget instanceof DocBlockWidget);
                doc_blocks = doc_blocks.update({
                    // Remove the old doc block. We assume there's only one
                    // block in the provided from/to range.
                    filter: (from, _to, _value) => from !== effect.value.from,
                    filterFrom: effect.value.from,
                    filterTo: effect.value.from,
                    // This adds the replacement doc block with updated
                    // indent/delimiter/content.
                    add: [
                        Decoration.replace({
                            widget: new DocBlockWidget(
                                effect.value.indent ?? prev_widget.indent,
                                effect.value.delimiter ?? prev_widget.delimiter,
                                typeof effect.value.contents === "string"
                                    ? effect.value.contents
                                    : apply_diff_str(
                                          prev_widget.contents,
                                          effect.value.contents,
                                      ),
                                // If autosave is allowed (meaning no autosave
                                // is not true), then this data came from the
                                // user, not the IDE.
                                tr.annotation(noAutosaveAnnotation) !== true,
                            ),
                            ...decorationOptions,
                        }).range(from, to),
                    ],
                });
            } else if (effect.is(deleteDocBlock)) {
                doc_blocks = doc_blocks.update({
                    filter: (from, _to, _value) => from !== effect.value.from,
                    filterFrom: effect.value.from,
                    filterTo: effect.value.from,
                });
            }
        return doc_blocks;
    },

    // Register this `DecorationSet` as a source of decorations for the editor
    // view — without it, the `StateField` holds the data but nothing tells
    // CodeMirror to render the doc block widgets. See
    // [Provide](https://codemirror.net/docs/ref/#state.StateField^define^config.provide),
    // [EditorView.decorations](https://codemirror.net/docs/ref/#view.EditorView^decorations)
    // and [from](https://codemirror.net/docs/ref/#state.Facet.from).
    provide: (field: StateField<DecorationSet>) =>
        EditorView.decorations.from(field),

    // Define a way to serialize this field; see
    // [toJSON](https://codemirror.net/docs/ref/#state.StateField^define^config.toJSON).
    // This provides a straightforward path to transform the entire editor's
    // contents (including these doc blocks) to JSON, which can then be sent
    // back to the server for reassembly into a source file.
    toJSON: (value: DecorationSet, _state: EditorState) => {
        const json_result = [];
        for (const iter = value.iter(); iter.value !== null; iter.next()) {
            const w = iter.value.spec.widget;
            assert(w instanceof DocBlockWidget);
            json_result.push([
                iter.from,
                iter.to,
                w.indent,
                w.delimiter,
                w.contents,
            ]);
        }
        return json_result;
    },

    // For loading a file from the server back into the editor, use
    // [fromJSON](https://codemirror.net/docs/ref/#state.StateField^define^config.fromJSON).
    fromJSON: (json: [CodeMirrorDocBlockTuple], _state: EditorState) =>
        Decoration.set(
            json.map(
                ([
                    from,
                    to,
                    indent,
                    delimiter,
                    contents,
                ]: CodeMirrorDocBlockTuple) =>
                    Decoration.replace({
                        widget: new DocBlockWidget(
                            indent,
                            delimiter,
                            contents,
                            false,
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
    contents: string;
}>({
    map: ({ from, to, indent, delimiter, contents }, change: ChangeDesc) => ({
        // Update the location (from/to) of this effect due to the transaction's
        // changes. See this
        // [thread](https://discuss.codemirror.net/t/mapping-ranges-in-a-decoration/9307/3).
        from: change.mapPos(from),
        to: change.mapPos(to),
        indent,
        delimiter,
        contents,
    }),
});

type updateDocBlockType = {
    from: number;
    from_new?: number;
    to?: number;
    indent?: string;
    delimiter?: string;
    contents: string | StringDiff[];
};

// Define an update.
export const updateDocBlock = StateEffect.define<updateDocBlockType>({
    map: (
        { from, from_new: fromNew, to, indent, delimiter, contents },
        change: ChangeDesc,
    ) => {
        const ret: updateDocBlockType = {
            // Update the position of this doc block due to the transaction's
            // changes.
            from: change.mapPos(from),
            indent,
            delimiter,
            contents,
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
export const deleteDocBlock = StateEffect.define<{ from: number }>({
    // Returning undefined deletes the block per the
    // [docs](https://codemirror.net/docs/ref/#state.StateEffect^define^spec.map).
    map: ({ from }, change: ChangeDesc) => ({
        from: change.mapPos(from),
    }),
});

// Create a [widget](https://codemirror.net/docs/ref/#view.WidgetType) which
// contains a doc block.
class DocBlockWidget extends WidgetType {
    constructor(
        readonly indent: string,
        readonly delimiter: string,
        readonly contents: string,
        readonly is_user_change: boolean,
    ) {
        // [Typescript parameter properties](https://www.typescriptlang.org/docs/handbook/2/classes.html#parameter-properties)
        // means these parameters are automatically promoted to class
        // properties.
        super();
    }

    eq(other: DocBlockWidget) {
        // Order these to do the fastest comparisons first.
        return (
            other.is_user_change == this.is_user_change &&
            other.delimiter === this.delimiter &&
            other.indent === this.indent &&
            other.contents === this.contents
        );
    }

    // See [toDom](https://codemirror.net/docs/ref/#view.WidgetType.toDOM).
    toDOM() {
        // Wrap this in an enclosing div.
        const wrap = document.createElement("div");
        wrap.className = "CodeChat-doc";
        wrap.innerHTML =
            // This doc block's indent. It's not editable (and not a tab stop)
            // until clicked; see the inline `onmousedown` handler below and the
            // `focusout` handler in `DocBlockPlugin`, which toggle
            // `contenteditable` on and off so that keyboard/IDE-driven
            // navigation between code and doc blocks skips over the indent. The
            // toggle must happen in an inline handler, not a `DocBlockPlugin`
            // `eventHandlers.mousedown` handler: CodeMirror appends its own
            // built-in `mousedown` handler (registered on `contentDOM`) after
            // any plugin handlers, and -- whenever the editor doesn't already
            // have focus -- that built-in handler unconditionally moves focus
            // to `contentDOM`, regardless of what a same-turn `contentEditable`
            // toggle just did. An inline attribute handler runs at the target,
            // ahead of that `contentDOM`-level listener, so calling
            // `stopPropagation()` here (after making the div editable) prevents
            // the event from ever reaching CodeMirror's handler, leaving the
            // browser's default action free to focus this now-editable div.
            // TODO: allow paste, but must only allow pasting whitespace.
            `<div class="CodeChat-doc-indent" onmousedown="this.contentEditable='true'; event.stopPropagation();" onpaste="return false" data-delimiter=${JSON.stringify(
                this.delimiter,
            )}>${this.indent}</div>` +
            // The contents of this doc block. Make it focusable by assigning a
            // tab stop, but not editable (until it's replaced by the TinyMCE
            // editor).
            `<div class="CodeChat-doc-contents" spellcheck="true" tabIndex="0">` +
            this.contents +
            "</div>";
        // TODO: this is an async call. However, CodeMirror doesn't provide
        // async support.
        mathJaxTypeset(wrap);
        return wrap;
    }

    // Per the
    // [docs](https://codemirror.net/docs/ref/#view.WidgetType.updateDOM),
    // "Update a DOM element created by a widget of the same type (but
    // different, non-eq content) to reflect this widget."
    updateDOM(dom: HTMLElement, _view: EditorView): boolean {
        // If this change was produced by a user edit and the DOM to "update" is
        // a TinyMCE editor, then the DOM was already updated. Stop here.
        const [contents_div, is_tinymce] = get_contents(dom);
        if (this.is_user_change && is_tinymce) {
            return true;
        }

        // Update the indent and delimiter. Assume both have already been
        // sanitized: the server only allows whitespace for the indent; only
        // specific, safe delimiters are allowed. The Client only allows editing
        // the indent, and only whitespace is allowed there as well.
        const dom_indent = dom.childNodes[0];
        assert(dom_indent instanceof HTMLDivElement);
        dom_indent.innerHTML = this.indent;
        dom.dataset.delimiter = this.delimiter;

        // Update the contents. The contents div could be a TinyMCE instance, or
        // just a plain div. Handle both cases. Again, we assume sanitized
        // content, since this comes from the server (which uses Ammonia) or
        // TinyMCE (which uses a
        // [sanitizer](https://www.tiny.cloud/docs/tinymce/latest/security/#sanitizing-html-input-to-protect-against-xss-attacks)
        // for all user input).
        window.MathJax?.typesetClear?.([contents_div]);
        if (is_tinymce) {
            // Save the cursor location before the update, then restore it
            // afterwards, if TinyMCE has focus.
            const sel = tinymce_instance()!.hasFocus()
                ? saveSelection()
                : undefined;
            tinymce_instance()!.setContent(this.contents);
            if (sel !== undefined) {
                restoreSelection(sel);
            }
        } else {
            contents_div.innerHTML = this.contents;
        }
        mathJaxTypeset(contents_div);

        // Indicate the update was successful. TODO: but, contents are still
        // pending if it contains math...
        return true;
    }

    ignoreEvent(event: Event) {
        // Avoid handling other events, since this causes
        // [weird problems with event routing](https://discuss.codemirror.net/t/how-to-get-focusin-events-on-a-custom-widget-decoration/6792).
        // `focusout` is also let through: `DocBlockPlugin`'s `focusout` handler
        // needs it to turn off the indent's `contenteditable` once it loses
        // focus (see the inline `onmousedown` handler above, which turns it
        // on).
        return event.type !== "focusin" && event.type !== "focusout";
    }

    // Per the [docs](https://codemirror.net/docs/ref/#view.WidgetType.destroy),
    // "This is called when the an instance of the widget is removed from the
    // editor view."
    destroy(dom: HTMLElement) {
        const [contents_div, is_tinymce] = get_contents(dom);
        // Forget about any typeset math in this node.
        window.MathJax?.typesetClear?.([contents_div]);
        // If this is the TinyMCE editor, save it.
        if (is_tinymce) {
            const codechat_body = document.getElementById("CodeChat-body")!;
            const tinymce_div = document.getElementById(TINYMCE_INST)!;
            codechat_body.insertBefore(tinymce_div, null);
            // Make TinyMCE invisible, since it's placed below the body of the
            // page.
            tinymce_instance()!.dom.addClass(tinymce_div, CODECHAT_DOC_HIDDEN);
            tinymce_instance()!.resetContent();
        }
    }
}

// Typeset the provided node; taken from the
// [MathJax docs](https://docs.mathjax.org/en/latest/web/typeset.html#handling-asynchronous-typesetting).
export const mathJaxTypeset = async (
    // The node to typeset.
    node: HTMLElement,
) => {
    // If MathJax isn't loaded, look for math on the page.
    if (window.MathJax === undefined) {
        const mathDelimiters = [
            // See `replace_math_node` in `processing.rs` -- this is how Math is
            // marked.
            { start: "$$", end: "$$" },
            { start: "\\(", end: "\\)" },
        ];

        // Check if Math tags or the text delimiters exist in the page body
        const nodeContent = node.innerHTML;
        const hasTeXMath = mathDelimiters.some((delimiter) => {
            const startIdx = nodeContent.indexOf(delimiter.start);
            return (
                startIdx !== -1 &&
                nodeContent.indexOf(
                    delimiter.end,
                    startIdx + delimiter.start.length,
                ) !== -1
            );
        });

        // If mathematical content is detected, load MathJax.
        if (hasTeXMath) {
            // Configure MathJax settings.
            window.MathJax = {
                // See the
                // [docs](https://docs.mathjax.org/en/latest/options/output/chtml.html#option-descriptions),
                // [postFilters](https://docs.mathjax.org/en/latest/options/output/index.html#output-postfilters);
                // see also the
                // [TinyMCE non-editable class](https://www.tiny.cloud/docs/tinymce/latest/non-editable-content-options/#noneditable_class).
                // After some experimentation, I discovered:
                //
                // * Setting the `classList` had no effect. I still think it's a
                //   good idea for the future, though.
                // * I can't use the `postFilter` to enclose this in a span with
                //   the appropriate class; MathJax disallows editing the
                //   `mjx-container` element.
                // * Simply setting `contentEditable` is what actually works.
                chtml: {
                    fontURL: "/static/mathjax-newcm-font/chtml/woff2",
                },
                output: {
                    postFilters: [
                        /*eslint-disable-next-line @typescript-eslint/no-explicit-any */
                        (obj: { data: any }) => {
                            obj.data.classList.add("mceNonEditable");
                            obj.data.contentEditable = false;
                        },
                    ],
                },
            };

            // Load MathJax. There are several states for MathJax loading:
            //
            // 1. Not loaded: `window.MathJax === undefined`.
            // 2. Load started: `windows.MathJax` is defined (see above -- this
            //    is required to configure MathJax properly, but doesn't
            //    guarantee that the library has finished loading and setup).
            // 3. Load complete: `window.MathJax.typesetPromise/untypeset/etc.`
            //    is loaded.
            // 4. Initial render complete.
            //
            // Unfortunately, since CodeMirror is synchronous, it will continue
            // calling this function and related functions even during an await.
            // To emulate a lock, put step 1-3 checks on all MathJax functions,
            // skipping calling them until step 4.
            await new Promise((resolve) => {
                const script = document.createElement("script");
                script.src = "/static/mathjax/tex-chtml.js";
                script.async = true;
                script.onload = resolve;
                script.onerror = () => {
                    report_error(`Failed to load script: ${script.src}`);
                    // We've already reported the error; don't `reject()`, which
                    // would propagate this error up the call chain and further
                    // break things.
                    resolve(0);
                };
                document.head.appendChild(script);
            });
            // Wait until MathJax is fully loaded and the initial render is
            // finished. Note that this also renders newly-added math.
            await window.MathJax.startup.promise;
        }
    } else {
        // MathJax is already loaded; just typeset the provided node.
        try {
            // MathJax may still be loading when this is called, since
            // CodeMirror lacks async support. Use `?.` to skip typesetting in
            // this case.
            await window.MathJax.typesetPromise?.([node]);
        } catch (err: unknown) {
            report_error(
                `Typeset failed: ${err instanceof Error ? err.message : "unknown"}`,
            );
        }
    }
};

// Transform a typeset node back to the original (untypeset) text.
export const mathJaxUnTypeset = (node: HTMLElement) => {
    window.MathJax?.startup?.document
        .getMathItemsWithin(node)
        /*eslint-disable-next-line @typescript-eslint/no-explicit-any */
        .forEach((item: any) => {
            item.removeFromDocument(true);
        });
};

// Given a doc block div element, return the contents div and if TinyMCE is
// attached to that div.
const get_contents = (element: Element): [HTMLDivElement, boolean] => {
    const contents_div = element.childNodes[1];
    assert(contents_div instanceof HTMLDivElement);
    const tinymce_inst = tinymce?.get(contents_div.id);
    // Note the use of `!=` to check both `undefined` (TinyMCE not loaded) and
    // `null`.
    return [contents_div, tinymce_inst != null];
};

// Determine if the element which generated the provided event was in a doc
// block or not. If not, return false; if so, return the doc block div.
const element_is_in_doc_block = (
    target: EventTarget | null,
): boolean | Element => {
    if (target instanceof HTMLElement) {
        // Look for either a CodeMirror ancestor or a CodeChat doc block ancestor.
        const ancestor = target.closest(".cm-line, .CodeChat-doc");
        // If it's a doc block, then tell CodeMirror not to handle this event.
        if (ancestor?.classList.contains("CodeChat-doc")) {
            return ancestor;
        }
    }
    return false;
};

// Called when a doc block is dirty...
//
// ...but it's more complicated than that. TinyMCE keeps track of a
// [dirty flag](https://www.tiny.cloud/docs/tinymce/latest/apis/tinymce.editor/#isDirty),
// but some dirty events it reports shouldn't be saved:
//
// 1. When the existing TinyMCE instance is updated with new text on a redraw,
//    the resulting dirty flag should be ignored.
// 2. When the existing TinyMCE instance is focused, existing math should be
//    untypeset, then the dirty ignored.
// 3. When MathJax typesets math on a TinyMCE focus out event, the dirty flag
//    gets set. This should be ignored. However, typesetting is an async
//    operation, so we assume it's OK to await the typeset completion. This will
//    lead to nasty bugs at some point.
// 4. When an HTML doc block is assigned to the TinyMCE instance for editing,
//    the dirty flag is set. This must be ignored.
//
// Potential bug: race condition. If one doc block is modified and schedules
// on\_dirty, but then another doc block is modified, then modifications to the
// first doc block would be lost. However, I doubt the user can switch doc
// blocks this fast.
const on_dirty = (
    // The div that's dirty. It must be a child of the doc block div.
    event_target: HTMLElement,
) => {
    if (on_dirty_scheduled) {
        return;
    }
    set_is_dirty();
    on_dirty_scheduled = true;

    // Only run this after typesetting is done, if MathJax is loaded; otherwise,
    // run this immediately.
    const whenReady =
        window.MathJax?.whenReady ?? (async (f: () => void) => f());
    whenReady(async () => {
        on_dirty_scheduled = false;
        // Find the doc block parent div.
        const target = event_target.closest(".CodeChat-doc")!;

        // We can only get the position (the `from` value) for the doc block.
        // Use this to find the `to` value for the doc block.
        let from;
        try {
            from = current_view.posAtDOM(target);
        } catch (_e) {
            console.error("Unable to get position from DOM.", target);
            return;
        }
        // Send an update to the state field associated with this DOM element.
        const indent_div = target.childNodes[0];
        assert(indent_div instanceof HTMLDivElement);
        const indent = indent_div.innerHTML;
        const delimiter = indent_div.getAttribute("data-delimiter")!;
        const [contents_div, is_tinymce] = get_contents(target);
        // I'd like to extract this string, then untypeset only that string, not
        // the actual div. But I don't know how.
        mathJaxUnTypeset(contents_div);
        // Use the raw format; see the implementation notes.
        const contents = is_tinymce
            ? tinymce_instance()!.save({ format: "raw" })
            : contents_div.innerHTML;
        // The `save()` flushes any duplicate `Dirty` events. After this,
        // following `Dirty` events are genuine.
        ignoreTinyMceDirty = false;
        await mathJaxTypeset(contents_div);
        // When editing large doc blocks, they may be deleted then re-created by
        // CodeMirror, which causes unexpected scrolling. To avoid this, save
        // then restore the scroll after updating CodeMirror.
        const currentScrollTop = current_view.scrollDOM.scrollTop;
        current_view.dispatch({
            effects: [
                updateDocBlock.of({
                    from,
                    indent,
                    delimiter,
                    contents,
                }),
            ],
        });
        requestAnimationFrame(
            () => (current_view.scrollDOM.scrollTop = currentScrollTop),
        );
    });
};

// Keyboard navigation between code and doc blocks
// -----------------------------------------------
//
// Doc blocks are `Decoration.replace` widgets drawn over empty lines in the
// document, so CodeMirror's default cursor movement treats them as an atomic
// region and arrow keys (mostly) skip over them to the next code block.
//
// ### Specification
//
// The requirements for correct keyboard cursor navigation are:
//
// * When the cursor is located at the beginning of a code/doc block preceded by
//   a code/doc block, pressing the left arrow key should move the cursor to the
//   end of the preceding code/doc block.
// * When the cursor is located on the first line of a code/doc block preceded
//   by a code/doc block, pressing the up arrow key should move the cursor into
//   the last line of the preceding code/doc block, moving the cursor as little
//   horizontally as possible.
// * When the cursor is located at the end of a code/doc block followed by a
//   code/doc block, pressing the right arrow key should move the cursor to the
//   beginning of the following code/doc block.
// * When the cursor is located on the last line of a code/doc block followed by
//   a code/doc block, pressing the down arrow key should move the cursor to the
//   following code/doc block, moving the cursor as little horizontally as
//   possible.
// * Pressing the PageUp/PageDown keys should move the cursor by viewport,
//   rather than limited cursor movement within the current code/doc block.
//
// ### Implementation notes
//
// The keymap below intercepts the arrow keys and, when the cursor would move
// into a doc block, dispatches a CodeMirror selection into that block's range
// instead. That selection change is then picked up by `DocBlockPlugin.update`,
// which focuses the block's contents div (the `focusin` handler promotes it to
// TinyMCE). This keeps a single focus path -- the same one used for mouse
// clicks and IDE-driven cursor sync.
//
// Given a doc position `pos`, return the range (`from`/`to`) of the doc block
// that starts exactly at `pos`, or `null` if there isn't one. Doc blocks can
// sit back-to-back (sharing a boundary position with a neighboring doc block),
// so this looks for an exact match on `from` rather than any block that merely
// touches `pos` -- otherwise, at a shared boundary, the block ending at `pos`
// could be returned instead of the one starting there.
const doc_block_starting_at = (
    // The CodeMirror view whose doc blocks are searched.
    view: EditorView,
    // The document position to check for a doc block starting there.
    pos: number,
): { from: number; to: number } | null => {
    let found: { from: number; to: number } | null = null;
    view.state.field(docBlockField).between(pos, pos, (from, to, _deco) => {
        if (from === pos) {
            found = { from, to };
            return false;
        }
    });
    return found;
};

// Same as `doc_block_starting_at`, but looks for a doc block that ends exactly
// at `pos`.
const doc_block_ending_at = (
    view: EditorView,
    pos: number,
): { from: number; to: number } | null => {
    let found: { from: number; to: number } | null = null;
    view.state.field(docBlockField).between(pos, pos, (from, to, _deco) => {
        if (to === pos) {
            found = { from, to };
            return false;
        }
    });
    return found;
};

// Move the CodeMirror selection to `pos` (an edge of a doc block range). The
// `DocBlockPlugin.update` handler reacts to the resulting selection change by
// focusing the block. Returns `true` so the keymap reports the key as handled.
const select_doc_block_edge = (view: EditorView, pos: number): boolean => {
    view.dispatch({ selection: { anchor: pos } });
    return true;
};

// A keymap (registered at high precedence) that moves the selection into an
// adjacent doc block on arrow-key navigation. Entering from above (ArrowDown,
// ArrowRight) lands the selection at the block's start; entering from below
// (ArrowUp, ArrowLeft) lands it at the block's end.
export const docBlockNavKeymap = keymap.of([
    {
        // Down arrow at the bottom of a code block: enter the doc block below,
        // caret at its start. A line's `.to` sits just before its trailing
        // newline, so the following doc block's placeholder starts one position
        // later -- hence `+ 1` (matches `main.head + 1` in the `ArrowRight`
        // handler below). Chaining from one doc block into the next happens
        // outside CodeMirror, in `DocBlockPlugin`'s `focusin` handler, so this
        // only needs to handle first entry from a code line.
        key: "ArrowDown",
        run: (view) => {
            const { main } = view.state.selection;
            const search_pos = view.state.doc.lineAt(main.head).to + 1;
            const range = doc_block_starting_at(view, search_pos);
            return range !== null
                ? select_doc_block_edge(view, range.from)
                : false;
        },
    },
    {
        // Up arrow at the top of a code block: enter the doc block above, caret
        // at its end. Look right before the current line's contents, which is
        // where a preceding doc block's decoration would end (see the
        // `ArrowDown` comment above for why no "chained" check is needed here
        // either).
        key: "ArrowUp",
        run: (view) => {
            const { main } = view.state.selection;
            const search_pos = view.state.doc.lineAt(main.head).from;
            const range = doc_block_ending_at(view, search_pos);
            return range !== null
                ? select_doc_block_edge(view, range.to)
                : false;
        },
    },
    {
        // Right arrow at the end of a line: if a doc block follows, enter it
        // with the caret at its start.
        key: "ArrowRight",
        run: (view) => {
            const { main } = view.state.selection;
            if (!main.empty) {
                return false;
            }
            const line = view.state.doc.lineAt(main.head);
            if (main.head !== line.to) {
                return false;
            }
            const range = doc_block_starting_at(view, main.head + 1);
            return range !== null
                ? select_doc_block_edge(view, range.from)
                : false;
        },
    },
    {
        // Left arrow next to a doc block. CodeMirror's default cursor motion
        // treats a doc block's `Decoration.replace` widget as atomic, so a
        // single ArrowLeft press from the position right after the following
        // code block's first line start (`line.from + 1`) jumps straight past
        // that line's start and into the doc block, skipping the "beginning of
        // the code block" stop entirely. Intercept only that specific press:
        // land the cursor at the line's start instead. Every other position on
        // the line (including the start itself, on a subsequent press) falls
        // through to normal handling below.
        key: "ArrowLeft",
        run: (view) => {
            const { main } = view.state.selection;
            if (!main.empty) {
                return false;
            }
            const line = view.state.doc.lineAt(main.head);
            if (main.head === line.from + 1) {
                // One character away from the line's start. If a doc block ends
                // exactly at this line's start, the default motion would jump
                // straight into it; land the cursor at the line's start
                // instead, so a further ArrowLeft press is needed to enter the
                // doc block.
                if (doc_block_ending_at(view, line.from) !== null) {
                    view.dispatch({
                        selection: { anchor: line.from },
                        annotations: stayInCodeBlockAnnotation.of(true),
                    });
                    return true;
                }
                return false;
            }
            if (main.head !== line.from) {
                return false;
            }
            const range = doc_block_ending_at(view, main.head);
            return range !== null
                ? select_doc_block_edge(view, range.to)
                : false;
        },
    },
    {
        // Home on a code line: same atomic-widget problem as ArrowLeft above,
        // but with no "second press" case -- Home always means "stay on this
        // line," so if a doc block ends exactly at the line's start, always
        // stop the cursor there ourselves, dispatching with
        // `stayInCodeBlockAnnotation` even when the selection doesn't move (a
        // redundant Home press), so `DocBlockPlugin.update` doesn't treat it as
        // entry into the preceding doc block. Falling through to `false` here
        // would let the default Home command dispatch a plain selection update
        // instead, without that annotation.
        key: "Home",
        run: (view) => {
            const { main } = view.state.selection;
            if (!main.empty) {
                return false;
            }
            const line = view.state.doc.lineAt(main.head);
            if (doc_block_ending_at(view, line.from) !== null) {
                view.dispatch({
                    selection: { anchor: line.from },
                    annotations: stayInCodeBlockAnnotation.of(true),
                });
                return true;
            }
            return false;
        },
    },
]);

// Handle cursor movement and mouse selection in a doc block.
export const DocBlockPlugin = ViewPlugin.fromClass(
    class {
        constructor(_view: EditorView) {}
        update(update: ViewUpdate) {
            // If the editor doesn't have focus, ignore selection changes. This
            // avoid the case where cursor movement in the IDE produces
            // selection changes in the Client, which then steals focus. TODO:
            // when the editor isn't focused, highlight the relevant line or
            // something similar.
            if (update.selectionSet && update.view.hasFocus) {
                // If focus is currently in a doc block's indent (made editable
                // by the inline `onmousedown` handler in
                // `DocBlockWidget.toDOM`), don't steal it away into the
                // contents div.
                if (document.activeElement?.closest(".CodeChat-doc-indent")) {
                    return;
                }
                // If one of this update's transactions deliberately stopped the
                // cursor at a code block's start (see
                // `stayInCodeBlockAnnotation`), don't treat the resulting
                // selection -- which sits at the same position as the preceding
                // doc block's `to` -- as entry into that doc block.
                if (
                    update.transactions.some(
                        (tr) =>
                            tr.annotation(stayInCodeBlockAnnotation) === true,
                    )
                ) {
                    return;
                }
                // See if the new main selection falls within a doc block.
                const main_selection = update.state.selection.main;
                update.state
                    .field(docBlockField)
                    .between(
                        main_selection.from,
                        main_selection.to,
                        (from: number, to: number, _value: Decoration) => {
                            // Is this range contained within this doc block? If
                            // the ranges also contains element outside it, then
                            // don't capture focus. TODO: not certain on the
                            // bounds -- should I use <= or <, etc.?
                            if (
                                main_selection.from < from ||
                                main_selection.to > to
                            ) {
                                return;
                            }

                            // Ensure we have a valid dom. This also checks for
                            // undefined.
                            const dom_at_pos = update.view.domAtPos(from);
                            const dom =
                                dom_at_pos.node.childNodes[dom_at_pos.offset];
                            if (
                                !(dom instanceof HTMLElement) ||
                                dom.className !== "CodeChat-doc"
                            ) {
                                return;
                            }

                            // Focus the contents div. This fires the `focusin`
                            // handler, which promotes the block to TinyMCE.
                            const contents = dom.childNodes[1];
                            assert(contents instanceof HTMLDivElement);
                            contents.focus();

                            // Place the caret at the natural edge: when the
                            // selection landed at the block's start (entered
                            // from above), put the caret at the start; when it
                            // landed at the end (entered from below), put it at
                            // the end. Once TinyMCE initializes it preserves
                            // this selection, so the edge placement carries
                            // over.
                            const at_end = main_selection.head >= to;
                            const range = document.createRange();
                            // Walk to the first/last actual text node under
                            // `contents`, rather than using
                            // `selectNodeContents` + `collapse` (which anchors
                            // the selection on `contents` itself, at a
                            // childNodes-index boundary). `saveSelection`
                            // (called later, when this doc block is promoted to
                            // TinyMCE) walks up from
                            // `window.getSelection().anchorNode` looking for an
                            // *ancestor* with the `CodeChat-doc-contents`
                            // class; if the anchor node already *is* that div,
                            // the walk's loop body never runs and it returns an
                            // empty `selection_path`, silently dropping this
                            // edge placement and leaving the caret wherever
                            // TinyMCE's own init happens to put it (its start).
                            // Anchoring on a text node instead keeps the walk
                            // -- and thus the edge placement -- intact.
                            let edge_node: Node = contents;
                            while (
                                at_end
                                    ? edge_node.lastChild
                                    : edge_node.firstChild
                            ) {
                                edge_node = at_end
                                    ? edge_node.lastChild!
                                    : edge_node.firstChild!;
                            }
                            if (edge_node.nodeType === Node.TEXT_NODE) {
                                const offset = at_end
                                    ? (edge_node.textContent?.length ?? 0)
                                    : 0;
                                range.setStart(edge_node, offset);
                                range.setEnd(edge_node, offset);
                            } else {
                                // No text node found (e.g. an empty doc block);
                                // fall back to the previous, element-anchored
                                // behavior.
                                range.selectNodeContents(contents);
                                // `collapse(true)` -> start, `collapse(false)`
                                // -> end.
                                range.collapse(!at_end);
                            }
                            const sel = window.getSelection();
                            sel?.removeAllRanges();
                            sel?.addRange(range);
                        },
                    );
            }
        }
    },
    {
        eventHandlers: {
            // When a doc block receives focus, turn it into a TinyMCE instance
            // so it can be edited. A simpler alternative is to do this in the
            // update() method above, but this is VERY slow, since update is
            // called frequently.
            focusin: (event: FocusEvent, _view: EditorView) => {
                const event_target = event.target;
                const target_or_false = element_is_in_doc_block(event_target);
                if (!(target_or_false instanceof HTMLDivElement)) {
                    return false;
                }
                // Set up for editing the indent of doc blocks.
                const target = target_or_false;
                const indent_div = target.childNodes[0];
                assert(indent_div instanceof HTMLDivElement);
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
                    // Allow only spaces and delete/backspaces when editing the
                    // indent of a doc block.
                    (event: InputEvent) => {
                        // Only modify the behavior of inserts.
                        if (event.data) {
                            // Block any insert that's not an insert of spaces.
                            // TODO: need to support tabs.
                            if (event.data !== " ".repeat(event.data.length)) {
                                event.preventDefault();
                            }
                        }
                    },
                );
                indent_div.addEventListener("input", (event) => {
                    // Signal that this indent is dirty.
                    const target = event.target;
                    if (target instanceof HTMLElement) {
                        on_dirty(target);
                    }
                });

                // If the target is in the indent, not the contents, then the
                // following code isn't needed.
                if (
                    !(event_target instanceof HTMLDivElement) ||
                    event_target.closest(".CodeChat-doc-contents") === null
                ) {
                    return false;
                }
                const [contents_div, is_tinymce] = get_contents(target);

                // Send updated cursor/scroll info.
                startAutoUpdateTimer();

                // See if this is already a TinyMCE instance; if not, move it
                // here.
                if (is_tinymce) {
                    // Nothing to do.
                } else {
                    // Wait until the focus event completes; this causes the
                    // cursor position (the selection) to be set in the
                    // contenteditable div. Then, save that location.
                    setTimeout(async () => {
                        // In case this node was modified during the timeout.
                        if (!contents_div.isConnected) {
                            return;
                        }
                        // Note whether this doc block still genuinely has focus
                        // before any of the DOM surgery below runs (which
                        // removes `contents_div` from the document, making
                        // `document.activeElement` an unreliable way to answer
                        // this question afterwards). If the user has since
                        // clicked or navigated elsewhere while this promotion
                        // was in flight, don't steal focus back to this (now
                        // stale) doc block once the promotion finishes -- see
                        // the check below.
                        const still_focused = target.contains(
                            document.activeElement,
                        );
                        // Create the TinyMCE instance if necessary. Note the
                        // use of `==` here to check for `null` (TinyMCE is
                        // loaded, but no instance exists) and `undefined`
                        // (TinyMCE isn't loaded).
                        if (tinymce_instance() == null) {
                            await init({
                                selector: "#TinyMCE-inst",
                                setup: (editor: Editor) => {
                                    // See the
                                    // [docs](https://www.tiny.cloud/docs/tinymce/latest/events/#editor-core-events).
                                    // After much experimentation, using both an
                                    // `input` event (which suppresses the
                                    // redundant `Dirty` event which follows it)
                                    // combined with a `Dirty` event (which
                                    // catches GUI interactions, undo, etc.
                                    // which doesn't produce an `input` event).
                                    // Just using `Dirty` produces one failing
                                    // case: insert a character (dirty event),
                                    // delete the character (no dirty event),
                                    // left arrow (delayed dirty event from
                                    // backspace).
                                    //
                                    // Here's a demonstration of the bug and its
                                    // fix:
                                    //
                                    // ```html
                                    // <!DOCTYPE html>
                                    // <html lang="en">
                                    // <head>
                                    //     <meta charset="UTF-8">
                                    //     <title>TinyMCE Dirty Event Test</title>
                                    // </head>
                                    // <body>
                                    //     <h1>TinyMCE Dirty Event Test</h1>
                                    //     <textarea id="editor">
                                    //         <p>Edit this content to trigger the dirty event.</p>
                                    //     </textarea>
                                    //     <script
                                    //         src="https://cdn.tiny.cloud/1/rrqw1m3511pf4ag8c5zao97ad7ymvnhqu6z0995b1v63rqb5/tinymce/8/tinymce.min.js"
                                    //         referrerpolicy="origin" crossorigin="anonymous">
                                    //     </script>
                                    //     <script>
                                    //         // Version 1: `dirty` event only; buggy.
                                    //         // Version 2: `input` and `dirty`; works.
                                    //         const version = 2;
                                    //         let ignoreDirty = false;
                                    //         const saveEditor = (eventDescription) => {
                                    //             console.log(`${eventDescription} fired. save() output: ${tinymce.activeEditor.save()}`);
                                    //             ignoreDirty = false;
                                    //         };
                                    //         tinymce.init({
                                    //             selector: '#editor',
                                    //             setup(editor) {
                                    //                 editor.on('dirty', () => {
                                    //                     if (!ignoreDirty || version === 1) {
                                    //                         saveEditor('dirty');
                                    //                     }
                                    //                 });
                                    //                 editor.on('input', () => {
                                    //                     if (version === 2) {
                                    //                         ignoreDirty = true;
                                    //                         saveEditor('input');
                                    //                     }
                                    //                 });
                                    //             }
                                    //         });
                                    //     </script>
                                    // </body>
                                    // ```
                                    editor.on(
                                        "Dirty",
                                        (
                                            event: EditorEvent<
                                                Events.EditorEventMap["dirty"]
                                            >,
                                        ) => {
                                            // Sometimes, `tinymce.activeEditor` is
                                            // null (perhaps when it's not focused).
                                            // Use the `event` data instead. Get the
                                            // div TinyMCE stores edits in.
                                            const target =
                                                event.target.bodyElement;
                                            if (target === null) {
                                                return;
                                            }
                                            if (!ignoreTinyMceDirty) {
                                                on_dirty(target);
                                            }
                                        },
                                    );

                                    editor.on("input", (event: InputEvent) => {
                                        const target = event.target;
                                        // Sometimes, I see non-elements here.
                                        if (target instanceof HTMLElement) {
                                            ignoreTinyMceDirty = true;
                                            on_dirty(target);
                                        }
                                    });

                                    // Send updates on cursor movement.
                                    editor.on(
                                        "SelectionChange",
                                        (
                                            _event: EditorEvent<
                                                Events.EditorEventMap["SelectionChange"]
                                            >,
                                        ) => {
                                            startAutoUpdateTimer();
                                        },
                                    );
                                },
                            });
                        }

                        // Before untypesetting, make sure all other typesets
                        // finish.
                        await new Promise<void>(
                            (resolve) =>
                                window.MathJax?.whenReady?.(() => resolve()) ??
                                resolve(),
                        );
                        // Untypeset math in the old doc block and the current
                        // doc block before moving its contents around.
                        const tinymce_div =
                            document.getElementById(TINYMCE_INST)!;
                        mathJaxUnTypeset(tinymce_div);
                        mathJaxUnTypeset(contents_div);
                        // The code which moves TinyMCE into this div disturbs
                        // all the nodes, which causes it to loose a selection
                        // tied to a specific node.
                        const sel = saveSelection();
                        // With the selection saved, it's safe to replace the
                        // contenteditable div with the TinyMCE instance (which
                        // would otherwise wipe the selection).
                        //
                        // Copy the current TinyMCE instance contents into a
                        // contenteditable div, unless the TinyMCE instance
                        // wasn't in use (currently hidden, since no previous
                        // doc block was being edited).
                        if (
                            !tinymce_div.classList.contains(CODECHAT_DOC_HIDDEN)
                        ) {
                            const old_contents_div =
                                document.createElement("div");
                            old_contents_div.className =
                                "CodeChat-doc-contents";
                            // If the contents aren't editable, then the div
                            // won't receive a `focusin` message (it instead
                            // goes to a CodeMirror layer).
                            old_contents_div.tabIndex = 0;
                            old_contents_div.innerHTML =
                                tinymce_instance()!.save();
                            tinymce_div.parentNode!.insertBefore(
                                old_contents_div,
                                null,
                            );
                            // The previous content edited by TinyMCE is now a
                            // div. Retypeset this after the transition.
                            await mathJaxTypeset(old_contents_div);
                        }
                        // Move TinyMCE to the new location, then remove the old
                        // div it will replace.
                        target.insertBefore(tinymce_div, null);

                        // Calling `setContent()` instead produces spurious
                        // `Dirty` events, observed after receiving a
                        // re-translation. In addition, `resetContent()` clears
                        // the undo history, which is appropriate given that
                        // edits to the previous doc block no longer apply here.
                        // TODO: Eventually, we need a way to chain TinyMCE's
                        // undo history with CodeMirror's undo history.
                        tinymce_instance()!.resetContent(
                            contents_div.innerHTML,
                        );
                        contents_div.remove();
                        tinymce_instance()!.dom.removeClass(
                            tinymce_div,
                            CODECHAT_DOC_HIDDEN,
                        );
                        // The new div is now a TinyMCE editor. Retypeset this.
                        await mathJaxTypeset(tinymce_div);

                        // This process causes TinyMCE to lose focus. Restore
                        // that -- but only if focus was still genuinely in this
                        // doc block just before the DOM surgery above began
                        // (see `still_focused`). Unconditionally focusing here
                        // would otherwise steal focus back to this (now stale)
                        // doc block even after the user clicked or navigated
                        // elsewhere while this promotion was in flight.
                        // Restoring the selection is skipped too, since it's
                        // meaningless once focus has moved on.
                        if (!still_focused) {
                            return;
                        }
                        // However, this causes TinyMCE to lose the selection,
                        // which the next bit of code then restores. When the
                        // doc block is longer than a screen, omitting the
                        // `preventScroll` parameter causes this to scroll to
                        // the top of the doc block, which is incorrect.
                        tinymce_div.focus({ preventScroll: true });

                        // Copy the selection over to TinyMCE by indexing the
                        // selection path to find the selected node.
                        restoreSelection(sel);
                    }, 0);
                }
                return false;
            },

            // The indent of a doc block is only editable while it's being
            // clicked on/focused; otherwise, it's plain (uneditable) text. This
            // keeps it out of the keyboard/IDE-driven navigation path (see the
            // "Keyboard navigation" section above), which only ever focuses the
            // contents div. Turning it editable on click is handled by an
            // inline `onmousedown` attribute in `DocBlockWidget.toDOM` rather
            // than here -- see the comment there for why a `ViewPlugin`
            // `eventHandlers.mousedown` handler doesn't work for this.
            //
            // Once the indent loses focus, make it uneditable again.
            focusout: (event: FocusEvent, _view: EditorView) => {
                const target = event.target;
                if (target instanceof HTMLElement) {
                    const indent_div = target.closest(".CodeChat-doc-indent");
                    if (indent_div instanceof HTMLElement) {
                        indent_div.contentEditable = "false";
                    }
                }
                return false;
            },
        },
    },
);

// UI
// --
//
// There doesn't seem to be any tracking of a dirty/clean flag built into
// CodeMirror v6 (although
// [v5 does](https://codemirror.net/5/doc/manual.html#isClean)). The best I've
// found is a
// [forum post](https://discuss.codemirror.net/t/codemirror-6-proper-way-to-listen-for-changes/2395/11)
// showing code to do this, which I use below.
//
// How this works: the
// [EditorView.updateListener](https://codemirror.net/docs/ref/#codemirror) is a
// [Facet](https://codemirror.net/docs/ref/#state.Facet) with an
// [of function](https://codemirror.net/docs/ref/#state.Facet.of) that creates a
// CodeMirror extension.
const autosaveExtension = EditorView.updateListener.of(
    // CodeMirror passes this function a
    // [ViewUpdate](https://codemirror.net/docs/ref/#view.ViewUpdate) which
    // describes a change being made to the document.
    (v: ViewUpdate) => {
        // Ignore any transaction group marked with a `noAutosaveAnnotation`.
        if (
            v.transactions.some(
                (tr) => tr.annotation(noAutosaveAnnotation) === true,
            )
        ) {
            return true;
        }

        // The
        // [docChanged](https://codemirror.net/docs/ref/#view.ViewUpdate.docChanged)
        // flag is the relevant part of this change description. However, this
        // only describes changes to the code blocks (the document, from
        // CodeMirror's perspective).
        let isChanged = v.docChanged;
        // Look for changes to doc blocks as well; skip if a change was already
        // detected for efficiency.
        if (!v.docChanged && v.transactions.length) {
            // Check each effect of each transaction.
            outer: for (const tr of v.transactions) {
                for (const effect of tr.effects) {
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
            startAutoUpdateTimer();
        } else if (v.selectionSet) {
            // Send an update if only the selection changed.
            startAutoUpdateTimer();
        }
    },
);

// Wrap a stream language dynamic import.
const import_stream_language = async (lang: StreamParser<unknown>) =>
    (await import("@codemirror/language")).StreamLanguage.define(lang);

// Given source code in a CodeMirror-friendly JSON format, load it into the
// provided div.
export const CodeMirror_load = async (
    // The div to place the loaded document in.
    codechat_body: HTMLDivElement,
    // The document to load.
    codechat_for_web: CodeChatForWeb,
    // Additional extensions.
    extensions: Array<Extension>,
    cursor_position?: CursorPosition,
    scroll_line?: number,
) => {
    if ("Plain" in codechat_for_web.source) {
        // Although the
        // [docs](https://codemirror.net/docs/ref/#state.EditorState^fromJSON)
        // specify a
        // [EditorStateConfig](https://codemirror.net/docs/ref/#state.EditorStateConfig)
        // which contains `doc` and `selection`, the implementation requires
        // these to be present in the `json` (first) argument. Therefore:
        const editor_state_json = {
            doc: codechat_for_web.source.Plain.doc,
            selection: EditorSelection.single(0).toJSON(),
            doc_blocks: codechat_for_web.source.Plain.doc_blocks,
        };
        // Save the current scroll position, to prevent the view from scrolling
        // back to the top after an update/reload.
        let scrollSnapshot;
        if (current_view !== undefined) {
            scrollSnapshot = current_view.scrollSnapshot();
            // For reloads, we need to remove previous instances; otherwise, Bad
            // Things happen.
            tinymce?.remove();
        }

        codechat_body.innerHTML = `<div class="CodeChat-CodeMirror"></div><div id="${TINYMCE_INST}" class="CodeChat-doc-contents ${CODECHAT_DOC_HIDDEN}" spellcheck="true"></div>`;
        let parser;
        // Dynamically load the parser.
        switch (codechat_for_web.metadata.mode) {
            // Languages with a parser.
            case "sh":
                parser = await import_stream_language(
                    (await import("@codemirror/legacy-modes/mode/shell")).shell,
                );
                break;
            case "cpp":
                parser = (await import("@codemirror/lang-cpp")).cpp();
                break;
            case "csharp":
                parser = (
                    await import("@codemirror/lang-javascript")
                ).javascript();
                break;
            case "css":
                parser = (await import("@codemirror/lang-css")).css();
                break;
            case "golang":
                parser = (await import("@codemirror/lang-go")).go();
                break;
            case "html":
                parser = (await import("@codemirror/lang-html")).html();
                break;
            case "java":
                parser = (await import("@codemirror/lang-java")).java();
                break;
            case "javascript":
                parser = (
                    await import("@codemirror/lang-javascript")
                ).javascript();
                break;
            // Octave is an open-source MATLAB-ish clone.
            case "matlab":
                parser = await import_stream_language(
                    (await import("@codemirror/legacy-modes/mode/octave"))
                        .octave,
                );
                break;
            case "python":
                parser = (await import("@codemirror/lang-python")).python();
                break;
            case "rust":
                parser = (await import("@codemirror/lang-rust")).rust();
                break;
            case "sql":
                parser = (await import("@codemirror/lang-sql")).sql();
                break;
            case "swift":
                parser = await import_stream_language(
                    (await import("@codemirror/legacy-modes/mode/swift")).swift,
                );
                break;
            case "toml":
                parser = await import_stream_language(
                    (await import("@codemirror/legacy-modes/mode/toml")).toml,
                );
                break;
            case "typescript":
                parser = (
                    await import("@codemirror/lang-javascript")
                ).javascript({ typescript: true });
                break;
            case "vhdl":
                parser = await import_stream_language(
                    (await import("@codemirror/legacy-modes/mode/vhdl")).vhdl,
                );
                break;
            case "verilog":
                parser = await import_stream_language(
                    (await import("@codemirror/legacy-modes/mode/verilog"))
                        .verilog,
                );
                break;
            case "yaml":
                parser = (await import("@codemirror/lang-yaml")).yaml();
                break;

            // Languages without a parser.
            //
            // JSON5 allows comments, but JSON doesn't.
            case "json5":
                parser = (await import("@codemirror/lang-json")).json();
                break;
            // An approximation for Vlang.
            case "v":
                parser = (
                    await import("@codemirror/lang-javascript")
                ).javascript();
                break;

            default:
                parser = (
                    await import("@codemirror/lang-javascript")
                ).javascript();
                report_error(
                    `Unknown lexer name ${codechat_for_web.metadata.mode}`,
                );
                break;
        }
        const state = EditorState.fromJSON(
            editor_state_json,
            {
                extensions: [
                    DocBlockPlugin,
                    // Move focus into adjacent doc blocks on arrow-key
                    // navigation. High precedence so it runs before the default
                    // arrow-key commands in `basicSetup`.
                    Prec.high(docBlockNavKeymap),
                    parser,
                    basicSetup,
                    EditorView.lineWrapping,
                    exceptionSink,
                    autosaveExtension,
                    // Make tab an indent per the
                    // [docs](https://codemirror.net/examples/tab/). TODO:
                    // document a way to escape the tab key per the same docs.
                    keymap.of([indentWithTab]),
                    // Change the font size. See
                    // [this post](https://discuss.codemirror.net/t/changing-the-font-size-of-cm6/2935/6).
                    [
                        // TODO: get these values from the IDE, so we match its
                        // size.
                        EditorView.theme({
                            "&": {
                                fontSize: "14px",
                            },
                            ".cm-content": {
                                fontFamily:
                                    "Consolas, 'Courier New', monospace",
                            },
                        }),
                    ],
                    ...extensions,
                ],
            },
            CodeMirror_JSON_fields,
        );
        const codechat_div = codechat_body.childNodes[0];
        assert(codechat_div instanceof HTMLDivElement);
        current_view = new EditorView({
            parent: codechat_div,
            state,
            scrollTo: scrollSnapshot,
        });
    } else {
        // This contains a diff, instead of plain text. Apply the text diff.
        //
        // First, apply just the text edits. Use an annotation so that the doc
        // blocks aren't changed; without this, the diff won't work (since
        // from/to values of doc blocks are changed by unfrozen text edits).
        current_view.dispatch({
            changes: codechat_for_web.source.Diff.doc,
            annotations: [
                docBlockFreezeAnnotation.of(true),
                noAutosaveAnnotation.of(true),
            ],
        });
        // Now, apply the diff in a separate transaction. Applying them in the
        // same transaction causes the text edits to modify from/to values in
        // the doc block effects, even when changes to the doc block state is
        // frozen.
        const stateEffects: StateEffect<unknown>[] = [];
        for (const transaction of codechat_for_web.source.Diff.doc_blocks) {
            if ("Add" in transaction) {
                const add = transaction.Add;
                stateEffects.push(
                    addDocBlock.of({
                        from: add[0],
                        to: add[1],
                        indent: add[2],
                        delimiter: add[3],
                        contents: add[4],
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
        current_view.dispatch({
            effects: stateEffects,
            annotations: noAutosaveAnnotation.of(true),
        });
    }
    scroll_to_line(cursor_position, scroll_line);
};

// Scroll to the provided `scroll_line`; place the cursor at `cursor_line`.
export const scroll_to_line = (
    cursor_position?: CursorPosition,
    scroll_line?: number,
) => {
    if (cursor_position === undefined && scroll_line === undefined) {
        return;
    }

    // Create a transaction to set the cursor and scroll position. Avoid an
    // autosave that sends updated cursor/scroll positions produced by this
    // transaction.
    const dispatch_data: TransactionSpec = {
        annotations: noAutosaveAnnotation.of(true),
    };
    if (cursor_position !== undefined) {
        // Translate the line numbers to a position.
        if ("Line" in cursor_position) {
            const cursor_pos = current_view?.state.doc.line(
                cursor_position.Line,
            ).from;
            dispatch_data.selection = {
                anchor: cursor_pos,
                head: cursor_pos,
            };
        } else {
            report_error("Not supported.");
        }
        // If a scroll position is provided, use it; otherwise, scroll the
        // cursor into the current view.
        if (scroll_line === undefined) {
            dispatch_data.scrollIntoView = true;
        }
    }

    if (scroll_line !== undefined) {
        const scroll_pos = current_view?.state.doc.line(scroll_line).from;
        dispatch_data.effects = EditorView.scrollIntoView(scroll_pos, {
            y: "start",
        });
    }

    // Run it.
    current_view?.dispatch(dispatch_data);

    // Restore the previous horizontal scroll position, overriding whatever
    // `scrollIntoView` set. Defer to the next frame so this runs after
    // CodeMirror has applied its own scroll from the transaction above.
    if (scroll_line !== undefined) {
        // With line wrapping enabled, the only source of horizontal scroll is a
        // doc block containing a long, non-wrapping line. CodeMirror's
        // `scrollIntoView` can't measure a position inside such a block
        // reliably and pins `scrollLeft` to its maximum regardless of the `x`
        // option. We only want to scroll vertically, so capture the horizontal
        // position now and restore it after the dispatch.
        const prev_scroll_left = current_view?.scrollDOM.scrollLeft;
        requestAnimationFrame(() => {
            if (current_view) {
                current_view.scrollDOM.scrollLeft = prev_scroll_left;
            }
        });
    }
};

// Apply a `StringDiff` to the before string to produce the after string.
export const apply_diff_str = (before: string, diffs: StringDiff[]) => {
    // Walk from the last diff to the first. JavaScript doesn't have reverse
    // iteration AFAIK.
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
    /// @ts-expect-error("This does exist.")
    delete code_mirror.selection;

    return { Plain: code_mirror };
};

export const set_CodeMirror_positions = (
    update_message_contents: UpdateMessageContents,
) => {
    // If a doc block has focus, then the CodeMirror selection reports line 1.
    // Use the starting line number of the doc block instead.
    const doc_block = document.activeElement?.closest(".CodeChat-doc");
    let cursor_position: CursorPosition;
    if (doc_block) {
        const from = current_view.posAtDOM(doc_block);
        const location = saveSelection();
        // If there's a selection in the doc block, pass the DOM location;
        // otherwise, pass the line where the doc block starts.
        if (location.selection_offset === undefined) {
            cursor_position = {
                Line: current_view.state.doc.lineAt(from).number,
            };
        } else {
            cursor_position = {
                DomLocation: {
                    dom_path: location.selection_path,
                    dom_offset: location.selection_offset,
                    from,
                },
            };
        }
    } else {
        // For a code block, we can simply retrieve the line number.
        cursor_position = {
            Line: current_view.state.doc.lineAt(
                current_view.state.selection.main.from,
            ).number,
        };
    }
    update_message_contents.cursor_position = cursor_position;

    // `current_view.viewport.from` isn't accurate, since it's not really the
    // top line, but a margin before it; see the
    // [docs](https://codemirror.net/docs/ref/#view.EditorView.viewport).
    // Instead, use
    // [this approach](https://discuss.codemirror.net/t/how-can-i-get-the-top-line-number-in-real-time/9404).
    // This value still seems a bit off, probably because CodeMirror doesn't
    // account for doc block sizing?
    update_message_contents.scroll_position = current_view.state.doc.lineAt(
        current_view.lineBlockAtHeight(-current_view.documentTop).from,
    ).number;
};

const report_error = (text: string) => {
    console.error(text);
    show_toast(text);
};

const halt_on_error = (text: string): never => {
    document.getElementById("error-overlay")!.style.display = "block";
    console.error(text);
    // The error handler will make this a toast.
    throw new Error(text);
};
