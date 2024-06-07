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
// # `tinymce-webpack.ts` -- integrate and configure the TinyMCE editor for use with the CodeChat Editor
//
// Import TinyMCE.
import {
    default as tinymce_,
    Editor,
    RawEditorOptions,
    TinyMCE,
} from "tinymce";
// TODO: The type of tinymce is broken; I don't know why. Here's a workaround.
export const tinymce = tinymce_ as any as TinyMCE;
export { Editor };

// Default icons are required for TinyMCE 5.3 or above.
import "tinymce/icons/default/index.js";

// A theme is also required.
import "tinymce/themes/silver/index.js";

// Import the skin to use; use're using an inline editor, so load the inline
// CSS.
import "tinymce/skins/ui/oxide/skin.css";
import "tinymce/skins/ui/oxide/content.inline.css";

// Without this, TinyMCE produces errors.
import "tinymce/models/dom/index.js";

// Import plugins.
import "tinymce/plugins/advlist/index.js";
import "tinymce/plugins/anchor/index.js";
import "tinymce/plugins/charmap/index.js";
import "tinymce/plugins/code/index.js";
import "tinymce/plugins/directionality/index.js";
import "tinymce/plugins/emoticons/index.js";
import "tinymce/plugins/emoticons/js/emojis.js";
import "tinymce/plugins/emoticons/js/emojiimages.js";
import "tinymce/plugins/help/index.js";
// TODO: this should be a dynamic import.
import "tinymce/plugins/help/js/i18n/keynav/en.js";
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

// Import premium plugins. NOTE: Download separately and add these to
// /src/plugins.
/// import './plugins/checklist/plugin';
/// import './plugins/powerpaste/plugin';
/// import './plugins/powerpaste/js/wordimport';

// Initialize TinyMCE.
export const init = async (
    // Provide editor options; don't set \`\`plugins\`\` or \`\`skin\`\`, since
    // these must be accompanied by the correct imports.
    options: RawEditorOptions,
) =>
    // See
    // [init()](https://www.tiny.cloud/docs/tinymce/6/apis/tinymce.root/#init).
    tinymce.init(
        Object.assign({}, options, {
            // See the list of
            // [plugins](https://www.tiny.cloud/docs/tinymce/6/plugins/). These
            // must be accompanied by the corresponding import above.
            plugins:
                "advlist anchor charmap directionality emoticons help image link lists media nonbreaking pagebreak quickbars searchreplace table visualblocks visualchars",
            // The imports above apply the skins; don't try to dynamically load
            // the skin's CSS.
            skin: false,
            // Enable the
            // [browser-supplied spellchecker](https://www.tiny.cloud/docs/tinymce/6/spelling/#browser_spellcheck),
            // since TinyMCE's spellchecker is a premium feature.
            browser_spellcheck: true,
            // Put more buttons on the
            // [quick toolbar](https://www.tiny.cloud/docs/tinymce/6/quickbars/)
            // that appears when text is selected. TODO: add a button for code
            // format (can't find this one -- it's only on the
            // [list of menu items](https://www.tiny.cloud/docs/tinymce/6/available-menu-items/#the-core-menu-items)
            // as `codeformat`).
            quickbars_selection_toolbar:
                "align | bold italic underline | quicklink h2 h3 blockquote",
            // Place the Tiny MCE menu bar at the top of the screen; otherwise,
            // it floats in front of text, sometimes obscuring what the user
            // wants to edit. See the
            // [docs](https://www.tiny.cloud/docs/configure/editor-appearance/#fixed_toolbar_container).
            fixed_toolbar_container: "#CodeChat-menu",
            inline: true,
            // When true, this still prevents hyperlinks to anchors on the
            // current page from working correctly. There's an onClick handler
            // that prevents links in the current page from working -- need to
            // look into this. See also
            // [a related GitHub issue](https://github.com/tinymce/tinymce/issues/3836).
            //readonly: true  // Per the comment above, this is commented out.
            // TODO: Notes on this setting.
            relative_urls: true,
            // This combines the
            // [default TinyMCE toolbar buttons](https://www.tiny.cloud/blog/tinymce-toolbar/)
            // with a few more from plugins. I like the default, so this is
            // currently disabled.
            //toolbar: 'undo redo | styleselect | bold italic | alignleft aligncenter alignright alignjustify | outdent indent | numlist bullist | ltr rtl | help',
            // See
            // [License key](https://www.tiny.cloud/docs/tinymce/latest/license-key).
            license_key: "gpl",

            // Settings for plugins
            //
            // [Image](https://www.tiny.cloud/docs/plugins/opensource/image/)
            image_caption: true,
            image_advtab: true,
            image_title: true,
            // Needed to allow custom elements.
            extended_valid_elements:
                "graphviz-graph[graph|scale],graphviz-script-editor[value|tab],graphviz-combined[graph|scale]",
            custom_elements:
                "graphviz-graph,graphviz-script-editor,graphviz-combined",
        }),
    );
