YAML Front Matter Preservation
==============================

\================================

Status
------

Draft specification. The rules below are normative for implementations.

Purpose
-------

Preserve YAML metadata at the beginning of a Markdown document across Markdown
to HTML and HTML to Markdown conversion.

YAML is opaque text: the editor must not parse, validate, reorder, or reformat
it. Opaqueness does not exempt it from this specification's character, UTF-8,
and HTML representation rules.

Scope
-----

This specification covers only a YAML block at the start of Markdown, delimited
by lines containing `---`. HTML represents it with `<script
type="application/yaml">`.

It does not define YAML syntax, YAML validation, or metadata editing features.

Terms
-----

* **Front matter**: the opening `---` line, YAML payload, and closing `---` line
  at the beginning of a Markdown document.
* **Payload**: text between the opening and closing delimiters, excluding both
  delimiters and their surrounding line terminators. Payloads distinguish no
  final newline from a final LF, CRLF, or CR.
* **YAML element**: an HTML `<script>` element whose `type` attribute exactly
  equals `application/yaml`.

Markdown to HTML
----------------

1. Before parsing Markdown, check whether the document begins with front matter.

2. A delimiter is a line whose entire content is exactly `---`, excluding its
   line terminator. Surrounding whitespace is not allowed.

3. The opening delimiter must be the first line. A UTF-8 byte-order mark at the
   start is not part of a delimiter; BOM followed by `---` has no front matter.

4. The first line record after the opening delimiter whose content is exactly
   `---` closes the block. A line exactly equal to `---` inside the payload is
   reserved for the closing delimiter and cannot be represented as payload text.

5. Recognize LF, CRLF, and CR. Split without interpreting or normalizing their
   character sequence, and retain each line record's terminator.

6. Remove exactly the opening and closing delimiter line records. The payload is
   the exact text and terminators in the intervening records. The opening
   delimiter's terminator is not payload text.

   One additional terminator may separate a nonempty unterminated payload from
   the closing delimiter. It is reconstructed by the converter and is not in the
   payload.

   Extraction starts after the opening terminator and ends before the closing
   delimiter's terminator. Remove exactly that closing separator and retain
   prior intervening terminators. Thus `a\n---` yields `a`, `a\n\n---` yields
   `a\n`, `\n---` yields an empty payload, and `\n\n---` yields `\n`. The first
   terminator after the opening delimiter is opening-only. When a payload ends
   in a line terminator, a blank line precedes the closing delimiter: one
   terminator belongs to the payload and the other separates it. An empty
   payload has neither. If the closing delimiter is not on a separate line, the
   block is not recognized.

7. If both delimiters are present, remove the front matter from the Markdown
   input passed to the Markdown renderer.

8. Add one YAML element at the beginning of the generated HTML, before every
   generated element. Render or omit the element's text content according to the
   reversible representation in \[Preservation and error cases\].

9. Render the remaining Markdown normally. If no complete block is found, render
   the whole document normally and add no YAML element.

For example, this Markdown:

```markdown
---
title: Example
author: A Writer
---

Hello
=====
```

is represented conceptually as:

```html
<script type="application/yaml"
        data-codechat-yaml-encoding="base64"
        data-codechat-yaml-base64="dGl0bGU6IEV4YW1wbGU="></script>
<h1>Hello</h1>
```

Whitespace around the element may follow the existing HTML serializer, but must
not become part of the YAML payload.

HTML to Markdown
----------------

1. Before converting HTML to Markdown, search the document or fragment in
   pre-order for matching YAML elements.
2. A matching element is an HTML-namespace `script` element whose `type`
   attribute is exactly `application/yaml`, case-sensitive, with no surrounding
   whitespace. Extra attributes are allowed. Exclude elements inside
   `<template>` and foreign namespaces. The first-element position is the first
   element node in scope; whitespace-only text nodes, comments, and a doctype do
   not prevent it. A preceding non-whitespace text node does. Exclude
   ancestor-subtree nodes outside the root. Documents traverse the root and
   ordinary element/fragment children; fragments traverse the root and ordinary
   element/fragment descendants. Exclude shadow roots unless included by the
   supplied root.
3. If exactly one matching element is present and is first, decode it using the
   representation rules below. If exactly one matching element is present but is
   not first, pass it to the normal converter and do not prepend front matter.
4. Count every match in the bounded traversal, including nested elements outside
   the first subtree. More than one is an error at any position. Report an
   explicit conversion error instead of selecting or discarding one. Template
   content is excluded from counting and recognition.
5. Recognize and exclude elements on the parsed DOM before invoking the normal
   converter. Exclude exactly the recognized YAML element node from the HTML
   passed to the converter. Do not remove adjacent text nodes or comments.
   Container-only whitespace may retain the normal converter's behavior. The
   excluded element must not produce visible Markdown or become an ordinary
   script or code block.
6. Prepend a front matter block to the converted Markdown using canonical LF
   delimiters and the exact payload character sequence: `---\n` + payload + the
   LF separator rule below + `---\n`.
7. If no YAML element is found, convert the HTML as usual.

The YAML element must not change conversion of the rest of the document.
Exclusion occurs after DOM parsing and before the normal converter; retained
text nodes and comments keep the converter's existing behavior.

Preservation and error cases
----------------------------

* Preserve payload characters and ordering, including indentation, spaces,
  comments, blank lines, YAML key order, and LF, CRLF, or CR terminators.
  Reconstructed delimiter lines use LF.
* Reconstruct HTML-to-Markdown as `---\n` + payload + an LF separator for a
  nonempty payload not ending in LF, CRLF, or CR + `---\n`. An empty payload is
  `---\n---\n`. Add no separator after a payload terminator; it separates
  payload content from the closing delimiter. The closing delimiter's trailing
  LF is outside the payload.
* For a payload ending in a line terminator, reconstruction requires that
  terminator followed by one additional separator terminator. A payload ending
  LF is followed by `LF + LF`, CRLF by `CRLF + CRLF`, and CR by `CR + CR` before
  the closing delimiter.
* Base64 recognition requires `data-codechat-yaml-encoding="base64"` and exactly
  one `data-codechat-yaml-base64` attribute. Use standard RFC 4648 Base64: only
  `[A-Za-z0-9+/]` and trailing `=` are allowed; `=` is forbidden elsewhere,
  whitespace and URL-safe characters are forbidden, and padding is minimal.
  Re-encode decoded bytes and require exact attribute equality. An empty
  attribute represents an empty payload. Decode bytes as strict UTF-8. Missing
  or duplicate attributes, malformed or noncanonical Base64, or invalid UTF-8 is
  an explicit conversion error; never discard the element or fall back to legacy
  text.
* HTML parsing must preserve attribute multiplicity or validate duplicates
  before DOM normalization. A deduplicated DOM that cannot retain parser
  diagnostics or reject duplicates must reject input instead of attempting
  conversion.
* Accept legacy direct text only when neither representation key is present. The
  payload is the element's DOM `textContent` after HTML parsing; do not perform
  another character-reference or parser-normalization pass. Legacy compatibility
  is limited to historical content, and raw `</script>` is forbidden.
  `data-codechat-yaml-encoding="base64"` requires exactly one valid Base64
  attribute; any `data-codechat-yaml-base64` attribute requires the exact
  encoding marker and exactly one instance of itself. No legacy fallback is
  permitted when either key is present.
* The HTML representation must be reversible and prevent `</script>` or parser
  normalization from changing the payload. Base64-encode the payload's UTF-8
  bytes in `data-codechat-yaml-base64` and set
  `data-codechat-yaml-encoding="base64"`. Text content and child nodes must be
  empty. New representations always serialize as `<script
  type="application/yaml" data-codechat-yaml-encoding="base64"
  data-codechat-yaml-base64="..."></script>` with exact case-sensitive
  attributes and HTML namespace. Surrounding whitespace is allowed only outside
  the element. New conversions must emit Base64; direct text may be read for
  compatibility but must not be emitted unless a named compatibility mode is
  explicitly enabled for data known to use the legacy format.
* Do not reject a payload because it is invalid YAML. YAML validation is out of
  scope.
* An opening delimiter without a closing delimiter is incomplete; leave the
  document unchanged and render it as ordinary Markdown.
* A closing delimiter is the first line record after the opening delimiter whose
  content is exactly `---`. Delimiter lines with extra spaces are not
  recognized.
* A payload cannot contain a line exactly equal to `---`: the first such line
  closes the block. Supporting YAML document separators within front matter is
  outside this specification.
* A UTF-8 BOM is not accepted as part of front-matter recognition. A BOM before
  ordinary content, including before `---`, is ordinary document text and has no
  special preservation rule.
* `<script>` parsing must not split or truncate the payload. HTML entity
  escaping is insufficient in raw-text script content.
* The YAML element must survive DOM serialization, sanitization, save/reopen,
  undo/redo, and copy/paste without losing its type or representation
  attributes. In-editor operations must preserve the element and both Base64
  attributes. External sanitization or import that removes it or them must
  produce an explicit conversion or import error, not normal text. A separate
  editor model may retain it only if it reinserts the element before every HTML
  conversion and preserves it across undo/redo and copy/paste DOM state. The
  element must not be visible.
* Exclude every element in `<template>` content fragments. Shadow roots are
  outside traversal unless the supplied root expressly includes them.

Acceptance tests
----------------

At minimum, tests must cover:

* A document with front matter converts to HTML with one YAML element and back
  with the payload unchanged; following Markdown is converted normally.
* A document without front matter converts as before without gaining an element.
* Empty and populated payloads preserve comments, blank lines, indentation,
  colons, non-ASCII text, and an exact `</script>` sequence.
* Round trips cover LF, CRLF, CR, empty payloads, and payloads with and without
  final newlines, including mixed terminators within one payload.
* Test a BOM, whitespace around delimiter lines, and an unclosed opening
  delimiter.
* HTML without a YAML element converts as before; a first YAML element is
  excluded from ordinary Markdown conversion.
* A second YAML element causes an explicit error and is never silently
  discarded.
* A YAML element not first in the HTML is not recognized as front matter.
* `type="APPLICATION/YAML"` and `type="application/yaml "` are not matches.
* Missing or duplicate Base64 attributes, conflicting representation attributes,
  malformed or noncanonical Base64, and Base64 decoding to invalid UTF-8 report
  explicit conversion errors.
* Nested YAML elements and elements inside `<template>` or foreign namespaces
  follow the bounded discovery rules and do not cause accidental recognition.
* The real editor serializer/parser does not display or remove metadata when
  opening and saving.

Change notes
------------

* Approximate length reduction: 2,156 to 1,833 words (about 15.0%).
* Merged repeated delimiter and closing-line explanations in Markdown-to-HTML
  and related preservation rules.
* Tightened repeated HTML traversal, exclusion, and Base64 conversion rules
  while retaining the same requirements and acceptance criteria.
* No authored pull-request comments, review comments, open issues, TODOs, or
  risks were identified, altered, or removed.
* No contradictions or ambiguities were resolved. Uncertain cases were retained.
* All comments/issues preserved: 0 of 0.
