// Copyright (C) 2025 Bryan A. Jones.
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
/// `cache.rs` - Keep a cache used to store all targets in a project.
/// ============================================================================
///
/// The cache stores the location (file name and anchor), numbering (of headings
/// in the TOC and figures/equations/etc. on a page), and contents (title text
/// or code/doc blocks for tags) of a target. Targets are HTML anchors (such as
/// headings, figure titles, display equations, tags, etc.), hyperlinks, gather
/// elements, or files.
///
/// The goal of the cache is to support auto-titled links, backlinks, and gather
/// elements, and to ensure that all anchors are unique within a project. This
/// means that links persist across moving or renaming files, since the anchors
/// will be found in the cache.
///
/// Auto-titled links
/// ----------------------------------------------------------------------------
///
/// A hyperlink with an empty title is auto-titled -- the contents of the anchor
/// it references provide the contents of the link. For example, after
/// processing, the link in the following Markdown
///
/// ```Markdown
/// <h1 id="foo">Bar</h1>
///
/// [](#foo)
/// ```
///
/// becomes `[Bar](#foo)`. This works even when the anchor is located in a
/// different file. Auto-titled links don't support indirection: link A whose
/// contents comes from link B whose contents comes from target C doesn't work.
///
/// Tags
/// ----------------------------------------------------------------------------
///
/// A gather element such as `<cc-gather id="baz">Bazzy things</cc-gather>` becomes a list of the
/// contents of tags which reference it after processing by the cache. A tag is
/// simply a link to a gather element, such as `[](#baz)`, which becomes `[Bazzy things](#baz)` after auto-titling. The tag's content by
/// default includes the contents of the current doc block and the contents of
/// the next code/doc block. Tags can also include an end query parameter to
/// enclose a wider range of code/doc blocks.
///
/// Tag contents may not include a gather element. They do support indirection:
/// gather element A includes contents from tag B, which contains an auto-titled
/// link to target C. Changes to target C makes B and A dirty.
///
/// Backlinks
/// ----------------------------------------------------------------------------
///
/// Given an anchor, this produces a list of backlinks which reference it. This
/// would provide a way to create an index, or show what references
/// footnotes/endnotes, etc. Backlinks are like gather elements, but instead of
/// capuring tag contents, they produce links, locations, and/or backlink
/// contents. While they are similar to a gather element, the difference is in
/// which content is included. In addition, backlinks don't support indirect
/// dependencies: backlink A, which link B refernces, doesn't depend on link B's
/// auto-titled text from target C.
///
/// Search
/// ----------------------------------------------------------------------------
///
/// The cache supports searching the contents of all Targets.
///
/// Goals
/// ----------------------------------------------------------------------------
///
/// * Given a path to a file, retrieve the associated location,
///   numbering, and contents (a list of all targets in the containing file).
/// * Perform a search of all Target contents, returning a list of matching
///   targets.
/// * Given an anchor, retrieve the anchor's Target, all Targets which reference this anchor but don't depend on it, and all Targets which reference this anchor and also depend on it.
///
/// Supported operations:
///
/// * Upsert a given file to the cache.
/// * Delete a given file from the cache.
/// * Walk the project, updating the cache for all files. Called when a project
///   is first opened.
/// * Monitor the project for filesystem changes, performing lazy updates based
///   on these changes.
/// * Keep the in-memory cache synchronized with the on-disk cache.
///
/// Thinking space:
///
/// * Any file can be submitted for a cache update. After the update finishes,
///   the Server checks to see if this update was to the file currently being
///   editing in the Client.
/// * Non-project files support a subset of this functionality: basically, treat
///   the project as a single file. Backlinks to other files work; tags and
///   backlinks within the current file work.
///
/// I'm concerned about keeping this struct coherent through changes. For
/// example, removing a target means removing any backlinks that refer to it.
/// Removing a file means removing all backlinks that refer to any target in
/// that file and all dependencies on this file. How do we get them all?
///
/// Perhaps we lazy manage backlinks: keep "primary" allocations an Arc, and backlinks/secondary allocations a backlink. For example, the primary references to a File is in Cache.path; everything else is a weak link. Removing it from Cache.path is all I need to do to delete a file.
///
/// Also, I just feel a bit lost. Even after writing about it, I can't quite
/// keep the whole structure in my mind. Perhaps I just need to write some tests
/// or some code. Writing sample tests certainly seems to help me think.
/// Should I keep track of dependencies at the file level, or target level? In the end, a target depends on another target, not on a file. This seems like a backlinks question.
// Imports
// -----------------------------------------------------------------------------
//
// ### Standard library
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, Mutex, Weak},
};

// ### Third-party
use markup5ever_rcdom::Node;

// ### Local
use crate::lexer::CodeDocBlock;

/// Data structures
/// ----------------------------------------------------------------------------
///
/// This defines the cache used to store all targets in a project.
pub struct Cache {
    /// Provide rapid access to a file by its absolute path; it must be within
    /// the project's root directory.
    path: HashMap<PathBuf, Arc<Mutex<File>>>,
    /// Provide rapid access to a `Target` by its unique anchor.
    pub(super) anchor: HashMap<String, AnchorTarget>,
    /// All dirty files.
    dirty: HashSet<PathBuf>,
    /// The root directory of this project.
    pub(super) root: PathBuf,
}

/// Targets may depend on data from another file within
/// this project. Typically, these are auto-titled hyperlinks or backlinks.
/// If this Target is a gather element, this contains both direct
/// dependencies (backlinks for the gather element's anchor) and indirect
/// dependencies (dependencies of each of these backlinks).
///
/// This contains all the data behind an anchor.
struct AnchorTarget {
    /// The Target itself.
    target: Weak<Target>,
    /// All references to this target which don't depend on it.
    references: HashSet<Weak<Target>>,
    /// All references to this target that also depend on it; if this Target changes, all these must be updated.
    dependencies: HashSet<Weak<Target>>
}

/// This stores metadata for given file. For non-page files (non-existent files,
/// images, PDFs, etc.) many of the fields are empty or `None`. For page files,
/// additional information is placed in the first element of `target`.
pub(super) struct File {
    /// The full path to this file; it must be within the project's root
    /// directory. This file may not exist -- it could be created by a broken
    /// link.
    pub(super) path: PathBuf,
    /// The TOC's numbering for this file; empty if it's either not in the TOC,
    /// or is a prefix/suffix chapter.
    pub(super) toc: Vec<u32>,
    /// All targets on this page, in order of appearance on the page.
    pub(super) target: Vec<Arc<Mutex<Target>>>,
    /// The first (and hopefully only) H1 target on this page.
    pub(super) h1: Weak<Target>,
}

/// Contains all information about a target.
pub(super) struct Target {
    /// The file which contains this target.
    pub(super) file: Weak<Mutex<File>>,
    /// The id (which functions as an anchor) of this target, if assigned; empty
    /// otherwise. It must be globally unique with the project.
    pub(super) anchor: String,
    /// The DOM node which defines this target. TODO: This isn't Sync, so store it elsewhere.
    //node: Weak<Node>,
    /// The line number of this target in `File`; ignored if the `type_` is
    /// `File`.
    pub(super) line: u32,
    /// The type of this target.
    pub(super) type_: TargetType,
    /// The HTML contents (or HTML context, if this target has no content, such
    /// as `<a id="x"></a>`). Tags, which contain multiple code and doc blocks,
    /// must be rendered to static HTML.
    pub(super) contents: String,
}

/// Stores data unique to each type of target.
pub(super) enum TargetType {
    /// This target is a file.
    File(Weak<File>),
    /// A heading tag (H1-H6).
    Heading {
        /// The level (1-6 for H1-H6) of this heading.
        level: u32,
    },
    /// An HTML element with only an id, which must be globally unique in this
    /// project. Termed an
    /// [anchor](https://developer.mozilla.org/en-US/docs/Learn_web_development/Howto/Web_mechanics/What_is_a_URL#anchor)
    /// or a document fragment identifier.
    Anchor,
    /// A tag, which is a link to a gathering tag.
    Tag {
        /// The anchor this hyperlink references.
        anchor: String,
        /// The index into this page's CodeDocBlock vec where the tag starts.
        start: usize,
        /// The index into this page's CodeDocBlock vec where the tag ends.
        end: usize,
    },
    /// A gathering tag.
    GatherTag {
        /// The name of this tag, used as the auto-title text for referencing
        /// links.
        name: String,
    },
    /// A numbered item, such as a figure caption, an equation, a table, etc.
    Numbered {
        /// The number of this item.
        number: u32,
        /// A string identifying the type of this number: equation, table, etc.
        /// TODO: a central registry for these? Or use an enum and pre-defined
        /// types?
        type_: String,
    },
    /// A hyperlink to a location within this project.
    Link {
        /// The file this hyperlink references.
        file: Weak<Mutex<File>>,
        /// The anchor this hyperlink references. If no anchor is provided, this
        /// is an empty string.
        anchor: String,
        /// Recognized query parameters.
        flags: LinkOptions,
    },
}

/// Query parameters parsed into known link options. TODO: perhaps use the
/// bitflags crate instead?
pub(super) enum LinkOptions {
    Plain,
    AutoTitle,
    AutoNumber,
    AutoTitleAndNumber,
}

// Code
// -----------------------------------------------------------------------------
impl Cache {
    pub fn new() -> Self {
        Cache {
            path: HashMap::new(),
            anchor: HashMap::new(),
            dirty: HashSet::new(),
            root: PathBuf::new(),
        }
    }
    /// Look up or create a `File` entry in the cache for the given path.
    pub(super) fn get_or_create_file(&mut self, path: &Path) -> Arc<Mutex<File>> {
        self.path
            .entry(path.to_path_buf())
            .or_insert_with(|| {
                /// FIXME: add new file to
                Arc::new(Mutex::new(File {
                    path: path.to_path_buf(),
                    toc: vec![],
                    h1: Weak::new(),
                    target: vec![],
                }))
            })
            .clone()
    }

    // ### Upsert
    //
    // Update the cache using the contents of the provided file.
    pub fn upsert_file_core(
        &mut self,
        // The file to process.
        file: &Path,
        // DOM of file to process.
        dom: Rc<Node>,
        // True indicates the file is now clean in the cache; false indicates
        // that it's still dirty, since it requires information from dirty
        // dependencies.
    ) -> bool {
        false
        // Pseudocode:
        //
        // 1. Upsert this file's page data structure. Pre-existing cache data
        //    provides the TOC numbering.
        // 2. Set numbering of all numbered items to 1.
        // 3. Walk the DOM. For each item in the walk:
        //    1. If this is a doc block separator, read and update the current
        //       doc block index.
        //    2. If this item is a target, upsert the target's data structure to
        //       the page's vector of targets and the cache state.
        //       1. Update the current numbering if this is a numbered item
        //          (equation, caption, etc.) and insert the HTML to set its
        //          number in the DOM.
        //       2. For tags: look up the tag by ID in the cache. Set the start
        //          and end indices: if this is a plain tag (no end query
        //          parameter), set the start index to the current block and the
        //          end index to the next block. If this tag includes an end
        //          query parameter, set the end index of the current tag, or
        //          (if there's no tag) set the start index to the current
        //          index - 1. How to handle links to files not in the cache? The simplest approach is to mark this as dirty, then re-resolve.
        //       3. If this link is auto-titled, add it to the page's map of
        //          dependencies.
        //       4. For anything with an anchor, check that this is unique in
        //          the project. Add it to the backlinks.
        //       5. For links, add this to the backlinks.
        // 4. For each item in the map of dependencies:
        //    1. Look for it in the cache. If it's not in the cache or if the
        //       cache for that file is dirty, add this file and the referring
        //       file to the set of dirty files.
        //    2. Update the auto-titled text and gather elements using cached
        //       data; if not in the cache use the title "pending."
    }
}

#[cfg(test)]
mod tests {
    use std::{
        borrow::BorrowMut,
        collections::{HashMap, HashSet},
        sync::{Arc, Mutex, Weak},
    };

    use indoc::indoc;
    use test_utils::prep_test_dir;

    use crate::processing::cache::{Cache, File, TargetCore};

    // Verify basic parsing
    fn test_1() {
        let (temp_dir, test_dir) = prep_test_dir!();
        let bar_cpp = indoc!(
            r#"
            // # Heading 1
            //
            // ## Heading 2
            //
            // <a id="anchor"></a>
            //
            // [File link](bar.cpp)
            //
            // [anchor link](bar.cpp#one)
            //
            // [][baz.cpp)
            //
            // [](baz.cpp#one)
            //
            // [](baz.cpp#one?number)
            //
            // [](baz.cpp#one?title&number)
            //
            // [][bar.cpp#gathering_tag)
            code();
            "#
        );

        let bar_cpp_path = test_dir.join("bar.cpp");
        let file_bar_cpp = Arc::new(Mutex::new(File {
            path: bar_cpp_path.clone(),
            toc: vec![1],
            // Since we haven't parsed the file, the `h1` hasn't been found.
            h1: Weak::new(),
            // Same for targets.
            target: vec![],
        }));
        let baz_cpp_path = test_dir.join("baz.cpp");

        // Create a baz file that's been processed. It contains one heading and a gather tag.
        let mut file_baz_cpp = Arc::new(Mutex::new(File {
            path: baz_cpp_path.clone(),
            toc: vec![2],
            // This is filled in below.
            h1: Weak::new(),
            // This is filled in below.
            target: vec![],
        }));
        file_baz_cpp.borrow_mut().lock().unwrap().target = vec![
            Arc::new(Mutex::new(TargetCore {
                file: file_baz_cpp.clone(),
                anchor: "one".to_string(),
                line: 1,
                type_: crate::processing::cache::TargetType::Heading { level: 1 },
                contents: "Heading one".to_string(),
            })),
            Arc::new(Mutex::new(TargetCore {
                file: file_baz_cpp.clone(),
                anchor: "gathering_tag".to_string(),
                line: 1,
                type_: crate::processing::cache::TargetType::GatherTag {
                    name: "gather".to_string(),
                },
                contents: "Gather tag".to_string(),
            })),
        ];
        let h1 = Arc::downgrade(&file_baz_cpp.lock().unwrap().target[1]);
        file_baz_cpp.borrow_mut().lock().unwrap().h1 = h1;

        let mut cache_path = HashMap::new();
        cache_path.insert(bar_cpp_path, file_bar_cpp);
        cache_path.insert(baz_cpp_path, file_baz_cpp.clone());
        let mut cache_id = HashMap::new();
        cache_id.insert(
            "one".to_string(),
            file_baz_cpp.lock().unwrap().target[0].clone(),
        );
        cache_id.insert(
            "gathering_tag".to_string(),
            file_baz_cpp.lock().unwrap().target[1].clone(),
        );
        let mut cache_anchor = HashMap::new();

        let mut cache = Cache {
            path: cache_path,
            anchor: cache_anchor,
            backlink: HashMap::new(),
            dependents: HashMap::new(),
            dirty: HashSet::new(),
            root: test_dir,
        };

        // Processing a file updates its values in the cache.
        //cache.upsert_file_core(&bar_cpp_path, );

        temp_dir.close().unwrap();
    }
}
