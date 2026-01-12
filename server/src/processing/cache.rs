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
/// The goal of the cache is to support auto-titled links and gather elements,
/// and to ensure that all anchors are unique within a project. This means that
/// links persists across moving or renaming files, since the anchors will be
/// found in the cache.
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
/// [](#bar)
/// ```
///
/// becomes `[Bar](#bar)`. This works even when the anchor is located in a
/// different file. Auto-titled links don't support indirection: link A whose
/// contents comes from link B whose contents comes from link C doesn't work.
///
/// Tags
/// ----------------------------------------------------------------------------
///
/// A gather element such as `<cc-gather id="baz"/>` becomes a list of the
/// contents of tags which reference it after processing by the cache. A tag is
/// simply a link to a gather element, such as `[](#baz)`. The tag's content by
/// default includes the contents of the current doc block and the contents of
/// the next code/doc block. Tags can also include start and end query
/// parameters to enclose a wider range of code/doc blocks.
///
/// Tag contents may not include a gather element. They do support indirection:
/// gather element A includes contents from tag B, which contains an auto-titled
/// link to target C. Changes to target C makes B and A dirty.
///
/// Backlinks
/// ----------------------------------------------------------------------------
///
/// I want a way to create a list of backlinks to an anchor. This would provide
/// a way to create an index, or show what references footnotes/endnotes, etc.
/// Backlinks are like gather elements, but instead of capuring tag contents,
/// they produce links, locations, and/or backlink contents. They are therefore
/// very similar to a gather element; the difference is in which content is
/// included. In addition, backlinks don't support indirect dependencies:
/// backlink A, which link B refernces, doesn't depend on link B's auto-titled
/// text from target C.
///
/// Search
/// ----------------------------------------------------------------------------
///
/// The cache supports searching the contents of all Targets.
///
/// Goals
/// ----------------------------------------------------------------------------
///
/// * Given a file name and/or anchor, retrieve the associated location,
///   numbering, and contents.
/// * Perform a search of all Target contents, returning a list of matching
///   targets.
/// * Given a file name and/or anchor, provide a list of all targets in the
///   containing file.
/// * Given an anchor, retrieve all Targets which reference this anchor.
/// * If a File is modified (becomes dirty in the cache), return a list of Files
///   which depend (directly or indirectly) on this File.
/// * After processing a file (so that it becomes clean in the cache), return a
///   list of Files whose dependencies are all clean, meaning they should be
///   processed.
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
// Imports
// -----------------------------------------------------------------------------
//
// ### Standard library
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    sync::{Arc, Mutex},
};

// ### Third-party
use markup5ever_rcdom::Node;

// ### Local
use crate::lexer::CodeDocBlock;

/// Data structures
/// ----------------------------------------------------------------------------
///
/// This defines the cache used to store all targets in a project.
struct Cache {
    /// Provide rapid access to a file by its absolute path; it must be within
    /// the project's root directory.
    path: HashMap<PathBuf, Arc<Mutex<File>>>,
    /// Provide rapid access to a `Target` by its unique anchor.
    anchor: HashMap<String, Target>,
    /// Backlinks: given an anchor, this contains all targets which reference
    /// this anchor.
    backlink: HashMap<String, Vec<Target>>,
    /// Given a File, retrieve a list all files which depend on this it.
    dependents: HashMap<File, Vec<File>>,
    /// When this file becomes clean, provide a list of
    dirty_dependents: HashMap<File, HashSet<File>>,
    /// The root directory of this project.
    root: PathBuf,
}

/// This stores metadata for given file. For non-page files (non-existent files,
/// images, PDFs, etc.) many of the fields are empty or `None`. For page files,
/// additional information is placed in the first element of `target`.
struct File {
    /// The full path to this file; it must be within the project's root
    /// directory. This file may not exist -- it could be created by a broken
    /// link.
    path: PathBuf,
    /// The cache state of this item.
    state: State,
    /// The TOC's numbering for this file; empty if it's either not in the TOC,
    /// or is a prefix/suffix chapter.
    toc: Vec<u32>,
    /// All targets on this page, in order of appearance on the page.
    target: Vec<Target>,
    /// The first (and hopefully only) H1 target on this page.
    h1: Option<Target>,
    /// All targets on this page which depend on data from another file within
    /// this project. Typically, these are auto-titled hyperlinks.
    dependency: Vec<Target>,
}

enum State {
    Clean,
    Dirty,
    Pending,
}

/// Contains all information about a target.
struct TargetCore {
    /// The file which contains this target.
    file: Arc<Mutex<File>>,
    /// The id of this target, if assigned; empty otherwise. It must be globally
    /// unique with the project.
    id: String,
    /// The DOM node which defines this target.
    node: Weak<Node>,
    /// The line number of this target in `file`; ignored if the `type_` is
    /// `File`.
    line: u32,
    /// The type of this target.
    type_: TargetType,
    /// The contents (or context, if this target has no content, such as `<a
    /// id="x"></a>`).
    contents: TargetContents,
}

/// A Target is always reference-counted, since its data is available from
/// multiple points in these data structures.
type Target = Arc<Mutex<TargetCore>>;

/// Stores data unique to each type of target.
enum TargetType {
    File,
    Heading {
        /// The level (1-6 for H1-H6) of this heading.
        level: u32,
        /// The section number of this heading.
        number: u32,
    },
    /// An HTML element with an id, which must be globally unique in this
    /// project. Termed an
    /// [anchor](https://developer.mozilla.org/en-US/docs/Learn_web_development/Howto/Web_mechanics/What_is_a_URL#anchor)
    /// or a document fragment identifier.
    Anchor,
    /// A tag, which is a link to a gathering tag.
    Tag {
        /// The file this hyperlink references.
        file: File,
        /// The ID this hyperlink references.
        id: String,
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
        file: File,
        /// The ID this hyperlink references.
        id: String,
        /// Recognized query parameters.
        flags: LinkOptions,
    },
}

/// The contents of a `Target`.
enum TargetContents {
    Html(String),
    // Only tags contain `CodeDocBlock`s.
    CodeDocBlock(Vec<CodeDocBlock>),
}

/// Query parameters parsed into known link options. TODO: perhaps used the
/// bitflags crate instead?
enum LinkOptions {
    Plain,
    AutoTitle,
    AutoNumber,
    AutoTitleAndNumber,
}

// Code
// -----------------------------------------------------------------------------
impl Cache {
    // ### Upsert
    //
    // Update the cache using the contents of the provided file.
    pub fn upsert_file_core(
        &mut self,
        // The file to process.
        file: &Path,
        // DOM of file to process.
        dom: Rc<Node>,
        // The file parsed into CodeDocBlocks.
        code_doc_block: Vec<CodeDocBlock>,
        // If true, then this function will update auto-titled text from current
        // cache contents and return a map of outdated dependencies need to
        // correct these titles.
        auto_title: bool,
        // If `dom` was provided, return a list of dependencies to update in
        // order to fully update this file; otherwise, return an empty list.
    ) -> HashMap<
        // The file containing the remote Target with the requested auto-title
        // contents.
        File,
        // Targets in the current file whose auto-titled text depends on this
        // remote target.
        Vec<Target>,
    > {
        HashMap::new()
        // Pseudocode:
        //
        // 1. Upsert this file's page data structure. Pre-existing cache data
        //    provides the TOC numbering.
        // 2. Set numbering of headings, captions, etc. to 1.
        // 3. Walk the DOM. For each item in the walk:
        //    1. If this is a doc block separator, read and update the current
        //       doc block index.
        //    2. If this item is a target, upsert the target's data structure to
        //       the page's vector of targets and the cache state.
        //       1. Update the current numbering if this is a numbered item
        //          (heading, caption, etc.) and insert the HTML to set its
        //          number in the DOM.
        //       2. For tags: look up the tag by ID in the cache. Update the
        //          start tag (if provided) or use 0, update the end tag (if
        //          provided) or use the last index of the CodeDocBlock vec. If
        //          this tag included an end, update the tag's contents.
        //       3. If this link is auto-titled and auto-titles are enabled, add
        //          it to the page's map of dependencies.
        // 4. For each item in the map of dependencies:
        //    1. Look for it in the cache. If it's not in the cache or if the
        //       cache for that file is outdated, add the referring file to the
        //       map of dependencies.
        //    2. Update the auto-titled text using cached data; if not in the
        //       cache use the title "pending."
    }
}

#[cfg(test)]
mod tests {
    use std::{
        borrow::{Borrow, BorrowMut},
        collections::HashMap,
        rc::Weak,
        sync::{Arc, Mutex},
    };

    use indoc::indoc;
    use test_utils::prep_test_dir;

    use crate::processing::cache::{Cache, File, TargetContents, TargetCore};

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
            // [anchor link](bar.cpp#bonk)
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
            metadata: None,
            toc: vec![1],
            // Since we haven't parsed the file, the `h1` hasn't been found.
            h1: None,
            // Likewise, no dependencies have been found yet.
            dependency: vec![],
            // Same for targets.
            target: vec![],
        }));
        let baz_cpp_path = test_dir.join("baz.cpp");

        // Create a baz file that's been processed. It contains one heading.
        let mut file_baz_cpp = Arc::new(Mutex::new(File {
            path: baz_cpp_path.clone(),
            metadata: Some(baz_cpp_path.metadata().unwrap()),
            toc: vec![2],
            // Since we haven't parsed the file, the `h1` hasn't been found.
            h1: None,
            // Likewise, no dependencies have been found yet.
            dependency: vec![],
            // Same for targets.
            target: vec![],
        }));
        file_baz_cpp.borrow_mut().lock().unwrap().target = vec![
            Arc::new(Mutex::new(TargetCore {
                file: file_baz_cpp.clone(),
                id: "one".to_string(),
                node: Weak::new(),
                line: 1,
                type_: crate::processing::cache::TargetType::Heading {
                    level: 1,
                    number: 1,
                },
                contents: TargetContents::Html("Heading one".to_string()),
            })),
            Arc::new(Mutex::new(TargetCore {
                file: file_baz_cpp.clone(),
                id: "gathering_tag".to_string(),
                node: Weak::new(),
                line: 1,
                type_: crate::processing::cache::TargetType::GatherTag {
                    name: "gather".to_string(),
                },
                contents: TargetContents::Html("Heading one".to_string()),
            })),
        ];
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

        let cache = Cache {
            path: cache_path,
            id: cache_id,
            anchor: cache_anchor,
            root: test_dir,
        };

        //cache.upsert_file_core(&bar_cpp_path, );

        temp_dir.close().unwrap();
    }
}
