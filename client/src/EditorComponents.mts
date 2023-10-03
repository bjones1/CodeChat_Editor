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
// # `EditorComponents.mts` -- Custom HTML tags which provide authoring support for the CodeChat Editor
//
// Create a combined editor/renderer component. It's not currently used, since
// TinyMCE doesn't allow the editor to be focused.
class GraphVizElement extends HTMLElement {
    constructor() {
        super();
        // Create the shadow DOM.
        const shadowRoot = this.attachShadow({ mode: "open" });
        const editor = document.createElement("graphviz-script-editor");
        const graph = document.createElement("graphviz-graph");

        // TODO: Copy other attributes (scale, tabs, etc.) which the editor and
        // graph renderer support.

        // Propagate the initial value on this tag to the tags in the shadow
        // DOM.
        const dot = this.getAttribute("graph") ?? "";
        graph.setAttribute("graph", dot);
        editor.setAttribute("value", dot);

        // Send edits to both this tag and the graphviz rendering tag.
        editor.addEventListener("input", (event) => {
            // Ignore InputEvents -- we want the custom event sent by this
            // component, which contains new text for the graph.
            if (event instanceof CustomEvent) {
                const dot = (event as any).detail;
                graph.setAttribute("graph", dot);
                // Update the root component as well, so that this value will be
                // correct when the user saves.
                this.setAttribute("graph", dot);
            }
        });

        // Populate the shadow DOM now that everything is ready.
        shadowRoot.append(editor, graph);
    }
}
customElements.define("graphviz-combined", GraphVizElement);
