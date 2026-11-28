Project 2 submission
====================

Team members are Altynay Kishkenebayeva, Aiym Saparkhan,
<span id="author-1794145613557">Daulet Bakytkhanov</span>,
<span id="author-1794145678874">Ali Zhumatayev</span>. The specification
consists of the file [yaml-spec.md](yaml-spec.md).

Responsibilities:
-----------------

* Altynay - responsible for draft and overall project management.
* Aiym - responsible for final specification review rounds 1–5.
* Ali - responsible for final specification review rounds 6–10, using Aiym’s revised specification.
* Daulet - responsible for finalizing implementation part.

Draft submission
----------------

### Compiler test

The following statements summarize LLM output on each team member's computer,
based on the prompt "Verify that you can compile and run the project in the
current git repo."

### Altynay

#### Contributions

* Created first draft version.
* Established strict parsing \& serialization boundaries.
* Completed editor lifecycle \& terminology specifications.

#### LLM Review (Copilot / GPT-Luna-6 balanced)

```
prompt: Audit the provided specification for logical flaws, ambiguities, edge cases, and missing requirements. Output your findings in a file named llm-review.md.
```

The specification clearly identifies the main transformation, uses a focused
scope, and provides a useful draft for recognizing and preserving front matter.
However, it is not yet implementation-ready: several preservation decisions
affect both Markdown parsing and HTML round-tripping, and the draft leaves them
to later implementation.

* The Markdown rules should define the exact byte or character model for line
  boundaries. In particular, state whether CRLF is retained or normalized, how a
  payload with no final newline is reconstructed, and whether the two delimiter
  lines have to have matching line terminators. The HTML-to-Markdown step also
  needs to account for the newline between a payload and the closing delimiter
  so that a round trip does not add or remove characters.
* The handling of `</script>` in a payload is underspecified even though the
  spec correctly identifies the risk. Entity escaping is not sufficient inside a
  script element. The HTML representation must be named and reversible, or the
  specification must explicitly exclude that input; an implementation should not
  silently split, truncate, or alter the payload.
* The behavior for multiple matching YAML elements is not decided. It should say
  whether this is an error or, as a deterministic alternative, which element is
  used and whether the others are preserved or discarded.
* The exact HTML identification rules are incomplete. Define whether `type`
  matching is case-sensitive and whitespace-sensitive, and whether extra
  attributes are allowed. It should also define whether the element must be the
  first HTML element and whether `<script>` in the body or `<head>` is accepted.
* The recognition rules need edge-case decisions for an optional UTF-8 BOM,
  surrounding whitespace on delimiter lines, an empty file, and a payload whose
  final line is not terminated. The first two open questions cannot remain
  implementation-dependent.
* The first closing delimiter is ambiguous for YAML that itself contains a line
  exactly equal to `---`. The spec should recognize this as an inherent
  front-matter limitation and decide whether such a payload is supported or
  rejected with an explicit result.

The current test list is appropriate, but it should also verify exact byte-level
or character-level round trips, multiple YAML elements, script text containing
`</script>`, a final newline in the payload, BOM input, delimiter lines with
surrounding whitespace, and editor save/reopen behavior through the real HTML
serializer and parser rather than only through string operations. These
additions would make the acceptance criteria testable and would avoid
differences between implementations.

#### Spec review rounds

| Round number | Number of findings            | Summary                                                                                                                                                                                                                                                                                                                                       |
| ------------ | ----------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1            | 6                             | Defined exact line-ending and payload boundaries, reversible `</script>` handling, multiple-element behavior, HTML element matching rules, BOM and delimiter edge cases, and the closing-delimiter limitation.                                                                                                                                |
| 2            | 3 severe / 6 medium / 1 small | Closed the remaining severe ambiguity in YAML element counting and strict Base64/UTF-8 decoding, added legacy text rules, bounded element discovery to document or fragment traversal, specified whitespace/comment/doctype handling, removed exactly the recognized element, and completed editor retention and reconstruction requirements. |
| 3            | 3 severe / 9 medium / 2 small | Clarified payload extraction and DOM traversal boundaries, strict duplicate/Base64/UTF-8 handling, exact HTML serialization and legacy emission mode, template/shadow exclusion, editor retention recovery, mixed line terminators, and the remaining terminology and opacity edge cases.                                                     |

#### Last LLM review

**High severity**

1. <strong>HTML whitespace is undefined.</strong> “Whitespace-only” could mean
   HTML ASCII whitespace, JavaScript whitespace, or Unicode whitespace. Define
   the exact character set, since it determines whether an element is first.

2. <strong>Duplicate attribute handling is not
   implementation-independent.</strong> The HTML parser can retain or silently
   merge duplicate attributes, and HTML attribute names are case-insensitive
   during normalization. Define duplicate detection before normalization,
   including ASCII-case rules for `data-codechat-yaml-*`.

3. <strong>Legacy text can lose CR and CRLF terminators.</strong> HTML parsing
   and serialization may normalize CR or CRLF to LF, making exact legacy
   preservation impossible. Restrict legacy compatibility to determinable line
   endings or require Base64 for CR/CRLF payloads.

4. <strong>Lone UTF-16 surrogates are unspecified.</strong> The rules require
   strict UTF-8 but do not say whether Markdown or DOM surrogate code units are
   rejected, preserved, or replaced. Replace neither; define explicit rejection
   and apply it to Markdown, Base64, and legacy text.

5. <strong>The legacy raw-text prohibition is incomplete.</strong> Script end
   tags are recognized case-insensitively and may include whitespace or `/`.
   Define all forbidden parser end-tag forms and validate legacy text after
   parsing, rather than matching only lowercase `</script>`.

**Medium severity**

6. <strong>Supplied-root traversal is ambiguous.</strong> It is unclear whether
   an element supplied as the traversal root is itself visited before its
   descendants. This changes recognition when that root is a YAML script. Define
   root inclusion and pre-order for documents, elements, fragments, templates,
   and shadow roots.

7. <strong>Pre-order boundaries need a formal definition.</strong> Foreign
   namespaces, template content, shadow roots, and nested elements have
   interacting rules, but the relative position of a supplied root and its
   descendants is not explicit. Define first-element position unambiguously for
   every root type.

8. <strong>Non-first-element behavior is contradictory.</strong> The text says
   such an element is passed to the normal converter and that recognized YAML
   elements are excluded. Clarify that exactly one non-first match is neither
   decoded nor excluded, and is converted normally.

9. <strong>EOF delimiter semantics are incomplete.</strong> It is unclear
   whether `---` at EOF can close a block or only an opening delimiter. Define
   EOF as an empty terminator and test closing delimiters without a trailing
   newline.

10. <strong>Source-file UTF-8 and NUL handling are unspecified.</strong> Invalid
    or replacement-decoded source bytes, embedded NULs, and overlong sequences
    have no rule. Define the accepted text model and explicit error behavior
    while keeping the Base64 path strict.

11. <strong>Editor survival lacks measurable copy/paste and sanitization
    tests.</strong> Opening and saving are insufficient. Test DOM
    clones/fragments, copy/paste, undo/redo, external sanitizers, and imports
    that remove or strip the YAML element or attributes.

**Missing acceptance coverage**

12. <strong>Legacy and parser edge cases need tests.</strong> Add coverage for
    character references and non-ASCII legacy text, all forbidden end-tag forms,
    noncanonical line endings, a second match outside the first subtree,
    matching supplied element roots, CR/CRLF legacy payloads, and duplicate or
    case-variant Base64 attributes.<br>

<!-- ================================ -->

---

Final submission
----------------

### Aiym

GitHub userid: aiymsaparkhan

#### Contributions

I completed the first five specification review rounds with Codex, clarified how YAML front matter is preserved during Markdown-to-HTML and HTML-to-Markdown conversion, corrected the example so it retains both metadata fields, and verified six line-ending examples.

#### Spec review rounds

| Round | Findings | Main changes made |
| --- | ---: | --- |
| 1 | 3 | Reconciled the two extraction rules, defined one reconstruction separator for each payload ending, and allowed a closing delimiter at end of file. |
| 2 | 4 | Clarified the Markdown-to-HTML and HTML-to-Markdown paths in CodeChat Editor, restored direct script text for ordinary input, fixed the example that lost the author field, and corrected the remaining LF-only cross-reference. |
| 3 | 5 | Defined traversal and leading whitespace, distinguished source and DOM duplicate handling, scoped metadata-loss detection to observable editor operations, and specified an error for decoded delimiter lines. |
| 4 | 3 | Added exact newline examples, direct-text and encoded HTML fixtures, and concrete discovery and rejection cases. |
| 5 | 2 | Fixed the broken representation link and removed draft-editing residue from the normative document. |

#### Last LLM spec review

Model: OpenAI Codex (GPT-6). Review instruction for round 5:
“Check the revised YAML specification for broken references and stale editorial
material before handing it to the next reviewer.”

1. Markdown-to-HTML rule 8 uses escaped square brackets, so the reference to
   the representation rules displays as text. Link it to the section heading.
2. The opening separator and the closing change notes are editing residue.
   The notes describe an earlier compression pass and say that no contradictions
   were resolved, which no longer describes this revision. Remove them from
   the normative document.

Both findings were addressed. The Base64 fallback for text that cannot safely
be emitted verbatim is a team design choice inherited in part from the draft.
The course example establishes the direct-text form; it does not prescribe
this fallback. Editor and converter implementation tests belong to the
implementation review.

<p><br></p>

### Ali

#### Contributions

Lorem ipsum

#### Spec review rounds

Lorem ipsum

#### Last LLM spec review

Lorem ipsum

<br><!-- ================================ -->

### Daulet

#### Contributions

Lorem ipsum

#### Implementation review rounds

Lorem ipsum

#### Last LLM implementation review

Lorem ipsum<!-- ================================ -->

<p><br></p>

### Altynay

#### Last LLM spec review

Lorem ipsum

#### Last LLM implementation review

Lorem ipsum

<p><br></p>

<p><br></p>
