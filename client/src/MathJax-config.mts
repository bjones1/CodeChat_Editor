// Configure MathJax.
window.MathJax = {
    chtml: {
        fontURL: "/static/mathjax/components/output/chtml/fonts/woff-v2",
    },
    tex: {
        inlineMath: [
            ["$", "$"],
            ["(", ")"],
        ],
    },
    svg: {
        fontCache: "global",
    },
};
