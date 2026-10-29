The CodeChat Editor, a word processor for software developers.<br>Copyright (C)
2024-2026 Bryan A. Jones.

This file is part of the CodeChat Editor.

The CodeChat Editor is free software: you can redistribute it and/or modify it
under the terms of the GNU General Public License as published by the Free
Software Foundation, either version 3 of the License, or (at your option) any
later version.

The CodeChat Editor is distributed in the hope that it will be useful, but
WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more
details.

You should have received a copy of the GNU General Public License along with the
CodeChat Editor. If not, see
[http://www.gnu.org/licenses](http://www.gnu.org/licenses).

Cache spec
==========

The cache stores the location (file name and ID) and contents (inner HTML or
code/doc blocks for fragments) of targets and fragments. Targets are HTML
elements (excluding `<fragment>`s and `<xref>`s) with an ID.

The goal of the cache is to support cross-references and gather elements, and to
assist in ensuring that all IDs are unique within a project. This means that
cross-references and gather elements persist across moving or renaming files,
since the IDs will be found in the cache.

Non-project files support a subset of this functionality: the "project" consists
only of the current file. Only targets, gather elements, cross-references, and
fragments to items within the file work as expected; references to other files
do not.

The cache reflects data read directly from disk/IDE; content edited in the
Client does not update the cache until it's written to disk/IDE, at which point
the cache must re-process the file. Since the Client is designed around an
autosave principle which updates disk/IDE regularly, there's little gap between
the two.

The cache hydrates data to the Client; the dehydration routes are responsible
for removing all cache hydration artifacts.

<h2 id="cc-HZyE2Bv-eI">Cross references</h2>

A `<xref ref="id"></xref>` is a cross reference to a `Target` or a gather
element. The `ref` attribute specifies the destination as an id; the cache then
hydrates the contents based on the location and contents of the target of the
provided destination `id` to e.g. `<xref ref="id" contenteditable="false"><a
href="../path/to/page#id">Inner HTML from target</a></xref>`; this hydrated form
is only present in the Client.

Details:

* This element allows no attribute except for `ref`. Since an `<xref>` is never
  a `Target`, an `id` attribute on it -- including `id="*"` -- would mislead, so
  it replaces the contents with an error message.

* The inner HTML is always taken before cache hydration, to prevent circular
  dependencies (see also <xref ref="cc-P-g5o100jN"></xref>):

  ```html
  <!-- file a -->
  <h1 id="a">See <xref ref="b"></xref></h1>
  <!-- file b -->
  <h1 id="b">See <xref ref="a"></xref></h1>
  ```

  Including cache hydration would cause the inner HTML to be updated each time
  the file is processed, outdating the other file.

* If the `ref` attribute is missing, the `id` referred to isn't found, refers to
  a duplicate id, or is an invalid CSS identifier, the inner text is instead an
  appropriate error message.

* If the cross reference is to a gather element, the text is the gather
  element's inner HTML, not the gathered code/doc blocks.

* Cross references to fragments are not allowed; this would create circular
  dependencies (see <xref ref="cc-P-g5o100jN"></xref>).

<h2 id="cc-gKXTXIZnFm">Fragments and gather elements</h2>

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
  duplication and circular dependencies. Cross-references in this rendering are
  hydrated in the fragment's file, so their links lead from there; a gather
  element re-hydrates each which hydrated to a link so that it leads from the
  gather element's file instead. An error message reads the same anywhere, so
  it's kept as is. See layer 4 under <xref ref="cc-P-g5o100jN"></xref>: this
  exclusion is what keeps a gather element and the fragments it lists from
  outdating each other forever. The HTML rendering of a fragment reproduces the
  layout of the source it came from: each doc block includes its indent, each
  line of a code block or the first line of a doc block is preceded by that
  line's number, and the two are aligned -- a doc block and a line of code
  indented equally in the source begin in the same column, with the line numbers
  in a gutter of their own to the left of both.
* The backlinks a `<fragment>` hydrates to are derived from the gather elements
  which list it: for each such element, its containing file, its `id`, and its
  inner HTML (the link text). A fragment's rendered output therefore depends on
  the gather elements which reference it -- the reverse of the direction in
  which references are written; see <xref ref="cc-P-g5o100jN"></xref>.
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

Search (future work)
--------------------

The cache will support searching the (cleaned) inner HTML of all `Target`s and
gather elements; search will not include `Fragment` contents.

Auto-assignment of ids
----------------------

If `id="*"` on either a fragment, target, or gather element, the cache replaces
this with a random autogenerated `id` placed in the resulting HTML contents, but
this new `id` is not yet recorded in the cache. It enters the cache only when
source containing it is next translated: the Server translates each edit the
Client saves back to the Client, to check for changes, which commits the edit;
otherwise, the consumer reads the file once it's written to disk. This helps
avoid cases where the cache and file contents become unsynchronized: if the `id`
is placed in the cache before the write and the write fails, or if the file is
never written (it was being scanned, but not actively edited, so the file
contents weren't written back).

TODO: an `id="*"` typed in the IDE reaches the source only when the Client sends
back the HTML containing its replacement, which it does only once the user edits
in the Client; until then, each translation of the IDE's text assigns a
different id. Resolve this by sending the replacement to the IDE when a
translation assigns ids.

The autogenerated id must be a valid
[CSS identifier](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Values/ident).
Autogenerated ids must be checked to make sure they don't collide with an
existing id.

<h2 id="cc-P-g5o100jN">Cache design</h2>

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
5. Gather-list hydration depends on layers 1 and 3. It also re-hydrates each
   `<xref>` within a fragment's content (layer 2) which hydrated to a link, so
   that it links from the gathering file. That needs no dependency of its own:
   everything about a target which this reads is embedded in the fragment's
   content, so any change to it also changes that content, which outdates the
   gathering file.

Only layers 1 and 3 are cached, and layer 3 depends only on layer 1 through
layer 2. So a change travels at most two steps (C → B → A), which matches the
indirection example, ensuring that propagation terminates.

Supporting cross-references, fragments, and gather elements requires the cache
to track dependents of an id, so that files containing these dependencies can be
marked `Outdated` (see below); these are sent to the `Cache::consumer_tx`
channel for processing. Dependencies are tracked at file granularity, on the id
rather than on the item defining it: `IdEntry::dependents` is the set of files
whose rendered output depends on this id, by any means -- an `<xref ref="id">`
or a `data-gather` list naming it. One uniform set suffices in all three
`IdState`s because the typical action taken on a dependent is marking this file
outdated (fragments also use their dependents to generate "See x" links).

Updating dependencies requires a definition of what constitutes a difference
which would trigger outdating dependencies. The state which is checked for
differences is:

* Target/gather element: type (a target/gather element), path of the containing
  file, id, IdState, and inner HTML.
* Fragment: type (a fragment), path of the containing file, id, IdState, and
  contents.

A gather element's `gather_ids` are absent from this list because no dependent
reads them: a cross-reference to a gather element renders only its inner HTML.
The fragments a gather element lists do depend on them, through the reverse edge
described below.

This also makes indirection (gather A includes fragment B, whose content
cross-references target C) work without extra machinery: a change to C outdates
B's file; reprocessing B's file changes B's content, which outdates A's file.

Updating the cache is a multi-phase process; see
<xref ref="cc-HfGbqqRaWs"></xref>. One output from this process (see
<xref ref="cc-MtbstS0OHd"></xref>) is a list of outdated files. For each
outdated file, mark its status as `Outdated` (see
<xref ref="cc-MxbstS0OHd"></xref>) then:

1. If the outdated file is the project TOC, then send `TocOutdated` to all
   connections.
2. If the outdated file is the current file for one or more connections, then
   send `CurrentFileOutdated` to those connections.
3. If neither 1 or 2 is true, the `Rescan(file)` message is sent to itself via
   the consumer TX channel for later processing.

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

Cache population and maintenance
--------------------------------

The cache must contain information about all renderable files in the project.
The following design employs a file watcher to update the cache when files
change, and a file walker to populate the cache initially and responds to
directory changes.

The `AppState` structure keeps an LRU map of project path -> cache (note that
the cache includes a watcher and walker). Adding/removing connections resizes
the LRU size to (connections \* `LRU_ENTRIES_PER_CONNECTION`) + 1; however, the
cache for all current connections may not be dropped during an LRU eviction.
This LRU-based approach seeks to balance cache memory consumption with the time
required to load the cache for an uncached project.

The cache fully supports multiple connections to the same project. It does not
walk symlinks, since these could lead to two cache entries for the same file.
There's no explicit support for nested projects.

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

The consumer decides whether a file changed by its stamp, as Git's index does: a
snapshot of the file's length and modification time (plus its change time and
inode on Unix), taken before its contents were read. A stamp taken within a few
seconds of the file's last write never matches, since a later write in the same
timestamp tick could leave this metadata unchanged (Git's "racy clean" problem).

* If the file exists and doesn't match its cached stamp or if the file's status
  is `Outdated`:

  * If the file is the TOC and doesn't match its cached stamp, send `TocChanged`
    to all connections once the processing below is done, even if it ignores the
    file: each connection's sidebar renders the TOC from the text it last
    loaded, which it must therefore re-load (see `Connections` below).

  * If the file is the current file for any of the cache's connections, ignore
    it. This is handled by the standard IDE to Client translation mechanism.

  * If not translatable, the file is still added to the cache, to prevent
    reprocessing if the directory is re-walked.

  * If translatable, it translates the file to the CodeChatForWeb format in
    order to update the cache for that file. If the `SaveOnScan` option is
    enabled, it then translates the file back to source code and writes the
    result if there are any changes, updating the file's stamp in the cache
    after the write. This costs additional CPU time and produces watcher events
    and may rewrite files open in the IDE, motivating the choice of a
    user-selected option.
* If the path doesn't exist, search the cache for it; if found, remove it.
  Otherwise, assume the path was a directory. Search the cache for all files in
  this directory, removing them.

* A `Rescan(file)` asks the consumer to bring a file the cache already has up to
  date: one a commit outdated, or a connection's current file once no connection
  shows it, since the consumer ignores current files. Unlike a path from the
  walker, a `Rescan` hasn't passed the walker's ignore rules. It's therefore
  skipped if the cache has since forgotten the file (a walk may have found it
  newly ignored, for example), and it doesn't keep a walk in progress from
  forgetting the file.

* If this is a `WalkStarted`, put all files in the cache which this directory
  contains in a set. Remove files produced by the walk from this set. After
  `WalkFinished`, remove any files remaining in the set from the cache, since
  these no longer exist.

Neither kind of removal applies to a file a connection shows -- its current
file, or the TOC -- since the connection's IDE may still hold the file's text.
Instead, each connection whose current file it is re-translates it, and, if it's
the TOC, every connection re-loads it.

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
* When the TOC changes on disk, the consumer sends `TocChanged` to every
  connection in the project. Since the TOC's text then differs from the text the
  sidebar last loaded, this prompts the connection to fetch the TOC's new text
  from its IDE (with a `LoadFile`), falling back to disk, before re-rendering
  it.

### Shutdown

Overall shutdown sequence:

1. The LRU map drops a `Cache` and the upgraded Weak reference by the consumer
   thread is dropped.
2. The `Cache` drops the `_watcher` and the `consumer_tx` channel.
3. When dropped, the `_watcher` closes its channel to the walker, stops the
   watcher, drops its `consumer_tx` channel, and exits.
4. The consumer thread exits when either its consumer rx channel closes or the
   weak pointer fails to upgrade.

<h2 id="cc-srNQxJ2znz">Links in hydrated content</h2>

Some content, such as math, cross-references, fragments, and gather lists (see
<xref ref="cc-HZyE2Bv-eI"></xref> and <xref ref="cc-gKXTXIZnFm"></xref>), sits
inside an element marked `contenteditable=false`. TinyMCE's link plugin offers
no context menu items for non-editable content, so a right click (mouse) or long
press (touch) on one of these links is ignored. Therefore, when the user
performs either of these actions, the Client must show the TinyMCE context menu,
whose only item, "Open link", follows the link as a Ctrl+click does. A right
click or long press on non-editable content beside its links offers no "Open
link". With a non-editable element selected, the keyboard's context menu
shortcut (the Menu key, or Shift+F10) shows the same menu, which follows the
element's first link, as Alt+Enter does. The Client also follows these links in
the other ways it follows any link (see `Link navigation` in
[implementation.md](../../../docs/implementation.md)):

* Ctrl+click (Cmd+click on macOS) follows the clicked link.
* Alt+Enter, with the non-editable element selected, follows its first link.

TODO
----

* `SaveOnScan` and search functionality are not implemented yet. This is future
  work.
