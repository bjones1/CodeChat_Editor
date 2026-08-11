# Used with client-side tests.
code()
# Gathered fragment:
#
# <h3 id="cc-test-gather" data-gather="cc-test-fragment">Gathered</h3>
def f():
    # <fragment id="cc-test-fragment"></fragment>An indented doc block, whose
    # indent must line up with the indent of the code below it.
    indented_code()
    more_indented_code()
# Graphviz:
#
# ```graphviz
# digraph {
#   A -> B
# }
# ```
#
# Mermaid:
#
# ```mermaid
# graph TD
#   A --> B
# ```
#
# MathJax:
#
# $x^2$
#
# $$x^3$$