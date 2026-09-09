// TODO: Remove these once outdated-file scheduling is implemented. It will use
// `root`, plus the hooks a file watcher or project scan needs -- `remove_file`
// and `mark_unknown` -- which nothing calls yet.
#![allow(unused_variables)]
#![allow(unused)]
// Copyright (C) 2026 Bryan A. Jones.
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
//! `cache.rs` -- Keep a cache used to store all targets in a project
//! =================================================================
//!
//! The cache stores the location (file name and ID) and contents (inner HTML or
//! code/doc blocks for fragments) of a target. Targets are HTML elements
//! (excluding `<fragment>`s) with an ID.
//!
//! The goal of the cache is to support cross-references and gather elements,
//! and to ensure that all IDs are unique within a project. This means that
//! cross-references and gather elements persist across moving or renaming
//! files, since the IDs will be found in the cache.
//!
//! Non-project files support a subset of this functionality: the "project"
//! consists only of the current file. Only targets, gather elements,
//! cross-references, and fragments to items within the file work as expected;
//! references to other files do not.
//!
//! The cache reflects data read directly from disk/IDE; content edited in the
//! Client does not update the cache until it's written to disk/IDE, at which
//! point the cache much re-process this unknown file. Since the Client is
//! designed around an autosave principle which updates disk/IDE regularly,
//! there's little gap between the two.
//!
//! The cache hydrates data to the Client; the dehydration routes are
//! responsible for removing all cache hydration artifacts.
//!
//! Cross references
//! ----------------
//!
//! A `<xref ref="id"></xref>` is a cross reference to a `Target` or a gather
//! element. The `id` specifies the destination; the cache then hydrates the
//! contents based on the location and contents of the target of the provided
//! `id` to e.g. `<xref ref="id" contenteditable="false"><a
//! href="../path/to/page#id">Inner HTML from target</a></xref>`; this hydrated
//! form is only present in the Client.
//!
//! Details:
//!
//! * This element does not allow an `id` attribute.
//!
//! * The inner HTML is always taken before cache hydration, to prevent circular
//!   dependencies:
//!
//!   ```html
//!   <h1 id="a">See <xref ref="b"></xref></h1>   <!-- file a -->
//!   <h1 id="b">See <xref ref="a"></xref></h1>   <!-- file b -->
//!   ```
//!
//!   Including cache hydration would cause the inner HTML to be updated each
//!   time the file is processed, outdating the other file.
//!
//! * If the `id` referred to isn't found or refers to a duplicate id, the inner
//!   text is instead an appropriate error message.
//!
//! * If the cross reference is to a gather element, the text is the gather
//!   element's inner HTML, not the gathered code/doc blocks.
//!
//! Fragments and gather elements
//! -----------------------------
//!
//! A gather element such as `<h3 id="bar" data-gather="id1 id2...">Bazzy
//! things</h3>` is a `Target` with the `data-gather` attribute. It becomes a
//! list of the contents of fragments it refers to after hydration by the cache.
//! An example fragment tag, after cache hydration: `<fragment id="id1"
//! contenteditable="false">See <a href="path/to/gather#bar">Bazzy things</a>,
//! <a href="path/to/another/gather#zap">Zappy things</a></fragment>`. A
//! fragment's content by default includes the contents of the current doc block
//! and the contents of the following code/doc block; fragments are not allowed
//! in Markdown documents (in the case, the fragment contents consist of an
//! error message). Fragments may include the `following` attribute to enclose a
//! specific number of the following code/doc blocks; for example, `<fragment
//! id="bar" following="3"></fragment>` includes the current doc block along
//! with the next 3 code/doc blocks; `following` must be a whole number.
//!
//! Details:
//!
//! * Fragment contents may not include a gather element; in this case, the
//!   gather element list of contents will simply include an error message.
//! * Fragments do support indirection: gather element A includes contents from
//!   fragment B, which contains a cross reference to target C. Changes to
//!   target C makes B and A outdated.
//! * If a gather element refers to an `id` that is a `Target` or a
//!   `GatherElement`, not a `Fragment`, the resulting output for this in the
//!   list of fragments is an error message.
//! * If a gather element refers to an `id` that wasn't found or is a duplicate,
//!   its contents will be replaced by an error message.
//! * Fragments store an HTML rendering of the code and doc blocks they contain,
//!   excluding the content produced by hydrating the `<fragment>` tags, to
//!   avoid duplication and circular dependencies. See layer 4 under `Design`:
//!   this exclusion is what keeps a gather element and the fragments it lists
//!   from outdating each other forever. The HTML rendering of a fragment
//!   reproduces the layout of the source it came from: each doc block includes
//!   its indent, each line of a code block is preceded by that line's number,
//!   and the two are aligned -- a doc block and a line of code indented
//!   equally in the source begin in the same column, with the line numbers in
//!   a gutter of their own to the left of both.
//! * The backlinks a `<fragment>` hydrates to are derived from the gather
//!   elements which list it: for each such element, its containing file, its
//!   `id`, and its inner HTML (the link text). A fragment's rendered output
//!   therefore depends on the gather elements which reference it -- the reverse
//!   of the direction in which references are written; see `Design`.
//! * A `<fragment following=0>` is valid; it contains only the current doc
//!   block.
//! * The `following` attribute is clamped if it would exceed the number of
//!   code/doc blocks in the document.
//! * If the `following` value cannot be parsed to a whole number, an error
//!   message replaces the fragment content.
//! * A gather element, as a type of `Target`, requires an `id`. A `data-gather`
//!   attribute on an element without an `id` produces an error message which
//!   requests the missing `id`.
//! * All ids must be valid
//!   [CSS identifiers](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Values/ident)
//!   per
//!   [MDN recommendations](https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Global_attributes/id).
//!   Invalid ids produce error messages in the hydrated tag content.
//!
//! Example hydration of the gather tag `<h3 id="bar" data-gather="id1
//! id2...">Bazzy things</h3>`:
//!
//! ```html
//! <h3 class="cc-gather" id="bar" data-gather="id1 id2...">
//!   Bazzy things
//! </h3>
//! <div class="cc-gather-items" contenteditable="false">
//!   <p class="cc-gather-item-link">
//!     From <a href="link/to/first/tag#id1">Path to file</a>:
//!   </p>
//!   (first item content)
//!   ...
//!   <p class="cc-gather-item-link">
//!     From <a href="link/to/last/tag#idn">Path to file</a>:
//!   </p>
//!   (last item content)
//! </div>
//! ```
//!
//! Search
//! ------
//!
//! The cache supports searching the (cleaned) inner HTML of all `Target`s and
//! gather elements; search does not include `Fragment` contents.
//!
//! ### Auto-assignment of ids
//!
//! If `id="*"` on either a fragment, target, or gather element, the cache
//! replaces this with an random autogenerated `id` placed in the resulting HTML
//! contents, but this new `id` is not yet recorded in the cache. The file is
//! then marked as `Unknown` when it is saved; when re-read, this `id` is then
//! incorporated into the cache. This helps avoid cases where the cache and file
//! contents become unsynchronized: if the `id` is placed in the cache before
//! the write and the write fails, or if the file is never written (it was being
//! scanned, but not actively edited, so the file contents wasn't written back).
//!
//! The autogenerated id must be a valid
//! [CSS identifier](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Values/ident).
//! Autogenerated ids must be checked to make sure they don't collide with an
//! existing id.
//!
//! Design
//! ------
//!
//! The cache is a single plain-data structure, shared as an
//! `Arc<Mutex<Cache>>`; all consistency comes from that one lock, so no
//! per-item locking (and therefore no lock ordering) is needed. Items refer to
//! each other by key -- files by path, targets and fragments by id -- rather
//! than by `Arc`/`Weak` pointers. This keeps the structure acyclic, `Send`, and
//! (in the future) serializable, and avoids garbage-collecting stale weak
//! references.
//!
//! Cached state always converges by design. Each layer depends only on
//! lower-numbered layers:
//!
//! 1. `Target::inner_html` (including a gather element's) and
//!    `Target::gather_ids` are pure functions of the source (pre-hydration) --
//!    they depend on nothing.
//! 2. `<xref>` hydration depends on layer 1 only.
//! 3. `Fragment::content` depends on the source plus layer 2.
//! 4. `<fragment>` backlink hydration depends on layer 1 only: it renders, for
//!    each gather element which lists this fragment, that element's containing
//!    file, `id`, and inner HTML. Critically, this output is *excluded* from
//!    `Fragment::content` (layer 3); were it included, a gather element and
//!    each fragment it lists would outdate one another forever.
//! 5. Gather-list hydration depends on layers 1 and 3.
//!
//! Nothing reads layers 2, 4, or 5, so propagation terminates.
//!
//! Updating the cache is a two-phase process:
//!
//! 1. Collect: while walking a file's DOM, record all cacheable facts
//!    (`FileFacts`) -- targets, cross-references, fragments, and gather
//!    elements -- without touching the cache. This keeps the non-`Send` DOM
//!    types out of the cache and off its critical section. Fact collection
//!    ignores content expanded by the cache (the contents of `xref` and
//!    `fragment` tags; the content following a gather tag).
//! 2. Commit: `Cache::commit_file` applies the facts in one transaction,
//!    diffing them against the file's previous state to compute the set of
//!    files outdated by these changes.
//!
//! Item 2 requires the cache to track dependencies of an item, so that files
//! containing these dependencies can be Outdated. Dependencies are tracked at
//! file granularity, on the id rather than on the item defining it:
//! `IdEntry::dependents` is the set of files whose rendered output depends on
//! this id, by any means -- an `<xref ref="id">` or a `data-gather` list naming
//! it. One uniform set suffices in all three `IdState`s because the typical
//! action taken on a dependent is marking its containing file outdated
//! (fragments also used their dependents to generate "See x" links).
//!
//! Item 2 also requires the cache to define what constitutes a difference which
//! would trigger outdating dependencies. The state which is checked for
//! differences is:
//!
//! * Target/gather element: type (a target/gather element), path of the
//!   containing file, id, IdState, inner HTML, and gather\_ids.
//! * Fragment: type (a fragment), path of the containing file, id, IdState, and
//!   contents.
//!
//! This also makes indirection (gather A includes fragment B, whose content
//! cross-references target C) work without extra machinery: a change to C
//! outdates B's file; reprocessing B's file changes B's content, which outdates
//! A's file.
//!
//! ### Gather elements: the reverse edge
//!
//! `dependents` alone is not enough for gather elements, because the reference
//! runs in both directions: a gather element renders the *contents* of each
//! fragment it lists, and each of those fragments renders a *backlink* to the
//! gather element (see layer 4 above). The set of gather elements referencing a
//! fragment is therefore not merely bookkeeping -- it is observable output in
//! the fragment's own file.
//!
//! This is why the outgoing references of gather elements cannot be maintained
//! by the unlink-all/relink-all round trip used for cross-references (see
//! `commit_file`): that round trip destroys the very information a diff would
//! need. `commit_file` instead diffs `Target::gather_ids` explicitly, and
//! applies this rule:
//!
//! > When a gather element `G` is committed, outdate the file defining each id
//! > in the symmetric difference of `G`'s old and new `gather_ids`; if `G`'s
//! > inner HTML also changed, outdate the file defining each id in the union
//! > instead, since the backlink text those fragments render comes from it.
//!
//! A gather element moving to a different file needs no special case: the old
//! file's commit sees the element deleted (new `gather_ids` empty) and the new
//! file's commit sees it added, so both ends of the move outdate the fragments.
//!
//! Propagation terminates here for the reason given under layer 4: rebuilding a
//! fragment's file updates its backlinks, but backlinks are excluded from
//! `Fragment::content`, so nothing further is outdated.
//!
//! An id referenced before (or without) being defined has an `IdEntry` whose
//! state is `IdState::Missing`; `dependents` holds the files waiting on it.
//! When the id later appears, the state transition marks those files outdated,
//! and they simply remain dependents; when a defined id disappears, the state
//! returns to `Missing` and the dependents are again waiters. An `IdEntry`
//! which is `Missing` with no dependents carries no information and is removed.
//!
//! Duplicate ids are never renamed (file timestamps can't reliably identify the
//! original, and renaming would silently modify user content). Instead,
//! duplicates are reported as errors to the user.
//!
//! ### Misc
//!
//! All file names (stored as `PathBuf`) must be canonicalized absolute paths.
//!
//! HTML stored as `Target` inner HTML or in `Fragment` contents requires
//! cleaning:
//!
//! * To avoid duplicate IDs, all `id` attributes should be stripped.
//! * Note that images, URLs, etc. may not work if the referring path in their
//!   new location isn't valid; these simply aren't supported.
//! * Inner HTML must allow only the
//!   [permitted content for an `<a>` element](https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/a#technical_summary).
// Imports
// -------
//
// ### Standard library
use std::{
    collections::{HashMap, HashSet},
    fs::Metadata,
    mem,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

// ### Third-party
use rand::random;

// ### Local
//
// None.

// Data structures
// ---------------
/// The set of all project caches, keyed by each project's root directory (the
/// directory containing its `toc.md`). The outer mutex guards the addition of
/// newly-discovered projects; each inner mutex guards a single project's cache,
/// so that work on one project doesn't block work on another.
pub type CacheMap = Arc<Mutex<HashMap<PathBuf, Arc<Mutex<Cache>>>>>;

/// This defines the cache used to store all targets in a project.
pub struct Cache {
    /// Provide rapid access to a file by its absolute path; it must be within
    /// the project's root directory. This owns all per-file data.
    pub(super) files: HashMap<PathBuf, FileEntry>,
    /// Provide rapid access to a `Target`/gather element or `Fragment` by its
    /// id.
    pub(super) ids: HashMap<String, IdEntry>,
    /// All files for which `FileEntry::status == FileStatus::Unknown`.
    pub(super) pending_files: Vec<PathBuf>,
    /// The root directory of this project.
    pub(super) root: PathBuf,
    // TODO: search engine data storage. Search fields: target ID, contents,
    // file name. Perhaps [Tantivy](https://docs.rs/tantivy/latest/tantivy/)?
}

/// This stores the cached data for a given file. For non-page files
/// (non-existent files, images, PDFs, etc.) many of the fields are empty or
/// `None`.
#[derive(Default)]
pub(super) struct FileEntry {
    /// Metadata used to determine if this data represents the actual state of
    /// the file; if the file is newer, then this file's status must be changed
    /// to `Unknown`. `None` if the file doesn't exist or the metadata can't be
    /// determined.
    pub(super) metadata: Option<Metadata>,
    /// The status of this file. Note that this overlaps with
    /// `Cache::pending_files` and should be kept in sync with it.
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

/// Everything the cache knows about one id: where (if anywhere) it's defined,
/// and which files must be rebuilt when that changes.
#[derive(Default)]
pub(super) struct IdEntry {
    /// The relationship between this id and the `Target`/gather element or
    /// `Fragment` which defines it.
    pub(super) state: IdState,
    /// Every file whose rendered output depends on this id. Note that this does
    /// *not* capture the reverse dependency of a `Fragment` on the gather
    /// elements which list it; see `Design` and `commit_file`.
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
    /// The file's content is unknown -- either the file hasn't been processed,
    /// or it's been modified since it was last processed.
    #[default]
    Unknown,
    /// The file needs to be re-processed to update cross-references or gather
    /// elements.
    Outdated,
    /// The file has been processed.
    UpToDate,
}

/// Contains all information about a target. A target is any HTML element with
/// an id; the id (which should be globally unique within the project; duplicate
/// ids are flagged as errors) is the key of `FileEntry::targets`, and the
/// containing file is the entry holding this value, so neither is duplicated
/// here.
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
    pub(super) line: usize,
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
    pub(super) line: usize,
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
    pub(super) doc_block_index: usize,
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
    pub(super) doc_block_start_index: usize,
    /// The index of the last code/doc block of this `Fragment` in the vec of
    /// `CodeDocBlock`s for this file.
    pub(super) code_doc_block_end_index: usize,
}

// ### Commit results
/// The result of committing one file's facts to the cache.
pub(super) struct CommitOutcome {
    /// Files (other than the committed file) whose rendered output is
    /// invalidated by this commit; their status has already been set to
    /// `Outdated`. The caller should schedule them for reprocessing.
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

// Code
// ----
impl Cache {
    #[must_use]
    pub fn new(
        // The root directory of this project, as a canonicalized absolute path;
        // all cached files must live within it.
        root: PathBuf,
    ) -> Self {
        Cache {
            files: HashMap::new(),
            ids: HashMap::new(),
            pending_files: vec![],
            root,
        }
    }

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
    /// See the `Design` section for why this doesn't cycle back.
    #[allow(clippy::too_many_lines)]
    pub(super) fn commit_file(
        &mut self,
        // The file whose facts these are.
        path: &Path,
        // The file's metadata, captured when its content was read; `None` if
        // unavailable. Read this *before* locking the cache, so no I/O happens
        // while the lock is held.
        metadata: Option<Metadata>,
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
                    line: fact.line,
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
                    line: fact.line,
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
        entry.metadata = metadata;
        entry.targets = new_targets;
        entry.fragments = new_fragments;
        entry.gathers = new_gathers;
        entry.xrefs = facts.xrefs.into_iter().map(|id| Xref { id }).collect();
        self.set_status(path, FileStatus::UpToDate);

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

        // ### Mark outdated files
        //
        // A file never outdates itself: its rendered output was just produced
        // from the state committed here.
        outdated.remove(path);
        for outdated_path in &outdated {
            self.set_status(outdated_path, FileStatus::Outdated);
        }

        CommitOutcome { outdated }
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
        self.pending_files.retain(|pending| pending != path);
        outcome
    }

    /// Record that a file's cached content no longer reflects the file itself
    /// -- it's been written or modified since it was last processed -- which
    /// queues it in `pending_files` for reprocessing. This is how an id
    /// auto-assigned during hydration reaches the cache: the assignment is only
    /// in the file until the file is saved, re-read, and committed.
    pub(super) fn mark_unknown(
        &mut self,
        // The file whose content is now unknown.
        path: &Path,
    ) {
        self.files.entry(path.to_path_buf()).or_default();
        self.set_status(path, FileStatus::Unknown);
    }

    /// Set a file's status, keeping `pending_files` -- which holds exactly the
    /// files whose status is `Unknown` -- in sync with it.
    fn set_status(
        &mut self,
        // The file whose status changed; nothing happens if the cache has no
        // entry for it.
        path: &Path,
        // Its new status.
        status: FileStatus,
    ) {
        let Some(entry) = self.files.get_mut(path) else {
            return;
        };
        let unknown = status == FileStatus::Unknown;
        entry.status = status;
        let queued = self.pending_files.iter().any(|pending| pending == path);
        if unknown && !queued {
            self.pending_files.push(path.to_path_buf());
        } else if !unknown && queued {
            self.pending_files.retain(|pending| pending != path);
        }
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
        // The files (other than `path`) marked outdated by this change; their
        // status has already been set to `Outdated`.
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
        for outdated_path in &outdated {
            self.set_status(outdated_path, FileStatus::Outdated);
        }
        outdated
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
            // An id which nothing defines and nothing references carries no
            // information; drop it rather than accumulating an entry for every
            // id ever mistyped.
            if id_entry.state == IdState::Missing && id_entry.dependents.is_empty() {
                self.ids.remove(id);
            }
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
        path::{Path, PathBuf},
    };

    use super::{Cache, FileFacts, FileStatus, FragmentFact, IdResolution, IdState, TargetFact};

    // ### Test helpers
    //
    // Build a target fact with unimportant location info.
    fn target_fact(id: &str, inner_html: &str) -> TargetFact {
        TargetFact {
            id: id.to_string(),
            inner_html: inner_html.to_string(),
            line: 0,
            doc_block_index: 0,
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
            doc_block_start_index: 0,
            code_doc_block_end_index: 1,
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

    // Verify that `pending_files` holds exactly those files whose status is
    // `Unknown`.
    #[test]
    fn test_pending_files() {
        let mut cache = Cache::default();
        let a = PathBuf::from("a.md");
        cache.commit_file(&a, None, facts_target("foo", "Foo!"));
        assert!(cache.pending_files.is_empty());

        // A file written or modified since it was processed is queued for
        // reprocessing, and queued only once.
        cache.mark_unknown(&a);
        cache.mark_unknown(&a);
        assert_eq!(*status(&cache, &a), FileStatus::Unknown);
        assert_eq!(cache.pending_files, vec![a.clone()]);

        // Reprocessing it dequeues it.
        cache.commit_file(&a, None, facts_target("foo", "Foo!"));
        assert_eq!(*status(&cache, &a), FileStatus::UpToDate);
        assert!(cache.pending_files.is_empty());
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
}
