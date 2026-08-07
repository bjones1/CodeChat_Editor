// TODO: Remove these after implementing the cache.
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
/// `cache.rs` - Keep a cache used to store all targets in a project
/// ================================================================
///
/// The cache stores the location (file name and ID) and contents (inner HTML or
/// code/doc blocks for fragments) of a target. Targets are HTML elements
/// (excluding `<fragment>`s) with an ID.
///
/// The goal of the cache is to support cross-references and gather elements,
/// and to ensure that all IDs are unique within a project. This means that
/// cross-references and gather elements persist across moving or renaming
/// files, since the IDs will be found in the cache.
///
/// Non-project files support a subset of this functionality: the "project"
/// consists only of the current file. Only targets, gather elements,
/// cross-references, and fragments to items within the file work as expected;
/// references to other files do not.
///
/// The cache reflects data read directly from disk/IDE; content edited in the Client does not update the cache until it's written to disk/IDE, at which point the cache much re-process this unknown file. Since the Client is designed around an autosave principle which updates disk/IDE regularly, there's little gap between the two.
///
/// Cross references
/// ----------------
///
/// A `<xref ref="id"></xref>` is a cross reference to a `Target` or a gather
/// element. The `id` specifies the destination; the cache then hydrates the
/// contents based on the location and contents of the target of the
/// provided `id` to e.g. `<xref ref="id" contenteditable="false"><a
/// href="../path/to/page#id">Inner HTML from target</a></xref>`; this hydrated
/// form is only present in the Client.
///
/// Details:
///
/// * This element does not allow an `id` attribute.
///
/// * The inner HTML is always taken before cache hydration, to prevent circular
///   dependencies:
///
///   ```html
///   <h1 id="a">See <xref ref="b"></xref></h1>   <!-- file a -->
///   <h1 id="b">See <xref ref="a"></xref></h1>   <!-- file b -->
///   ```
///
///   Including cache hydration would cause the inner HTML to be updated each
///   time the file is processed, outdating the other file.
///
/// * The inner HTML is cleaned to allow only the
///   [permitted content for an `<a>` element](https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/a#technical_summary).
///
/// * If the `id` referred to isn't found or refers to a duplicate id, the inner
///   text is instead an appropriate error message.
///
/// * If the cross reference is to a gather element, the text is the gather
///   element's inner HTML, not the gathered code/doc blocks.
///
/// Fragments and gather elements
/// -----------------------------
///
/// A gather element such as `<h3 id="bar" data-gather="id1 id2...">Bazzy
/// things</h3>` is a `Target` with the `data-gather` attribute. It becomes a
/// list of the contents of fragments it refers to after hydration by the cache.
/// An example fragment tag, after cache hydration: `<fragment id="id1"
/// contenteditable="false">See <a href="path/to/gather#bar">Bazzy things</a>,
/// <a href="path/to/another/gather#zap">Zappy things</a></fragment>`. A
/// fragment's content by default includes the contents of the current doc block
/// and the contents of the following code/doc block; fragments are not allowed
/// in Markdown documents (in the case, the fragment contents consist of an error message). Fragments may include the `following` attribute to
/// enclose a specific number of the following code/doc blocks; for example,
/// `<fragment id="bar" following="3"></fragment>` includes the current doc
/// block along with the next 3 code/doc blocks; `following` must be a whole
/// number.
///
/// Details:
///
/// * Fragment contents may not include a gather element; in this case, the
///   gather element list of contents will simply include an error message.
/// * Fragments do support indirection: gather element A includes contents from
///   fragment B, which contains a cross reference to target C. Changes to
///   target C makes B and A outdated.
/// * If a gather element refers to an `id` that is a `Target` or a
///   `GatherElement`, not a `Fragment`, the resulting output for this in the
///   list of fragments is an error message.
/// * If a gather element refers to an `id` that wasn't found or is a duplicate,
///   its contents will be replaced by an error message.
/// * Fragments store an HTML rendering of the code and doc blocks they contain,
///   excluding the content produced by hydrating the `<fragment>` tags, to
///   avoid duplication and circular dependencies.
/// * A `<fragment following=0>` is valid; it contains only the current doc
///   block. The `following` attribute is clamped if it would exceed the number of code/doc blocks in the document. If the `following` value cannot be parsed to a whole number, an error message replaces the fragment content.
/// * A gather element, as a type of `Target`, requires an `id`. A `data-gather`
///   attribute on an element without an `id` produces an error message which
///   requests the missing `id`.
/// * All ids must be valid [CSS identifiers](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Values/ident) per [MDN recommendations](https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Global_attributes/id). Invalid ids produce error messages in the hydrated tag content.
///
/// Example hydration of the gather tag `<h3 id="bar" data-gather="id1
/// id2...">Bazzy things</h3>`:
///
/// ```html
/// <h3 class="cc-gather" id="bar" data-gather="id1 id2...">
///   Bazzy things
/// </h3>
/// <div class="cc-gather-items" contenteditable="false">
///   <p class="cc-gather-item-link">
///     From <a href="link/to/first/tag#id1">Path to file</a>:
///   </p>
///   (first item content)
///   ...
///   <p class="cc-gather-item-link">
///     From <a href="link/to/last/tag#idn">Path to file</a>:
///   </p>
///   (last item content)
/// </div>
/// ```
///
/// Search
/// ------
///
/// The cache supports searching the (cleaned) inner HTML of all `Target`s and gather
/// elements; search does not include `Fragment` contents.
///
/// ### Auto-assignment of ids
///
/// If `id="*"` on either a fragment, target, or gather element, the cache
/// replaces this with an random autogenerated `id` placed in the resulting HTML
/// contents, but this new `id` is not yet recorded in the cache. The file is
/// then marked as `Unknown` when it is saved; when re-read, this `id` is then
/// incorporated into the cache. This helps avoid cases where the cache and file
/// contents become unsynchronized: if the `id` is placed in the cache before
/// the write and the write fails, or if the file is never written (it was being
/// scanned, but not actively edited, so the file contents wasn't written back).
///
/// The autogenerated id must be a valid [CSS identifier](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Values/ident). Autogenerated ids must be checked to make sure they don't collide with an existing id.
///
/// Design
/// ------
///
/// The cache is a single plain-data structure, shared as an
/// `Arc<Mutex<Cache>>`; all consistency comes from that one lock, so no
/// per-item locking (and therefore no lock ordering) is needed. Items refer to
/// each other by key -- files by path, targets and fragments by id -- rather
/// than by `Arc`/`Weak` pointers. This keeps the structure acyclic, `Send`, and
/// (in the future) serializable, and avoids garbage-collecting stale weak
/// references.
///
/// Cached state always converged by design:
///
/// 1. `Target::inner_html`` is a pure function of the source (pre-hydration) — depends on nothing.
/// 2. `<xref>`` hydration depends on layer 1 only.
/// 3. `Fragment::content`` depends on source + layer 2.
/// 4. Gather-list hydration depends on layer 3, and nothing reads layer 4.
///
/// Because no layer reads a higher one, propagation terminates.
///
/// Updating the cache is a two-phase process:
///
/// 1. Collect: while walking a file's DOM, record all cacheable facts
///    (`FileFacts`) -- targets, cross-references, fragments, and gather
///    elements -- without touching the cache. This keeps the non-`Send` DOM
///    types out of the cache and off its critical section. Fact collection
///    ignores content expanded by the cache (the contents of `xref` and
///    `fragment` tags; the content following a gather tag).
/// 2. Commit: `Cache::commit_file` applies the facts in one transaction,
///    diffing them against the file's previous state to compute the set of
///    other files made outdated by this update.
///
/// Dependencies are tracked at file granularity: each target or fragment stores
/// the set of files (its `dependents` and, for gather elements,
/// `dependent_ids`) whose rendered output depends on it. This suffices because
/// the only action ever taken on a dependent is marking its containing file
/// outdated, and it makes indirection (gather A includes fragment B, whose
/// content cross-references target C) work without extra machinery: a change to
/// C outdates B's file; reprocessing B's file changes B's content, which
/// outdates A's file.
///
/// An id referenced before (or without) being defined is recorded in
/// `Cache::unresolved`, which maps the id to the set of files waiting on it.
/// When the id later appears, those files are marked outdated and become the
/// initial dependents; when a defined id disappears, its dependents move back
/// to `unresolved`.
///
/// Duplicate ids are never renamed (file timestamps can't reliably identify the
/// original, and renaming would silently modify user content). Instead,
/// duplicates are reported as errors to the user.
///
/// All file names (stored as `PathBuf`) must be canonicalized absolute paths.
// Imports
// -------
//
// ### Standard library
use std::{
    collections::{HashMap, HashSet},
    fs::Metadata,
    mem,
    path::{Path, PathBuf},
};

// ### Third-party
//
// None.
//
// ### Local
//
// None.

// Data structures
// ---------------
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
    /// All gather elements on this page.
    pub(super) gathers: Vec<String>,
}

/// Given an id, this enum list the possible relationship between an id and the
/// `Target`/gather element or `Fragment` which define that id.
///
/// If an id changes its state (Single -> Multiple, Missing -> Single, etc.), then all dependencies must be `Outdated`; for `Single` or `Multiple`, `resolve_id` will produce a `Target` with the dependencies; for `Missing`, the dependencies are already provided.
pub(super) struct IdEntry {
    state: IdState,
    /// When `state == Missing`, this contains a list of all files that reference it. Otherwise, the contents are:
    ///
    /// - For a `Fragment`, this contains all gather elements referencing this `Fragment`. If this `Fragment`'s
    ///   state changes, then these need to be rebuilt. This represents one half
    ///   of the bidirectional link between `Fragment`s and gather elements derived from processing gather elements.
    ///
    /// - For a `Target` or gather element, this contains all files which cross reference this target. If this `Target`'s
    ///   state changes, then these need to be rebuilt.
    ///
    /// Any change to `IdState` outdates every file in its dependents; likewise, any change to the state of the `Target`/gather element/`Fragment` this id refers to outdates every dependent.
    dependents: HashSet<PathBuf>
}

pub(super) enum IdState {
    /// The expected relationship: this `id` maps to exactly one `Target`/gather
    /// element or `Fragment`.
    Single(PathBuf),
    /// Duplicate id: one id maps to multiple `Target`s/gather elements or
    /// `Fragment`s; the value indicates the number of definitions in that file.
    Multiple(HashMap<PathBuf, u32>),
    /// Ids that appeared in cross-references or gather elements but aren't
    /// (currently) defined by any file.
    Missing
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
    /// The inner HTML of this element. Together with its id and the path which contains this Target, this defines the
    /// state of the `Target` that cross-references depend on. This is a cleaned form (see cross-references) of the inner HTML present in the actual element.
    pub(super) inner_html: String,
    /// IDs gathered, if this is a gather element; empty otherwise. This is additional state that `Fragment`s also depend on. This
    /// represents one half of the bidirectional link between `Fragment`s and
    /// gather elements; this half is directly defined by the gather element.
    pub(super) gather_ids: Vec<String>,
    /// The line number of this target in its file. It is not state; therefore, changes to this do not cause all dependents to be outdated. Always 0 until the
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
    /// The code/doc block content of this element rendered as HTML. Together
    /// with its id, this defines the state of the `Fragment` that gather
    /// elements depend on. This is empty until the fragment's doc blocks are
    /// finalized and the caller stores the result via
    /// `Cache::update_fragment_content`.
    pub(super) content: String,
    /// The line number of this `Fragment` in its file; see `Target::line`.
    pub(super) line: usize,
}

// ### Facts
//
// TODO: this and all following code is outdated with respect to the contents
// above and should be updated.
//
// Plain data collected while walking a file's DOM, then applied to the cache in
// a single transaction by `Cache::commit_file`. Keeping these free of DOM types
// lets the walk run without holding the cache lock.
/// All cacheable facts found in one file.
#[derive(Default)]
pub(super) struct FileFacts {
    /// Every element with an id (excluding fragments), in document order.
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
    /// `CodeDocBlock`s for this file.
    pub(super) doc_block_index: usize,
    pub(super) gather_ids: Vec<String>
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
    /// Multiple files define this id.
    Multiple(&'a HashSet<PathBuf>)
}

// Code
// ----
impl Cache {
    pub fn new() -> Self {
        Cache {
            files: HashMap::new(),
            ids: HashMap::new(),
            pending_files: vec![],
            // TODO: this should be a required parameter to construct the cache.
            root: PathBuf::new(),
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
    /// * Determine if any files containing `Fragment`s need to be rebuilt due
    ///   to changes in the `gather_ids` of the gather element which references
    ///   them.
    ///
    /// Because cross-references carry no id to match them against their
    /// previous versions, the diff instead unlinks all of the old version's
    /// outgoing references, then links all of the new version's. Dependency
    /// sets are only ever mutated by this round trip, never used to detect
    /// change, so it causes no spurious rebuilds.
    ///
    /// TODO: this implementation should be revised based on changes made to the
    /// spec but not yet committed.
    ///
    /// TODO: add a `remove_file` function, which probably calls this with an empty set of `FileFacts` to remove all data that used to be in this file first.
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

        // ### Unlink the old version's outgoing references
        //
        // Remove this file from the dependents of every id its previous version
        // referenced; the new version's references are linked below.
        let old_refs: Vec<String> = if let Some(entry) = self.files.get(path) {
            entry
                .xrefs
                .iter()
                .map(|xref| xref.id.clone())
                .chain(
                    entry
                        .gathers
                        .iter()
                        .flat_map(|gather| gather.ids.iter().cloned()),
                )
                .collect()
        } else {
            Vec::new()
        };
        for id in &old_refs {
            self.unlink_reference(id, path);
        }

        // ### Take the old definitions
        //
        // Move the previous targets and fragments out of the entry (creating
        // the entry if this is the first commit for this file). Definitions
        // which survive into the new version are matched by id and their
        // dependents carried over; the leftovers are deletions.
        let entry = self.files.entry(path.to_path_buf()).or_default();
        let mut old_targets = mem::take(&mut entry.targets);
        let mut old_fragments = mem::take(&mut entry.fragments);

        // The set of ids the new version references, saved before `facts` is
        // consumed; used to link references below.
        let new_refs: HashSet<String> = facts
            .xrefs
            .iter()
            .cloned()
            .chain(
                facts
                    .gathers
                    .iter()
                    .flat_map(|gather| gather.ids.iter().cloned()),
            )
            .collect();

        // ### Install the new targets
        let mut new_targets: HashMap<String, Target> = HashMap::with_capacity(facts.targets.len());
        for fact in facts.targets {
            // An id already defined by another file is a duplicate: the first
            // definition wins, so skip this one.
            if let Some(owner) = self.ids.get(&fact.id)
                && owner != path
            {
                duplicates.push(DuplicateId {
                    id: fact.id,
                    defined_in: owner.clone(),
                });
                continue;
            }
            // An id defined twice within this file is likewise a duplicate.
            if new_targets.contains_key(&fact.id) {
                duplicates.push(DuplicateId {
                    id: fact.id,
                    defined_in: path.to_path_buf(),
                });
                continue;
            }
            self.ids.insert(fact.id.clone(), path.to_path_buf());
            let dependents = if let Some(old_target) = old_targets.remove(&fact.id) {
                // The target survives. If the state cross-references depend on
                // changed, its dependents must be rebuilt; either way, they
                // remain dependents.
                if old_target.inner_html != fact.inner_html {
                    outdated.extend(old_target.dependents.iter().cloned());
                }
                old_target.dependents
            } else if let Some(waiters) = self.unresolved.remove(&fact.id) {
                // The id was referenced before it existed: the files waiting on
                // it must be rebuilt, and they become its dependents.
                outdated.extend(waiters.iter().cloned());
                waiters
            } else {
                HashSet::new()
            };
            new_targets.insert(
                fact.id,
                Target {
                    inner_html: fact.inner_html,
                    dependents,
                    line: fact.line,
                    doc_block_index: fact.doc_block_index,
                },
            );
        }

        // ### Install the new fragments
        //
        // Same logic as targets; targets and fragments share the id namespace.
        let mut new_fragments: HashMap<String, Fragment> =
            HashMap::with_capacity(facts.fragments.len());
        for fact in facts.fragments {
            if let Some(owner) = self.ids.get(&fact.id)
                && owner != path
            {
                duplicates.push(DuplicateId {
                    id: fact.id,
                    defined_in: owner.clone(),
                });
                continue;
            }
            if new_targets.contains_key(&fact.id) || new_fragments.contains_key(&fact.id) {
                duplicates.push(DuplicateId {
                    id: fact.id,
                    defined_in: path.to_path_buf(),
                });
                continue;
            }
            self.ids.insert(fact.id.clone(), path.to_path_buf());
            let (content, dependents) = if let Some(old_fragment) = old_fragments.remove(&fact.id) {
                // The fragment survives: keep its old content until the caller
                // supplies the new content via `update_fragment_content`, which
                // also detects content changes.
                (old_fragment.content, old_fragment.dependents)
            } else if let Some(waiters) = self.unresolved.remove(&fact.id) {
                outdated.extend(waiters.iter().cloned());
                (String::new(), waiters)
            } else {
                (String::new(), HashSet::new())
            };
            new_fragments.insert(
                fact.id,
                Fragment {
                    content,
                    dependents,
                    line: fact.line,
                    doc_block_start_index: fact.doc_block_start_index,
                    code_doc_block_end_index: fact.code_doc_block_end_index,
                },
            );
        }

        // ### Process deleted definitions
        //
        // Anything left in the old maps wasn't matched by a same-kind
        // definition in the new version. Its dependents must be rebuilt. If the
        // id changed kind (target to fragment or vice versa) the dependents
        // transfer to the new definition; otherwise the id is gone and its
        // dependents wait in `unresolved` for it to reappear.
        for (id, old_target) in old_targets {
            outdated.extend(old_target.dependents.iter().cloned());
            if let Some(new_fragment) = new_fragments.get_mut(&id) {
                new_fragment.dependents.extend(old_target.dependents);
            } else {
                if self.ids.get(&id).is_some_and(|owner| owner == path) {
                    self.ids.remove(&id);
                }
                if !old_target.dependents.is_empty() {
                    self.unresolved
                        .entry(id)
                        .or_default()
                        .extend(old_target.dependents);
                }
            }
        }
        for (id, old_fragment) in old_fragments {
            outdated.extend(old_fragment.dependents.iter().cloned());
            if let Some(new_target) = new_targets.get_mut(&id) {
                new_target.dependents.extend(old_fragment.dependents);
            } else {
                if self.ids.get(&id).is_some_and(|owner| owner == path) {
                    self.ids.remove(&id);
                }
                if !old_fragment.dependents.is_empty() {
                    self.unresolved
                        .entry(id)
                        .or_default()
                        .extend(old_fragment.dependents);
                }
            }
        }

        // ### Store the new state
        let entry = self.files.get_mut(path).expect("entry was created above");
        entry.metadata = metadata;
        // TODO: if this contained autogenerated IDs, mark this as "unknown if
        // saved".
        entry.status = FileStatus::UpToDate;
        entry.targets = new_targets;
        entry.fragments = new_fragments;
        entry.xrefs = facts.xrefs.into_iter().map(|id| Xref { id }).collect();
        entry.gathers = facts
            .gathers
            .into_iter()
            .map(|fact| GatherElement {
                ids: fact.ids,
                target: Target {
                    inner_html: fact.inner_html,
                    // TODO: fix. This should be a `xref`s to this
                    // `GatherElement`.
                    dependents: HashSet::new(),
                    // TODO: fix.
                    line: 0,
                    doc_block_index: fact.doc_block_index,
                },
            })
            .collect();

        // ### Link the new version's outgoing references
        //
        // Add this file to the dependents of every id it references; ids with
        // no definition go to `unresolved`.
        for id in &new_refs {
            if let Some(owner) = self.ids.get(id) {
                let owner = owner.clone();
                if let Some(owner_entry) = self.files.get_mut(&owner) {
                    if let Some(target) = owner_entry.targets.get_mut(id) {
                        target.dependents.insert(path.to_path_buf());
                        continue;
                    }
                    if let Some(fragment) = owner_entry.fragments.get_mut(id) {
                        fragment.dependents.insert(path.to_path_buf());
                        continue;
                    }
                }
            }
            self.unresolved
                .entry(id.clone())
                .or_default()
                .insert(path.to_path_buf());
        }

        // ### Mark outdated files
        //
        // A file never outdates itself: its rendered output was just produced
        // from the state committed here.
        outdated.remove(path);
        for outdated_path in &outdated {
            if let Some(outdated_entry) = self.files.get_mut(outdated_path) {
                outdated_entry.status = FileStatus::Outdated;
            }
        }

        CommitOutcome {
            outdated,
            duplicates,
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
        let mut outdated = HashSet::new();
        if let Some(entry) = self.files.get_mut(path)
            && let Some(fragment) = entry.fragments.get_mut(id)
            && fragment.content != content
        {
            fragment.content = content;
            outdated = fragment.dependents.clone();
            // Gather elements in the fragment's own file are updated by the
            // caller in the same processing pass.
            outdated.remove(path);
            for outdated_path in &outdated {
                if let Some(outdated_entry) = self.files.get_mut(outdated_path) {
                    outdated_entry.status = FileStatus::Outdated;
                }
            }
        }
        outdated
    }

    /// Look up an id, returning the target or fragment it names along with the
    /// defining file, or `Missing` if no file defines it.
    pub(super) fn resolve_id(&self, id: &str) -> IdResolution<'_> {
        if let Some(owner) = self.ids.get(id)
            && let Some(entry) = self.files.get(owner)
        {
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

    /// Remove this file from the dependents of the given id (or from the
    /// unresolved waiters, if the id has no definition).
    fn unlink_reference(
        &mut self,
        // The referenced id.
        id: &str,
        // The file which contained the reference.
        referrer: &Path,
    ) {
        if let Some(owner) = self.ids.get(id) {
            let owner = owner.clone();
            if let Some(entry) = self.files.get_mut(&owner) {
                if let Some(target) = entry.targets.get_mut(id) {
                    target.dependents.remove(referrer);
                } else if let Some(fragment) = entry.fragments.get_mut(id) {
                    fragment.dependents.remove(referrer);
                }
            }
        } else if let Some(waiters) = self.unresolved.get_mut(id) {
            waiters.remove(referrer);
            if waiters.is_empty() {
                self.unresolved.remove(id);
            }
        }
    }
}

impl Default for Cache {
    fn default() -> Self {
        Cache::new()
    }
}

// Tests
// -----
#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        path::{Path, PathBuf},
    };

    use super::{
        Cache, DuplicateId, FileFacts, FileStatus, FragmentFact, GatherFact, IdResolution,
        TargetFact,
    };

    // ### Test helpers
    //
    // Build a target fact with unimportant location info.
    fn target_fact(id: &str, inner_html: &str) -> TargetFact {
        TargetFact {
            id: id.to_string(),
            inner_html: inner_html.to_string(),
            line: 0,
            doc_block_index: 0,
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

    // Shorthand for the status of a cached file.
    fn status<'a>(cache: &'a Cache, path: &Path) -> &'a FileStatus {
        &cache.files[path].status
    }

    // Verify that a cross-reference to an existing target records the
    // dependency and resolves.
    #[test]
    fn test_xref_to_existing_target() {
        let mut cache = Cache::new();
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");

        let outcome = cache.commit_file(&a, None, facts_target("foo", "Foo!"));
        assert!(outcome.outdated.is_empty());
        assert!(outcome.duplicates.is_empty());

        let outcome = cache.commit_file(&b, None, facts_xref("foo"));
        assert!(outcome.outdated.is_empty());

        // The target must know its dependent and resolve to its defining file.
        let IdResolution::Target { path, target } = cache.resolve_id("foo") else {
            panic!("expected a target");
        };
        assert_eq!(path, a);
        assert_eq!(target.inner_html, "Foo!");
        assert_eq!(target.dependents, HashSet::from([b.clone()]));
    }

    // Verify that referencing an id before its definition marks the referring
    // file outdated when the definition appears.
    #[test]
    fn test_forward_reference() {
        let mut cache = Cache::new();
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");

        cache.commit_file(&b, None, facts_xref("foo"));
        assert!(matches!(cache.resolve_id("foo"), IdResolution::Missing));
        assert_eq!(cache.unresolved["foo"], HashSet::from([b.clone()]));

        // Defining the id resolves the reference: `b` must be rebuilt and
        // becomes a dependent.
        let outcome = cache.commit_file(&a, None, facts_target("foo", "Foo!"));
        assert_eq!(outcome.outdated, HashSet::from([b.clone()]));
        assert_eq!(*status(&cache, &b), FileStatus::Outdated);
        assert!(cache.unresolved.is_empty());
        let IdResolution::Target { target, .. } = cache.resolve_id("foo") else {
            panic!("expected a target");
        };
        assert_eq!(target.dependents, HashSet::from([b.clone()]));
    }

    // Verify that only a change to a target's content outdates its dependents.
    #[test]
    fn test_target_change_outdates_dependents() {
        let mut cache = Cache::new();
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
        assert_eq!(outcome.outdated, HashSet::from([b.clone()]));
        assert_eq!(*status(&cache, &b), FileStatus::Outdated);
    }

    // Verify that deleting a target moves its dependents to `unresolved`, and
    // that a later re-definition (in another file) finds them again.
    #[test]
    fn test_target_deletion() {
        let mut cache = Cache::new();
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");
        let c = PathBuf::from("c.md");
        cache.commit_file(&a, None, facts_target("foo", "Foo!"));
        cache.commit_file(&b, None, facts_xref("foo"));

        // Delete the target: the dependent is rebuilt and now waits on the id.
        let outcome = cache.commit_file(&a, None, FileFacts::default());
        assert_eq!(outcome.outdated, HashSet::from([b.clone()]));
        assert!(matches!(cache.resolve_id("foo"), IdResolution::Missing));
        assert_eq!(cache.unresolved["foo"], HashSet::from([b.clone()]));

        // The id reappears in a different file: the waiter is rebuilt again.
        let outcome = cache.commit_file(&c, None, facts_target("foo", "Foo!"));
        assert_eq!(outcome.outdated, HashSet::from([b.clone()]));
    }

    // Verify that duplicate ids are reported.
    #[test]
    fn test_duplicate_ids() {
        let mut cache = Cache::new();
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");
        cache.commit_file(&a, None, facts_target("foo", "Foo!"));

        // A duplicate in another file loses to the existing definition.
        let outcome = cache.commit_file(&b, None, facts_target("foo", "Imposter!"));
        assert_eq!(
            outcome.duplicates,
            vec![DuplicateId {
                id: "foo".to_string(),
                defined_in: a.clone()
            }]
        );
        let IdResolution::Target { path, target } = cache.resolve_id("foo") else {
            panic!("expected a target");
        };
        assert_eq!(path, a);
        assert_eq!(target.inner_html, "Foo!");
        assert!(cache.files[&b].targets.is_empty());

        // A duplicate within a single file is reported against that file.
        let outcome = cache.commit_file(
            &b,
            None,
            FileFacts {
                targets: vec![target_fact("bar", "1"), target_fact("bar", "2")],
                ..Default::default()
            },
        );
        assert_eq!(
            outcome.duplicates,
            vec![DuplicateId {
                id: "bar".to_string(),
                defined_in: b.clone()
            }]
        );
    }

    // Verify that removing a cross-reference unlinks the dependency.
    #[test]
    fn test_unlink_on_recommit() {
        let mut cache = Cache::new();
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");
        cache.commit_file(&a, None, facts_target("foo", "Foo!"));
        cache.commit_file(&b, None, facts_xref("foo"));

        // Recommit `b` without the cross-reference; changing the target must no
        // longer outdate `b`.
        cache.commit_file(&b, None, FileFacts::default());
        let outcome = cache.commit_file(&a, None, facts_target("foo", "Bar!"));
        assert!(outcome.outdated.is_empty());
    }

    // Verify the fragment/gather flow: a gather element depends on a fragment,
    // and only a change to the fragment's content outdates the gathering file.
    #[test]
    fn test_fragment_gather_flow() {
        let mut cache = Cache::new();
        let a = PathBuf::from("a.py");
        let b = PathBuf::from("b.py");

        cache.commit_file(
            &a,
            None,
            FileFacts {
                fragments: vec![fragment_fact("frag")],
                ..Default::default()
            },
        );
        cache.commit_file(
            &b,
            None,
            FileFacts {
                gathers: vec![GatherFact {
                    ids: vec!["frag".to_string()],
                    inner_html: "Gathered".to_string(),
                    doc_block_index: 0,
                }],
                ..Default::default()
            },
        );
        let IdResolution::Fragment { fragment, .. } = cache.resolve_id("frag") else {
            panic!("expected a fragment");
        };
        assert_eq!(fragment.dependents, HashSet::from([b.clone()]));

        // Storing the fragment's first content outdates the gathering file;
        // storing identical content afterwards does not.
        let outdated = cache.update_fragment_content(&a, "frag", "content".to_string());
        assert_eq!(outdated, HashSet::from([b.clone()]));
        assert_eq!(*status(&cache, &b), FileStatus::Outdated);
        let outdated = cache.update_fragment_content(&a, "frag", "content".to_string());
        assert!(outdated.is_empty());
    }

    // Verify that an id changing kind (target to fragment) transfers its
    // dependents and rebuilds them.
    #[test]
    fn test_id_changes_kind() {
        let mut cache = Cache::new();
        let a = PathBuf::from("a.py");
        let b = PathBuf::from("b.py");
        cache.commit_file(&a, None, facts_target("foo", "Foo!"));
        cache.commit_file(&b, None, facts_xref("foo"));

        // The id becomes a fragment: the cross-referencing file must be rebuilt
        // (its cross-reference is now an error), and the dependency edge
        // transfers.
        let outcome = cache.commit_file(
            &a,
            None,
            FileFacts {
                fragments: vec![fragment_fact("foo")],
                ..Default::default()
            },
        );
        assert_eq!(outcome.outdated, HashSet::from([b.clone()]));
        let IdResolution::Fragment { fragment, .. } = cache.resolve_id("foo") else {
            panic!("expected a fragment");
        };
        assert_eq!(fragment.dependents, HashSet::from([b.clone()]));
    }
}
