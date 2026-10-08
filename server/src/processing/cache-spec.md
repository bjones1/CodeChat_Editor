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
the cache must re-process this unknown file. Since the Client is designed around
an autosave principle which updates disk/IDE regularly, there's little gap
between the two.

The cache hydrates data to the Client; the dehydration routes are responsible
for removing all cache hydration artifacts.

Cross references
----------------

A `<xref ref="id"></xref>` is a cross reference to a `Target` or a gather
element. The `ref` attribute specifies the destination as an id; the cache then
hydrates the contents based on the location and contents of the target of the
provided destination `id` to e.g. `<xref ref="id" contenteditable="false"><a
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
documents (in this case, the fragment contents consist of an error message).
Fragments may include the `following` attribute to enclose a specific number of
the following code/doc blocks; for example, `<fragment id="bar"
following="3"></fragment>` includes the current doc block along with the next 3
code/doc blocks; `following` must be a whole number.

Details:

* Fragment contents may not include a gather element; in this case, the gather
  element list of contents will simply include an error message.
* Fragments do support indirection: gather element A includes contents from
  fragment B, which contains a cross reference to target C. Changes to target C
  make B and A outdated.
* If a gather element refers to an `id` that is a `Target`, not a `Fragment`,
  the resulting output for this in the list of fragments is an error message.
* If a gather element refers to an `id` that wasn't found or is a duplicate, its
  contents will be replaced by an error message.
* Fragments store an HTML rendering of the code and doc blocks they contain,
  excluding the content produced by hydrating the `<fragment>` tags, to avoid
  duplication and circular dependencies. See layer 4 under `Design`: this
  exclusion is what keeps a gather element and the fragments it lists from
  outdating each other forever. The HTML rendering of a fragment reproduces the
  layout of the source it came from: each doc block includes its indent, each
  line of a code block or doc block is preceded by that line's number, and the
  two are aligned -- a doc block and a line of code indented equally in the
  source begin in the same column, with the line numbers in a gutter of their
  own to the left of both.
* The backlinks a `<fragment>` hydrates to are derived from the gather elements
  which list it: for each such element, its containing file, its `id`, and its
  inner HTML (the link text). A fragment's rendered output therefore depends on
  the gather elements which reference it -- the reverse of the direction in
  which references are written; see `Design`.
* A `<fragment following="0">` is valid; it contains only the current doc block.
* The `following` attribute is clamped if it would exceed the number of code/doc
  blocks in the document.
* If the `following` value cannot be parsed to a whole number, an error message
  replaces the fragment content.
* A gather element, as a type of `Target`, requires an `id`. A `data-gather`
  attribute on an element without an `id` produces an error message which
  requests the missing `id`.
* Likewise, a fragment requires an id, producing an error message when the `id`
  is missing.
* All ids must be valid
  [CSS identifiers](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Values/ident)
  per
  [MDN recommendations](https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Global_attributes/id).
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

Auto-assignment of ids
----------------------

If `id="*"` on either a fragment, target, or gather element, the cache replaces
this with a random autogenerated `id` placed in the resulting HTML contents, but
this new `id` is not yet recorded in the cache. The file is then marked as
`Unknown` when it is saved; when re-read, this `id` is then incorporated into
the cache. This helps avoid cases where the cache and file contents become
unsynchronized: if the `id` is placed in the cache before the write and the
write fails, or if the file is never written (it was being scanned, but not
actively edited, so the file contents weren't written back).

The autogenerated id must be a valid
[CSS identifier](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Values/ident).
Autogenerated ids must be checked to make sure they don't collide with an
existing id.

Design
------

The cache is a single plain-data structure, shared as an `Arc<Mutex<Cache>>`;
all consistency comes from that one lock, so no per-item locking (and therefore
no lock ordering) is needed. Items refer to each other by key -- files by path,
targets and fragments by id -- rather than by `Arc`/`Weak` pointers. This keeps
the structure acyclic, `Send`, and (in the future) serializable, and avoids
garbage-collecting stale weak references. Note that the watcher, consumer tx
channel, and connections struct cache members aren't serializable, but can be
re-created after deserialization.

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

Only layers 1 and 3 are cached, and layer 3 depends only on layer 1 through
layer 2. So a change travels at most two steps (C → B → A), which matches the
indirection example, ensuring that propagation terminates.

Updating the cache is a multi-phase process:

1. Collect: while walking a file's DOM, record all cacheable facts
   (`FileFacts`) -- targets, cross-references, fragments, and gather elements --
   without touching the cache. This keeps the non-`Send` DOM types out of the
   cache and off its critical section. Fact collection ignores content expanded
   by the cache (the contents of `xref` and `fragment` tags; the content
   following a gather tag).
2. Commit: `Cache::commit_file` applies the initial facts in one transaction,
   diffing them against the file's previous state to compute the set of files
   outdated by these changes.
3. Update: `update_fragment_content` store a fragment's rendered content, once
   the caller has finalized its doc blocks.
4. Hydrate: `hydrate_gathers` marks each gather element with the `cc-gather`
   class and insert its list of gathered fragment contents as its next sibling.

Item 2 requires the cache to track dependents of an id, so that files containing
these dependencies can be Outdated; these are sent to the `Cache::consumer_tx`
channel for processing. Dependencies are tracked at file granularity, on the id
rather than on the item defining it: `IdEntry::dependents` is the set of files
whose rendered output depends on this id, by any means -- an `<xref ref="id">`
or a `data-gather` list naming it. One uniform set suffices in all three
`IdState`s because the typical action taken on a dependent is marking this file
outdated (fragments also use their dependents to generate "See x" links).

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

This is why changes to the outgoing references of gather elements cannot be
determined by the unlink-all/relink-all round trip used for cross-references
(see `commit_file`): that round trip destroys the very information a diff would
need. `commit_file` instead diffs `Target::gather_ids` explicitly, and applies
this rule:

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

### Missing and duplicate ids

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

Cache, watcher, and walker
--------------------------

The `AppState` structure keeps an LRU map of project path -> cache (note that
the cache includes a watcher and walker). Adding/removing connections resizes
the LRU size to (connections \* `LRU_ENTRIES_PER_CONNECTION`) + 1; however, the
cache for all current connections may not be dropped during an LRU eviction.
This approach seeks to balance cache memory consumption with the time required
to load the cache for an uncached project.

The cache fully supports multiple connections to the same project. It does not
walk symlinks, since these could lead to two cache entries for the same file.
There's no explicit support for nested projects. While it's possible for one
project to be nested inside another, this design does not support this case.

The cache initially registers a watcher with the provided project path and a
consumer thread which receives results from the walker, along with a message
channel connecting them.

### Watcher/walker

The watcher/walker thread monitors the root directory of the project and all
children for changes using the
[notify-debouncer-full](https://docs.rs/notify-debouncer-full/latest/notify_debouncer_full/)
crate. At startup:

1. It registers the watcher, to ensure that any changes during the walk are
   still reported.
   * The watcher sends file names of files that were created, modified, or
     deleted and the path of created or deleted directories to the walker
     encoded as `WatcherPathState`s via the walker channel; renames are treated
     as a delete of the `from` path followed by a create of the `to` path.
   * If the watcher encounters lost events (`EventKind::Other` with
     `Flag::Rescan`), it requests a re-walk of the entire project.
2. Even if the watcher fails, it then runs a filesystem walker loop rooted at
   the project path, initially sending it the root directory to walk and
   providing it the consumer channel and a second walker channel.

A closed channel signals the watcher/walker thread to exit. Note that this
approach means extraneous watcher events from ignored directories, such as
`.git`, and additional use of OS resources such as inotify watches.

#### Walker

The walker is based on the [ignore](https://docs.rs/ignore/latest/ignore/)
crate, allowing users to exclude files from the project. After the initial
traverse of the project, it keeps
an [IncrementalIgnore](https://docs.rs/ignore/latest/ignore/struct.IncrementalIgnore.html)
for checking files in the `WatcherPathState`s received from the watcher.

* If the path is a deleted file or directory and is not ignored by the walker,
  it's sent directly to the consumer thread.
* If the path refers to any of the walker's ignore files (per the
  [ignore docs](https://docs.rs/ignore/latest/ignore/struct.WalkBuilder.html),
  this is `.gitignore`, `.git/info/exclude`, and `.ignore`):
  * The walker and incremental ignores are reloaded.
  * For `.gitignore` and `.ignore`, the path containing this file is re-walked;
    for `.git/info/exclude`, the entire tree is re-walked. For simplicity, this
    occurs even outside a Git repository, or in subrepos, etc. TODO: also have
    the watcher monitor global ignore files.
* If the path is a file and is not ignored by the walker, it's sent to the
  consumer thread.
* If the path is a directory, it's walked; any files found are sent to the
  consumer thread.

Before performing a walk, the walker sends a `WalkStarted` to the consumer
thread via the `Cache::consumer_tx` channel; when the walk is completed, it
sends a `WalkFinished`.

After processing a path, the walker then loops to wait for the next path. The
thread ends when either channel is closed (indicating the cache was dropped). It
ignores any errors produced during the walk, assuming that files it can't walk
aren't usable by the IDE.

### Consumer

A consumer reads paths from the receiver paired to the `Cache::consumer_tx`
channel. It holds a weak reference to the cache, and exits when this channel is
closed or the weak reference can't be upgraded. It is implemented as a thread.

* For a `WatcherPathState`: if the file exists and differs from its cached
  metadata's modification time and length, perform the following steps. For an
  `Outdated`, always perform the following steps:

  * If not translatable, the file is still added to the cache, to prevent
    reprocessing if the directory is re-walked.

  * If translatable, it translates the file to the CodeChatForWeb format in
    order to update the cache for that file. If the `SaveOnScan` option is
    enabled, it then translates the file back to source code and writes the
    result if there are any changes, updating the file's metadata in the cache
    after the write. This costs additional CPU time and produces watcher events
    and may rewrite files open in the IDE, motivating the choice of a
    user-selected option.

    If processing a file marks any connection's current file or its TOC as
    outdated or deleted, the connection(s) are sent a `CurrentFileOutdated`
    and/or `TocOutdated`/`TocChanged` message(s) to retranslate the file/IDE
    contents; otherwise, the `Outdated(file)` message is sent to itself via the
    consumer TX channel for processing. Note that `TocOutdated`/`TocChanged`
    messages are sent to all the cache's connections, since they share the same
    project, while `CurrentFileOutdated` is only sent to multiple connections is
    it being simultaneously edited. If the TOC is the current file, both
    messages are sent. `TocChanged` is sent if the TOC was changed, while
    `TocOutdated` is sent when the TOC is only outdated.
* If the path doesn't exist, search the cache for it; if found, remove it.
  Otherwise, assume the path was a directory. Search the cache for all files in
  this directory, removing them.

* If this is a `WalkStarted`, put all files in the cache which this directory
  contains in a set. Remove files produced by the walk from this set. After
  `WalkFinished`, remove any files remaining in the set from the cache, since
  these no longer exist.

The cache doesn't record images or multimedia files referenced by a source file
as dependencies, requiring the user to manually refresh the page when these
files change.

### Connections

Every connection in a project also shows the project's TOC in its Client's
sidebar, so whenever a commit marks the TOC outdated, the cache sends
`TocOutdated` to every connection in the project. The connection re-renders the
TOC from the text its sidebar last loaded -- from the IDE if the IDE has the TOC
open, otherwise from disk -- then, if the rendering changed, sends the new HTML
to the Client in an `UpdateToc` message. The Client replaces the contents of its
sidebar in place, which preserves the sidebar's scroll position along with the
listener its page attached to intercept clicks on the TOC's links; reloading the
sidebar would discard both.

Committing the TOC itself doesn't outdate the TOC, since a commit never outdates
its own file. This matters because each sidebar load translates, and therefore
commits, the TOC: were that commit to outdate the TOC, each refresh would
trigger another. Instead, edits to the TOC reach the sidebar in other ways:

* When a connection's current file is the TOC, the connection refreshes its
  sidebar after each edit to the TOC, whether made in the IDE or the Client.
* TODO: once the consumer thread exists, have it notify every connection in the
  project when the TOC changes on disk. Since the TOC's text then differs from
  the text the sidebar last loaded, this needs a notification other than
  `TocOutdated`, prompting the connection to fetch the TOC's new text from its
  IDE (with a `LoadFile`), falling back to disk, before re-rendering it.

### Shutdown

Overall shutdown sequence:

1. The LRU map drops a `Cache` and the upgraded Weak reference by the consumer
   thread is dropped.
2. The `Cache` drops the `_watcher` and the `consumer_tx` channel.
3. When dropped, the `_watcher` closes its channel to the walker, stops the
   watcher, drops its `consumer_tx` channel, and exits.
4. The consumer thread exits when either the its consumer rx channel closes or
   the weak pointer fails to upgrade.

TODO:

* `SaveOnScan` and search functionality is not implemented yet. This is future
  work.

<p><br></p>

<p><br></p>

<p><br></p>

<p><br></p>
