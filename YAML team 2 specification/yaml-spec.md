# YAML Front Matter Preservation
================================

## Status

Draft specification. The recognition and preservation rules below are
normative for implementations of this feature.

## Purpose

Preserve a YAML metadata block at the beginning of a Markdown document when the
CodeChat Editor converts Markdown to HTML and later converts that HTML back to
Markdown.

The YAML is opaque text. The editor must not parse, validate, reorder, or
reformat it.

## Scope

This specification covers only a YAML block at the beginning of a Markdown
document, delimited by lines containing `---`. In HTML, the block is represented
by a `<script type="application/yaml">` element.

This specification does not define YAML syntax, YAML validation, or metadata
editing features.

## Terms

* **Front matter**: the opening `---` line, YAML payload, and closing `---` line
  at the beginning of a Markdown document.
* **Payload**: the text between the opening and closing delimiter lines, not
  including the delimiter lines themselves.
* **YAML element**: an HTML `<script>` element whose `type` attribute exactly
  equals `application/yaml`.

## Markdown to HTML

1. Before parsing Markdown, check whether the document begins with a front
   matter block.
2. A delimiter is a line whose entire content is exactly `---`, excluding its
   line terminator. Surrounding whitespace is not allowed.
3. The opening delimiter must be the first line of the document. A UTF-8
   byte-order mark at the start of the document is not a delimiter; a document
   whose first line is BOM followed by `---` has no front matter.
4. The first subsequent delimiter line closes the block. A line exactly equal
   to `---` inside the payload is therefore reserved for the closing delimiter
   and cannot be represented as payload text.
5. Recognize LF, CRLF, and CR as line terminators. Split the document into
   lines without otherwise interpreting or normalizing it.
6. For a complete block, use the text after the opening delimiter's line
   terminator and before the line terminator immediately preceding the closing
   delimiter as the payload. This means that payload text ending with a
   newline has a blank line before the closing delimiter. The implementation
   must preserve each payload character and line terminator exactly.
7. If both delimiters are present, remove the front matter from the Markdown
   input passed to the Markdown renderer.
8. Add one YAML element at the beginning of the generated HTML, before any
   generated element. Render or omit the YAML element's text content according
   to the reversible representation in [Preservation and error cases].
9. Render the remaining Markdown as usual. If no complete front matter block
   is found, render the whole document as usual and do not add a YAML element.

For example, this Markdown:

```markdown
---
title: Example
author: A Writer
---
# Hello
```

is represented conceptually as:

```html
<script type="application/yaml"
        data-codechat-yaml-encoding="base64"
        data-codechat-yaml-base64="dGl0bGU6IEV4YW1wbGU..."></script>
<h1>Hello</h1>
```

The precise whitespace around the element may follow the existing HTML
serializer, but it must not become part of the YAML payload.

## HTML to Markdown

1. Before converting the HTML document to Markdown, find YAML elements.
2. A matching element is a `script` element whose `type` attribute is exactly
   `application/yaml` (case-sensitive, with no surrounding whitespace).
   Extra attributes are allowed. The element must be the first HTML element in
   the document or fragment; HTML text nodes containing only whitespace before
   it do not count.
3. If exactly one matching element is present and it is first, decode it using
   `data-codechat-yaml-encoding="base64"` and
   `data-codechat-yaml-base64`. Also accept the legacy direct-text
   representation from the example for compatibility: its `textContent`, before
   HTML parser normalization, is the payload.
4. Multiple matching elements are an error. An implementation must not silently
   select or discard one. The editor must report an explicit conversion error.
   YAML elements elsewhere than first are not recognized; they are passed to
   the normal converter and must not be interpreted as document front matter.
5. Exclude the recognized YAML element from the HTML passed to the normal
   HTML-to-Markdown converter. The element must not produce visible Markdown or
   be converted as an ordinary script or code block.
6. Prepend a front matter block to the converted Markdown: an opening `---`
   line, the extracted payload, and a closing `---` line.
7. If no YAML element is found, convert the HTML as usual.

The presence of the YAML element must not change how the rest of the document
is converted.

## Preservation and error cases

* Preserve payload characters and ordering. In particular, do not normalize
  indentation, spaces, comments, blank lines, YAML key ordering, or payload
  line terminators. The recognized payload is an exact character sequence,
  with LF, CRLF, and CR retained. The delimiter lines reconstructed by
  HTML-to-Markdown use LF.
* The HTML representation must be reversible and must not allow `</script>` or
  parser-normalized text to change the payload. Implementations must encode
  the payload's UTF-8 bytes as Base64 in
  `data-codechat-yaml-base64`, and set
  `data-codechat-yaml-encoding="base64"`. The element's text content should be
  empty. A direct text representation may be emitted only as a legacy
  compatibility fallback; it must not be used by a new implementation for
  payload text containing a raw `</script>` sequence.
* Do not reject a payload because it is not valid YAML. YAML validation is out
  of scope.
* An opening delimiter without a closing delimiter is not a complete block;
  leave the document unchanged and render it as ordinary Markdown.
* A closing delimiter is the first later line that exactly matches `---`.
  Delimiter lines containing extra spaces are not recognized.
* A payload cannot contain a line exactly equal to `---`: the first such line
  closes the block. Supporting YAML document separators inside front matter is
  outside this specification.
* A UTF-8 BOM is not accepted as part of front-matter recognition. A BOM
  before ordinary content, including before `---`, is ordinary document text
  and has no special preservation rule.
* HTML `<script>` parsing must not be allowed to split or truncate the payload.
  HTML entity escaping is insufficient inside raw-text script content.
* The editor's HTML processing and save/reopen path must retain the YAML
  element, and the element must not appear as visible document content.

## Acceptance tests

At minimum, tests must cover:

* A document with front matter converts to HTML with exactly one YAML element
  and converts back with the payload unchanged.
* Markdown after the closing delimiter is still rendered and converted
  normally.
* A document without front matter converts as before and does not gain a YAML
  element.
* An empty payload and a payload containing comments, blank lines, indentation,
  colons, non-ASCII text, and an exact `</script>` sequence are preserved.
* Payload and line terminator round trips cover LF, CRLF, CR, an empty payload,
  a payload with no final newline, and a payload with a final newline.
* A BOM, delimiter lines with surrounding whitespace, and an unclosed opening
  delimiter follow the specified recognition rules.
* HTML without a YAML element converts as before.
* HTML with a first YAML element is excluded from ordinary Markdown conversion.
* A second YAML element causes an explicit error and is never silently
  discarded.
* A YAML element that is not the first HTML element is not recognized as front
  matter.
* `type="APPLICATION/YAML"` and `type="application/yaml "` are not matches.
* Opening and saving a document through the real editor HTML serializer/parser
  does not display or remove the metadata.