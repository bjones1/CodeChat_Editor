# YAML Front Matter Preservation
================================

## Status

Draft specification. Recognition rules and the exact preservation of line
endings and HTML-sensitive text still need review.

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
* **YAML element**: an HTML `<script>` element whose `type` attribute is
  `application/yaml`.

## Markdown to HTML

1. Before parsing Markdown, check whether the document begins with a front
   matter block.
2. For this draft, a delimiter is a line whose entire content is exactly
   `---`, excluding its line terminator. The opening delimiter must be the
   first line of the document. The first subsequent delimiter line closes the
   block.
3. If both delimiters are present, remove the front matter from the Markdown
   input passed to the Markdown renderer.
4. Add one YAML element at the beginning of the generated HTML. Its text
   content must be the payload, without interpreting it as Markdown or YAML.
5. Render the remaining Markdown as usual. If no complete front matter block
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
<script type="application/yaml">
title: Example
author: A Writer
</script>
<h1>Hello</h1>
```

The precise whitespace around the element may follow the existing HTML
serializer, but it must not become part of the YAML payload.

## HTML to Markdown

1. Before converting the HTML document to Markdown, find its YAML element.
2. Extract the element's text content without parsing or formatting it.
3. Exclude the YAML element from the HTML passed to the normal HTML-to-Markdown
   converter. The element must not produce visible Markdown or be converted as
   an ordinary script or code block.
4. If one YAML element is found, prepend a front matter block to the converted
   Markdown: an opening `---` line, the extracted payload, and a closing `---`
   line.
5. If no YAML element is found, convert the HTML as usual.

The presence of the YAML element must not change how the rest of the document
is converted.

## Preservation and error cases

* Preserve payload characters and ordering. In particular, do not normalize
  indentation, spaces, comments, blank lines, or YAML key ordering.
* Do not reject a payload because it is not valid YAML. YAML validation is out
  of scope.
* An opening delimiter without a closing delimiter is not a complete block;
  under this draft, leave the document unchanged and render it as ordinary
  Markdown.
* A closing delimiter is the first later line that exactly matches `---`.
  Delimiter lines containing extra spaces are not recognized under this draft.
* The expected document has at most one YAML element. Handling multiple
  matching elements must be decided before implementation; do not silently
  discard additional payloads.
* HTML `<script>` content has special parsing rules. In particular, payload
  text containing a sequence such as `</script>` can terminate the element
  early. The implementation must use a reversible representation or define a
  limitation that prevents data loss. HTML entity escaping alone may not
  preserve text inside a script element.
* HTML parsers can normalize line endings. The required fidelity for CRLF versus
  LF must be decided: preserve the original line-ending representation, or
  preserve the payload text with line endings normalized.
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
  colons, and non-ASCII text are preserved.
* An unclosed opening delimiter follows the specified incomplete-block rule.
* HTML without a YAML element converts as before.
* The YAML element is not included in the Markdown produced by the ordinary
  HTML-to-Markdown conversion.
* HTML-sensitive payload text and line endings follow the final preservation
  decisions above.
* Opening and saving a document through the editor does not display or remove
  the metadata.

## Open questions for review

1. Should delimiters allow surrounding whitespace, or must a delimiter line be
   exactly `---`?
2. Should a UTF-8 byte-order mark before the first delimiter be accepted?
3. Must payload preservation include the exact original line-ending
   representation?
4. What reversible representation should be used for payload text that could
   close the HTML script element?
5. Should multiple YAML elements be an error, or should a deterministic one be
   selected?