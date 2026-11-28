Fragments in Markdown documents
-------------------------------

A Markdown document has no code blocks or doc blocks, so the rules which count
code/doc blocks cannot apply to it. This section defines what a `<fragment>`
encloses in a Markdown document, so that a gather element can display it just as
it displays a fragment in a source file. The rest of the cache (ids, dependents,
the reverse edge, outdating) is unchanged; only the *extent* and *content* of a
Markdown fragment are new.

This replaces the earlier rule that fragments are not allowed in Markdown
documents: such a fragment is now valid and has the extent defined below.

### What a fragment encloses

The rules are expressed in terms of the document's **top-level blocks**: the
children of the rendered document's root (a paragraph, heading, list, block
quote, code block, table, and so on). A block nested inside another (a list item
inside a list) belongs to its top-level block.

* **Default.** The fragment encloses the top-level block containing the
  `<fragment>` tag, then every following top-level block up to, but not
  including, the next heading whose level is the same as or higher (a smaller
  number) than that of the closest heading at or before the tag. If no heading
  precedes the tag, the fragment ends at the end of the document. If the tag is
  inside a heading, that heading is the closest one, and it is included in the
  fragment. Skipped heading levels (a `###` directly under a `#`) make no
  difference: only the numeric levels are compared.
* **`following="N"`.** The fragment encloses the block containing the tag plus
  the next `N` top-level blocks, regardless of headings. `N` must be a whole
  number; `following="0"` encloses only the block containing the tag. A value
  larger than the number of remaining blocks is clamped to the end of the
  document. A value which isn't a whole number replaces the fragment's content
  with an error message, as in source files.
* The `<fragment>` tag itself is never part of the content: it is removed, along
  with any backlinks hydrated into it.

### Example

````markdown
## Request

<fragment id="req"></fragment>

The client sends a `POST` to `/upload`.

* The body is JSON.
* The reply is `204`.

## Response

The server replies...
````

`req` encloses the block holding the tag, the paragraph, and the list, and stops
before `## Response`, since that heading has the same level as `## Request`.
Replacing the tag with `<fragment id="req" following="1"></fragment>` encloses
only the tag's block and the paragraph.

### Content stored in the cache

The cache stores each Markdown fragment's content as cleaned HTML, produced by
the same cleaner used for source-file fragments:

* All `id` attributes are removed, so a gather element never duplicates an id.
* `<fragment>` tags are removed with their contents, so backlink hydration is
  excluded from the stored content; this is what keeps a gather element and the
  fragments it lists from outdating each other forever (see the reverse edge).
* Cross-references in the content are hydrated in the fragment's file, then
  re-hydrated by the gather element so that links lead from the gather
  element's file.
* Relative images and URLs in the content aren't rewritten; as stated under
  "Misc", they aren't supported when the gathering file lies in a different
  directory.

Unlike a source-file fragment, the content has no line-number gutter or indents,
since a Markdown document has none.

### Keeping the cache current

Because a Markdown fragment's extent depends on the document's structure, the
cache re-derives it each time the file is processed. A fragment's stored content
changes when any enclosed block changes, when the enclosing section gains or
loses blocks, or when a heading is added, removed, or changes level so that the
section ends elsewhere. As for any fragment, changed content outdates the files
containing gather elements which list it (`update_fragment_content`).

### Errors

A Markdown fragment produces an error message, as a source-file fragment does,
when:

* it has no `id`, or its `id` isn't a valid CSS identifier;
* its `following` value isn't a whole number;
* its enclosed blocks contain a gather element, including one whose `id` is
  missing or invalid (a fragment must not gather);
* its `id` is a duplicate.

A gather element listing a Markdown fragment follows the existing rules: an
error message replaces the content for an id which is missing, duplicated, or
names a target rather than a fragment.

### Edge cases

* Two fragments in the same section both enclose the rest of the section, so
  their contents overlap; this is allowed.
* A fragment tag in the last block of the document encloses only that block.
* A tag inside a code block or inline code is literal text, not a fragment.
* Nested block quotes and lists count as one top-level block.

### Why this design

Alternatives considered:

1. **Everything in the current heading, ignoring the tag's position.** Simple,
   but the author can't enclose less than a whole section, and content above
   the tag within the same heading is always included. Starting at the block
   holding the tag lets the author choose where a fragment begins by placing
   the tag, while still ending at the section's end by default.
2. **Only the text inside the tag** (`<fragment>...</fragment>`). Markdown's HTML
   block rules end an HTML block at the first blank line, so multi-paragraph
   content wouldn't stay inside the tag.
3. **An explicit end tag.** Flexible, but easy to forget, and an unclosed tag
   silently swallows the rest of the document.
4. **Block counting only.** Matches source files, but breaks when blocks are
   added or removed; the section default adapts automatically.

The chosen design keeps the existing `following` attribute, so source files and
Markdown behave alike, and adds a default which follows the document's own
structure.
