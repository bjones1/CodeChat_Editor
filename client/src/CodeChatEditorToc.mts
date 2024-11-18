// # `CodeChatEditorToc` -- Load the JavaScript needed for the TOC.
// Currently, it's simple -- only MathJax.

import "./MathJax-config.mts";
// Likewise, this must be imported _after_ the previous setup import, so it's placed here,
// instead of in the third-party category above.
import "mathjax/tex-chtml.js";
