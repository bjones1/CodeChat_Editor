 /* Import TinyMCE */
 import tinymce, { RawEditorOptions } from 'tinymce';

 /* Default icons are required for TinyMCE 5.3 or above */
 import 'tinymce/icons/default';

 /* A theme is also required */
 import 'tinymce/themes/silver';

 /* Import the skin */
 import 'tinymce/skins/ui/oxide/skin.css';

 /* Import content css for a skin. */
 import 'tinymce/skins/ui/oxide/content.inline.css';
 import 'tinymce/skins/content/default/content.css';

 /* Import plugins */
 import 'tinymce/plugins/advlist';
 import 'tinymce/plugins/anchor';
 import 'tinymce/plugins/charmap';
 import 'tinymce/plugins/code';
 import 'tinymce/plugins/directionality';
 import 'tinymce/plugins/emoticons';
 import 'tinymce/plugins/emoticons/js/emojis';
 import 'tinymce/plugins/help';
 import 'tinymce/plugins/image';
 import 'tinymce/plugins/link';
 import 'tinymce/plugins/lists';
 import 'tinymce/plugins/media';
 import 'tinymce/plugins/nonbreaking';
 import 'tinymce/plugins/pagebreak';
 import 'tinymce/plugins/quickbars';
 import 'tinymce/plugins/searchreplace';
 import 'tinymce/plugins/table';
 import 'tinymce/plugins/visualblocks';
 import 'tinymce/plugins/visualchars';

 /* Import premium plugins */
 /* NOTE: Download separately and add these to /src/plugins */
 /** import './plugins/checklist/plugin'; */
 /** import './plugins/powerpaste/plugin'; */
 /** import './plugins/powerpaste/js/wordimport'; */

 /* Initialize TinyMCE */
 export function init(
    // Provide editor options; don't set ``plugins`` or ``skin``, since these must be accompanied by the correct imports.
    options: RawEditorOptions
) {
    tinymce.init(Object.assign({}, options, {
        plugins: 'advlist anchor charmap directionality emoticons help image link lists media nonbreaking pagebreak quickbars searchreplace table visualblocks visualchars',
        // The imports above apply the skins; don't try to dynamically load the skin's CSS.
        skin: false,
    }));
};
