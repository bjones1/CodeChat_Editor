// The CodeChat Editor, a word processor for software developers.<br>Copyright
// (C) 2024-2026 Bryan A. Jones.
//
// This file is part of the CodeChat Editor. The CodeChat Editor is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, either
// version 3 of the License, or (at your option) any later version.
//
// The CodeChat Editor is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along with
// the CodeChat Editor. If not, see
// [http://www.gnu.org/licenses](http://www.gnu.org/licenses).
//! `cache.rs` -- Cache all targets and fragments in a project
//! ==========================================================
//!
//! See the [cache spec](cache-spec.md).
// Imports
// -------
//
// ### Standard library
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::{self, File, Metadata},
    io::{self, Read},
    mem,
    num::NonZeroUsize,
    ops::Bound,
    path::{Path, PathBuf},
    str,
    sync::{
        Arc, Mutex, PoisonError, Weak,
        mpsc::{self, Receiver, SendError, Sender},
    },
    thread,
    time::{Duration, SystemTime},
};

// ### Third-party
use ignore::{IncrementalIgnore, WalkBuilder};
use log::warn;
use lru::LruCache;
use notify_debouncer_full::{
    DebounceEventResult, Debouncer, RecommendedCache, new_debouncer_opt,
    notify::{
        self, Event, EventKind, RecommendedWatcher, RecursiveMode,
        event::{ModifyKind, RenameMode},
    },
};
use rand::random;
use tokio::sync::mpsc::UnboundedSender;

// ### Local
use super::{SourceToCodeChatForWebError, source_to_codechat_for_web};

// Data structures
// ---------------
/// The set of all project caches, keyed by each project's root directory (the
/// directory containing its `toc.md`). The outer mutex guards the addition of
/// newly-discovered projects; each inner mutex guards a single project's cache,
/// so that work on one project doesn't block work on another. When both are
/// needed, lock the outer mutex first.
pub type CacheMap = Arc<Mutex<ProjectCaches>>;

/// The contents of a `CacheMap`. Insert into and resize it only through
/// `get_or_insert_cache` and `set_cache_map_capacity`, which evict the least
/// recently used cache when full -- but never a cache in which a connection is
/// registered.
pub type ProjectCaches = LruCache<PathBuf, Arc<Mutex<Cache>>>;

/// The number of project caches the `CacheMap` holds per connection; see
/// `cache_map_capacity`.
pub const LRU_ENTRIES_PER_CONNECTION: usize = 10;

/// This defines the cache used to store all targets and fragments in a project.
pub struct Cache {
    /// Provide rapid access to a file by its normalized, absolute path; it must
    /// be within the project's root directory. This owns all per-file data.
    /// Paths order component by component, so the files beneath a directory
    /// immediately follow it in this map: finding them, as a deleted or
    /// re-walked directory requires, is a range query rather than a search of
    /// every file.
    pub(super) files: BTreeMap<PathBuf, FileEntry>,
    /// Provide rapid access to a `Target`/gather element or `Fragment` by its
    /// id.
    pub(super) ids: HashMap<String, IdEntry>,
    /// The root directory of this project, or a specific file if this is a
    /// single-file "project".
    pub(super) root: PathBuf,
    /// Sends work to this project's consumer thread: the walker sends what it
    /// finds through a clone of this sender, `mark_outdated` sends outdated
    /// files, and dropping a `ConnectionRegistration` sends the file it showed.
    /// `None` if there's no consumer -- a single-file "project", or a test.
    consumer_tx: Option<Sender<ConsumerPathState>>,
    /// This project's watcher, which the watcher/walker thread stores here once
    /// it registers the watcher; `None` until then, if there's no consumer, or
    /// if the watcher couldn't start. Dropping it stops the watch, which begins
    /// the shutdown sequence described in the spec.
    _watcher: Option<Debouncer<RecommendedWatcher, RecommendedCache>>,
    /// The connections whose current file lies in this project, keyed by
    /// connection id; each entry is owned by a `ConnectionRegistration`.
    connections: HashMap<String, ConnectionView>,
    // TODO: search engine data storage. Search fields: target ID, contents,
    // file name. Perhaps [Tantivy](https://docs.rs/tantivy/latest/tantivy/)?
}

/// This stores the cached data for a given file. For non-page files
/// (non-existent files, images, PDFs, etc.) many of the fields are empty or
/// `None`.
#[derive(Default)]
pub(super) struct FileEntry {
    /// The file's stamp, taken when its content was read; the consumer
    /// reprocesses the file when it no longer matches. `None` if the file
    /// doesn't exist, its metadata can't be determined, or the consumer's last
    /// translation of it failed.
    pub(super) stamp: Option<FileStamp>,
    /// The status of this file.
    pub(super) status: FileStatus,
    /// All targets on this page, keyed by id.
    pub(super) targets: HashMap<String, Target>,
    /// All cross references on this page.
    pub(super) xrefs: Vec<Xref>,
    /// All fragments on this page, keyed by id.
    pub(super) fragments: HashMap<String, Fragment>,
    /// The ids of all gather elements on this page, in document order. This is
    /// an index into `targets` (a gather element is a `Target` whose
    /// `gather_ids` is non-empty), maintained so that the gather elements
    /// referencing a given fragment can be found without scanning every target.
    pub(super) gathers: Vec<String>,
}

/// How long after a file's modification time a stamp of it must be taken for
/// that time to be trusted; see `FileStamp::is_racy`. This spans FAT's 2-second
/// timestamp resolution plus some clock skew on network shares.
const RACY_WINDOW: Duration = Duration::from_secs(3);

/// A snapshot of a file's metadata, taken immediately before its contents are
/// read, which tells the consumer whether the cache still represents the file
/// without reading it again. Git's index stores the same snapshot of each
/// tracked file for the same purpose; see
/// [Use of index and Racy Git problem](https://github.com/git/git/blob/master/Documentation/technical/racy-git.adoc).
pub(super) struct FileStamp {
    /// The metadata compared against a later snapshot.
    stat: StatData,
    /// True if the file was modified within `RACY_WINDOW` of this snapshot. A
    /// file's timestamp advances only every few milliseconds -- every 2
    /// seconds, on FAT -- so a write which follows this snapshot within the
    /// same tick, without changing the file's length, changes its contents
    /// while leaving its metadata unchanged. A racy stamp therefore never
    /// matches, forcing the file to be read again; the stamp taken then is
    /// trustworthy once the file's last write is old enough. Git calls such
    /// entries "racily clean".
    is_racy: bool,
}

/// The parts of a file's metadata which change when its contents do. Unix
/// supplies the fields Git compares. On Windows, the change time and the file
/// index which would stand in for Unix's change time and inode are available
/// only in nightly Rust, leaving just the length and modification time. The
/// creation time can't take their place: NTFS caches a file's creation time for
/// a short period after the file is deleted or renamed away, then gives it to a
/// new file which takes that name -- precisely what an editor does when it
/// saves by writing a temporary file, then renaming it over the original.
/// Windows calls this file system tunneling; see the remarks in
/// [`FileSystemInfo.CreationTime`](https://learn.microsoft.com/en-us/dotnet/api/system.io.filesysteminfo.creationtime#remarks)
/// and Raymond Chen's
/// [The apocryphal history of file system tunnelling](https://devblogs.microsoft.com/oldnewthing/20050715-14/?p=34923).
#[derive(PartialEq, Eq)]
struct StatData {
    len: u64,
    /// The modification time; `None` if the platform doesn't report one. Two
    /// `None`s compare equal, which would report every same-length edit as no
    /// change, so `FileStamp::new` marks a stamp without a modification time as
    /// racy, which keeps it from ever matching. Compare these times only for
    /// equality, never order:
    /// [`SystemTime`](https://doc.rust-lang.org/std/time/struct.SystemTime.html)
    /// isn't monotonic, so a file saved after another may carry an earlier
    /// time.
    modified: Option<SystemTime>,
    /// The change time as (seconds, nanoseconds). Unlike the modification time,
    /// which `touch -d`, `cp -p`, `rsync -t`, and archive extraction set to
    /// arbitrary values, no program can set it.
    #[cfg(unix)]
    ctime: (i64, i64),
    /// The device and inode numbers, which change when an editor saves by
    /// renaming a new file over the old one.
    #[cfg(unix)]
    dev_ino: (u64, u64),
}

/// Everything the cache knows about one id: where (if anywhere) it's defined,
/// and which files must be rebuilt when that changes.
#[derive(Default)]
pub(super) struct IdEntry {
    /// The relationship between this id and the `Target`/gather element or
    /// `Fragment` which defines it.
    pub(super) state: IdState,
    /// Every file whose rendered output depends on this id. Note that this does
    /// *not* capture the reverse dependency of a `Fragment` on the gather
    /// elements which list it; see <xref ref="cc-P-g5o100jN"></xref> and
    /// `commit_file`.
    pub(super) dependents: HashSet<PathBuf>,
}

/// Given an id, this enumerates the possible relationships between that id and
/// the `Target`/gather element or `Fragment` which defines it.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) enum IdState {
    /// The expected relationship: this `id` maps to exactly one `Target`/gather
    /// element or `Fragment`.
    Single(PathBuf),
    /// Duplicate id: one id maps to multiple `Target`s/gather elements or
    /// `Fragment`s; the keys give a list of a files in which that id is
    /// defined, while the value indicates the number of definitions in that
    /// file.
    Multiple(HashMap<PathBuf, u32>),
    /// Ids that appeared in cross-references or gather elements but aren't
    /// defined by any file.
    #[default]
    Missing,
}

/// The status of a file from the cache's perspective.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) enum FileStatus {
    /// The file hasn't been processed: an entry's state until its first commit.
    #[default]
    Unknown,
    /// The file needs to be re-processed to update cross-references or gather
    /// elements.
    Outdated,
    /// The file has been processed.
    UpToDate,
}

/// Contains all information about a target: any HTML element with an id other
/// than a `<fragment>`, which is a `Fragment` instead. The id (which should be
/// globally unique within the project; duplicate ids are flagged as errors) is
/// the key of `FileEntry::targets`, and the containing file is the entry
/// holding this value, so neither is duplicated here.
pub(super) struct Target {
    /// The inner HTML of this element.
    pub(super) inner_html: String,
    /// The ids gathered, in `data-gather` order, if this is a gather element;
    /// empty otherwise.
    pub(super) gather_ids: Vec<String>,
    /// The line number of this target in its file. Always 0 until the
    /// pulldown-cmark HTML writer preserves line numbers. TODO: when the URL
    /// given to the Client includes an anchor, the Client must use the anchor
    /// to find the corresponding Target (the anchor is the id), then use this
    /// field to identify the line on which that id resides in order to scroll
    /// to the appropriate location in the document.
    pub(super) _line: usize,
}

/// This defines a cross reference to a `Target`.
pub(super) struct Xref {
    /// The id cross-referenced.
    pub(super) id: String,
}

/// This is a unique id that encompasses a series of code/doc blocks, always
/// starting with a doc block, which `GatherElement`s operate on. As with
/// `Target`, the id is the key of `FileEntry::fragments` and the containing
/// file is the entry holding this value.
pub(super) struct Fragment {
    /// The code/doc block content of this element rendered as HTML. This is
    /// empty until the fragment's doc blocks are finalized and the caller
    /// stores the result via `Cache::update_fragment_content`.
    pub(super) content: String,
    /// The line number of this `Fragment` in its file; see `Target::line`.
    pub(super) _line: usize,
}

// ### Facts
//
// Plain data collected while walking a file's DOM, then applied to the cache in
// a single transaction by `Cache::commit_file`. Keeping these free of DOM types
// lets the walk run without holding the cache lock.
/// All cacheable facts found in one file.
#[derive(Default)]
pub(super) struct FileFacts {
    /// Every element with an id (excluding fragments), in document order.
    /// Gather elements appear here too, distinguished by a non-empty
    /// `TargetFact::gather_ids`.
    pub(super) targets: Vec<TargetFact>,
    /// The destination id of every cross-reference, in document order.
    pub(super) xrefs: Vec<String>,
    /// Every fragment, in document order.
    pub(super) fragments: Vec<FragmentFact>,
}

/// A target found in the DOM; see `Target` for field documentation.
pub(super) struct TargetFact {
    /// The id of this target.
    pub(super) id: String,
    pub(super) inner_html: String,
    pub(super) line: usize,
    /// The index of the doc block which contains this `Target` in the vec of
    /// `CodeDocBlock`s for this file. This is pass-local -- the cache doesn't
    /// store it -- but the caller needs it to place hydrated content.
    pub(super) _doc_block_index: usize,
    /// The ids listed by this element's `data-gather` attribute; empty if this
    /// isn't a gather element.
    pub(super) gather_ids: Vec<String>,
}

/// A fragment found in the DOM; see `Fragment` for field documentation. The
/// fragment's content isn't known during the walk (doc blocks aren't finalized
/// yet), so it's stored later via `Cache::update_fragment_content`.
pub(super) struct FragmentFact {
    /// The id of this fragment.
    pub(super) id: String,
    pub(super) line: usize,
    /// The index of the first doc block of this `Fragment` in the vec of
    /// `CodeDocBlock`s for this file.
    pub(super) _doc_block_start_index: usize,
    /// The index of the last code/doc block of this `Fragment` in the vec of
    /// `CodeDocBlock`s for this file.
    pub(super) _code_doc_block_end_index: usize,
}

// ### Commit results
/// The result of committing one file's facts to the cache.
pub(super) struct CommitOutcome {
    /// Files (other than the committed file) whose rendered output is
    /// invalidated by this commit. Their status has already been set to
    /// `Outdated`, and each has been passed on to be re-rendered: to the
    /// connections showing it, or else to the consumer; see
    /// `Cache::mark_outdated`.
    #[cfg(test)]
    pub(super) outdated: HashSet<PathBuf>,
}

/// The result of looking up an id; borrows from the cache, so it must be used
/// while the cache lock is still held.
pub(super) enum IdResolution<'a> {
    /// The id names a target.
    Target {
        /// The file defining the target.
        path: &'a Path,
        target: &'a Target,
    },
    /// The id names a fragment.
    Fragment {
        /// The file defining the fragment.
        path: &'a Path,
        fragment: &'a Fragment,
    },
    /// No file defines this id.
    Missing,
    /// The id is defined more than once: a map from each defining file to the
    /// number of definitions it contains. Carrying the counts lets the caller
    /// report a duplicate within a single file.
    ///
    /// TODO: this is currently unused.
    #[allow(dead_code)]
    Multiple(&'a HashMap<PathBuf, u32>),
}

/// One gather element which lists a given fragment, as returned by
/// `Cache::gathers_referencing`; this is everything needed to render one of the
/// backlinks a `<fragment>` hydrates to.
pub(super) struct GatherBacklink<'a> {
    /// The file containing the gather element, which the backlink points into.
    pub(super) path: &'a Path,
    /// The gather element's id, which is the anchor the backlink points to.
    pub(super) id: &'a str,
    /// The gather element itself; its `inner_html` is the backlink's text.
    pub(super) target: &'a Target,
}

// ### Watcher, walker, and consumer
//
// The watcher reports changed paths to the walker, which forwards those its
// ignore files don't exclude -- and the files found by walking each directory
// reported -- to the consumer, which applies them to the cache. See `Cache
// population and maintenance` in the spec.
/// A path reported by the watcher. A rename is reported as the deletion of its
/// old path followed by the creation of its new one.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum WatcherPathState {
    /// A file which was created or modified, or a directory which was created.
    CreatedOrModified(PathBuf),
    /// A file or directory which was deleted.
    Deleted(PathBuf),
}

/// Messages sent to the consumer.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum ConsumerPathState {
    /// A path from the walker: a file a walk found, or a path the watcher
    /// reported.
    WatcherPathState(WatcherPathState),
    /// The walker is about to walk this directory. Every file the walk finds
    /// arrives before the matching `WalkFinished`.
    WalkStarted(PathBuf),
    /// The walk announced by the preceding `WalkStarted` is complete.
    WalkFinished,
    /// A request to bring a file the cache already has up to date: a commit
    /// outdated its rendered output (see `Cache::mark_outdated`), or a
    /// connection stopped showing it (see `ConnectionRegistration::drop`).
    /// Unlike a path from the walker, this is no evidence that the file isn't
    /// ignored, so it can't keep a walk in progress from forgetting the file,
    /// and is moot once the cache has forgotten it.
    Rescan(PathBuf),
}

// ### Connections
//
// A connection's translation task owns its current file; the cache keeps only a
// copy, refreshed by the task each time that file changes. See `Connections` in
// the cache spec.
/// One connection whose current file lies in this project.
struct ConnectionView {
    /// The file shown in the connection's Client, canonicalized to match the
    /// paths the cache stores.
    current_file: PathBuf,
    /// Where to notify the connection. This is unbounded, since the cache lock
    /// is held while sending.
    notify_tx: UnboundedSender<CacheNotification>,
}

/// Notifications the cache sends to a registered connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheNotification {
    /// A change elsewhere in the project outdated the rendered output of the
    /// connection's current file, which the connection should re-translate.
    CurrentFileOutdated,
    /// A change elsewhere in the project outdated the rendered output of the
    /// project's TOC, which every connection in the project shows in its
    /// sidebar; the connection should re-render it.
    TocOutdated,
    /// The contents of the TOC changed on disk, or the TOC was deleted; the
    /// connection should re-load then re-render it. (Since the IDE is editing
    /// the current file, changes made to it on disk are superseded by the IDE
    /// content, so that a `CurrentFileChanged` message is meaningless.)
    TocChanged,
}

/// Lists a connection in `Cache::connections` for as long as this value lives;
/// dropping it removes the listing.
pub struct ConnectionRegistration {
    /// The cache the connection is listed in. This is weak, so that only the
    /// `CacheMap` keeps a cache alive; the map in turn never evicts a cache
    /// while a connection is listed in it.
    cache: Weak<Mutex<Cache>>,
    /// The connection's key in `Cache::connections`.
    connection_id: String,
}

// Code
// ----
/// The capacity of the `CacheMap` when the Server has the given number of
/// connections; `translation_task` applies this as connections come and go. Per
/// the spec, this balances the memory the caches consume against the time
/// needed to load the cache of a project which isn't cached.
#[must_use]
pub fn cache_map_capacity(
    // The number of connections, $n$.
    connections: usize,
    // $nc + 1$, where $c$ is `LRU_ENTRIES_PER_CONNECTION`.
) -> NonZeroUsize {
    NonZeroUsize::MIN.saturating_add(connections.saturating_mul(LRU_ENTRIES_PER_CONNECTION))
}

/// Return the cache of the project rooted at `root`, first creating it if the
/// `CacheMap` has none.
pub fn get_or_insert_cache(
    // The contents of the locked `CacheMap`.
    caches: &mut ProjectCaches,
    // The project's root directory, as a canonicalized absolute path.
    root: &Path,
    // Creates the project's cache, if the map has none.
    create: impl FnOnce() -> Arc<Mutex<Cache>>,
    // The project's cache.
) -> Arc<Mutex<Cache>> {
    if let Some(cache) = caches.get(root) {
        return cache.clone();
    }
    // Inserting into a full map evicts its least recently used cache, even one
    // with connections, so make room first.
    evict_unconnected(caches, caches.cap().get() - 1);
    if caches.len() == caches.cap().get() {
        caches.resize(caches.cap().saturating_add(1));
    }
    let cache = create();
    caches.put(root.to_path_buf(), cache.clone());
    cache
}

/// Set the capacity of the `CacheMap`; `translation_task` applies
/// `cache_map_capacity` as connections come and go.
pub fn set_cache_map_capacity(
    // The contents of the locked `CacheMap`.
    caches: &mut ProjectCaches,
    // The new capacity.
    capacity: NonZeroUsize,
) {
    evict_unconnected(caches, capacity.get());
    // Shrinking the map evicts its least recently used caches, even those with
    // connections, so keep room for any which remain.
    caches.resize(capacity.max(NonZeroUsize::new(caches.len()).unwrap_or(capacity)));
}

/// Evict the least recently used caches in which no connection is registered,
/// until the `CacheMap` holds no more than `capacity` caches. Per the spec, a
/// cache with connections is never evicted: a `ConnectionRegistration` holds
/// only a weak reference to its cache, so evicting the cache would drop it
/// while its connections still depend on it. Since a connection registers with
/// at most one cache, and the map's capacity exceeds the number of connections
/// (see `cache_map_capacity`), this normally leaves the map within `capacity`.
fn evict_unconnected(
    // The contents of the locked `CacheMap`.
    caches: &mut ProjectCaches,
    // The number of caches to keep, if possible.
    capacity: usize,
) {
    let excess = caches.len().saturating_sub(capacity);
    let evicted: Vec<PathBuf> = caches
        .iter()
        .rev()
        .filter(|(_, cache)| {
            cache
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .connections
                .is_empty()
        })
        .map(|(root, _)| root.clone())
        .take(excess)
        .collect();
    for root in evicted {
        caches.pop(&root);
    }
}

impl Cache {
    /// Create an empty cache with no consumer to keep it current: the cache of
    /// a single-file "project", which consists only of that file, or of a test.
    /// A project's cache comes from `Cache::new_project`.
    #[must_use]
    pub fn new(
        // The root directory of this project, as a canonicalized absolute path;
        // all cached files must live within it. For a single-file "project",
        // this is that file.
        root: PathBuf,
        // The empty cache.
    ) -> Self {
        Cache {
            files: BTreeMap::new(),
            ids: HashMap::new(),
            root,
            consumer_tx: None,
            _watcher: None,
            connections: HashMap::new(),
        }
    }

    /// Create the cache of a project, then start the threads which keep it
    /// current: a watcher thread, which registers a watcher to report changes
    /// to the project's files, then becomes the walker, which filters those
    /// reports through the project's ignore files; and a consumer, which
    /// applies the walker's results to the cache. The walker's first task is a
    /// walk of the entire project, so that the consumer brings every file in it
    /// into the cache. See `Cache population and maintenance` in the spec.
    #[must_use]
    pub fn new_project(
        // The project's root directory, as a canonicalized absolute path; all
        // cached files must live within it.
        root: PathBuf,
        // The cache, shared with the threads which keep it current.
    ) -> Arc<Mutex<Self>> {
        let (consumer_tx, consumer_rx) = mpsc::channel();
        let walker_consumer_tx = consumer_tx.clone();
        let cache = Arc::new(Mutex::new(Cache {
            consumer_tx: Some(consumer_tx),
            ..Cache::new(root.clone())
        }));
        // Give these threads only weak references, so that they don't keep the
        // cache alive; see `Shutdown` in the spec.
        let weak_cache = Arc::downgrade(&cache);
        thread::spawn(move || consume(&weak_cache, &consumer_rx));
        let weak_cache = Arc::downgrade(&cache);
        thread::spawn(move || watch_then_walk(&weak_cache, root, walker_consumer_tx));
        cache
    }

    /// <h3 id="cc-MtbstS0OHd">commit_file</h3>
    ///
    /// Apply the facts collected from one file's DOM walk to the cache in a
    /// single transaction. This satisfies several requirements:
    ///
    /// * Determine if any files containing cross-references need to be rebuilt
    ///   due to changes in the `Target`s in this file: any target which was
    ///   added, deleted, or modified marks its dependent files outdated. Note
    ///   that "modified" refers only to the `Target` state that
    ///   cross-references depend on (its id and inner HTML).
    /// * Determine if any files containing gather elements need to be rebuilt
    ///   due to changes in the `Fragment`s in this file. Fragment additions and
    ///   deletions are handled here; content changes are detected by
    ///   `update_fragment_content`, since a fragment's rendered content is only
    ///   known after doc block processing completes.
    /// * Determine if any files defining a gathered `Fragment` need to be
    ///   rebuilt due to changes in the gather elements which list it -- the
    ///   reverse edge described below.
    ///
    /// Because cross-references carry no id to match them against their
    /// previous versions, the diff instead unlinks all of the old version's
    /// outgoing references, then links all of the new version's. For
    /// cross-references this causes no spurious rebuilds, because their
    /// dependency sets are only ever mutated by that round trip and never used
    /// to detect change: the target's file renders nothing that depends on who
    /// references it.
    ///
    /// A gather element's outgoing references are different, and cannot be
    /// maintained that way. Each fragment it lists renders a backlink to it, so
    /// the reference set *is* observable output in the fragment's file, and the
    /// round trip would erase the information needed to detect a change in it.
    /// The old and new `gather_ids` are therefore diffed explicitly, before the
    /// old ones are overwritten, and the file defining each affected fragment
    /// is outdated:
    ///
    /// * If the gather element's inner HTML changed (including when the element
    ///   was added or deleted, which counts as a change to or from "no inner
    ///   HTML"), every id in the union of the old and new lists is affected --
    ///   that inner HTML is the text of the backlink each of them renders.
    /// * Otherwise only the symmetric difference is affected: the fragments
    ///   which gained or lost this backlink.
    ///
    /// See <xref ref="cc-P-g5o100jN"></xref> for why this doesn't cycle back.
    #[allow(clippy::too_many_lines)]
    pub(super) fn commit_file(
        &mut self,
        // The file whose facts these are.
        path: &Path,
        // The file's stamp, taken when its content was read; `None` if its
        // metadata is unavailable. Take this *before* locking the cache, so no
        // I/O happens while the lock is held.
        stamp: Option<FileStamp>,
        // The facts collected from the file's DOM.
        facts: FileFacts,
        // The outcome: outdated files. Reprocessing these outdated files may
        // reveal additional outdated files.
    ) -> CommitOutcome {
        let mut outdated: HashSet<PathBuf> = HashSet::new();

        // ### Take the old state
        //
        // Move the previous version out of the entry (creating the entry if
        // this is the first commit for this file), so the new version can be
        // diffed against it. Everything the diff needs must be read from these
        // locals; the cache no longer holds the old version.
        let entry = self.files.entry(path.to_path_buf()).or_default();
        let old_targets = mem::take(&mut entry.targets);
        let old_fragments = mem::take(&mut entry.fragments);
        let old_gathers = mem::take(&mut entry.gathers);
        let old_xrefs = mem::take(&mut entry.xrefs);

        // ### Unlink the old version's outgoing references
        //
        // Remove this file from the dependents of every id its previous version
        // referenced; the new version's references are linked below. This is
        // only bookkeeping -- the gather diff further below reads
        // `old_targets`, not these sets.
        let old_referenced_ids: Vec<String> = old_xrefs
            .iter()
            .map(|xref| xref.id.clone())
            .chain(gathered_ids(&old_gathers, &old_targets).cloned())
            .collect();
        for id in &old_referenced_ids {
            self.unlink_reference(id, path);
        }

        // ### Build the new definitions
        //
        // Targets and fragments share one id namespace, so definitions of both
        // are counted together; the count is what distinguishes
        // `IdState::Single` from `IdState::Multiple`. A duplicate within this
        // file has nowhere to live in the id-keyed maps, so the first
        // definition in document order is the one stored; every reference to a
        // duplicated id renders an error, so which one is stored only matters
        // once the duplicate is resolved, at which point this file is
        // recommitted anyway.
        let mut new_counts: HashMap<String, u32> = HashMap::new();
        let mut new_targets: HashMap<String, Target> = HashMap::with_capacity(facts.targets.len());
        let mut new_gathers: Vec<String> = Vec::new();
        for fact in facts.targets {
            *new_counts.entry(fact.id.clone()).or_default() += 1;
            if new_targets.contains_key(&fact.id) {
                continue;
            }
            if !fact.gather_ids.is_empty() {
                new_gathers.push(fact.id.clone());
            }
            new_targets.insert(
                fact.id,
                Target {
                    inner_html: fact.inner_html,
                    gather_ids: fact.gather_ids,
                    _line: fact.line,
                },
            );
        }
        let mut new_fragments: HashMap<String, Fragment> =
            HashMap::with_capacity(facts.fragments.len());
        for fact in facts.fragments {
            *new_counts.entry(fact.id.clone()).or_default() += 1;
            if new_fragments.contains_key(&fact.id) {
                continue;
            }
            // A surviving fragment keeps its old content until the caller
            // supplies the new content via `update_fragment_content`, which
            // also detects content changes.
            let content = old_fragments
                .get(&fact.id)
                .map_or_else(String::new, |old_fragment| old_fragment.content.clone());
            new_fragments.insert(
                fact.id,
                Fragment {
                    content,
                    _line: fact.line,
                },
            );
        }

        // The set of ids the new version references, saved before `new_targets`
        // is moved into the entry; used to link references below.
        let new_refs: HashSet<String> = facts
            .xrefs
            .iter()
            .cloned()
            .chain(gathered_ids(&new_gathers, &new_targets).cloned())
            .collect();

        // ### Update the definition side of `ids`
        //
        // For every id this file defined before or defines now, replace this
        // file's contribution to the id's definition map. Any id whose
        // `IdState` changes -- appearing, disappearing, moving, or
        // becoming/ceasing to be a duplicate -- outdates every file which
        // references it.
        let affected_ids: HashSet<&String> = old_targets
            .keys()
            .chain(old_fragments.keys())
            .chain(new_counts.keys())
            .collect();
        // The ids this commit left without any definition, which are forgotten
        // below unless something references them.
        let mut undefined_ids: Vec<String> = Vec::new();
        for id in affected_ids {
            let id_entry = self.ids.entry(id.clone()).or_default();
            let mut definitions = id_entry.state.definitions();
            match new_counts.get(id) {
                Some(&count) => definitions.insert(path.to_path_buf(), count),
                None => definitions.remove(path),
            };
            let new_state = IdState::from_definitions(definitions);
            if id_entry.state != new_state {
                id_entry.state = new_state;
                outdated.extend(id_entry.dependents.iter().cloned());
            }
            if id_entry.state == IdState::Missing {
                undefined_ids.push(id.clone());
            }
        }

        // ### Outdate the dependents of changed definitions
        //
        // `IdState` can't see two changes which leave the defining file the
        // same: a target's inner HTML changing, and an id changing kind between
        // target and fragment (which turns a cross-reference into an error, or
        // back). A fragment's content isn't known yet;
        // `update_fragment_content` handles it.
        let changed_ids = new_targets
            .iter()
            .filter(|(id, new_target)| {
                old_targets.get(*id).map_or_else(
                    || old_fragments.contains_key(*id),
                    |old_target| old_target.inner_html != new_target.inner_html,
                )
            })
            .map(|(id, _)| id)
            .chain(
                new_fragments
                    .keys()
                    .filter(|id| old_targets.contains_key(*id)),
            );
        let mut changed_dependents: HashSet<PathBuf> = HashSet::new();
        for id in changed_ids {
            if let Some(id_entry) = self.ids.get(id) {
                changed_dependents.extend(id_entry.dependents.iter().cloned());
            }
        }
        outdated.extend(changed_dependents);

        // ### Outdate the files defining fragments this file gathers
        //
        // The reverse edge; see this function's documentation. Outdate the
        // file(s) defining each fragment whose backlinks changed. An id with no
        // definition needs nothing: whenever it does appear, the file defining
        // it renders its own backlinks in that same pass.
        let mut regathered_files: HashSet<PathBuf> = HashSet::new();
        for id in regathered_ids(&old_gathers, &old_targets, &new_gathers, &new_targets) {
            match self.ids.get(id).map(|id_entry| &id_entry.state) {
                Some(IdState::Single(owner)) => {
                    regathered_files.insert(owner.clone());
                }
                Some(IdState::Multiple(definitions)) => {
                    regathered_files.extend(definitions.keys().cloned());
                }
                Some(IdState::Missing) | None => {}
            }
        }
        outdated.extend(regathered_files);

        // ### Store the new state
        //
        // Note that ids auto-assigned during this pass are deliberately absent
        // from `facts`: they live only in the HTML sent to the Client until the
        // file is written and re-read (see `Auto-assignment of ids`), at which
        // point they arrive here like any other id.
        let entry = self.files.get_mut(path).expect("entry was created above");
        entry.stamp = stamp;
        entry.targets = new_targets;
        entry.fragments = new_fragments;
        entry.gathers = new_gathers;
        entry.xrefs = facts.xrefs.into_iter().map(|id| Xref { id }).collect();
        entry.status = FileStatus::UpToDate;

        // ### Link the new version's outgoing references
        //
        // Add this file to the dependents of every id it references, creating a
        // `Missing` entry for ids nothing defines yet.
        for id in new_refs {
            self.ids
                .entry(id)
                .or_default()
                .dependents
                .insert(path.to_path_buf());
        }

        // ### Forget unused ids
        //
        // Only now, with this file's references linked, is it known which of
        // the ids it stopped defining nothing references.
        for id in &undefined_ids {
            self.remove_if_unused(id);
        }

        // ### Mark outdated files
        //
        // A file never outdates itself: its rendered output was just produced
        // from the state committed here.
        outdated.remove(path);
        self.mark_outdated(&outdated);

        CommitOutcome {
            #[cfg(test)]
            outdated,
        }
    }

    /// Remove everything the cache knows about a file -- when the file is
    /// deleted or moved out of the project, for example. Committing an empty
    /// set of facts unlinks every reference the file made and withdraws every
    /// definition it provided, outdating the files which depended on them; the
    /// file's now-empty entry is then dropped.
    pub(super) fn remove_file(
        &mut self,
        // The file to forget.
        path: &Path,
        // The outcome: files outdated by the removal.
    ) -> CommitOutcome {
        let outcome = self.commit_file(path, None, FileFacts::default());
        self.files.remove(path);
        outcome
    }

    /// Generate an id for an element which requests one (`id="*"`). The result
    /// is deliberately *not* recorded in the cache: it enters the cache only
    /// once the file holding it is written and re-read, so that a write which
    /// never happens (or fails) can't leave the cache describing an id no file
    /// contains. See `Auto-assignment of ids`.
    pub(super) fn new_id(
        &self,
        // Ids already assigned during this pass, which aren't in the cache and
        // so can't be found there.
        assigned: &HashSet<String>,
        // A random id which collides with no id the cache knows of, and which
        // is a valid CSS identifier.
    ) -> String {
        // The characters a generated id may contain; all are valid in a CSS
        // identifier. There are exactly 64 of them, so each character consumes
        // 6 bits of randomness with no modulo bias.
        const ID_CHARS: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        // The number of random characters in a generated id. At 6 bits each,
        // these fit in the 64 bits a single `random` call provides.
        const ID_LEN: usize = 10;

        loop {
            // The `cc-` prefix keeps the identifier from starting with a digit
            // or a hyphen, neither of which CSS identifiers may do.
            let mut id = String::from("cc-");
            let mut bits = random::<u64>();
            for _ in 0..ID_LEN {
                id.push(ID_CHARS[(bits & 0x3f) as usize] as char);
                bits >>= 6;
            }
            if !self.ids.contains_key(&id) && !assigned.contains(&id) {
                return id;
            }
        }
    }

    /// Store a fragment's rendered content, once the caller has finalized its
    /// doc blocks. If the content changed, all files containing gather elements
    /// which reference the fragment are marked outdated.
    pub(super) fn update_fragment_content(
        &mut self,
        // The file containing the fragment.
        path: &Path,
        // The fragment's id.
        id: &str,
        // The fragment's code/doc block content, rendered as HTML.
        content: String,
        // The files (other than `path`) marked outdated by this change, as
        // described by `CommitOutcome::outdated`.
    ) -> HashSet<PathBuf> {
        // Apply the new content first, so that no borrow of `self.files`
        // outlives the check; report whether it differed from the old content.
        let changed = self
            .files
            .get_mut(path)
            .and_then(|entry| entry.fragments.get_mut(id))
            .is_some_and(|fragment| {
                let changed = fragment.content != content;
                if changed {
                    fragment.content = content;
                }
                changed
            });
        if !changed {
            return HashSet::new();
        }
        let mut outdated = self
            .ids
            .get(id)
            .map_or_else(HashSet::new, |id_entry| id_entry.dependents.clone());
        // Gather elements in the fragment's own file are updated by the caller
        // in the same processing pass.
        outdated.remove(path);
        self.mark_outdated(&outdated);
        outdated
    }

    /// <h3 id="cc-MxbstS0OHd">mark_outdated</h3>
    ///
    /// Mark files outdated, then see that each is re-rendered. A connection
    /// re-renders a file it shows -- its current file, or the TOC in its
    /// sidebar, which every connection in the project shows -- since the text
    /// it shows may be newer than the file on disk; the consumer reprocesses
    /// the rest from disk.
    fn mark_outdated(
        &mut self,
        // The files to mark.
        outdated: &HashSet<PathBuf>,
    ) {
        for path in outdated {
            if let Some(entry) = self.files.get_mut(path) {
                entry.status = FileStatus::Outdated;
            }
        }
        self.notify_current_file_outdated(outdated);
        if outdated.contains(&self.toc_path()) {
            self.notify_toc(CacheNotification::TocOutdated);
        }
        if let Some(consumer_tx) = &self.consumer_tx {
            for path in outdated.iter().filter(|path| !self.is_shown(path)) {
                // A send fails only once the consumer has exited, when this
                // cache is being dropped and so needs no more updates.
                let _ = consumer_tx.send(ConsumerPathState::Rescan(path.clone()));
            }
        }
    }

    /// Notify each connection whose current file is one of these files, so that
    /// the connection re-translates it. Several connections may share a current
    /// file, if it's being edited in each simultaneously.
    fn notify_current_file_outdated(
        &self,
        // The files whose rendered output is outdated.
        outdated: &HashSet<PathBuf>,
    ) {
        for connection in self.connections.values() {
            if outdated.contains(&connection.current_file) {
                // A send fails only once the connection's translation task has
                // exited, leaving nothing to notify.
                let _ = connection
                    .notify_tx
                    .send(CacheNotification::CurrentFileOutdated);
            }
        }
    }

    /// Notify every connection about the TOC, which every connection in the
    /// project shows in its sidebar.
    fn notify_toc(
        &self,
        // `TocOutdated` if only the TOC's rendering is outdated; `TocChanged`
        // if its text changed.
        notification: CacheNotification,
    ) {
        for connection in self.connections.values() {
            // A send fails only once the connection's translation task has
            // exited, leaving nothing to notify.
            let _ = connection.notify_tx.send(notification);
        }
    }

    /// The project's TOC: the project's root is the directory containing it;
    /// see `find_path_to_toc`.
    fn toc_path(
        &self,
        // The TOC's path.
    ) -> PathBuf {
        self.root.join("toc.md")
    }

    /// Report whether a connection shows this file, which makes the connection
    /// responsible for re-rendering it: it's a connection's current file, or
    /// it's the TOC, which every connection's sidebar shows.
    fn is_shown(
        &self,
        // The file to check.
        path: &Path,
        // True if a connection shows the file.
    ) -> bool {
        self.is_current_file(path) || (!self.connections.is_empty() && path == self.toc_path())
    }

    /// Report whether this file is some connection's current file.
    fn is_current_file(
        &self,
        // The file to check.
        path: &Path,
        // True if it's some connection's current file.
    ) -> bool {
        self.connections
            .values()
            .any(|connection| connection.current_file == path)
    }

    /// Iterate over the cached files beneath a directory, which immediately
    /// follow the directory itself in `files`.
    fn files_beneath<'a>(
        &'a self,
        // The directory.
        dir: &'a Path,
        // The cached files beneath it, at any depth.
    ) -> impl Iterator<Item = &'a PathBuf> {
        self.files
            .range::<Path, _>((Bound::Excluded(dir), Bound::Unbounded))
            .map(|(path, _)| path)
            .take_while(move |path| path.starts_with(dir))
    }

    /// Forget a file which no longer exists, or which the walker now ignores --
    /// unless a connection shows it. The connection's IDE may still hold the
    /// file's text, so instead have the connection re-translate that text,
    /// which keeps the file in the cache; for the TOC, every connection
    /// re-loads it.
    fn forget_file(
        &mut self,
        // The file to forget.
        path: &Path,
    ) {
        if self.is_shown(path) {
            self.notify_current_file_outdated(&HashSet::from([path.to_path_buf()]));
            if path == self.toc_path() {
                self.notify_toc(CacheNotification::TocChanged);
            }
        } else if self.files.contains_key(path) {
            self.remove_file(path);
        }
    }

    /// Look up an id, returning the target or fragment it names along with the
    /// defining file, `Missing` if no file defines it, or `Multiple` if more
    /// than one definition exists.
    pub(super) fn resolve_id(&self, id: &str) -> IdResolution<'_> {
        let Some(id_entry) = self.ids.get(id) else {
            return IdResolution::Missing;
        };
        match &id_entry.state {
            IdState::Missing => IdResolution::Missing,
            IdState::Multiple(definitions) => IdResolution::Multiple(definitions),
            IdState::Single(owner) => {
                if let Some(entry) = self.files.get(owner) {
                    if let Some(target) = entry.targets.get(id) {
                        return IdResolution::Target {
                            path: owner,
                            target,
                        };
                    }
                    if let Some(fragment) = entry.fragments.get(id) {
                        return IdResolution::Fragment {
                            path: owner,
                            fragment,
                        };
                    }
                }
                IdResolution::Missing
            }
        }
    }

    /// Find the gather elements which list the given fragment id -- the reverse
    /// of `Target::gather_ids`, and the data a `<fragment>` needs to hydrate
    /// its backlinks. `commit_file` outdates this fragment's file whenever this
    /// result would change.
    pub(super) fn gathers_referencing(
        &self,
        // The fragment id to find gather elements for.
        id: &str,
        // Every gather element listing `id`, ordered by file then by gather id
        // so that the rendered backlinks are deterministic -- `dependents` is a
        // `HashSet`, whose iteration order is not.
    ) -> Vec<GatherBacklink<'_>> {
        let Some(id_entry) = self.ids.get(id) else {
            return Vec::new();
        };
        let mut backlinks = Vec::new();
        // Dependents include files which merely cross-reference the id, so each
        // candidate file's gather elements must still be checked.
        for dependent in &id_entry.dependents {
            let Some(entry) = self.files.get(dependent) else {
                continue;
            };
            for gather_id in &entry.gathers {
                if let Some(target) = entry.targets.get(gather_id)
                    && target.gather_ids.iter().any(|gathered| gathered == id)
                {
                    backlinks.push(GatherBacklink {
                        path: dependent,
                        id: gather_id,
                        target,
                    });
                }
            }
        }
        backlinks.sort_unstable_by(|a, b| (a.path, a.id).cmp(&(b.path, b.id)));
        backlinks
    }

    /// Remove this file from the dependents of the given id.
    fn unlink_reference(
        &mut self,
        // The referenced id.
        id: &str,
        // The file which contained the reference.
        referrer: &Path,
    ) {
        if let Some(id_entry) = self.ids.get_mut(id) {
            id_entry.dependents.remove(referrer);
        }
        self.remove_if_unused(id);
    }

    /// Forget an id which nothing defines and nothing references. Its entry
    /// carries no information, so keeping it would accumulate an entry for
    /// every id ever mistyped, deleted, or renamed.
    fn remove_if_unused(
        &mut self,
        // The id to check.
        id: &str,
    ) {
        if self.ids.get(id).is_some_and(|id_entry| {
            id_entry.state == IdState::Missing && id_entry.dependents.is_empty()
        }) {
            self.ids.remove(id);
        }
    }
}

impl IdState {
    /// Every file defining this id, mapped to the number of definitions it
    /// contains -- the representation-independent form of an `IdState`, used to
    /// add or remove one file's definitions.
    fn definitions(&self) -> HashMap<PathBuf, u32> {
        match self {
            IdState::Missing => HashMap::new(),
            IdState::Single(path) => HashMap::from([(path.clone(), 1)]),
            IdState::Multiple(definitions) => definitions.clone(),
        }
    }

    /// Reduce a definition map to an `IdState`. Normalizing here (rather than
    /// leaving, say, a one-entry `Multiple`) is what lets `commit_file` detect
    /// a state change by comparing the old and new states.
    fn from_definitions(definitions: HashMap<PathBuf, u32>) -> Self {
        let mut entries = definitions.iter();
        match (entries.next(), entries.next()) {
            (None, _) => IdState::Missing,
            (Some((path, 1)), None) => IdState::Single(path.clone()),
            _ => IdState::Multiple(definitions),
        }
    }
}

impl FileStamp {
    pub(super) fn new(
        // The file's metadata, which this converts to a `FileStamp`.
        metadata: &Metadata,
    ) -> Self {
        let modified = metadata.modified().ok();
        // A modification time in the future -- from clock skew between this
        // machine and a network share, for example -- is racy as well.
        let is_racy = modified.is_none_or(|modified| {
            !SystemTime::now()
                .duration_since(modified)
                .is_ok_and(|age| age >= RACY_WINDOW)
        });
        FileStamp {
            stat: StatData {
                len: metadata.len(),
                modified,
                #[cfg(unix)]
                ctime: (metadata.ctime(), metadata.ctime_nsec()),
                #[cfg(unix)]
                dev_ino: (metadata.dev(), metadata.ino()),
            },
            is_racy,
        }
    }

    /// Report whether the file's contents are unchanged since this stamp was
    /// taken.
    fn is_unmodified(
        &self,
        // A stamp taken from the file now.
        current: &FileStamp,
        // True if the contents are unchanged; false if they may have changed.
    ) -> bool {
        !self.is_racy && self.stat == current.stat
    }
}

impl ConnectionRegistration {
    /// List a connection in a project's cache. Drop any earlier registration of
    /// this connection first: dropping it afterward would remove this listing,
    /// since both share one key.
    #[must_use]
    pub fn new(
        // The cache of the project containing `current_file`.
        cache: &Arc<Mutex<Cache>>,
        // The connection's id, unique among all connections.
        connection_id: String,
        // The file shown in the connection's Client, as a canonicalized
        // absolute path.
        current_file: PathBuf,
        // Where the cache sends this connection's notifications.
        notify_tx: UnboundedSender<CacheNotification>,
    ) -> Self {
        cache.lock().unwrap().connections.insert(
            connection_id.clone(),
            ConnectionView {
                current_file,
                notify_tx,
            },
        );
        ConnectionRegistration {
            cache: Arc::downgrade(cache),
            connection_id,
        }
    }
}

impl Drop for ConnectionRegistration {
    fn drop(&mut self) {
        // The `CacheMap` keeps a cache with connections alive, so this fails
        // only once the map itself is gone, taking the listing with it.
        let Some(cache) = self.cache.upgrade() else {
            return;
        };
        // Tolerate a poisoned lock: this may run while unwinding from another
        // panic, where a second panic aborts the process.
        let mut cache = cache.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(connection) = cache.connections.remove(&self.connection_id) else {
            return;
        };
        // The consumer leaves a connection's current file to that connection,
        // so it may have skipped a change to this file on disk, or its
        // deletion. Once no connection shows the file, have the consumer check
        // the file against the disk. If the file on disk is unchanged, what the
        // connection committed from its IDE's text stays in the cache, since
        // the IDE may still hold that text.
        if !cache.is_current_file(&connection.current_file)
            && let Some(consumer_tx) = &cache.consumer_tx
        {
            let _ = consumer_tx.send(ConsumerPathState::Rescan(connection.current_file));
        }
    }
}

/// Diff a file's old and new gather elements, reporting the ids whose rendered
/// backlinks changed as a result -- the reverse edge described in
/// `commit_file`, whose documentation gives the rule applied here. The file
/// defining each id returned must be outdated.
fn regathered_ids<'a>(
    // The gather elements the file used to contain, and the targets they index
    // into.
    old_gathers: &'a [String],
    old_targets: &'a HashMap<String, Target>,
    // The gather elements the file now contains, and the targets they index
    // into.
    new_gathers: &'a [String],
    new_targets: &'a HashMap<String, Target>,
    // The ids which gained, lost, or changed a backlink.
) -> HashSet<&'a String> {
    let mut regathered: HashSet<&String> = HashSet::new();
    for gather_id in old_gathers.iter().chain(new_gathers.iter()) {
        let old_gather = old_targets.get(gather_id);
        let new_gather = new_targets.get(gather_id);
        let old_ids = old_gather.map_or(&[][..], |target| &target.gather_ids);
        let new_ids = new_gather.map_or(&[][..], |target| &target.gather_ids);
        if old_gather.map(|target| &target.inner_html)
            == new_gather.map(|target| &target.inner_html)
        {
            // The backlink text is unchanged, so only ids which gained or lost
            // this gather element are affected.
            let old_set: HashSet<&String> = old_ids.iter().collect();
            let new_set: HashSet<&String> = new_ids.iter().collect();
            regathered.extend(old_set.symmetric_difference(&new_set).copied());
        } else {
            // The backlink text changed (or the gather element was added or
            // deleted), so every id on either list must re-render.
            regathered.extend(old_ids.iter().chain(new_ids.iter()));
        }
    }
    regathered
}

/// Iterate over the ids gathered by the given gather elements, which must all
/// be targets in `targets`.
fn gathered_ids<'a>(
    // The ids of the gather elements to read, i.e. a `FileEntry::gathers`.
    gathers: &'a [String],
    // The targets those ids index into, i.e. the matching `FileEntry::targets`.
    targets: &'a HashMap<String, Target>,
    // Every id listed by those gather elements, with duplicates retained.
) -> impl Iterator<Item = &'a String> {
    gathers
        .iter()
        .filter_map(|gather_id| targets.get(gather_id))
        .flat_map(|target| target.gather_ids.iter())
}

// ### Watcher
/// The watcher thread: register a watcher for the project and give it to the
/// cache, then become the walker, starting with a walk of the entire project.
/// Registering the watcher can take a while -- on some platforms, it visits
/// every file in the project -- so this runs on its own thread, spawned by
/// `Cache::new_project`, rather than delaying the translation which created the
/// cache.
fn watch_then_walk(
    // The cache which owns the watcher.
    cache: &Weak<Mutex<Cache>>,
    // The project's root directory.
    root: PathBuf,
    // Where the walker sends what it finds.
    consumer_tx: Sender<ConsumerPathState>,
) {
    let (walker_tx, walker_rx) = mpsc::channel();
    // Start watching before walking, so that a file created during the walk is
    // reported by one or the other. A watcher failure isn't fatal: the cache
    // still works, but won't learn of files which change after the walk. On
    // Linux, this happens once the per-user inotify limits are reached, since
    // each watcher uses its own inotify instance.
    let watcher = watch_project(&root, walker_tx.clone())
        .inspect_err(|err| warn!("Unable to watch {}: {err}", root.display()))
        .ok();
    // The cache owns the watcher, so that dropping the cache stops the watch. A
    // cache which is already gone leaves nothing to walk for; returning drops
    // the watcher, stopping the watch.
    let Some(cache) = cache.upgrade() else {
        return;
    };
    // The field's leading underscore marks it as held only for what dropping it
    // does; this is the one place which sets it.
    #[allow(clippy::used_underscore_binding)]
    {
        cache.lock().unwrap()._watcher = watcher;
    }
    drop(cache);
    // Give the walker its first task, then drop this sender. Only the watcher
    // holds another, so if it failed, the walker exits after this walk. The
    // send can't fail, since `walker_rx` is alive.
    let _ = walker_tx.send(WatcherPathState::CreatedOrModified(root.clone()));
    drop(walker_tx);
    Walker::new(root, consumer_tx).run(&walker_rx);
}

/// Watch `root` recursively, sending the walker each path created, modified, or
/// deleted within it. The event handler runs on the debouncer's thread.
fn watch_project(
    // The project's root directory.
    root: &Path,
    // Where to send each path reported; the walker owns the receiving end.
    walker_tx: Sender<WatcherPathState>,
    // The running watcher; dropping it stops the watch.
) -> notify::Result<Debouncer<RecommendedWatcher, RecommendedCache>> {
    let handler_root = root.to_path_buf();
    // The watcher only needs to learn of files added to, removed from, or
    // modified in the project. notify 8 has no `Config::with_event_kinds`, so
    // `watcher_path_states` must discard the events which don't matter.
    //
    // TODO: pass the kinds which matter to `Config::with_event_kinds` once
    // notify-debouncer-full depends on notify 9.
    let mut debouncer = new_debouncer_opt::<_, RecommendedWatcher, RecommendedCache>(
        Duration::from_secs(2),
        None,
        move |result: DebounceEventResult| match result {
            Ok(debounced_events) => {
                for path_state in debounced_events
                    .iter()
                    .flat_map(|debounced_event| watcher_path_states(debounced_event, &handler_root))
                {
                    // A send fails only once the walker has exited, when the
                    // cache is being dropped -- along with this watcher.
                    if walker_tx.send(path_state).is_err() {
                        return;
                    }
                }
            }
            // A path which can't be watched isn't usable by the IDE either, so
            // report this only to the log.
            Err(errors) => {
                for err in errors {
                    warn!("Unable to watch {}: {err}", handler_root.display());
                }
            }
        },
        RecommendedCache::new(),
        // Per the spec, symlinks aren't followed, since a file reached through
        // one would be cached twice. The walker skips a symlink the watcher
        // reports; this keeps the watcher from reporting the files beneath a
        // symlinked directory, which on some platforms it would otherwise
        // watch.
        notify::Config::default().with_follow_symlinks(false),
    )?;
    debouncer.watch(root, RecursiveMode::Recursive)?;
    Ok(debouncer)
}

/// Translate one watcher event into the paths it reports to the walker: per the
/// spec, the files created, modified, or deleted, and the directories created
/// or deleted.
fn watcher_path_states(
    // The event to translate.
    event: &Event,
    // The project's root directory, which is re-walked if the watcher lost
    // events.
    root: &Path,
    // The paths to report, in order.
) -> Vec<WatcherPathState> {
    use WatcherPathState::{CreatedOrModified, Deleted};

    if event.need_rescan() {
        return vec![CreatedOrModified(root.to_path_buf())];
    }
    let mut paths = event.paths.iter().cloned();
    match event.kind {
        // The debouncer stitches the two halves of a rename into one event,
        // listing the path renamed from, then the path renamed to, when it can
        // match them; otherwise, it passes each half through on its own, as a
        // deletion or a creation.
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => paths
            .next()
            .map(Deleted)
            .into_iter()
            .chain(paths.map(CreatedOrModified))
            .collect(),
        EventKind::Create(_) | EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
            paths.map(CreatedOrModified).collect()
        }
        EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
            paths.map(Deleted).collect()
        }
        // The platform couldn't tell which half of a rename this is, so ask the
        // filesystem.
        EventKind::Modify(ModifyKind::Name(_)) => paths
            .map(|path| {
                if path.exists() {
                    CreatedOrModified(path)
                } else {
                    Deleted(path)
                }
            })
            .collect(),
        // A directory is modified whenever its contents change, which are
        // reported in their own right, so skip directories here.
        EventKind::Modify(_) | EventKind::Any => paths
            .filter(|path| !path.is_dir())
            .map(CreatedOrModified)
            .collect(),
        EventKind::Access(_) | EventKind::Other => vec![],
    }
}

// ### Walker
/// The walker, which filters the paths the watcher reports through the
/// project's ignore files and walks each directory reported, sending the
/// results to the consumer. See `Walker` in the spec.
struct Walker {
    /// The project's root directory.
    root: PathBuf,
    /// Decides whether a path the watcher reports is ignored, without walking
    /// the project. It keeps the ignore files it reads, so it must be rebuilt
    /// when one of them changes.
    ignore: IncrementalIgnore,
    /// Where to send the files found.
    consumer_tx: Sender<ConsumerPathState>,
}

impl Walker {
    /// Create the walker for a project.
    fn new(
        // The project's root directory.
        root: PathBuf,
        // Where to send the files found.
        consumer_tx: Sender<ConsumerPathState>,
        // The walker, which reads the project's ignore files only as it needs
        // them.
    ) -> Self {
        Walker {
            ignore: build_ignore(&root),
            root,
            consumer_tx,
        }
    }

    /// Process each path the watcher sends, until the walker's channel
    /// closes -- when the cache is dropped, which drops the watcher, or after
    /// the first walk if the watcher couldn't start -- or the consumer exits,
    /// closing its own channel. This runs on the watcher thread; see
    /// `watch_then_walk`.
    fn run(
        mut self,
        // The paths the watcher sends.
        walker_rx: &Receiver<WatcherPathState>,
    ) {
        while let Ok(path_state) = walker_rx.recv() {
            if self.process(path_state).is_err() {
                return;
            }
        }
    }

    /// Process one path the watcher sent, sending the results to the consumer.
    fn process(
        &mut self,
        // The path to process.
        path_state: WatcherPathState,
        // An error once the consumer has exited.
    ) -> Result<(), SendError<ConsumerPathState>> {
        let (WatcherPathState::CreatedOrModified(path) | WatcherPathState::Deleted(path)) =
            &path_state;
        // An ignore file is hidden, which makes it ignored itself, so look for
        // one first.
        if let Some(dir) = self.rewalk_dir(path) {
            return self.walk(&dir);
        }
        if let WatcherPathState::CreatedOrModified(path) = &path_state
            && let Ok(metadata) = fs::symlink_metadata(path)
        {
            // Per the spec, symlinks aren't walked, since a file reached
            // through one would be cached twice.
            if metadata.is_dir() {
                return self.walk(path);
            }
            if metadata.is_symlink() {
                return Ok(());
            }
        }
        // Nothing says whether a deleted path was a file or a directory.
        // Checking it as a file errs toward sending it to the consumer, which
        // costs only a search of the cache for files which aren't there.
        if self.is_ignored(path, false) {
            Ok(())
        } else {
            self.consumer_tx
                .send(ConsumerPathState::WatcherPathState(path_state))
        }
    }

    /// If `path` is one of the ignore files which the walk obeys, return the
    /// directory to re-walk, which reloads the ignore files: the directory an
    /// `.ignore` or `.gitignore` governs, or the entire project for
    /// `.git/info/exclude`. A file with an ignore file's name prompts this even
    /// where Git wouldn't read it -- outside a repository, for example -- which
    /// costs only an unneeded walk.
    fn rewalk_dir(
        &self,
        // The path to check.
        path: &Path,
        // The directory to re-walk, or `None` if this isn't an ignore file.
    ) -> Option<PathBuf> {
        let dir = path.parent()?;
        match path.file_name()?.to_str()? {
            ".ignore" | ".gitignore" => Some(dir.to_path_buf()),
            "exclude" if dir.ends_with(Path::new(".git").join("info")) => Some(self.root.clone()),
            _ => None,
        }
    }

    /// Report whether the walk skips this path: it's hidden or excluded by an
    /// ignore file, or it lies within a directory which is.
    fn is_ignored(
        &mut self,
        // The path to check.
        path: &Path,
        // True if the path is a directory, which some ignore rules match
        // specifically.
        is_dir: bool,
        // True if the walk skips the path.
    ) -> bool {
        // The matcher takes paths relative to the project's root; anything
        // outside the project is ignored.
        path.strip_prefix(&self.root)
            .ok()
            .is_none_or(|relative| self.ignore.matched(relative, is_dir).is_ignore())
    }

    /// Walk a directory, sending the consumer each file found, preceded by a
    /// `WalkStarted` and followed by a `WalkFinished`.
    fn walk(
        &mut self,
        // The directory to walk.
        dir: &Path,
        // An error once the consumer has exited.
    ) -> Result<(), SendError<ConsumerPathState>> {
        // A walk never filters its own starting point, so skip an ignored
        // directory here. This includes a directory whose ignore file changed:
        // the walk never enters that directory, so the change can't matter,
        // which spares a reload for each such file a package manager creates,
        // say.
        if self.is_ignored(dir, true) {
            return Ok(());
        }
        // The walk reads the ignore files afresh, so rebuild the matcher to
        // agree with it. The ignore files may have changed without the watcher
        // reporting it: the debouncer drops the events within a deleted
        // directory, for example, including the deletion of an ignore file
        // there.
        self.ignore = build_ignore(&self.root);
        self.consumer_tx
            .send(ConsumerPathState::WalkStarted(dir.to_path_buf()))?;
        // The walk reports an error (an unreadable directory, say) as an entry
        // of its own, then continues. Skip these, since a file the walk can't
        // read isn't usable by the IDE either. Symlinks aren't followed, so a
        // symlink loop can't make the walk run forever.
        for entry in WalkBuilder::new(dir).build().flatten() {
            if entry
                .file_type()
                .is_some_and(|file_type| file_type.is_file())
            {
                self.consumer_tx.send(ConsumerPathState::WatcherPathState(
                    WatcherPathState::CreatedOrModified(entry.into_path()),
                ))?;
            }
        }
        self.consumer_tx.send(ConsumerPathState::WalkFinished)
    }
}

/// Build the matcher which decides whether a path the watcher reports is
/// ignored. It applies the same filters as every walk: the
/// [ignore](https://docs.rs/ignore) crate's defaults, which are ripgrep's.
/// These skip hidden files and directories (such as `.git`), anything excluded
/// by an `.ignore` file, and -- only inside a Git repository -- anything
/// excluded by `.gitignore` or Git's other exclude rules.
fn build_ignore(
    // The project's root directory, to which the matcher's paths are relative.
    root: &Path,
    // The matcher, which reads the ignore files only as it needs them.
) -> IncrementalIgnore {
    WalkBuilder::new(root)
        .build_matchers()
        .pop()
        .expect("A builder given one path builds one matcher.")
}

// ### Consumer
/// Apply each message sent to the consumer, until the channel closes or the
/// cache is dropped. This runs on its own thread, spawned by
/// `Cache::new_project`.
fn consume(
    // The cache to update. This is upgraded only while applying a message, so
    // that the consumer doesn't keep a dropped cache alive while it waits for
    // the next one.
    cache: &Weak<Mutex<Cache>>,
    // The messages to apply.
    consumer_rx: &Receiver<ConsumerPathState>,
) {
    let mut consumer = Consumer::default();
    while let Ok(message) = consumer_rx.recv() {
        let Some(cache) = cache.upgrade() else {
            return;
        };
        consumer.apply(&cache, message);
    }
}

/// The consumer's state between messages.
#[derive(Default)]
struct Consumer {
    /// While a walk is in progress, the cached files beneath the directory
    /// being walked which the walk hasn't found; `None` otherwise.
    unwalked: Option<HashSet<PathBuf>>,
}

impl Consumer {
    /// Apply one message to the cache.
    fn apply(
        &mut self,
        // The cache to update.
        cache: &Arc<Mutex<Cache>>,
        // The message to apply.
        message: ConsumerPathState,
    ) {
        match message {
            // The walker walks one directory at a time, so this replaces only
            // the state of a walk which never finished; lacking its
            // `WalkFinished`, the files it didn't find can't be presumed gone.
            ConsumerPathState::WalkStarted(dir) => {
                self.unwalked = Some(cache.lock().unwrap().files_beneath(&dir).cloned().collect());
            }
            // A cached file which the walk didn't find no longer exists, or is
            // now ignored. Release the lock between files, so that forgetting
            // many doesn't hold up translations.
            ConsumerPathState::WalkFinished => {
                if let Some(unwalked) = self.unwalked.take() {
                    for path in unwalked {
                        cache.lock().unwrap().forget_file(&path);
                    }
                }
            }
            ConsumerPathState::WatcherPathState(WatcherPathState::CreatedOrModified(path)) => {
                if let Some(unwalked) = &mut self.unwalked {
                    unwalked.remove(&path);
                }
                scan_file(cache, &path, true);
            }
            // Per the spec, only a path which no longer exists is forgotten.
            // One which exists again by the time this is applied -- recreated
            // by a version control checkout, say, or by an editor which saves
            // by renaming a new file over the old one -- is scanned instead.
            ConsumerPathState::WatcherPathState(WatcherPathState::Deleted(path)) => {
                match fs::symlink_metadata(&path) {
                    Ok(metadata) if !metadata.is_symlink() => scan_file(cache, &path, true),
                    _ => forget_path(cache, &path),
                }
            }
            ConsumerPathState::Rescan(path) => scan_file(cache, &path, false),
        }
    }
}

/// Forget a path which no longer exists. Nothing says whether it was a file or
/// a directory, so if the cache has no file at this path, assume it was a
/// directory and forget every cached file beneath it. The lock is released
/// between files, so that forgetting a large directory doesn't hold up
/// translations.
fn forget_path(
    // The cache to update.
    cache: &Mutex<Cache>,
    // The path which no longer exists.
    path: &Path,
) {
    let paths: Vec<PathBuf> = {
        let cache = cache.lock().unwrap();
        if cache.files.contains_key(path) {
            vec![path.to_path_buf()]
        } else {
            cache.files_beneath(path).cloned().collect()
        }
    };
    for path in paths {
        cache.lock().unwrap().forget_file(&path);
    }
}

/// The outcome of the consumer's translation of a file.
enum ScanOutcome {
    /// The translation committed the file's facts to the cache.
    Translated,
    /// The file isn't text, or no lexer recognizes it, so nothing was
    /// committed.
    Untranslatable,
    /// The translation failed, perhaps after committing the file's facts.
    Failed,
}

/// Bring the cache's record of a file up to date with the file on disk by
/// translating the file to the CodeChatForWeb format, which commits its facts
/// to the cache. A connection's current file is skipped: the connection keeps
/// it up to date from its IDE's text, which supersedes the file on disk.
fn scan_file(
    // The cache to update.
    cache: &Arc<Mutex<Cache>>,
    // The file to scan.
    path: &Path,
    // True if the walker sent this file, which therefore passed its ignore
    // rules; only such a file is scanned when the cache doesn't have it. A
    // rescan of a file the cache has since forgotten -- one a walk found
    // ignored, say -- is moot.
    is_from_walker: bool,
) {
    // Perform I/O before locking the cache, so that the lock isn't held during
    // it.
    let current_stamp = match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => FileStamp::new(&metadata),
        // The file no longer exists -- or a directory has replaced it, which
        // the walker reports separately.
        _ => {
            cache.lock().unwrap().forget_file(path);
            return;
        }
    };
    // True if the TOC's text differs from the text the cache last read, which a
    // connection's sidebar may still show. When the cache has no record of the
    // TOC, no sidebar can show it: loading it in a sidebar records it.
    let is_toc_changed;
    {
        let cache = cache.lock().unwrap();
        let entry = cache.files.get(path);
        let is_modified = entry
            .and_then(|entry| entry.stamp.as_ref())
            .is_none_or(|cached| !cached.is_unmodified(&current_stamp));
        is_toc_changed = is_modified && entry.is_some() && path == cache.toc_path();
        if cache.is_current_file(path) {
            // The connections' sidebars show the TOC from their own IDE's text
            // or from disk, so each must still re-load it.
            if is_toc_changed {
                cache.notify_toc(CacheNotification::TocChanged);
            }
            return;
        }
        // A cached file needs scanning if it changed, or if a commit outdated
        // it, which its status records.
        let is_skipped = match entry {
            None => !is_from_walker,
            Some(entry) => !is_modified && entry.status == FileStatus::UpToDate,
        };
        if is_skipped {
            return;
        }
    }
    // A file which can't be read -- another program may have it locked, for
    // example -- leaves the cache as it is; the watcher reports the file again
    // once it changes.
    let Ok(contents) = read_text(path) else {
        return;
    };
    // The lexer requires Unix-style line endings.
    //
    // TODO: once the Server has user-selected options, implement the spec's
    // `SaveOnScan` option here: translate the result back to source, then write
    // any changes to the file and update its cached stamp.
    let outcome = match contents.map(|contents| {
        source_to_codechat_for_web(
            &contents.replace("\r\n", "\n"),
            path,
            0.0,
            false,
            Some(cache.clone()),
        )
    }) {
        Some(Ok(_)) => ScanOutcome::Translated,
        None
        | Some(Err(
            SourceToCodeChatForWebError::NoLexer | SourceToCodeChatForWebError::UnknownLexer(_),
        )) => ScanOutcome::Untranslatable,
        Some(Err(err)) => {
            warn!("Unable to scan {}: {err}", path.display());
            ScanOutcome::Failed
        }
    };
    let mut cache = cache.lock().unwrap();
    if cache.is_current_file(path) {
        // A connection made this file its current file during the translation,
        // which may then have committed the file's content on disk over the
        // content the connection shows, which may be newer. Have the connection
        // re-translate its content to restore it.
        cache.notify_current_file_outdated(&HashSet::from([path.to_path_buf()]));
    } else {
        match outcome {
            // The translation's commit recorded a stamp taken as of the commit,
            // which follows the reading of its text. A write in between would
            // leave the cache describing the old text with the new stamp, so
            // the consumer would skip the watcher's report of that write.
            // Record the stamp taken before the text was read instead, which no
            // such write leaves the file matching.
            ScanOutcome::Translated => {
                if let Some(entry) = cache.files.get_mut(path) {
                    entry.stamp = Some(current_stamp);
                }
            }
            // A file which can't be translated defines nothing. Committing it
            // as such withdraws whatever it defined before it stopped being
            // translatable, and records its stamp, so that later walks skip it
            // until it changes rather than reading it again.
            ScanOutcome::Untranslatable => {
                cache.commit_file(path, Some(current_stamp), FileFacts::default());
            }
            // The translation may have failed after committing the file's
            // facts, along with a stamp which matches the file; discard that
            // stamp, so that the next walk tries again rather than skipping a
            // file whose processing never finished.
            ScanOutcome::Failed => {
                if let Some(entry) = cache.files.get_mut(path) {
                    entry.stamp = None;
                }
            }
        }
    }
    if is_toc_changed {
        cache.notify_toc(CacheNotification::TocChanged);
    }
}

/// Read a file as text. The walker sends the consumer every file in the
/// project, including any large media files, so a file is read in full only
/// once its first block proves to be text.
fn read_text(
    // The file to read.
    path: &Path,
    // Its contents; `None` if it isn't UTF-8, which most likely makes it a
    // binary file.
) -> io::Result<Option<String>> {
    // The size of the block checked before reading the rest of the file.
    const FIRST_BLOCK_LEN: u64 = 8192;
    let mut file = File::open(path)?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(FIRST_BLOCK_LEN)
        .read_to_end(&mut bytes)?;
    // A character may straddle the end of the block, leaving an incomplete
    // sequence there, for which `error_len` returns `None`.
    if let Err(err) = str::from_utf8(&bytes)
        && err.error_len().is_some()
    {
        return Ok(None);
    }
    file.read_to_end(&mut bytes)?;
    Ok(String::from_utf8(bytes).ok())
}

impl Default for Cache {
    fn default() -> Self {
        Cache::new(PathBuf::new())
    }
}

// Tests
// -----
#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        fs,
        num::NonZeroUsize,
        path::{Path, PathBuf},
        sync::{
            Arc, Mutex,
            mpsc::{self as std_mpsc, Receiver},
        },
        thread::sleep,
        time::{Duration, Instant, SystemTime},
    };

    use notify_debouncer_full::notify::{
        Event, EventKind,
        event::{AccessKind, CreateKind, DataChange, Flag, ModifyKind, RemoveKind, RenameMode},
    };
    use test_utils::prep_test_dir;
    use tokio::sync::mpsc::{self, error::TryRecvError};

    use super::{
        Cache, CacheNotification, ConnectionRegistration, Consumer, ConsumerPathState, FileFacts,
        FileStamp, FileStatus, FragmentFact, IdResolution, IdState, LRU_ENTRIES_PER_CONNECTION,
        ProjectCaches, RACY_WINDOW, TargetFact, Walker, WatcherPathState, cache_map_capacity,
        get_or_insert_cache, set_cache_map_capacity, watcher_path_states,
    };
    use crate::webserver::canonicalize;

    // ### Test helpers
    //
    // Build a target fact with unimportant location info.
    fn target_fact(id: &str, inner_html: &str) -> TargetFact {
        TargetFact {
            id: id.to_string(),
            inner_html: inner_html.to_string(),
            line: 0,
            _doc_block_index: 0,
            gather_ids: vec![],
        }
    }

    // Build a gather element: a target which also lists ids to gather.
    fn gather_fact(id: &str, inner_html: &str, gather_ids: &[&str]) -> TargetFact {
        TargetFact {
            gather_ids: gather_ids.iter().map(|id| (*id).to_string()).collect(),
            ..target_fact(id, inner_html)
        }
    }

    // Build a fragment fact with unimportant location info.
    fn fragment_fact(id: &str) -> FragmentFact {
        FragmentFact {
            id: id.to_string(),
            line: 0,
            _doc_block_start_index: 0,
            _code_doc_block_end_index: 1,
        }
    }

    // Build facts for a file containing a single target.
    fn facts_target(id: &str, inner_html: &str) -> FileFacts {
        FileFacts {
            targets: vec![target_fact(id, inner_html)],
            ..Default::default()
        }
    }

    // Build facts for a file containing a single cross-reference.
    fn facts_xref(id: &str) -> FileFacts {
        FileFacts {
            xrefs: vec![id.to_string()],
            ..Default::default()
        }
    }

    // Build facts for a file containing a single fragment.
    fn facts_fragment(id: &str) -> FileFacts {
        FileFacts {
            fragments: vec![fragment_fact(id)],
            ..Default::default()
        }
    }

    // Build facts for a file containing a single gather element.
    fn facts_gather(id: &str, inner_html: &str, gather_ids: &[&str]) -> FileFacts {
        FileFacts {
            targets: vec![gather_fact(id, inner_html, gather_ids)],
            ..Default::default()
        }
    }

    // Shorthand for the status of a cached file.
    fn status<'a>(cache: &'a Cache, path: &Path) -> &'a FileStatus {
        &cache.files[path].status
    }

    // Shorthand for the files which depend on an id.
    fn dependents<'a>(cache: &'a Cache, id: &str) -> &'a HashSet<PathBuf> {
        &cache.ids[id].dependents
    }

    // Build the expected set of outdated files.
    fn paths(paths: &[&PathBuf]) -> HashSet<PathBuf> {
        paths.iter().map(|path| (*path).clone()).collect()
    }

    // Collect every message sent so far on a channel.
    fn drain<T>(rx: &Receiver<T>) -> Vec<T> {
        rx.try_iter().collect()
    }

    // Build the message the walker sends the consumer for a file.
    fn found(path: &Path) -> ConsumerPathState {
        ConsumerPathState::WatcherPathState(WatcherPathState::CreatedOrModified(path.to_path_buf()))
    }

    // Register a connection whose current file is `current_file`, returning its
    // registration and the receiver of its notifications.
    fn register(
        cache: &Arc<Mutex<Cache>>,
        connection_id: &str,
        current_file: &Path,
    ) -> (
        ConnectionRegistration,
        mpsc::UnboundedReceiver<CacheNotification>,
    ) {
        let (notify_tx, notify_rx) = mpsc::unbounded_channel();
        let registration = ConnectionRegistration::new(
            cache,
            connection_id.to_string(),
            current_file.to_path_buf(),
            notify_tx,
        );
        (registration, notify_rx)
    }

    fn set_modified(path: &Path, modified: SystemTime) {
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(modified)
            .unwrap();
    }

    // Move a file's modification time back past `RACY_WINDOW`, as for a file
    // which wasn't just written, so that a stamp taken of it can match.
    fn backdate(path: &Path) {
        set_modified(path, SystemTime::now() - 2 * RACY_WINDOW);
    }

    // ### Cross-reference tests
    //
    // Verify that a cross-reference to an existing target records the
    // dependency and resolves.
    #[test]
    fn test_xref_to_existing_target() {
        let mut cache = Cache::default();
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");

        let outcome = cache.commit_file(&a, None, facts_target("foo", "Foo!"));
        assert!(outcome.outdated.is_empty());

        let outcome = cache.commit_file(&b, None, facts_xref("foo"));
        assert!(outcome.outdated.is_empty());

        // The target must know its dependent and resolve to its defining file.
        let IdResolution::Target { path, target } = cache.resolve_id("foo") else {
            panic!("expected a target");
        };
        assert_eq!(path, a);
        assert_eq!(target.inner_html, "Foo!");
        assert_eq!(*dependents(&cache, "foo"), paths(&[&b]));
    }

    // Verify that referencing an id before its definition marks the referring
    // file outdated when the definition appears.
    #[test]
    fn test_forward_reference() {
        let mut cache = Cache::default();
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");

        cache.commit_file(&b, None, facts_xref("foo"));
        assert!(matches!(cache.resolve_id("foo"), IdResolution::Missing));
        assert_eq!(cache.ids["foo"].state, IdState::Missing);
        assert_eq!(*dependents(&cache, "foo"), paths(&[&b]));

        // Defining the id resolves the reference: `b` must be rebuilt and
        // remains a dependent.
        let outcome = cache.commit_file(&a, None, facts_target("foo", "Foo!"));
        assert_eq!(outcome.outdated, paths(&[&b]));
        assert_eq!(*status(&cache, &b), FileStatus::Outdated);
        assert_eq!(cache.ids["foo"].state, IdState::Single(a.clone()));
        assert_eq!(*dependents(&cache, "foo"), paths(&[&b]));
    }

    // Verify that only a change to a target's content outdates its dependents.
    #[test]
    fn test_target_change_outdates_dependents() {
        let mut cache = Cache::default();
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");
        cache.commit_file(&a, None, facts_target("foo", "Foo!"));
        cache.commit_file(&b, None, facts_xref("foo"));

        // Recommitting identical content causes no rebuilds...
        let outcome = cache.commit_file(&a, None, facts_target("foo", "Foo!"));
        assert!(outcome.outdated.is_empty());
        assert_eq!(*status(&cache, &b), FileStatus::UpToDate);

        // ...while changed content outdates the dependent.
        let outcome = cache.commit_file(&a, None, facts_target("foo", "Bar!"));
        assert_eq!(outcome.outdated, paths(&[&b]));
        assert_eq!(*status(&cache, &b), FileStatus::Outdated);
    }

    // Verify that deleting a target leaves its dependents waiting on the id,
    // and that a later re-definition (in another file) finds them again.
    #[test]
    fn test_target_deletion() {
        let mut cache = Cache::default();
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");
        let c = PathBuf::from("c.md");
        cache.commit_file(&a, None, facts_target("foo", "Foo!"));
        cache.commit_file(&b, None, facts_xref("foo"));

        // Delete the target: the dependent is rebuilt and now waits on the id.
        let outcome = cache.commit_file(&a, None, FileFacts::default());
        assert_eq!(outcome.outdated, paths(&[&b]));
        assert!(matches!(cache.resolve_id("foo"), IdResolution::Missing));
        assert_eq!(*dependents(&cache, "foo"), paths(&[&b]));

        // The id reappears in a different file: the waiter is rebuilt again.
        let outcome = cache.commit_file(&c, None, facts_target("foo", "Foo!"));
        assert_eq!(outcome.outdated, paths(&[&b]));
    }

    // Verify that removing a cross-reference unlinks the dependency, and that
    // an id which is neither defined nor referenced is forgotten entirely.
    #[test]
    fn test_unlink_on_recommit() {
        let mut cache = Cache::default();
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");
        cache.commit_file(&a, None, facts_target("foo", "Foo!"));
        cache.commit_file(&b, None, facts_xref("foo"));

        // Recommit `b` without the cross-reference; changing the target must no
        // longer outdate `b`.
        cache.commit_file(&b, None, FileFacts::default());
        let outcome = cache.commit_file(&a, None, facts_target("foo", "Bar!"));
        assert!(outcome.outdated.is_empty());

        // A reference to an id nothing defines is dropped once withdrawn.
        cache.commit_file(&b, None, facts_xref("nobody"));
        assert!(cache.ids.contains_key("nobody"));
        cache.commit_file(&b, None, FileFacts::default());
        assert!(!cache.ids.contains_key("nobody"));
    }

    // Verify that an id a file stops defining is forgotten once nothing
    // references it -- but not while anything does, including the file which
    // stopped defining it.
    #[test]
    fn test_undefined_id_forgotten() {
        let mut cache = Cache::default();
        let a = PathBuf::from("a.py");

        // A target which nothing references is forgotten once deleted...
        cache.commit_file(&a, None, facts_target("foo", "Foo!"));
        cache.commit_file(&a, None, FileFacts::default());
        assert!(!cache.ids.contains_key("foo"));

        // ...as is a fragment in a file which is removed.
        cache.commit_file(&a, None, facts_fragment("frag"));
        cache.remove_file(&a);
        assert!(cache.ids.is_empty());

        // A file which references its own target keeps the id after deleting
        // the target, until the reference goes too.
        cache.commit_file(
            &a,
            None,
            FileFacts {
                targets: vec![target_fact("foo", "Foo!")],
                xrefs: vec!["foo".to_string()],
                ..Default::default()
            },
        );
        cache.commit_file(&a, None, facts_xref("foo"));
        assert!(matches!(cache.resolve_id("foo"), IdResolution::Missing));
        assert_eq!(*dependents(&cache, "foo"), paths(&[&a]));
        cache.commit_file(&a, None, FileFacts::default());
        assert!(!cache.ids.contains_key("foo"));
    }

    // ### File lifecycle tests
    //
    // Verify that removing a file withdraws both its definitions and its
    // references, outdating what depended on them, and forgets the file.
    #[test]
    fn test_remove_file() {
        let mut cache = Cache::default();
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");
        cache.commit_file(&a, None, facts_target("foo", "Foo!"));
        cache.commit_file(&b, None, facts_xref("foo"));

        let outcome = cache.remove_file(&a);
        assert_eq!(outcome.outdated, paths(&[&b]));
        assert!(!cache.files.contains_key(&a));
        // The referring file is left waiting on the id, as if the definition
        // had simply been deleted.
        assert!(matches!(cache.resolve_id("foo"), IdResolution::Missing));
        assert_eq!(*dependents(&cache, "foo"), paths(&[&b]));

        // Removing the last file referring to the id forgets the id too.
        cache.remove_file(&b);
        assert!(!cache.ids.contains_key("foo"));
        assert!(cache.files.is_empty());
    }

    // Verify that an auto-assigned id is 10 characters drawn from
    // `[A-Za-z0-9_-]` and collides neither with a cached id nor with another id
    // assigned in the same pass.
    #[test]
    fn test_new_id() {
        let mut cache = Cache::default();
        cache.commit_file(&PathBuf::from("a.md"), None, facts_target("foo", "Foo!"));

        let mut assigned: HashSet<String> = HashSet::new();
        for _ in 0..10 {
            let id = cache.new_id(&assigned);
            let random_part = id.strip_prefix("cc-").unwrap();
            assert_eq!(random_part.chars().count(), 10);
            assert!(
                random_part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            );
            assert!(!cache.ids.contains_key(&id));
            assert!(assigned.insert(id));
        }
    }

    // Verify that the files beneath a directory are found, at any depth, and
    // that no file merely sharing a prefix of the directory's name is.
    #[test]
    fn test_files_beneath() {
        let mut cache = Cache::default();
        let beneath = [PathBuf::from("a/b/c.md"), PathBuf::from("a/b/d/e.md")];
        let not_beneath = [
            PathBuf::from("a/b"),
            PathBuf::from("a/b.md"),
            PathBuf::from("a/bc/f.md"),
            PathBuf::from("g.md"),
        ];
        for path in beneath.iter().chain(not_beneath.iter()) {
            cache.commit_file(path, None, FileFacts::default());
        }
        let found: Vec<&PathBuf> = cache.files_beneath(Path::new("a/b")).collect();
        assert_eq!(found, beneath.iter().collect::<Vec<_>>());
        assert_eq!(cache.files_beneath(Path::new("h")).count(), 0);
    }

    // ### Duplicate id tests
    //
    // Verify that duplicate ids are recorded as such, in one file or across
    // files, and that resolving one reverts the id to a single definition.
    #[test]
    fn test_duplicate_ids() {
        let mut cache = Cache::default();
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");
        let c = PathBuf::from("c.md");
        cache.commit_file(&a, None, facts_target("foo", "Foo!"));
        cache.commit_file(&c, None, facts_xref("foo"));

        // A second definition in another file makes the id a duplicate, which
        // outdates everything referencing it.
        let outcome = cache.commit_file(&b, None, facts_target("foo", "Imposter!"));
        assert_eq!(outcome.outdated, paths(&[&c]));
        let IdResolution::Multiple(definitions) = cache.resolve_id("foo") else {
            panic!("expected duplicate definitions");
        };
        assert_eq!(
            *definitions,
            HashMap::from([(a.clone(), 1), (b.clone(), 1)])
        );

        // Removing one definition restores the other.
        let outcome = cache.commit_file(&b, None, FileFacts::default());
        assert_eq!(outcome.outdated, paths(&[&c]));
        let IdResolution::Target { path, .. } = cache.resolve_id("foo") else {
            panic!("expected a target");
        };
        assert_eq!(path, a);

        // A duplicate within a single file is counted against that file.
        cache.commit_file(
            &b,
            None,
            FileFacts {
                targets: vec![target_fact("bar", "1"), target_fact("bar", "2")],
                ..Default::default()
            },
        );
        let IdResolution::Multiple(definitions) = cache.resolve_id("bar") else {
            panic!("expected duplicate definitions");
        };
        assert_eq!(*definitions, HashMap::from([(b.clone(), 2)]));
    }

    // Verify that an id changing kind (target to fragment) rebuilds its
    // dependents even though its defining file is unchanged, which is invisible
    // to `IdState`.
    #[test]
    fn test_id_changes_kind() {
        let mut cache = Cache::default();
        let a = PathBuf::from("a.py");
        let b = PathBuf::from("b.py");
        cache.commit_file(&a, None, facts_target("foo", "Foo!"));
        cache.commit_file(&b, None, facts_xref("foo"));

        // The id becomes a fragment: the cross-referencing file must be
        // rebuilt, since its cross-reference is now an error.
        let outcome = cache.commit_file(&a, None, facts_fragment("foo"));
        assert_eq!(outcome.outdated, paths(&[&b]));
        assert!(matches!(
            cache.resolve_id("foo"),
            IdResolution::Fragment { .. }
        ));
        assert_eq!(*dependents(&cache, "foo"), paths(&[&b]));

        // ...and back again.
        let outcome = cache.commit_file(&a, None, facts_target("foo", "Foo!"));
        assert_eq!(outcome.outdated, paths(&[&b]));
    }

    // ### Gather element tests
    //
    // Verify the forward direction: a gather element depends on the fragments
    // it lists, and only a change to a fragment's content outdates the
    // gathering file.
    #[test]
    fn test_fragment_content_outdates_gathering_file() {
        let mut cache = Cache::default();
        let a = PathBuf::from("a.py");
        let b = PathBuf::from("b.py");

        cache.commit_file(&a, None, facts_fragment("frag"));
        cache.commit_file(&b, None, facts_gather("bar", "Gathered", &["frag"]));
        assert_eq!(*dependents(&cache, "frag"), paths(&[&b]));

        // Storing the fragment's first content outdates the gathering file;
        // storing identical content afterwards does not.
        let outdated = cache.update_fragment_content(&a, "frag", "content".to_string());
        assert_eq!(outdated, paths(&[&b]));
        assert_eq!(*status(&cache, &b), FileStatus::Outdated);
        let outdated = cache.update_fragment_content(&a, "frag", "content".to_string());
        assert!(outdated.is_empty());
    }

    // Verify that a gather element is also a target, so it can be
    // cross-referenced.
    #[test]
    fn test_gather_element_is_a_target() {
        let mut cache = Cache::default();
        let a = PathBuf::from("a.py");
        let b = PathBuf::from("b.py");
        cache.commit_file(&a, None, facts_gather("bar", "Bazzy things", &["frag"]));
        cache.commit_file(&b, None, facts_xref("bar"));

        let IdResolution::Target { path, target } = cache.resolve_id("bar") else {
            panic!("expected a target");
        };
        assert_eq!(path, a);
        assert_eq!(target.inner_html, "Bazzy things");
        assert_eq!(target.gather_ids, vec!["frag".to_string()]);
    }

    // ### The reverse edge
    //
    // A fragment renders a backlink to each gather element listing it, so the
    // fragment's file must be rebuilt whenever that set of gather elements --
    // or the text of one of them -- changes. Verify that adding a fragment to a
    // gather list outdates the file defining that fragment.
    #[test]
    fn test_gather_addition_outdates_fragment_file() {
        let mut cache = Cache::default();
        let a = PathBuf::from("a.py");
        let b = PathBuf::from("b.py");
        cache.commit_file(&a, None, facts_fragment("frag"));

        // `a` must re-render to show the new backlink.
        let outcome = cache.commit_file(&b, None, facts_gather("bar", "Gathered", &["frag"]));
        assert_eq!(outcome.outdated, paths(&[&a]));
        assert_eq!(*status(&cache, &a), FileStatus::Outdated);

        // Recommitting the same gather element changes nothing, so nothing is
        // outdated: the diff must not report a spurious rebuild.
        let outcome = cache.commit_file(&b, None, facts_gather("bar", "Gathered", &["frag"]));
        assert!(outcome.outdated.is_empty());
    }

    // Verify that removing a fragment from a gather list, and deleting the
    // gather element outright, both outdate the fragment's file.
    #[test]
    fn test_gather_removal_outdates_fragment_file() {
        let mut cache = Cache::default();
        let a = PathBuf::from("a.py");
        let b = PathBuf::from("b.py");
        cache.commit_file(
            &a,
            None,
            FileFacts {
                fragments: vec![fragment_fact("frag"), fragment_fact("other")],
                ..Default::default()
            },
        );
        cache.commit_file(
            &b,
            None,
            facts_gather("bar", "Gathered", &["frag", "other"]),
        );

        // Drop one id from the list: only that fragment's file is affected, but
        // both live in `a`.
        let outcome = cache.commit_file(&b, None, facts_gather("bar", "Gathered", &["other"]));
        assert_eq!(outcome.outdated, paths(&[&a]));
        assert!(!dependents(&cache, "frag").contains(&b));

        // Delete the gather element entirely.
        let outcome = cache.commit_file(&b, None, FileFacts::default());
        assert_eq!(outcome.outdated, paths(&[&a]));
        assert!(!dependents(&cache, "other").contains(&b));
    }

    // Verify that changing a gather element's inner HTML -- the text of the
    // backlink each listed fragment renders -- outdates every file defining a
    // listed fragment, not just those added or removed.
    #[test]
    fn test_gather_text_change_outdates_listed_fragments() {
        let mut cache = Cache::default();
        let a = PathBuf::from("a.py");
        let b = PathBuf::from("b.py");
        let c = PathBuf::from("c.py");
        cache.commit_file(&a, None, facts_fragment("frag_a"));
        cache.commit_file(&b, None, facts_fragment("frag_b"));
        cache.commit_file(
            &c,
            None,
            facts_gather("bar", "Bazzy", &["frag_a", "frag_b"]),
        );

        // The list is unchanged, but its text isn't: both fragments' files must
        // re-render their backlinks.
        let outcome = cache.commit_file(
            &c,
            None,
            facts_gather("bar", "Zappy", &["frag_a", "frag_b"]),
        );
        assert_eq!(outcome.outdated, paths(&[&a, &b]));
    }

    // Verify that a gather element moving to another file outdates the listed
    // fragment at both ends of the move, with no special case for renames.
    #[test]
    fn test_gather_moves_between_files() {
        let mut cache = Cache::default();
        let a = PathBuf::from("a.py");
        let b = PathBuf::from("b.py");
        let c = PathBuf::from("c.py");
        cache.commit_file(&a, None, facts_fragment("frag"));
        cache.commit_file(&b, None, facts_gather("bar", "Gathered", &["frag"]));

        // The old file loses the gather element...
        let outcome = cache.commit_file(&b, None, FileFacts::default());
        assert_eq!(outcome.outdated, paths(&[&a]));
        // ...and the new file gains it; the backlink now points at `c`.
        let outcome = cache.commit_file(&c, None, facts_gather("bar", "Gathered", &["frag"]));
        assert_eq!(outcome.outdated, paths(&[&a]));
        assert_eq!(*dependents(&cache, "frag"), paths(&[&c]));
    }

    // Verify that listing an id which doesn't exist yet outdates nothing, and
    // that the fragment renders its own backlink when it does appear -- so no
    // rebuild of the gathering file's peers is needed at that point either.
    #[test]
    fn test_gather_of_undefined_fragment() {
        let mut cache = Cache::default();
        let a = PathBuf::from("a.py");
        let b = PathBuf::from("b.py");

        let outcome = cache.commit_file(&b, None, facts_gather("bar", "Gathered", &["frag"]));
        assert!(outcome.outdated.is_empty());
        assert_eq!(cache.ids["frag"].state, IdState::Missing);

        // The fragment appears: only the gathering file needs a rebuild, since
        // the fragment's own file just rendered its backlinks.
        let outcome = cache.commit_file(&a, None, facts_fragment("frag"));
        assert_eq!(outcome.outdated, paths(&[&b]));
    }

    // Verify that a gather element listing a fragment in its own file outdates
    // nothing: the file renders both sides in the same pass.
    #[test]
    fn test_gather_within_one_file() {
        let mut cache = Cache::default();
        let a = PathBuf::from("a.py");

        let outcome = cache.commit_file(
            &a,
            None,
            FileFacts {
                targets: vec![gather_fact("bar", "Gathered", &["frag"])],
                fragments: vec![fragment_fact("frag")],
                ..Default::default()
            },
        );
        assert!(outcome.outdated.is_empty());
    }

    // Verify the read side of the reverse edge: the gather elements listing a
    // fragment are found, in a deterministic order, and files which merely
    // cross-reference the id are excluded.
    #[test]
    fn test_gathers_referencing() {
        let mut cache = Cache::default();
        let a = PathBuf::from("a.py");
        let b = PathBuf::from("b.py");
        let c = PathBuf::from("c.py");
        cache.commit_file(&a, None, facts_fragment("frag"));
        cache.commit_file(
            &c,
            None,
            FileFacts {
                targets: vec![gather_fact("zap", "Zappy", &["frag"])],
                ..Default::default()
            },
        );
        cache.commit_file(
            &b,
            None,
            FileFacts {
                targets: vec![
                    gather_fact("bar", "Bazzy", &["frag"]),
                    gather_fact("aaa", "Other", &["nothing"]),
                ],
                // A cross-reference to the fragment isn't a gather element.
                xrefs: vec!["frag".to_string()],
                ..Default::default()
            },
        );

        let backlinks = cache.gathers_referencing("frag");
        let found: Vec<(&Path, &str, &str)> = backlinks
            .iter()
            .map(|backlink| {
                (
                    backlink.path,
                    backlink.id,
                    backlink.target.inner_html.as_str(),
                )
            })
            .collect();
        assert_eq!(
            found,
            vec![(b.as_path(), "bar", "Bazzy"), (c.as_path(), "zap", "Zappy"),]
        );

        // An id nothing gathers has no backlinks.
        assert!(cache.gathers_referencing("nobody").is_empty());
    }

    // Verify that the reverse edge terminates: rebuilding a fragment's file
    // updates its backlinks, but backlinks are excluded from
    // `Fragment::content`, so nothing further is outdated.
    #[test]
    fn test_reverse_edge_terminates() {
        let mut cache = Cache::default();
        let a = PathBuf::from("a.py");
        let b = PathBuf::from("b.py");
        cache.commit_file(&a, None, facts_fragment("frag"));
        cache.update_fragment_content(&a, "frag", "content".to_string());
        cache.commit_file(&b, None, facts_gather("bar", "Gathered", &["frag"]));

        // `a` was outdated by the new backlink; reprocessing it produces the
        // same facts and the same content, so the cascade stops here.
        let outcome = cache.commit_file(&a, None, facts_fragment("frag"));
        assert!(outcome.outdated.is_empty());
        let outdated = cache.update_fragment_content(&a, "frag", "content".to_string());
        assert!(outdated.is_empty());
    }

    // ### Connection tests
    //
    // Verify that a registered connection is notified only when its current
    // file is outdated, and not after its registration is dropped.
    #[test]
    fn test_connection_notification() {
        let cache = Arc::new(Mutex::new(Cache::default()));
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");
        cache
            .lock()
            .unwrap()
            .commit_file(&b, None, facts_xref("foo"));
        let (registration, mut notify_rx) = register(&cache, "conn", &b);

        // Defining the id `b` references outdates `b`.
        cache
            .lock()
            .unwrap()
            .commit_file(&a, None, facts_target("foo", "Foo!"));
        assert_eq!(
            notify_rx.try_recv(),
            Ok(CacheNotification::CurrentFileOutdated)
        );
        assert_eq!(notify_rx.try_recv(), Err(TryRecvError::Empty));

        // Recommitting `b` itself outdates nothing.
        cache
            .lock()
            .unwrap()
            .commit_file(&b, None, facts_xref("foo"));
        assert_eq!(notify_rx.try_recv(), Err(TryRecvError::Empty));

        // Dropping the registration drops the cache's only sender.
        drop(registration);
        assert!(cache.lock().unwrap().connections.is_empty());
        cache
            .lock()
            .unwrap()
            .commit_file(&a, None, facts_target("foo", "Bar!"));
        assert_eq!(notify_rx.try_recv(), Err(TryRecvError::Disconnected));

        // A registration doesn't keep its cache alive, and outlives it
        // harmlessly.
        let (registration, _notify_rx) = register(&cache, "conn", &b);
        let weak_cache = Arc::downgrade(&cache);
        drop(cache);
        assert!(weak_cache.upgrade().is_none());
        drop(registration);
    }

    // Verify that outdating the TOC notifies every registered connection, since
    // each shows the TOC in its sidebar, and that recommitting the TOC itself
    // notifies none of them.
    #[test]
    fn test_toc_notification() {
        let cache = Arc::new(Mutex::new(Cache::default()));
        // The default cache's root is empty, so this is its TOC.
        let toc = PathBuf::from("toc.md");
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");
        // The TOC cross-references a heading in `a`.
        cache
            .lock()
            .unwrap()
            .commit_file(&a, None, facts_target("foo", "Foo"));
        cache
            .lock()
            .unwrap()
            .commit_file(&toc, None, facts_xref("foo"));
        let (_definer, mut definer_rx) = register(&cache, "definer", &a);
        let (_bystander, mut bystander_rx) = register(&cache, "bystander", &b);
        let (_toc_viewer, mut toc_viewer_rx) = register(&cache, "toc_viewer", &toc);

        // Renaming the heading outdates the TOC. A connection whose current
        // file is the TOC must re-translate it as well.
        cache
            .lock()
            .unwrap()
            .commit_file(&a, None, facts_target("foo", "Bar"));
        for notify_rx in [&mut definer_rx, &mut bystander_rx] {
            assert_eq!(notify_rx.try_recv(), Ok(CacheNotification::TocOutdated));
            assert_eq!(notify_rx.try_recv(), Err(TryRecvError::Empty));
        }
        assert_eq!(
            toc_viewer_rx.try_recv(),
            Ok(CacheNotification::CurrentFileOutdated)
        );
        assert_eq!(toc_viewer_rx.try_recv(), Ok(CacheNotification::TocOutdated));
        assert_eq!(toc_viewer_rx.try_recv(), Err(TryRecvError::Empty));

        // Every sidebar load translates, and therefore recommits, the TOC; this
        // must not outdate the TOC again.
        cache
            .lock()
            .unwrap()
            .commit_file(&toc, None, facts_xref("foo"));
        for notify_rx in [&mut definer_rx, &mut bystander_rx, &mut toc_viewer_rx] {
            assert_eq!(notify_rx.try_recv(), Err(TryRecvError::Empty));
        }
    }

    // Verify that an outdated file goes to the consumer unless a connection
    // shows it, as its current file or as the TOC; a connection re-renders
    // those itself.
    #[test]
    fn test_outdated_files_go_to_consumer() {
        let cache = Arc::new(Mutex::new(Cache::default()));
        let (consumer_tx, consumer_rx) = std_mpsc::channel();
        cache.lock().unwrap().consumer_tx = Some(consumer_tx);
        let toc = PathBuf::from("toc.md");
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");
        let c = PathBuf::from("c.md");
        for referrer in [&toc, &b, &c] {
            cache
                .lock()
                .unwrap()
                .commit_file(referrer, None, facts_xref("foo"));
        }

        // With no connections, the consumer reprocesses every outdated file,
        // the TOC included.
        cache
            .lock()
            .unwrap()
            .commit_file(&a, None, facts_target("foo", "Foo"));
        let mut sent = drain(&consumer_rx);
        sent.sort_by_key(|message| format!("{message:?}"));
        assert_eq!(
            sent,
            vec![
                ConsumerPathState::Rescan(b.clone()),
                ConsumerPathState::Rescan(c.clone()),
                ConsumerPathState::Rescan(toc.clone()),
            ]
        );

        // A connection showing `c` re-renders `c` and the TOC; the consumer
        // reprocesses only `b`.
        let (_registration, mut notify_rx) = register(&cache, "conn", &c);
        cache
            .lock()
            .unwrap()
            .commit_file(&a, None, facts_target("foo", "Bar"));
        assert_eq!(drain(&consumer_rx), vec![ConsumerPathState::Rescan(b)]);
        let mut notifications = vec![notify_rx.try_recv().unwrap(), notify_rx.try_recv().unwrap()];
        notifications.sort_by_key(|notification| format!("{notification:?}"));
        assert_eq!(
            notifications,
            vec![
                CacheNotification::CurrentFileOutdated,
                CacheNotification::TocOutdated
            ]
        );
    }

    // Verify that the `CacheMap` holds `LRU_ENTRIES_PER_CONNECTION` caches per
    // connection, plus one.
    #[test]
    fn test_cache_map_capacity() {
        assert_eq!(cache_map_capacity(0).get(), 1);
        assert_eq!(
            cache_map_capacity(3).get(),
            3 * LRU_ENTRIES_PER_CONNECTION + 1
        );
    }

    // Verify that the `CacheMap` evicts its least recently used caches, except
    // for those in which a connection is registered.
    #[test]
    fn test_cache_map_eviction() {
        let a = PathBuf::from("a");
        let b = PathBuf::from("b");
        let c = PathBuf::from("c");
        let d = PathBuf::from("d");
        let mut caches = ProjectCaches::new(NonZeroUsize::new(3).unwrap());
        let insert = |caches: &mut ProjectCaches, root: &PathBuf| {
            get_or_insert_cache(caches, root, || {
                Arc::new(Mutex::new(Cache::new(root.clone())))
            })
        };
        let roots = |caches: &ProjectCaches| {
            let mut roots: Vec<PathBuf> = caches.iter().map(|(root, _)| root.clone()).collect();
            roots.sort();
            roots
        };
        let cache_a = insert(&mut caches, &a);
        insert(&mut caches, &b);
        insert(&mut caches, &c);
        // Getting an existing cache returns it, rather than a new one.
        assert!(Arc::ptr_eq(&insert(&mut caches, &a), &cache_a));

        // A connection in the least recently used cache keeps it from being
        // evicted, so the next least recently used is evicted instead.
        let (_registration, _notify_rx) = register(&cache_a, "conn", &a.join("x.md"));
        insert(&mut caches, &b);
        insert(&mut caches, &c);
        insert(&mut caches, &d);
        assert_eq!(roots(&caches), vec![a.clone(), c.clone(), d.clone()]);

        // Shrinking the map likewise keeps the cache with a connection.
        set_cache_map_capacity(&mut caches, NonZeroUsize::MIN);
        assert_eq!(roots(&caches), vec![a.clone()]);
        assert_eq!(caches.cap(), NonZeroUsize::MIN);

        // When only caches with connections remain, the map grows rather than
        // evict one.
        let cache_d = insert(&mut caches, &d);
        let (_registration, _notify_rx) = register(&cache_d, "conn2", &d.join("x.md"));
        set_cache_map_capacity(&mut caches, NonZeroUsize::MIN);
        assert_eq!(roots(&caches), vec![a.clone(), d.clone()]);
        insert(&mut caches, &b);
        assert_eq!(roots(&caches), vec![a, b, d]);
    }

    // ### Watcher tests
    //
    // Verify that watcher events become the paths the walker receives: renames
    // become a deletion then a creation, a modified directory is skipped, and
    // lost events request a walk of the entire project.
    #[test]
    fn test_watcher_path_states() {
        use WatcherPathState::{CreatedOrModified, Deleted};

        let (temp_dir, root) = prep_test_dir!();
        let file = root.join("file.md");
        fs::write(&file, "").unwrap();
        let missing = root.join("missing.md");
        let translate = |kind: EventKind, event_paths: &[&PathBuf]| {
            let event = event_paths.iter().fold(Event::new(kind), |event, path| {
                event.add_path((*path).clone())
            });
            watcher_path_states(&event, &root)
        };

        assert_eq!(
            translate(EventKind::Create(CreateKind::File), &[&file]),
            vec![CreatedOrModified(file.clone())]
        );
        assert_eq!(
            translate(EventKind::Remove(RemoveKind::Any), &[&missing]),
            vec![Deleted(missing.clone())]
        );
        assert_eq!(
            translate(
                EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
                &[&missing, &file]
            ),
            vec![Deleted(missing.clone()), CreatedOrModified(file.clone())]
        );
        assert_eq!(
            translate(
                EventKind::Modify(ModifyKind::Name(RenameMode::From)),
                &[&missing]
            ),
            vec![Deleted(missing.clone())]
        );
        assert_eq!(
            translate(
                EventKind::Modify(ModifyKind::Name(RenameMode::To)),
                &[&file]
            ),
            vec![CreatedOrModified(file.clone())]
        );
        // An unclassified rename is resolved by checking for the path.
        assert_eq!(
            translate(
                EventKind::Modify(ModifyKind::Name(RenameMode::Any)),
                &[&file, &missing]
            ),
            vec![CreatedOrModified(file.clone()), Deleted(missing.clone())]
        );
        assert_eq!(
            translate(
                EventKind::Modify(ModifyKind::Data(DataChange::Any)),
                &[&file, &root]
            ),
            vec![CreatedOrModified(file.clone())]
        );
        assert_eq!(
            translate(EventKind::Access(AccessKind::Any), &[&file]),
            vec![]
        );
        let rescan = Event::new(EventKind::Other).set_flag(Flag::Rescan);
        assert_eq!(
            watcher_path_states(&rescan, &root),
            vec![CreatedOrModified(root.clone())]
        );

        // Report any errors produced when removing the temporary directory.
        temp_dir.close().unwrap();
    }

    // ### Walker tests
    //
    // Verify that the walker forwards only those paths its ignore files don't
    // exclude, walks each directory reported, and re-walks after an ignore file
    // changes. The fixture holds hidden files, and files excluded by an
    // `.ignore` file.
    #[test]
    fn test_walk_project() {
        let (temp_dir, test_dir) = prep_test_dir!();
        let root = canonicalize(&test_dir).unwrap();
        let a = root.join("a.md");
        let ignored = root.join("ignored.md");
        let b = root.join("sub").join("b.py");
        let sub_ignored = root.join("sub").join("ignored.md");
        // Confirm that copying the fixture preserved the hidden files, so their
        // absence from the walk below is the walker's doing.
        assert!(root.join(".ignore").is_file());
        assert!(root.join(".hidden").join("c.md").is_file());

        let (consumer_tx, consumer_rx) = std_mpsc::channel();
        let mut walker = Walker::new(root.clone(), consumer_tx);
        // Process one path from the watcher, returning everything the walker
        // sends the consumer as a result. The order in which a walk finds files
        // varies by platform, so sort the files found.
        let mut process = |path_state: WatcherPathState| {
            walker.process(path_state).unwrap();
            let mut sent = drain(&consumer_rx);
            if let [
                ConsumerPathState::WalkStarted(_),
                files @ ..,
                ConsumerPathState::WalkFinished,
            ] = sent.as_mut_slice()
            {
                files.sort_by_key(|message| format!("{message:?}"));
            }
            sent
        };
        // The messages a walk of `dir` which finds `files` sends.
        let walk = |dir: &PathBuf, files: &[&PathBuf]| {
            let mut sent = vec![ConsumerPathState::WalkStarted(dir.clone())];
            let mut files: Vec<&PathBuf> = files.to_vec();
            files.sort();
            sent.extend(files.into_iter().map(|file| found(file)));
            sent.push(ConsumerPathState::WalkFinished);
            sent
        };

        // The initial walk skips hidden and ignored files.
        assert_eq!(
            process(WatcherPathState::CreatedOrModified(root.clone())),
            walk(&root, &[&a, &b])
        );

        // A modified file is forwarded, unless it's ignored or hidden.
        assert_eq!(
            process(WatcherPathState::CreatedOrModified(a.clone())),
            vec![found(&a)]
        );
        assert_eq!(
            process(WatcherPathState::CreatedOrModified(ignored.clone())),
            vec![]
        );
        assert_eq!(
            process(WatcherPathState::CreatedOrModified(
                root.join(".hidden").join("c.md")
            )),
            vec![]
        );

        // Likewise for a deleted path.
        let gone = root.join("gone.md");
        assert_eq!(
            process(WatcherPathState::Deleted(gone.clone())),
            vec![ConsumerPathState::WatcherPathState(
                WatcherPathState::Deleted(gone)
            )]
        );
        assert_eq!(process(WatcherPathState::Deleted(ignored.clone())), vec![]);

        // A created directory is walked, unless it's hidden.
        let new_dir = root.join("new_dir");
        let d = new_dir.join("d.md");
        let new_dir_ignored = new_dir.join("ignored.md");
        fs::create_dir(&new_dir).unwrap();
        fs::write(&d, "# d.md").unwrap();
        fs::write(&new_dir_ignored, "# Ignored.").unwrap();
        assert_eq!(
            process(WatcherPathState::CreatedOrModified(new_dir.clone())),
            walk(&new_dir, &[&d])
        );
        assert_eq!(
            process(WatcherPathState::CreatedOrModified(root.join(".hidden"))),
            vec![]
        );

        // An ignore file inside an ignored directory can't change what the walk
        // finds, so it prompts no walk.
        let hidden_ignore = root.join(".hidden").join(".ignore");
        fs::write(&hidden_ignore, "c.md\n").unwrap();
        assert_eq!(
            process(WatcherPathState::CreatedOrModified(hidden_ignore)),
            vec![]
        );

        // Changing an `.ignore` file reloads the ignore files, then re-walks
        // the directory it governs.
        fs::write(root.join(".ignore"), "a.md\n").unwrap();
        assert_eq!(
            process(WatcherPathState::CreatedOrModified(root.join(".ignore"))),
            walk(&root, &[&ignored, &b, &sub_ignored, &d, &new_dir_ignored])
        );
        assert_eq!(
            process(WatcherPathState::CreatedOrModified(a.clone())),
            vec![]
        );

        // Git's ignore files apply only within a repository.
        fs::create_dir_all(root.join(".git").join("info")).unwrap();
        fs::write(new_dir.join(".gitignore"), "d.md\n").unwrap();
        assert_eq!(
            process(WatcherPathState::CreatedOrModified(
                new_dir.join(".gitignore")
            )),
            walk(&new_dir, &[&new_dir_ignored])
        );
        // A change to `.git/info/exclude` re-walks the entire project.
        let exclude = root.join(".git").join("info").join("exclude");
        fs::write(&exclude, "b.py\n").unwrap();
        assert_eq!(
            process(WatcherPathState::CreatedOrModified(exclude)),
            walk(&root, &[&ignored, &sub_ignored, &new_dir_ignored])
        );

        // Report any errors produced when removing the temporary directory.
        temp_dir.close().unwrap();
    }

    // ### File stamp tests
    //
    // Verify that a stamp taken soon after its file was written never matches,
    // while a stamp of an older file matches until the file changes.
    #[test]
    fn test_file_stamp() {
        let (temp_dir, test_dir) = prep_test_dir!();
        let path = test_dir.join("a.md");
        let stamp = |path: &Path| FileStamp::new(&fs::metadata(path).unwrap());

        // Simulate a second write in the same timestamp tick as the first,
        // which leaves the length and modification time unchanged.
        fs::write(&path, "one").unwrap();
        let racy = stamp(&path);
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        fs::write(&path, "two").unwrap();
        set_modified(&path, modified);
        let current = stamp(&path);
        // Only the racy rule tells these apart on Windows; on Unix, the write
        // also changes the change time.
        #[cfg(not(unix))]
        assert!(racy.stat == current.stat);
        assert!(!racy.is_unmodified(&current));

        backdate(&path);
        let trusted = stamp(&path);
        assert!(trusted.is_unmodified(&stamp(&path)));
        fs::write(&path, "three").unwrap();
        assert!(!trusted.is_unmodified(&stamp(&path)));

        // Report any errors produced when removing the temporary directory.
        temp_dir.close().unwrap();
    }

    // ### Consumer tests
    //
    // Verify that the consumer scans the files the walker sends into the cache,
    // rescanning one only when it's changed or outdated, and leaves a
    // connection's current file to that connection.
    #[test]
    fn test_consumer_scans() {
        let (temp_dir, test_dir) = prep_test_dir!();
        let root = canonicalize(&test_dir).unwrap();
        let a = root.join("a.md");
        let b = root.join("b.md");
        fs::write(&a, "<h1 id=\"foo\">Foo</h1>\n").unwrap();
        fs::write(&b, "See <xref ref=\"foo\"></xref>.\n").unwrap();
        backdate(&a);
        backdate(&b);
        let cache = Arc::new(Mutex::new(Cache::new(root.clone())));
        let (consumer_tx, consumer_rx) = std_mpsc::channel();
        cache.lock().unwrap().consumer_tx = Some(consumer_tx);
        let mut consumer = Consumer::default();
        let inner_html = |cache: &Arc<Mutex<Cache>>| {
            cache.lock().unwrap().files[&a].targets["foo"]
                .inner_html
                .clone()
        };

        consumer.apply(&cache, found(&a));
        consumer.apply(&cache, found(&b));
        assert_eq!(inner_html(&cache), "Foo");
        assert_eq!(*dependents(&cache.lock().unwrap(), "foo"), paths(&[&b]));
        assert_eq!(*status(&cache.lock().unwrap(), &b), FileStatus::UpToDate);

        // An unmodified file isn't rescanned. To observe this, alter its cached
        // target, which a rescan would restore.
        cache
            .lock()
            .unwrap()
            .files
            .get_mut(&a)
            .unwrap()
            .targets
            .get_mut("foo")
            .unwrap()
            .inner_html = "Stale".to_string();
        consumer.apply(&cache, found(&a));
        assert_eq!(inner_html(&cache), "Stale");

        // A modified file is rescanned, which outdates the file referencing it;
        // the consumer is sent that file.
        fs::write(&a, "<h1 id=\"foo\">Foo bar</h1>\n").unwrap();
        consumer.apply(&cache, found(&a));
        assert_eq!(inner_html(&cache), "Foo bar");
        assert_eq!(*status(&cache.lock().unwrap(), &b), FileStatus::Outdated);
        assert_eq!(
            drain(&consumer_rx),
            vec![ConsumerPathState::Rescan(b.clone())]
        );
        // Scanning the outdated file brings it up to date, so a second request,
        // as when several commits outdate one file, isn't scanned. To observe
        // this, discard its cached cross-references, which a rescan restores.
        consumer.apply(&cache, ConsumerPathState::Rescan(b.clone()));
        assert_eq!(*status(&cache.lock().unwrap(), &b), FileStatus::UpToDate);
        cache
            .lock()
            .unwrap()
            .files
            .get_mut(&b)
            .unwrap()
            .xrefs
            .clear();
        consumer.apply(&cache, ConsumerPathState::Rescan(b.clone()));
        assert!(cache.lock().unwrap().files[&b].xrefs.is_empty());
        // A request for a file the cache has since forgotten is moot.
        let forgotten = root.join("forgotten.md");
        fs::write(&forgotten, "# Forgotten\n").unwrap();
        consumer.apply(&cache, ConsumerPathState::Rescan(forgotten.clone()));
        assert!(!cache.lock().unwrap().files.contains_key(&forgotten));

        // A connection's current file isn't scanned...
        let (registration, mut notify_rx) = register(&cache, "conn", &a);
        fs::write(&a, "<h1 id=\"foo\">Foo baz</h1>\n").unwrap();
        consumer.apply(&cache, found(&a));
        assert_eq!(inner_html(&cache), "Foo bar");
        // ...nor forgotten when it's deleted: the connection's IDE may still
        // hold its text, so the connection is told to re-translate it.
        fs::remove_file(&a).unwrap();
        consumer.apply(
            &cache,
            ConsumerPathState::WatcherPathState(WatcherPathState::Deleted(a.clone())),
        );
        assert!(cache.lock().unwrap().files.contains_key(&a));
        assert_eq!(
            notify_rx.try_recv(),
            Ok(CacheNotification::CurrentFileOutdated)
        );
        // Once no connection shows the file, the consumer is asked to check it;
        // this finds it gone, so it's forgotten, which outdates the file
        // referencing it.
        drop(registration);
        assert_eq!(
            drain(&consumer_rx),
            vec![ConsumerPathState::Rescan(a.clone())]
        );
        consumer.apply(&cache, ConsumerPathState::Rescan(a.clone()));
        assert!(!cache.lock().unwrap().files.contains_key(&a));
        assert_eq!(
            drain(&consumer_rx),
            vec![ConsumerPathState::Rescan(b.clone())]
        );

        // A file which isn't text, or which no lexer recognizes, defines
        // nothing; it's cached as such, so that it isn't read again until it
        // changes. One with a lexer directive is translated, whatever its
        // extension.
        let binary = root.join("binary.md");
        let unknown = root.join("unknown.xyz");
        let directive = root.join("directive.xyz");
        fs::write(&binary, [0xff, 0xfe, 0x00]).unwrap();
        fs::write(&unknown, "<h1 id=\"unknown\">Unknown</h1>\n").unwrap();
        fs::write(
            &directive,
            // Split this so the lexer doesn't get set for this file.
            concat!(
                "# CodeChat Editor ",
                "lexer: python\n# <h1 id=\"directive\">Directive</h1>\n"
            ),
        )
        .unwrap();
        for path in [&binary, &unknown, &directive] {
            consumer.apply(&cache, found(path));
        }
        {
            let cache_guard = cache.lock().unwrap();
            for path in [&binary, &unknown] {
                assert!(cache_guard.files[path].targets.is_empty());
                assert_eq!(*status(&cache_guard, path), FileStatus::UpToDate);
            }
            assert!(matches!(
                cache_guard.resolve_id("unknown"),
                IdResolution::Missing
            ));
            assert!(matches!(
                cache_guard.resolve_id("directive"),
                IdResolution::Target { .. }
            ));
        }
        // A file which stops being translatable withdraws its definitions.
        fs::write(&directive, "# <h1 id=\"directive\">Directive</h1>\n").unwrap();
        consumer.apply(&cache, found(&directive));
        assert!(matches!(
            cache.lock().unwrap().resolve_id("directive"),
            IdResolution::Missing
        ));

        // Report any errors produced when removing the temporary directory.
        temp_dir.close().unwrap();
    }

    // Verify that the consumer tells every connection to re-load the TOC when
    // its text changes on disk -- even when the TOC is a connection's current
    // file, which the consumer leaves to that connection -- and when it's
    // deleted.
    #[test]
    fn test_consumer_toc_changed() {
        let (temp_dir, test_dir) = prep_test_dir!();
        let root = canonicalize(&test_dir).unwrap();
        let toc = root.join("toc.md");
        let a = root.join("a.md");
        fs::write(&toc, "# Contents\n").unwrap();
        let cache = Arc::new(Mutex::new(Cache::new(root.clone())));
        let mut consumer = Consumer::default();
        let (_registration, mut notify_rx) = register(&cache, "conn", &a);
        let is_target = |cache: &Arc<Mutex<Cache>>, id: &str| {
            matches!(
                cache.lock().unwrap().resolve_id(id),
                IdResolution::Target { .. }
            )
        };

        // No sidebar can show a TOC the cache has no record of, since loading
        // the TOC in a sidebar records it, so its first scan is not a change.
        consumer.apply(&cache, found(&toc));
        assert_eq!(notify_rx.try_recv(), Err(TryRecvError::Empty));

        // A changed TOC is scanned, then reported; an unchanged one isn't.
        fs::write(&toc, "<h1 id=\"contents\">Contents</h1>\n").unwrap();
        backdate(&toc);
        consumer.apply(&cache, found(&toc));
        assert!(is_target(&cache, "contents"));
        assert_eq!(notify_rx.try_recv(), Ok(CacheNotification::TocChanged));
        assert_eq!(notify_rx.try_recv(), Err(TryRecvError::Empty));
        consumer.apply(&cache, found(&toc));
        assert_eq!(notify_rx.try_recv(), Err(TryRecvError::Empty));

        // When the TOC is a connection's current file, the change isn't
        // scanned, but is still reported to every connection.
        let (_toc_registration, mut toc_notify_rx) = register(&cache, "toc_conn", &toc);
        fs::write(&toc, "<h1 id=\"changed\">Contents</h1>\n").unwrap();
        consumer.apply(&cache, found(&toc));
        assert!(is_target(&cache, "contents"));
        for notify_rx in [&mut notify_rx, &mut toc_notify_rx] {
            assert_eq!(notify_rx.try_recv(), Ok(CacheNotification::TocChanged));
            assert_eq!(notify_rx.try_recv(), Err(TryRecvError::Empty));
        }

        // Deleting the TOC is reported as a change, too. The connection whose
        // current file it is keeps the TOC in the cache by re-translating it.
        fs::remove_file(&toc).unwrap();
        consumer.apply(
            &cache,
            ConsumerPathState::WatcherPathState(WatcherPathState::Deleted(toc.clone())),
        );
        assert!(cache.lock().unwrap().files.contains_key(&toc));
        assert_eq!(notify_rx.try_recv(), Ok(CacheNotification::TocChanged));
        assert_eq!(notify_rx.try_recv(), Err(TryRecvError::Empty));
        assert_eq!(
            toc_notify_rx.try_recv(),
            Ok(CacheNotification::CurrentFileOutdated)
        );
        assert_eq!(toc_notify_rx.try_recv(), Ok(CacheNotification::TocChanged));
        assert_eq!(toc_notify_rx.try_recv(), Err(TryRecvError::Empty));

        // Report any errors produced when removing the temporary directory.
        temp_dir.close().unwrap();
    }

    // Verify that the consumer forgets deleted files and directories, and the
    // cached files a walk doesn't find.
    #[test]
    fn test_consumer_forgets() {
        let (temp_dir, test_dir) = prep_test_dir!();
        let root = canonicalize(&test_dir).unwrap();
        let sub = root.join("sub");
        fs::create_dir(&sub).unwrap();
        let a = root.join("a.md");
        let b = sub.join("b.md");
        let c = sub.join("c.md");
        let d = sub.join("d.md");
        for path in [&a, &b, &c, &d] {
            fs::write(path, "# Text\n").unwrap();
        }
        let cache = Arc::new(Mutex::new(Cache::new(root.clone())));
        let mut consumer = Consumer::default();
        for path in [&a, &b, &c, &d] {
            consumer.apply(&cache, found(path));
        }
        let cached = |cache: &Arc<Mutex<Cache>>| {
            cache
                .lock()
                .unwrap()
                .files
                .keys()
                .cloned()
                .collect::<Vec<PathBuf>>()
        };
        assert_eq!(
            cached(&cache),
            vec![a.clone(), b.clone(), c.clone(), d.clone()]
        );

        // A walk which doesn't find a cached file means it no longer exists.
        // Files outside the directory walked are unaffected.
        fs::remove_file(&c).unwrap();
        consumer.apply(&cache, ConsumerPathState::WalkStarted(sub.clone()));
        consumer.apply(&cache, found(&b));
        consumer.apply(&cache, found(&d));
        consumer.apply(&cache, ConsumerPathState::WalkFinished);
        assert_eq!(cached(&cache), vec![a.clone(), b.clone(), d.clone()]);

        // A file a connection stopped showing during a walk wasn't found by
        // that walk, so the walk still forgets it -- as it should if, say, the
        // walk skipped the file as ignored.
        consumer.apply(&cache, ConsumerPathState::WalkStarted(sub.clone()));
        consumer.apply(&cache, found(&b));
        consumer.apply(&cache, ConsumerPathState::Rescan(d.clone()));
        consumer.apply(&cache, ConsumerPathState::WalkFinished);
        assert_eq!(cached(&cache), vec![a.clone(), b.clone()]);

        // A deletion reported for a path which exists again, as after a version
        // control checkout, rescans the file rather than forgetting it.
        fs::write(&a, "<h1 id=\"recreated\">Recreated</h1>\n").unwrap();
        consumer.apply(
            &cache,
            ConsumerPathState::WatcherPathState(WatcherPathState::Deleted(a.clone())),
        );
        assert_eq!(cached(&cache), vec![a.clone(), b.clone()]);
        assert!(matches!(
            cache.lock().unwrap().resolve_id("recreated"),
            IdResolution::Target { .. }
        ));

        // A deleted directory takes its files with it.
        fs::remove_dir_all(&sub).unwrap();
        consumer.apply(
            &cache,
            ConsumerPathState::WatcherPathState(WatcherPathState::Deleted(sub)),
        );
        assert_eq!(cached(&cache), vec![a.clone()]);

        // As does a deleted file.
        fs::remove_file(&a).unwrap();
        consumer.apply(
            &cache,
            ConsumerPathState::WatcherPathState(WatcherPathState::Deleted(a.clone())),
        );
        assert_eq!(cached(&cache), Vec::<PathBuf>::new());

        // Report any errors produced when removing the temporary directory.
        temp_dir.close().unwrap();
    }

    // Verify the whole pipeline: a project's cache learns of its files through
    // the initial walk, then of files created and deleted afterward through the
    // watcher.
    #[test]
    fn test_new_project() {
        // Wait for `condition` to hold, since the threads which maintain the
        // cache run on their own schedule. The watcher debounces events for 2
        // seconds, which this timeout comfortably exceeds.
        fn wait_for(condition: impl Fn() -> bool) {
            let start = Instant::now();
            while !condition() {
                assert!(
                    start.elapsed() < Duration::from_secs(30),
                    "timed out waiting for the cache"
                );
                sleep(Duration::from_millis(50));
            }
        }

        let (temp_dir, test_dir) = prep_test_dir!();
        let root = canonicalize(&test_dir).unwrap();
        let a = root.join("a.md");
        let b = root.join("b.md");
        fs::write(&a, "<h1 id=\"foo\">Foo</h1>\n").unwrap();
        let cache = Cache::new_project(root.clone());
        let is_cached = |path: &Path| cache.lock().unwrap().files.contains_key(path);

        wait_for(|| is_cached(&a));
        fs::write(&b, "See <xref ref=\"foo\"></xref>.\n").unwrap();
        wait_for(|| is_cached(&b));
        assert_eq!(*dependents(&cache.lock().unwrap(), "foo"), paths(&[&b]));
        fs::remove_file(&a).unwrap();
        wait_for(|| !is_cached(&a));
        assert!(matches!(
            cache.lock().unwrap().resolve_id("foo"),
            IdResolution::Missing
        ));

        // Dropping the cache stops its watcher, so that the temporary directory
        // can be removed.
        drop(cache);
        // Report any errors produced when removing the temporary directory.
        temp_dir.close().unwrap();
    }
}
