Cache specification
===================

**Status: DRAFT.** This document specifies the strategy used by `cache.rs` to
cache all targets and fragments in a CodeChat Editor project. It is written for
review; items marked *Open question* are unresolved.

Purpose and scope
-----------------

The CodeChat Editor renders a file by translating its source to HTML. Some of
that HTML depends on *other* files: a cross-reference (`<xref ref="id">`) shows
text taken from its target, and a `<fragment>` shows backlinks to the gather
elements (`data-gather`) which list it. The cache records, for every file in a
project, what each file defines and what it references, so that:

1. An id can be resolved to its definition without reading the file defining it
   (`resolve_id`).
2. A change to one file identifies exactly the other files whose rendered output
   is now stale, so that only those are re-rendered.
3. The above stays correct as files are created, edited, renamed, and deleted
   by any program, not only by the IDE connected to the Server.

The cache does *not* store rendered pages, and does not (yet) index file
contents for search.

Terminology
-----------

* **Project**: a directory tree whose root contains `toc.md`. A file opened
  without a `toc.md` is a single-file "project" whose cache has no consumer or
  watcher.
* **Target**: any HTML element with an `id`, other than a `<fragment>`. A
  *gather element* is a target with a non-empty `data-gather` list.
* **Fragment**: a `<fragment id=...>`: a series of code/doc blocks starting with
  a doc block.
* **Cross-reference**: an `<xref>` naming an id.
* **Dependent** (of an id): a file whose rendered output depends on that id,
  because it cross-references it or lists it in a gather element.
* **Outdated**: a file whose cached facts are current, but whose *rendered*
  output must be regenerated.
* **Connection**: one Client/IDE session. Its *current file* is the file it
  shows; the connection, not the cache, owns that file's text.

Data model
----------

A project's `Cache` holds:

* `files`: a map from each file's normalized absolute path to a `FileEntry`.
  The map is ordered by path component, so all files beneath a directory are
  contiguous and can be found by a range query.
* `ids`: a map from id to an `IdEntry`.
* `root`, the project root.
* `connections`: the connections whose current file lies in this project.

A `FileEntry` records: a **stamp** (see below), a **status** (`Unknown`,
`Outdated`, `UpToDate`), the file's **targets** and **fragments** (each keyed by
id), its **cross-references**, and an index of its **gather elements**.

An `IdEntry` records the id's **state** and its **dependents**:

| State      | Meaning                                                           |
| ---------- | ----------------------------------------------------------------- |
| `Single`   | Exactly one definition, in one file.                              |
| `Multiple` | More than one definition; maps each defining file to its count.   |
| `Missing`  | Referenced but not defined.                                       |

Targets and fragments share one id namespace. A duplicate id is an error which
is reported wherever the id is referenced.

Invariants
----------

1. Every id in `ids` is either defined or has at least one dependent. An entry
   which is neither is removed.
2. `IdEntry::dependents` contains file *F* if and only if *F*'s most recently
   committed version references that id.
3. A file is never marked outdated by its own commit.
4. A cache is never evicted from the `CacheMap` while a connection is registered
   in it.
5. The lock on a cache is never held during file system I/O or during a long
   operation spanning many files.

Committing a file (`commit_file`)
---------------------------------

The facts collected from translating one file (its targets, cross-references,
and fragments, in document order) are applied in one transaction, under one
lock acquisition:

1. **Take the old version** out of the entry.
2. **Unlink** the file from the dependents of every id the old version
   referenced.
3. **Build the new definitions.** If an id is defined several times in the file,
   the first definition in document order is stored, but all definitions are
   counted. A surviving fragment keeps its previous content until
   `update_fragment_content` supplies the new content.
4. **Update id states.** For each id defined before or now, replace this file's
   contribution to the id's definition map. If the resulting state differs from
   the old state (id appeared, disappeared, moved, or became or ceased to be a
   duplicate), every dependent of that id is outdated.
5. **Outdate dependents of modified definitions.** A change to a target's inner
   HTML, or an id changing kind between target and fragment, leaves the state
   unchanged but still outdates the id's dependents.
6. **Outdate the files defining regathered fragments** (the *reverse edge*,
   below).
7. **Store** the new version, set the status to `UpToDate`, and **link** the
   file as a dependent of every id it references (creating `Missing` entries as
   needed).
8. **Forget unused ids**, then mark the outdated files, excluding the committed
   file.

### The reverse edge

A `<fragment>` renders a backlink to each gather element listing it, so a
fragment's file depends on those gather elements. Unlike cross-references, this
cannot be handled by unlinking and relinking, because the set of gather elements
is observable output. Instead the old and new gather lists are diffed before
being overwritten:

* If a gather element's inner HTML changed (including being added or removed),
  every id in the union of its old and new lists is affected, since that HTML is
  the text of each backlink.
* Otherwise only the symmetric difference of the old and new lists is affected.

The file defining each affected fragment is outdated. An affected id with no
definition needs nothing: when it is defined, its file renders its backlinks in
that same pass. <a id="cc-P-g5o100jN"></a>This does not loop: outdating a
fragment's file re-renders it, which changes the fragment's content only if
doing so changes what gather elements display, and gather elements display
their own text, not the fragment's.

*Open question*: this termination argument should be stated more rigorously; it
holds as long as a gather element's `inner_html` never includes rendered
fragment content.

### Fragment content

A fragment's rendered content is known only after its doc blocks are finalized,
after `commit_file`. `update_fragment_content` stores it; if it changed, every
dependent of the fragment's id other than the fragment's own file is marked
outdated. Gather elements in the fragment's own file are updated in the same
processing pass.

### Removing a file

`remove_file` commits an empty set of facts (which withdraws all definitions and
references, outdating dependents) and then drops the entry.

Resolving ids
-------------

`resolve_id` returns the target or fragment an id names along with its file,
`Missing`, or `Multiple`. `gathers_referencing` returns every gather element
listing a fragment id, sorted by (file, gather id) so that rendered backlinks
are deterministic.

Auto-assignment of ids
----------------------

An element with `id="*"` is assigned a new id of the form `cc-` followed by 10
characters from `A-Z a-z 0-9 - _` chosen at random, which collides with no id
in the cache and none assigned earlier in the same pass. The new id is
deliberately **not** recorded in the cache: it enters the cache only when the
file containing it is saved and read again. This way a failed or abandoned save
cannot leave the cache describing an id which no file contains.

Cache population and maintenance
--------------------------------

A project cache is kept current by three cooperating pieces. Each holds only a
weak reference to the cache, so none keeps a dropped cache alive.

* The **watcher** reports changes to files and directories in the project
  (debounced for 2 seconds). A rename is reported as a deletion plus a creation.
  If the platform reports lost events, the whole project is re-walked.
  Symlinks are not followed. If the watcher cannot start (for example, inotify
  limits), a warning is logged; the cache remains usable but learns only of
  files found by the initial walk.
* The **walker** filters reported paths and walks directories. It obeys the same
  rules as ripgrep: hidden files and directories, `.ignore`, and (inside a Git
  repository) `.gitignore` and Git's exclude rules are skipped. A change to an
  ignore file triggers a re-walk of the directory it governs (the whole project
  for `.git/info/exclude`). A directory reported as created is walked.
* The **consumer** applies the walker's results to the cache.

The watcher is registered *before* the initial walk, so a file created during
the walk is reported by at least one of them. Registration may be slow, so it
occurs on its own thread rather than delaying the translation which created the
cache.

### Messages to the consumer

| Message                        | Meaning                                                    |
| ------------------------------ | ---------------------------------------------------------- |
| `CreatedOrModified(path)`      | Scan this file.                                            |
| `Deleted(path)`                | The path may no longer exist.                              |
| `WalkStarted(dir)`, `WalkFinished` | Bracket the files found by walking `dir`.            |
| `Rescan(path)`                 | Bring an already-cached file up to date; not evidence that the file isn't ignored. |

### Scanning a file

1. Read the file's metadata (without holding the lock). If the path is no longer
   a file, forget it.
2. Skip the file if it is some connection's current file (that connection keeps
   it current from the IDE's text). If it is also the TOC and has changed,
   notify connections.
3. Skip the file if the cache has it, it is unmodified per its stamp, and its
   status is `UpToDate`. A file not in the cache is scanned only if the walker
   sent it.
4. Read the file as text. If the first 8 KB is not UTF-8 the file is binary and
   is not translated; a file which cannot be read is left as-is.
5. Translate the file (converting `\r\n` to `\n`), which commits its facts.
6. Record the stamp taken *before* reading, not after: a write between reading
   and stamping would otherwise make the cache describe the old text with the
   new stamp, so the watcher's report of that write would be skipped.
7. A file with no lexer commits empty facts and its stamp, withdrawing anything
   it previously defined. A failed translation discards the stamp so that the
   next walk retries it.

### Detecting changes: file stamps

A stamp is a snapshot of length, modification time, and (on Unix) change time,
device, and inode. Modification times are compared only for equality, never
order. As in Git, a stamp is **racy**, and never matches, if the file was
modified within 3 seconds of the snapshot, if its modification time is in the
future, or if the platform reports none. On Windows the creation time is not
used: NTFS "tunneling" can give a newly-saved file the creation time of the
file it replaced.

### Deletions and walks

* A deleted path whose type is unknown is looked up as a file; if the cache has
  no such file, every cached file beneath it is forgotten. A deletion for a
  path which exists again (a VCS checkout, or an editor which saves by
  renaming) is a scan, not a forget.
* A walk of directory *D* records the cached files beneath *D* before it starts.
  Those not found by `WalkFinished` are forgotten, since they were deleted or
  are now ignored. A `Rescan` never counts as "found".
* The lock is released between files while forgetting.

### Forgetting a shown file

A file shown by a connection (its current file, or the TOC when any connection
exists) is never removed from the cache by a deletion or an ignore change, since
the IDE may still hold its text. Instead the connection is told to re-translate
it, which keeps it cached; for the TOC every connection reloads it.

Connections
-----------

A `ConnectionRegistration` lists a connection and its current file in the cache
and removes the listing when dropped. Registering replaces any earlier
registration for the same connection id (the old one is dropped first). On drop,
if no other connection shows the file, a `Rescan` checks it against disk; if
unchanged, the content committed from the IDE remains.

When files become outdated, `mark_outdated` sets their status and then:

* sends `CurrentFileOutdated` to each connection whose current file is outdated;
* sends `TocOutdated` to every connection if the TOC is outdated;
* sends `Rescan` to the consumer for every outdated file *not* shown by a
  connection.

`TocChanged` is sent when the TOC's text changes on disk or the TOC is deleted.
Notification channels are unbounded because the cache lock is held while
sending; a failed send means the connection has exited and is ignored.

Project cache map and eviction
------------------------------

The Server keeps a `CacheMap`: an LRU map from project root to cache.

* Capacity is *n* × 10 + 1 for *n* connections. This balances memory against
  the cost of reloading an evicted project.
* Insertion and resizing evict the least recently used caches **with no
  connections** first. A cache with connections is never evicted; if all
  entries have connections, the map is grown by one to make room.
* Lock order: the outer map lock before any single cache's lock.

Shutdown
--------

Only the `CacheMap` owns a cache strongly. When a cache is dropped, its watcher
is dropped (stopping the watch), the watcher's channel closes, the walker exits,
the consumer's sender closes, and the consumer exits. The consumer upgrades its
weak reference per message, so it never delays this.

Error handling
--------------

| Condition                         | Behavior                                              |
| --------------------------------- | ----------------------------------------------------- |
| Watcher cannot start              | Log a warning; continue without watching.             |
| Watcher reports an error          | Log only.                                             |
| File unreadable (locked)          | Leave cache unchanged; rely on a later watch event.   |
| File not UTF-8                    | Not translated; no definitions.                       |
| Translation fails                 | Log; discard stamp so it is retried.                  |
| Duplicate id                      | Reported at each reference; first definition stored.  |
| Poisoned lock in `Drop`           | Recover the inner value rather than panic.            |

Open questions and known gaps
-----------------------------

* `IdResolution::Multiple` is not yet used by callers, so duplicates within a
  single file are not yet reported distinctly.
* Line numbers in targets and fragments are always 0 until the markdown HTML
  writer preserves them; the Client cannot yet scroll to an anchor.
* The `SaveOnScan` option (write auto-assigned ids back during a scan) is
  unimplemented.
* Search indexing (id, contents, file name) is not yet designed.
* `notify` cannot yet filter event kinds at the source.
* On Windows, stamps lack change time and file index, so same-length edits
  within the racy window depend on the 3-second rule.
