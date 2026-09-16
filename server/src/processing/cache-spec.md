Cache spec
==========

The cache stores the location (file name and ID) and contents (inner HTML or
code/doc blocks for fragments) of a target. Targets are HTML elements (excluding
`<fragment>`s) with an ID.

The goal of the cache is to support cross-references and gather elements, and to
ensure that all IDs are unique within a project. This means that
cross-references and gather elements persist across moving or renaming files,
since the IDs will be found in the cache.

Non-project files support a subset of this functionality: the "project" consists
only of the current file. Only targets, gather elements, cross-references, and
fragments to items within the file work as expected; references to other files
do not.

The cache reflects data read directly from disk/IDE; content edited in the
Client does not update the cache until it's written to disk/IDE, at which point
the cache much re-process this unknown file. Since the Client is designed around
an autosave principle which updates disk/IDE regularly, there's little gap
between the two.

The cache hydrates data to the Client; the dehydration routes are responsible
for removing all cache hydration artifacts.

Cross references
----------------

A `<xref ref="id"></xref>` is a cross reference to a `Target` or a gather
element. The `id` specifies the destination; the cache then hydrates the
contents based on the location and contents of the target of the provided `id`
to e.g. `<xref ref="id" contenteditable="false"><a
href="../path/to/page#id">Inner HTML from target</a></xref>`; this hydrated form
is only present in the Client.

Details:

* This element does not allow an `id` attribute.

* The inner HTML is always taken before cache hydration, to prevent circular
  dependencies:

  ```html
  <h1 id="a">See <xref ref="b"></xref></h1>
  <!-- file a -->
  <h1 id="b">See <xref ref="a"></xref></h1>
  <!-- file b -->
  ```

  Including cache hydration would cause the inner HTML to be updated each time
  the file is processed, outdating the other file.

* If the `id` referred to isn't found or refers to a duplicate id, the inner
  text is instead an appropriate error message.

* If the cross reference is to a gather element, the text is the gather
  element's inner HTML, not the gathered code/doc blocks.

Fragments and gather elements
-----------------------------

A gather element such as `<h3 id="bar" data-gather="id1 id2...">Bazzy
things</h3>` is a `Target` with the `data-gather` attribute. It becomes a list
of the contents of fragments it refers to after hydration by the cache. An
example fragment tag, after cache hydration: `<fragment id="id1"
contenteditable="false">See <a href="path/to/gather#bar">Bazzy things</a>, <a
href="path/to/another/gather#zap">Zappy things</a></fragment>`. A fragment's
content by default includes the contents of the current doc block and the
contents of the following code/doc block; fragments are not allowed in Markdown
documents (in the case, the fragment contents consist of an error message).
Fragments may include the `following` attribute to enclose a specific number of
the following code/doc blocks; for example, `<fragment id="bar"
following="3"></fragment>` includes the current doc block along with the next 3
code/doc blocks; `following` must be a whole number.

Details:

* Fragment contents may not include a gather element; in this case, the gather
  element list of contents will simply include an error message.
* Fragments do support indirection: gather element A includes contents from
  fragment B, which contains a cross reference to target C. Changes to target C
  makes B and A outdated.
* If a gather element refers to an `id` that is a `Target` or a `GatherElement`,
  not a `Fragment`, the resulting output for this in the list of fragments is an
  error message.
* If a gather element refers to an `id` that wasn't found or is a duplicate, its
  contents will be replaced by an error message.
* Fragments store an HTML rendering of the code and doc blocks they contain,
  excluding the content produced by hydrating the `<fragment>` tags, to avoid
  duplication and circular dependencies. See layer 4 under `Design`: this
  exclusion is what keeps a gather element and the fragments it lists from
  outdating each other forever. The HTML rendering of a fragment reproduces the
  layout of the source it came from: each doc block includes its indent, each
  line of a code block is preceded by that line's number, and the two are
  aligned -- a doc block and a line of code indented equally in the source begin
  in the same column, with the line numbers in a gutter of their own to the left
  of both.
* The backlinks a `<fragment>` hydrates to are derived from the gather elements
  which list it: for each such element, its containing file, its `id`, and its
  inner HTML (the link text). A fragment's rendered output therefore depends on
  the gather elements which reference it -- the reverse of the direction in
  which references are written; see `Design`.
* A `<fragment following=0>` is valid; it contains only the current doc block.
* The `following` attribute is clamped if it would exceed the number of code/doc
  blocks in the document.
* If the `following` value cannot be parsed to a whole number, an error message
  replaces the fragment content.
* A gather element, as a type of `Target`, requires an `id`. A `data-gather`
  attribute on an element without an `id` produces an error message which
  requests the missing `id`.
* All ids must be valid
  [CSS identifiers](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/es/ident)
  per
  [MDN recommendations](https://developer.mozilla.org/en-US/docs/Web/HTML/rence/Global_attributes/id).
  Invalid ids produce error messages in the hydrated tag content.

Example hydration of the gather tag `<h3 id="bar" data-gather="id1 id2...">Bazzy
things</h3>`:

```html
<h3 class="cc-gather" id="bar" data-gather="id1 id2...">Bazzy things</h3>
<div class="cc-gather-items" contenteditable="false">
  <p class="cc-gather-item-link">
    From <a href="link/to/first/tag#id1">Path to file</a>:
  </p>
  (first item content) ...
  <p class="cc-gather-item-link">
    From <a href="link/to/last/tag#idn">Path to file</a>:
  </p>
  (last item content)
</div>
```

Search
------

The cache supports searching the (cleaned) inner HTML of all `Target`s and
gather elements; search does not include `Fragment` contents.

### Auto-assignment of ids

If `id="*"` on either a fragment, target, or gather element, the cache replaces
this with an random autogenerated `id` placed in the resulting HTML contents,
but this new `id` is not yet recorded in the cache. The file is then marked as
`Unknown` when it is saved; when re-read, this `id` is then incorporated into
the cache. This helps avoid cases where the cache and file contents become
unsynchronized: if the `id` is placed in the cache before the write and the
write fails, or if the file is never written (it was being scanned, but not
actively edited, so the file contents wasn't written back).

The autogenerated id must be a valid
[CSS identifier](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/es/ident).
Autogenerated ids must be checked to make sure they don't collide with an
existing id.

Design
------

The cache is a single plain-data structure, shared as an `Arc<Mutex<Cache>>`;
all consistency comes from that one lock, so no per-item locking (and therefore
no lock ordering) is needed. Items refer to each other by key -- files by path,
targets and fragments by id -- rather than by `Arc`/`Weak` pointers. This keeps
the structure acyclic, `Send`, and (in the future) serializable, and avoids
garbage-collecting stale weak references.

Cached state always converges by design. Each layer depends only on
lower-numbered layers:

1. `Target::inner_html` (including a gather element's) and `Target::gather_ids`
   are pure functions of the source (pre-hydration) -- they depend on nothing.
2. `<xref>` hydration depends on layer 1 only.
3. `Fragment::content` depends on the source plus layer 2.
4. `<fragment>` backlink hydration depends on layer 1 only: it renders, for each
   gather element which lists this fragment, that element's containing file,
   `id`, and inner HTML. Critically, this output is *excluded* from
   `Fragment::content` (layer 3); were it included, a gather element and each
   fragment it lists would outdate one another forever.
5. Gather-list hydration depends on layers 1 and 3.

Nothing reads layers 2, 4, or 5, so propagation terminates.

Updating the cache is a two-phase process:

1. Collect: while walking a file's DOM, record all cacheable facts
   (`FileFacts`) -- targets, cross-references, fragments, and gather elements --
   without touching the cache. This keeps the non-`Send` DOM types out of the
   cache and off its critical section. Fact collection ignores content expanded
   by the cache (the contents of `xref` and `fragment` tags; the content
   following a gather tag).
2. Commit: `Cache::commit_file` applies the facts in one transaction, diffing
   them against the file's previous state to compute the set of files outdated
   by these changes.

Item 2 requires the cache to track dependencies of an item, so that files
containing these dependencies can be Outdated. Dependencies are tracked at file
granularity, on the id rather than on the item defining it:
`IdEntry::dependents` is the set of files whose rendered output depends on this
id, by any means -- an `<xref ref="id">` or a `data-gather` list naming it. One
uniform set suffices in all three `IdState`s because the typical action taken on
a dependent is marking its containing file outdated (fragments also used their
dependents to generate "See x" links).

Item 2 also requires the cache to define what constitutes a difference which
would trigger outdating dependencies. The state which is checked for differences
is:

* Target/gather element: type (a target/gather element), path of the containing
  file, id, IdState, inner HTML, and gather\_ids.
* Fragment: type (a fragment), path of the containing file, id, IdState, and
  contents.

This also makes indirection (gather A includes fragment B, whose content
cross-references target C) work without extra machinery: a change to C outdates
B's file; reprocessing B's file changes B's content, which outdates A's file.

### Gather elements: the reverse edge

`dependents` alone is not enough for gather elements, because the reference runs
in both directions: a gather element renders the *contents* of each fragment it
lists, and each of those fragments renders a *backlink* to the gather element
(see layer 4 above). The set of gather elements referencing a fragment is
therefore not merely bookkeeping -- it is observable output in the fragment's
own file.

This is why the outgoing references of gather elements cannot be maintained by
the unlink-all/relink-all round trip used for cross-references (see
`commit_file`): that round trip destroys the very information a diff would need.
`commit_file` instead diffs `Target::gather_ids` explicitly, and applies this
rule:

> When a gather element `G` is committed, outdate the file defining each id in
> the symmetric difference of `G`'s old and new `gather_ids`; if `G`'s inner
> HTML also changed, outdate the file defining each id in the union instead,
> since the backlink text those fragments render comes from it.

A gather element moving to a different file needs no special case: the old
file's commit sees the element deleted (new `gather_ids` empty) and the new
file's commit sees it added, so both ends of the move outdate the fragments.

Propagation terminates here for the reason given under layer 4: rebuilding a
fragment's file updates its backlinks, but backlinks are excluded from
`Fragment::content`, so nothing further is outdated.

An id referenced before (or without) being defined has an `IdEntry` whose state
is `IdState::Missing`; `dependents` holds the files waiting on it. When the id
later appears, the state transition marks those files outdated, and they simply
remain dependents; when a defined id disappears, the state returns to `Missing`
and the dependents are again waiters. An `IdEntry` which is `Missing` with no
dependents carries no information and is removed.

Duplicate ids are never renamed (file timestamps can't reliably identify the
original, and renaming would silently modify user content). Instead, duplicates
are reported as errors to the user.

### Misc

All file names (stored as `PathBuf`) must be canonicalized absolute paths.

HTML stored as `Target` inner HTML or in `Fragment` contents requires cleaning:

* To avoid duplicate IDs, all `id` attributes should be stripped.
* Note that images, URLs, etc. may not work if the referring path in their new
  location isn't valid; these simply aren't supported.
* Inner HTML must allow only the
  [permitted content for an `<a>` element](https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/a#technical_summary).

Watcher and walker
------------------

Need a watcher/walker (WW). Each time a project is found, the code must ask for
the cache for this project path. The WW maintains a connection id -> project
path mapping. If the current request doesn't change the mapping, simply return
the existing cache. If the mapping changes, then update the WW thingy. The map
must also be updated when a connection is closed (to remove an existing
mapping).

The WW thingy is another map from project path -> (cache, vec of project paths
that are contained with this path, including the same path \[when multiple
connections edit the same project\]). Operations:

* Insert: to insert a new project path, walk all existing top-level project
  paths. If the new project path is contained within any of these, add it to the
  appropriate list. Otherwise, add a new entry.
* Delete: find the path my looking fir
