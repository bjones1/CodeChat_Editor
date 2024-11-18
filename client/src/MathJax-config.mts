// # `MathJax-config.mts` -- Configure MathJax.
window.MathJax = {
    // See the [docs](https://docs.mathjax.org/en/latest/options/output/chtml.html#option-descriptions).
    chtml: {
        fontURL: "/static/mathjax-modern-font/chtml/woff",
    },
    tex: {
        inlineMath: [
            ["$", "$"],
            ["\\(", "\\)"],
        ],
        // Per the [docs](https://docs.mathjax.org/en/latest/options/input/tex.html#option-descriptions), this is enabled as suggested.
        processEscapes: true,
    },
};
