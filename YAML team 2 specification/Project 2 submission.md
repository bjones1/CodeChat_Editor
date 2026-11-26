Project 2 submission
====================

Team members are Altynay Kishkenebayeva, Aiym Saparkhan,
<span id="author-1794145613557">Daulet Bakytkhanov</span>,
<span id="author-1794145678874">Ali Zhumatayev</span>. The specification
consists of the file [yaml-spec.md](yaml-spec.md).

Draft submission
----------------

### Compiler test

The following statements summarize LLM output on each team member's computer,
based on the prompt "Verify that you can compile and run the project in the
current git repo."

### Altynay

#### Contributions
- Created first draft version.

#### LLM Review (Copilot / GPT-Luna-6 balanced)

```
prompt: Please review @attachment:yaml-spec.md and give your feedback
```

The specification clearly identifies the main transformation, uses a focused
scope, and provides a useful draft for recognizing and preserving front matter.
However, it is not yet implementation-ready: several preservation decisions
affect both Markdown parsing and HTML round-tripping, and the draft leaves them
to later implementation.

* The Markdown rules should define the exact byte or character model for line
  boundaries. In particular, state whether CRLF is retained or normalized, how
  a payload with no final newline is reconstructed, and whether the two
  delimiter lines have to have matching line terminators. The HTML-to-Markdown
  step also needs to account for the newline between a payload and the closing
  delimiter so that a round trip does not add or remove characters.
* The handling of `</script>` in a payload is underspecified even though the
  spec correctly identifies the risk. Entity escaping is not sufficient inside
  a script element. The HTML representation must be named and reversible, or
  the specification must explicitly exclude that input; an implementation
  should not silently split, truncate, or alter the payload.
* The behavior for multiple matching YAML elements is not decided. It should
  say whether this is an error or, as a deterministic alternative, which
  element is used and whether the others are preserved or discarded.
* The exact HTML identification rules are incomplete. Define whether `type`
  matching is case-sensitive and whitespace-sensitive, and whether extra
  attributes are allowed. It should also define whether the element must be
  the first HTML element and whether `<script>` in the body or `<head>` is
  accepted.
* The recognition rules need edge-case decisions for an optional UTF-8 BOM,
  surrounding whitespace on delimiter lines, an empty file, and a payload whose
  final line is not terminated. The first two open questions cannot remain
  implementation-dependent.
* The first closing delimiter is ambiguous for YAML that itself contains a line
  exactly equal to `---`. The spec should recognize this as an inherent
  front-matter limitation and decide whether such a payload is supported or
  rejected with an explicit result.

The current test list is appropriate, but it should also verify exact
byte-level or character-level round trips, multiple YAML elements, script text
containing `</script>`, a final newline in the payload, BOM input, delimiter
lines with surrounding whitespace, and editor save/reopen behavior through the
real HTML serializer and parser rather than only through string operations.
These additions would make the acceptance criteria testable and would avoid
differences between implementations.

#### Spec review rounds

| Round number | Number of findings | Summary |
| --- | ---: | --- |
| 1 | 6 | Defined exact line-ending and payload boundaries, reversible `</script>` handling, multiple-element behavior, HTML element matching rules, BOM and delimiter edge cases, and the closing-delimiter limitation. |

#### Last LLM review

Lorem ipsum

<!-- ================================ -->

### Ali


<!-- ================================ -->

### Daulet


<!-- ================================ -->

### Aiym





---

Final submission
----------------

### Altynay

#### Contributions

Lorem ipsum

#### Spec review rounds

Lorem ipsum

#### Implementation review rounds

Lorem ipsum

#### Last LLM spec review

Lorem ipsum

#### Last LLM implementation review

Lorem ipsum

<!-- ================================ -->

### Ali


<!-- ================================ -->

### Daulet


<!-- ================================ -->

### Aiym