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
// `tinymce-config.ts` -- integrate and configure the TinyMCE editor for use
// with the CodeChat Editor
// =============================================================================
//
// Import TinyMCE.
import {
    default as tinymce_,
    Editor,
    RawEditorOptions,
    TinyMCE,
} from "tinymce";
// TODO: The type of tinymce is broken; I don't know why. Here's a workaround.
export const tinymce = tinymce_ as unknown as TinyMCE;
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
import "tinymce/plugins/quickbars/index.js";
import "tinymce/plugins/searchreplace/index.js";
import "tinymce/plugins/table/index.js";
import "tinymce/plugins/visualblocks/index.js";
import "tinymce/plugins/visualchars/index.js";

// Import premium plugins. NOTE: Download separately and add these to
// `src/plugins`.
/// import './plugins/checklist/plugin';
/// import './plugins/powerpaste/plugin';
/// import './plugins/powerpaste/js/wordimport';

// Initialize TinyMCE.
export const init = async (
    // Provide editor options; don't set `plugins` or `skin`, since these must
    // be accompanied by the correct imports.
    options: RawEditorOptions,
) => {
    // Merge the provided options with these default options.
    const combinedOptions = Object.assign({}, options, {
        // See the list of
        // [plugins](https://www.tiny.cloud/docs/tinymce/6/plugins/). These must
        // be accompanied by the corresponding import above.
        plugins:
            "advlist anchor charmap directionality emoticons help image link lists media quickbars searchreplace table visualblocks visualchars",
        // The imports above apply the skins; don't try to dynamically load the
        // skin's CSS.
        skin: false,
        // Enable the
        // [browser-supplied spellchecker](https://www.tiny.cloud/docs/tinymce/6/spelling/#browser_spellcheck),
        // since TinyMCE's spellchecker is a premium feature.
        browser_spellcheck: true,
        // Place the Tiny MCE menu bar at the top of the screen; otherwise, it
        // floats in front of text, sometimes obscuring what the user wants to
        // edit. See the
        // [docs](https://www.tiny.cloud/docs/configure/editor-appearance/#fixed_toolbar_container).
        fixed_toolbar_container: "#CodeChat-menu",
        inline: true,
        // When true, this still prevents hyperlinks to anchors on the current
        // page from working correctly. There's an onClick handler that prevents
        // links in the current page from working -- need to look into this. See
        // also
        // [a related GitHub issue](https://github.com/tinymce/tinymce/issues/3836).
        //readonly: true  // Per the comment above, this is commented out.
        // Use relative URLs in hyperlinks.
        relative_urls: true,
        // Disable the
        // [TinyMCE toolbar buttons](https://www.tiny.cloud/blog/tinymce-toolbar/)
        // to provide more real estate on the screen.
        toolbar: false,
        // Don't show the file option on the
        // [menu](https://www.tiny.cloud/docs/tinymce/6/menus-configuration-options/#menubar),
        // which is useless.
        menubar: "edit insert view format table tools help",
        // See
        // [License key](https://www.tiny.cloud/docs/tinymce/latest/license-key).
        license_key: "gpl",
        // Block drag-and-drop of unsupported images and files. See the
        // [docs](https://www.tiny.cloud/docs/tinymce/latest/file-image-upload/#block_unsupported_drop).
        block_unsupported_drop: true,
        // Prevent drag-and-dropping images; this create a mess. See the
        // [docs](https://www.tiny.cloud/docs/tinymce/latest/copy-and-paste/#paste_data_images).
        paste_data_images: false,

        // ### Settings for plugins
        //
        // [Image](https://www.tiny.cloud/docs/plugins/opensource/image/)
        image_caption: true,
        image_advtab: true,
        image_title: true,

        // Quickbar config: disable the insert toolbar (which doesn't seem
        // useful, and also has the image insert, which is problematic
        // currently).
        quickbars_insert_toolbar: false,
        // Put more buttons on the
        // [quick toolbar](https://www.tiny.cloud/docs/tinymce/6/quickbars/)
        // that appears when text is selected. TODO: add a button for code
        // format (can't find this one -- it's only on the
        // [list of menu items](https://www.tiny.cloud/docs/tinymce/6/available-menu-items/#the-core-menu-items)
        // as `codeformat`).
        quickbars_selection_toolbar:
            "bold italic underline codeformat | quicklink h2 h3",

        // Needed to allow custom elements.
        extended_valid_elements: "graphviz-graph[scale],wc-mermaid",
        custom_elements: "graphviz-graph,wc-mermaid",
    });

    // Merge in additional setup code.
    const oldSetup = combinedOptions.setup;
    combinedOptions.setup =
        // Add a "Format as code" button (generated by Gemini).
        (editor: Editor) => {
            oldSetup?.(editor);
            editor.ui.registry.addToggleButton("codeformat", {
                text: "<>",
                tooltip: "Format as code",
                onAction: () =>
                    editor.execCommand("mceToggleFormat", false, "code"),
                onSetup: (api) => {
                    const changed = editor.formatter.formatChanged(
                        "code",
                        (state) => api.setActive(state),
                    );
                    return () => changed.unbind();
                },
            });
        };

    // Use these combined options to
    // [init](https://www.tiny.cloud/docs/tinymce/6/apis/tinymce.root/#init)
    // TinyMCE.
    return tinymce.init(combinedOptions);
};
