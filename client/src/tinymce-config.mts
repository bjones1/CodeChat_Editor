// Copyright (C) 2023 Bryan A. Jones.
//
// This file is part of the CodeChat Editor. The CodeChat Editor is free
// software: you can redistribute it and/or modify it under the terms of the
// GNU General Public License as published by the Free Software Foundation,
// either version 3 of the License, or (at your option) any later version.
//
// The CodeChat Editor is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY
// or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for
// more details.
//
// You should have received a copy of the GNU General Public License along with
// the CodeChat Editor. If not, see
// [http://www.gnu.org/licenses](http://www.gnu.org/licenses).
//
// <h1><code>tinymce-webpack.ts</code> -- integrate and configure the TinyMCE editor for use with the CodeChat Editor</h1>
// <p>Import TinyMCE.</p>
import {
    default as tinymce_,
    Editor,
    RawEditorOptions,
    TinyMCE,
} from "tinymce";
// TODO: The type of tinymce is broken; I don't know why. Here's a workaround.
export const tinymce = tinymce_ as any as TinyMCE;
export { Editor };

// <p>Default icons are required for TinyMCE 5.3 or above.</p>
import "tinymce/icons/default/index.js";

// <p>A theme is also required.</p>
import "tinymce/themes/silver/index.js";

// <p>Import the skin to use; use're using an inline editor, so load the inline
//     CSS.</p>
import "tinymce/skins/ui/oxide/skin.css";
import "tinymce/skins/ui/oxide/content.inline.css";

// <p>Without this, TinyMCE produces errors.</p>
import "tinymce/models/dom/index.js";

// <p>Import plugins.</p>
import "tinymce/plugins/advlist/index.js";
import "tinymce/plugins/anchor/index.js";
import "tinymce/plugins/charmap/index.js";
import "tinymce/plugins/code/index.js";
import "tinymce/plugins/directionality/index.js";
import "tinymce/plugins/emoticons/index.js";
import "tinymce/plugins/emoticons/js/emojis.js";
import "tinymce/plugins/emoticons/js/emojiimages.js";
import "tinymce/plugins/help/index.js";
import "tinymce/plugins/image/index.js";
import "tinymce/plugins/link/index.js";
import "tinymce/plugins/lists/index.js";
import "tinymce/plugins/media/index.js";
import "tinymce/plugins/nonbreaking/index.js";
import "tinymce/plugins/pagebreak/index.js";
import "tinymce/plugins/quickbars/index.js";
import "tinymce/plugins/searchreplace/index.js";
import "tinymce/plugins/table/index.js";
import "tinymce/plugins/visualblocks/index.js";
import "tinymce/plugins/visualchars/index.js";

// <p>Import premium plugins. NOTE: Download separately and add these to
//     /src/plugins.</p>
/// import './plugins/checklist/plugin';
/// import './plugins/powerpaste/plugin';
/// import './plugins/powerpaste/js/wordimport';

// <p>Initialize TinyMCE.</p>
export const init = async (
    // <p>Provide editor options; don't set ``plugins`` or ``skin``, since these
    //     must be accompanied by the correct imports.</p>
    options: RawEditorOptions
) =>
    // See <a href="https://www.tiny.cloud/docs/tinymce/6/apis/tinymce.root/#init">init()</a>.
    tinymce.init(
        Object.assign({}, options, {
            // <p>See the list of <a
            //         href="https://www.tiny.cloud/docs/tinymce/6/plugins/">plugins</a>. These must be accompanied by the corresponding import above.
            // </p>
            plugins:
                "advlist anchor charmap directionality emoticons help image link lists media nonbreaking pagebreak quickbars searchreplace table visualblocks visualchars",
            // <p>The imports above apply the skins; don't try to dynamically
            //     load the skin's CSS.</p>
            skin: false,
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
        })
    );
