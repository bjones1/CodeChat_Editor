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
// <h1><code>CodeChat-editor.mts</code> &mdash; <strong>JavaScrip</strong>t
//     which implements part of the client-side portion of the CodeChat Editor
// </h1>
// <p>The CodeChat Editor provides a simple IDE which allows editing of mixed
//     code and doc blocks.</p>
// <h2>Imports</h2>
import { init } from "./tinymce-webpack.mjs";
import "./ace-webpack.mts";

// Configure the Graphviz web component to load the (large) renderer only when a Graphviz web component is found on a page. See the <a href="https://github.com/prantlf/graphviz-webcomponent#configuration">docs</a>.
(window as any).graphvizWebComponent = {
    rendererUrl: "/static/graphviz-webcomponent/renderer.min.js",
    delayWorkerLoading: true,
};
import "graphviz-webcomponent";

// <h2>UI</h2>
// <h3>DOM ready event</h3>
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
// <p>Export this to the browser's Window object. Use a typecast to allow the
//     assignment.</p>
(window as any).on_dom_content_loaded = on_dom_content_loaded;

init({});

// Create a combined editor/renderer component. It's not currently used, since TinyMCE doesn't allow the editor to be focused.
class GraphVizElement extends HTMLElement {
    constructor() {
        super();
        // <p>Create the shadow DOM.</p>
        const shadowRoot = this.attachShadow({ mode: "open" });
        const editor = document.createElement("graphviz-script-editor");
        const graph = document.createElement("graphviz-graph");

        // <p>TODO: Copy other attributes (scale, tabs, etc.) which the editor
        //     and graph renderer support.</p>

        // <p>Propagate the initial value on this tag to the tags in the shadow
        //     DOM.</p>
        const dot = this.getAttribute("graph") ?? "";
        graph.setAttribute("graph", dot);
        editor.setAttribute("value", dot);

        // <p>Send edits to both this tag and the graphviz rendering tag.</p>
        editor.addEventListener("input", (event) => {
            // <p>Ignore InputEvents -- we want the custom event sent by this
            //     component, which contains new text for the graph.</p>
            if (event instanceof CustomEvent) {
                const dot = (event as any).detail;
                graph.setAttribute("graph", dot);
                // <p>Update the root component as well, so that this value will
                //     be correct when the user saves.</p>
                this.setAttribute("graph", dot);
            }
        });

        // <p>Populate the shadow DOM now that everything is ready.</p>
        shadowRoot.append(editor, graph);
    }
}
customElements.define("graphviz-combined", GraphVizElement);
