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
// <h1><code>CodeChatEditor.js</code> &mdash; <strong>JavaScrip</strong>t which
//     implements the client-side portion of the CodeChat Editor</h1>
// <p>The CodeChat Editor provides a simple IDE which allows editing of mixed
//     code and doc blocks.</p>
//
// <h2>UI</h2>
// <h3>DOM ready event</h3>
// <p>This is copied from <a
//         href="https://developer.mozilla.org/en-US/docs/Web/API/Document/DOMContentLoaded_event#checking_whether_loading_is_already_complete">MDN</a>.
// </p>
const on_dom_content_loaded = (on_load_func: (() => void)) => {
    if (document.readyState === "loading") {
        // <p>Loading hasn't finished yet.</p>
        document.addEventListener("DOMContentLoaded", on_load_func);
    } else {
        // <p><code>DOMContentLoaded</code> has already fired.</p>
        on_load_func();
    }
}
// Export this to the browser's Window object. Use a typecast to allow the assignment.
(window as any).on_dom_content_loaded = on_dom_content_loaded;

import { init } from "./tinymce-webpack.mjs"
init({});

import "./ace-webpack.mts";

class GraphVizElement extends HTMLElement {
    constructor() {
        super();
        // Dynamically import the graphviz package, then finish construction.
        import("graphviz-webcomponent/bundled").then(this.async_constructor);
    }

    async_constructor = async (_module: Promise<any>) => {
        // Create the shadow DOM.
        const shadowRoot = this.attachShadow({ mode: "open" });
        const editor = document.createElement("graphviz-script-editor");
        const graph = document.createElement("graphviz-graph");

        // TODO: Copy other attributes (scale, tabs, etc.) which the editor and graph renderer support.

        // Propagate the initial value on this tag to the tags in the shadow DOM.
        const dot = this.getAttribute("graph") ?? ""
        graph.setAttribute("graph", dot);
        editor.setAttribute("value", dot);

        // Send edits to both this tag and the graphviz rendering tag.
        editor.addEventListener("input", event => {
            // Ignore InputEvents -- we want the custom event sent by this component, which contains new text for the graph.
            if (event instanceof CustomEvent) {
                const dot = (event as any).detail;
                graph.setAttribute("graph", dot)
                // Update the root component as well, so that this value will be correct when the user saves.
                this.setAttribute("graph", dot)
            }
        });

        // Populate the shadow DOM now that everything is ready.
        shadowRoot.append(editor, graph);
    }
}
customElements.define("graphviz-combined", GraphVizElement);
