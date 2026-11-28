YAML Front Matter Preservation
==============================

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

1. Enable the YAML-style metadata option in the CodeChat pulldown-cmark
   rendering path. Use source text for the payload so parser normalization
   cannot change it. The reverse conversion belongs in htmd.

2. A delimiter is a line whose entire content is exactly `---`, excluding its
   line terminator. Surrounding whitespace is not allowed.

3. The opening delimiter must be the first line. A UTF-8 byte-order mark at the
   start is not part of a delimiter; BOM followed by `---` has no front matter.

4. The first line record after the opening delimiter whose content is exactly
   `---` closes the block. A line exactly equal to `---` inside the payload is
   reserved for the closing delimiter and cannot be represented as payload text.

5. Recognize LF, CRLF, and CR. Split without interpreting or normalizing their
   character sequence, and retain each line record's terminator.

6. Take the substring from immediately after the opening line terminator up
   to the first character of the closing delimiter. If nonempty, remove
   exactly its last line terminator (treat CRLF as one). The rest is the
   payload. A closing delimiter at EOF is valid. Its own terminator, when
   present, belongs to the closing line and precedes the remaining Markdown.

7. If both delimiters are present, remove the front matter from the Markdown
   input passed to the Markdown renderer.

8. Add one YAML element at the beginning of the generated HTML, before every
   generated element. Render or omit the element's text content according to the
   reversible representation in [Preservation and error cases](#preservation-and-error-cases).

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
<script type="application/yaml">title: Example
author: A Writer</script>
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
   whitespace. Extra attributes are allowed. Visit the supplied root itself,
   then ordinary children in document order, depth first. For an HTML string,
   use its parsed content fragment as the root. Skip template contents,
   foreign-namespace subtrees, and attached shadow trees. An explicitly supplied
   shadow root is a fragment. Only comments, a doctype, and text consisting
   solely of TAB, LF, FF, CR, or SPACE may precede a first-position match.
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
   separator rule below + `---\n`.
7. If no YAML element is found, convert the HTML as usual.

The YAML element must not change conversion of the rest of the document.
Exclusion occurs after DOM parsing and before the normal converter; retained
text nodes and comments keep the converter's existing behavior.

Preservation and error cases
----------------------------

* Preserve payload characters and ordering, including indentation, spaces,
  comments, blank lines, YAML key order, and LF, CRLF, or CR terminators.
  Reconstructed delimiter lines use LF.
* For an empty payload, emit `---\n---\n`. Otherwise emit `---\n`, the
  payload, one separator, and `---\n`. Choose CR as the separator if the
  payload ends in CR, and LF otherwise. This prevents a final payload CR
  and its separator from being read as a single CRLF record.
* Base64 recognition requires `data-codechat-yaml-encoding="base64"` and exactly
  one `data-codechat-yaml-base64` attribute. Use standard RFC 4648 Base64: only
  `[A-Za-z0-9+/]` and trailing `=` are allowed; `=` is forbidden elsewhere,
  whitespace and URL-safe characters are forbidden, and padding is minimal.
  Re-encode decoded bytes and require exact attribute equality. An empty
  attribute represents an empty payload. Decode bytes as strict UTF-8. Missing
  or duplicate attributes, malformed or noncanonical Base64, or invalid UTF-8 is
  an explicit conversion error; never discard the element or fall back to legacy
  text.
* For HTML source, check the recognized element's start tag for duplicate
  representation attributes before the parser discards duplicates. Compare
  HTML attribute names ASCII-case-insensitively. For an already constructed
  DOM, validate the attributes actually present; prior source spelling and
  duplicate occurrences cannot be recovered.
* Accept direct text only when neither representation key is present. The
  payload is the element's DOM `textContent` after HTML parsing; do not perform
  another character-reference or parser-normalization pass.
  `data-codechat-yaml-encoding="base64"` requires exactly one valid Base64
  attribute; any `data-codechat-yaml-base64` attribute requires the exact
  encoding marker and exactly one instance of itself. No legacy fallback is
  permitted when either key is present.
* Emit the payload as script text when it contains none of U+003C (less-than),
  U+000D (CR), or U+0000 (NUL). Write literal text, including ampersands.
  For other payloads, keep the draft's reversible representation: encode UTF-8
  bytes in `data-codechat-yaml-base64`, set `data-codechat-yaml-encoding="base64"`,
  and leave the element empty. Readers reject child nodes on an encoded element.
  Both representations use the same script type.
* Do not reject a payload because it is invalid YAML. YAML validation is out of
  scope.
* An opening delimiter without a closing delimiter is incomplete; leave the
  document unchanged and render it as ordinary Markdown.
* A closing delimiter is the first line record after the opening delimiter whose
  content is exactly `---`. Delimiter lines with extra spaces are not
  recognized.
* A payload cannot contain a line exactly equal to `---`: the first such line
  closes the block. Supporting YAML document separators within front matter is
  outside this specification. If a decoded HTML payload contains such a line,
  report a conversion error before emitting ambiguous Markdown.
* A UTF-8 BOM is not accepted as part of front-matter recognition. A BOM before
  ordinary content, including before `---`, is ordinary document text and has no
  special preservation rule.
* `<script>` parsing must not split or truncate the payload. HTML entity
  escaping is insufficient in raw-text script content.
* The YAML element must survive DOM serialization, sanitization, save/reopen,
  undo/redo, and copy/paste without losing its type or representation
  attributes. In-editor operations must preserve the element and its text or both Base64
  attributes. An editor operation with a known original YAML element that loses it or its payload must
  produce an explicit conversion or import error, not normal text. A separate
  editor model may retain it only if it reinserts the element before every HTML
  conversion and preserves it across undo/redo and copy/paste DOM state. The
  element must not be visible.
* Standalone HTML imported without a YAML element follows ordinary conversion.
  Absence alone cannot establish that an external tool deleted metadata.

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

Exact examples use escape notation for line endings. Each row has an empty
Markdown body. The last column is the canonical Markdown returned by htmd.

| Markdown input | Script payload after decoding | Returned Markdown |
| --- | --- | --- |
| `---\n---` | empty | `---\n---\n` |
| `---\na\n---` | `a` | `---\na\n---\n` |
| `---\na\n\n---` | `a\n` | `---\na\n\n---\n` |
| `---\ra\r\r---` | `a\r` | `---\na\r\r---\n` |
| `---\r\na\r\n\r\n---` | `a\r\n` | `---\na\r\n\n---\n` |
| `---\na\r\nb\rc\n---` | `a\r\nb\rc` | `---\na\r\nb\rc\n---\n` |

* A direct script containing `name: A &amp; B` returns those same characters,
  including the literal five-character sequence `&amp;`.
* For payload `</script>`, expect an empty script with the encoding marker
  and Base64 value `PC9zY3JpcHQ+`. Converting it back returns the original text.
* A fragment beginning with a comment and SPACE before the script recognizes
  it. Replacing SPACE with U+00A0 sends the fragment to ordinary conversion.
* A decoded payload consisting of the single line `---` reports a conversion
  error. Two matching sibling scripts also report an error, before decoding.
