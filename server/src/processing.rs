/// Copyright (C) 2023 Bryan A. Jones.
///
/// This file is part of the CodeChat Editor. The CodeChat Editor is free
/// software: you can redistribute it and/or modify it under the terms of the
/// GNU General Public License as published by the Free Software Foundation,
/// either version 3 of the License, or (at your option) any later version.
///
/// The CodeChat Editor is distributed in the hope that it will be useful, but
/// WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY
/// or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for
/// more details.
///
/// You should have received a copy of the GNU General Public License along with
/// the CodeChat Editor. If not, see
/// [http://www.gnu.org/licenses](http://www.gnu.org/licenses).
///
/// `processing.rs` -- Transform source code to its web-editable equivalent and
/// back
/// ===========================================================================
// Imports
// -------
//
// ### Standard library
//
// For commented-out caching code.
/**
use std::collections::{HashMap, HashSet};
use std::fs::Metadata;
use std::io;
use std::ops::Deref;
use std::rc::{Rc, Weak};
*/
use std::cmp::max;
use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;

// ### Third-party
use lazy_static::lazy_static;
use pulldown_cmark::{Options, Parser, html};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::lexer::LEXERS;
// ### Local
use crate::lexer::{CodeDocBlock, DocBlock, LanguageLexerCompiled, source_lexer};

// Data structures
// ---------------
//
// ### Translation between a local (traditional) source file and its web-editable,
// client-side representation
//
// There are three ways that a source file is represented:
//
// 1.  As traditional source code, in a plain text file.
// 2.  As a alternating series of code and doc blocks, produced by the lexer.
//     See `lexer.rs\CodeDocBlock`.
// 3.  As a CodeMirror data structure, which consists of a single block of text,
//     to which are attached doc blocks at specific character offsets.
//
// The lexer translates between items 1 and 2; `processing.rs` translates
// between 2 and 3. The following data structures define the format for item 3.

/// <a id="LexedSourceFile"></a>Define the JSON data structure used to represent
/// a source file in a web-editable format.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CodeChatForWeb {
    pub metadata: SourceFileMetadata,
    pub source: CodeMirror,
}

/// <a id="SourceFileMetadata"></a>Metadata about a source file sent along with
/// it both to and from the client. TODO: currently, this is too simple to
/// justify a struct. This allows for future growth -- perhaps the valid types
/// of comment delimiters?
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SourceFileMetadata {
    /// The lexer used to transforms source code into code and doc blocks and vice versa.
    pub mode: String,
}

/// The format used by CodeMirror to serialize/deserialize editor contents.
/// TODO: Link to JS code where this data structure is defined.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CodeMirror {
    /// The document being edited.
    pub doc: String,
    /// Doc blocks
    pub doc_blocks: CodeMirrorDocBlocks,
}

/// This defines a doc block for CodeMirror.
pub type CodeMirrorDocBlocks = Vec<(
    // From -- the starting character this doc block is anchored to.
    usize,
    // To -- the ending character this doc block is anchored to.
    usize,
    // Indent.
    String,
    // delimiter
    String,
    // contents
    String,
)>;

/// This enum contains the results of translating a source file to the CodeChat
/// Editor format.
#[derive(Debug, PartialEq)]
pub enum TranslationResults {
    /// This file is unknown to and therefore not supported by the CodeChat
    // Editor.
    Unknown,
    /// This is a CodeChat Editor file but it contains errors that prevent its
    /// translation. The string contains the error message.
    Err(String),
    /// A CodeChat Editor file; the struct contains the file's contents
    /// translated to CodeMirror.
    CodeChat(CodeChatForWeb),
}

/// This enum contains the results of translating a source file to a string
/// rendering of the CodeChat Editor format.
#[derive(Debug, PartialEq)]
pub enum TranslationResultsString {
    /// This is a binary file; it must be viewed raw or using the simple viewer.
    Binary,
    /// This file is unknown to the CodeChat
    /// Editor. It must be viewed raw or using the simple viewer.
    Unknown,
    /// This is a CodeChat Editor file but it contains errors that prevent its
    /// translation. The string contains the error message.
    Err(String),
    /// A CodeChat Editor file; the struct contains the file's contents
    /// translated to CodeMirror.
    CodeChat(CodeChatForWeb),
    /// The table of contents file, translated to HTML.
    Toc(String),
}

// On save, the process is CodeChatForWeb -> Vec\<CodeDocBlocks> -> source code.
//
// Globals
// -------
lazy_static! {
    /// Match the lexer directive in a source file.
    static ref LEXER_DIRECTIVE: Regex = Regex::new(r"CodeChat Editor lexer: (\w+)").unwrap();
    /// Match the doc block separator string translated from Markdown to HTML as
    /// itself, or when inside a fenced code block.
    static ref DOC_BLOCK_SEPARATOR_STRING_REGEX: Regex = Regex::new("<CodeChatEditor-separator/>\n|&lt;CodeChatEditor-separator/&gt;\n").unwrap();
}

const DOC_BLOCK_SEPARATOR_STRING: &str = "\n<CodeChatEditor-separator/>\n\n";

// Determine if the provided file is part of a project.
// ----------------------------------------------------
pub fn find_path_to_toc(file_path: &Path) -> Option<PathBuf> {
    // To determine if this source code is part of a project, look for a project
    // file by searching the current directory, then all its parents, for a file
    // named `toc.md`.
    let mut path_to_toc = PathBuf::new();
    let mut current_dir = file_path.to_path_buf();
    // Drop the last element (the current file name) from the search.
    current_dir.pop();
    loop {
        let mut project_file = current_dir.clone();
        project_file.push("toc.md");
        if project_file.is_file() {
            path_to_toc.push("toc.md");
            return Some(path_to_toc);
        }
        if !current_dir.pop() {
            return None;
        }
        path_to_toc.push("../");
    }
}

// Transform `CodeChatForWeb` to source code
// -----------------------------------------
/// This function takes in a source file in web-editable format (the
/// `CodeChatForWeb` struct) and transforms it into source code.
pub fn codechat_for_web_to_source(
    // The file to save plus metadata, stored in the `LexedSourceFile`
    codechat_for_web: &CodeChatForWeb,
) -> Result<String, String> {
    // Given the mode, find the lexer.
    let lexer: &std::sync::Arc<crate::lexer::LanguageLexerCompiled> = match LEXERS
        .map_mode_to_lexer
        .get(&codechat_for_web.metadata.mode)
    {
        Some(v) => v,
        None => return Err("Invalid mode".to_string()),
    };

    // Convert from `CodeMirror` to a `SortaCodeDocBlocks`.
    let code_doc_block_vec = code_mirror_to_code_doc_blocks(&codechat_for_web.source);
    code_doc_block_vec_to_source(&code_doc_block_vec, lexer)
}

/// Translate from CodeMirror to CodeDocBlocks.
fn code_mirror_to_code_doc_blocks(code_mirror: &CodeMirror) -> Vec<CodeDocBlock> {
    let doc_blocks = &code_mirror.doc_blocks;
    // A CodeMirror "document" is really source code. Convert it from UTF-8
    // bytes to an array of characters, which is indexable by character.
    let code: Vec<char> = code_mirror.doc.chars().collect();
    let mut code_doc_block_arr: Vec<CodeDocBlock> = Vec::new();
    // Keep track of the to index of the previous doc block. Since we haven't
    // processed any doc blocks, start at 0.
    let mut code_index: usize = 0;

    // Walk through each doc block, inserting the previous code block followed
    // by the doc block.
    for codemirror_doc_block in doc_blocks {
        // Append the code block, unless it's empty.
        let code_contents = &code[code_index..codemirror_doc_block.0];
        if !code_contents.is_empty() {
            // Convert back from a character array to a string.
            let s: String = code_contents.iter().collect();
            code_doc_block_arr.push(CodeDocBlock::CodeBlock(s.to_string()))
        }
        // Append the doc block.
        code_doc_block_arr.push(CodeDocBlock::DocBlock(DocBlock {
            indent: codemirror_doc_block.2.to_string(),
            delimiter: codemirror_doc_block.3.to_string(),
            contents: codemirror_doc_block.4.to_string(),
            lines: 0,
        }));
        code_index = codemirror_doc_block.1 + 1;
    }

    // See if there's a code block after the last doc block.
    let code_contents = &code[code_index..];
    if !code_contents.is_empty() {
        // Convert back from a character array to a string.
        let s: String = code_contents.iter().collect();
        code_doc_block_arr.push(CodeDocBlock::CodeBlock(s.to_string()));
    }

    code_doc_block_arr
}

// Turn this vec of CodeDocBlocks into a string of source code.
fn code_doc_block_vec_to_source(
    code_doc_block_vec: &Vec<CodeDocBlock>,
    lexer: &LanguageLexerCompiled,
) -> Result<String, String> {
    let mut file_contents = String::new();
    for code_doc_block in code_doc_block_vec {
        match code_doc_block {
            CodeDocBlock::DocBlock(doc_block) => {
                // Append a doc block, adding a space between the opening
                // delimiter and the contents when necessary.
                let mut append_doc_block = |indent: &str, delimiter: &str, contents: &str| {
                    file_contents += indent;
                    file_contents += delimiter;
                    // Add a space between the delimiter and comment body,
                    // unless the comment was a newline or we're at the end of
                    // the file.
                    if contents.is_empty() || contents == "\n" {
                        // Nothing to append in this case.
                    } else {
                        // Put a space between the delimiter and the contents.
                        file_contents += " ";
                    }
                    file_contents += contents;
                };

                let is_inline_delim = lexer
                    .language_lexer
                    .inline_comment_delim_arr
                    .contains(&doc_block.delimiter);

                // Build a comment based on the type of the delimiter.
                if is_inline_delim {
                    // To produce an inline comment, split the contents into a
                    // series of lines, adding the indent and inline comment
                    // delimiter to each line.
                    //
                    // A special case: an empty string processed by
                    // `split_inclusive` becomes an empty list, not `[""]`. Note
                    // that this mirrors what Python's
                    // [splitlines](https://docs.python.org/3/library/stdtypes.html#str.splitlines)
                    // does, and is also the subject of a [Rust bug
                    // report](https://github.com/rust-lang/rust/issues/111457).
                    let lines: Vec<_> = doc_block.contents.split_inclusive('\n').collect();
                    let lines_fixed = if lines.is_empty() { vec![""] } else { lines };
                    for content_line in lines_fixed {
                        append_doc_block(&doc_block.indent, &doc_block.delimiter, content_line);
                    }
                } else {
                    // Block comments are more complex.
                    //
                    // First, determine the closing comment delimiter matching
                    // the provided opening delimiter.
                    let block_comment_closing_delimiter = match lexer
                        .language_lexer
                        .block_comment_delim_arr
                        .iter()
                        .position(|bc| bc.opening == doc_block.delimiter)
                    {
                        Some(index) => &lexer.language_lexer.block_comment_delim_arr[index].closing,
                        None => {
                            return Err(format!(
                                "Unknown comment opening delimiter '{}'.",
                                doc_block.delimiter
                            ));
                        }
                    };

                    // Then, split the contents into a series of lines. Build a
                    // properly-indented block comment around these lines.
                    let content_lines: Vec<&str> =
                        doc_block.contents.split_inclusive('\n').collect();
                    for (index, content_line) in content_lines.iter().enumerate() {
                        // Note: using `.len()` here is correct -- it refers to
                        // an index into `content_lines`, not an index into a
                        // string.
                        let is_last = index == content_lines.len() - 1;
                        // Process each line, based on its location (first/not
                        // first/last). Note that the first line can also be the
                        // last line in a one-line comment.
                        //
                        // On the last line, include a properly-formatted
                        // closing comment delimiter:
                        let content_line_updated = if is_last {
                            match content_line.strip_suffix('\n') {
                                // include a space then the closing delimiter
                                // before the final newline (if it exists; at
                                // the end of a file, it may not);
                                Some(stripped_line) => {
                                    stripped_line.to_string()
                                        + " "
                                        + block_comment_closing_delimiter
                                        + "\n"
                                }
                                // otherwise (i.e. there's no final newline),
                                // just include a space and the closing
                                // delimiter.
                                None => {
                                    content_line.to_string() + " " + block_comment_closing_delimiter
                                }
                            }
                        } else {
                            // Since this isn't the last line, don't include the
                            // closing comment delimiter.
                            content_line.to_string()
                        };

                        // On the first line, include the indent and opening
                        // delimiter.
                        let is_first = index == 0;
                        if is_first {
                            append_doc_block(
                                &doc_block.indent,
                                &doc_block.delimiter,
                                &content_line_updated,
                            );
                        // Since this isn't a first line:
                        } else {
                            // *   If this line is just a newline, include just
                            //     the newline.
                            if *content_line == "\n" {
                                append_doc_block("", "", "\n");
                            // *   Otherwise, include spaces in place of the
                            //     delimiter.
                            } else {
                                append_doc_block(
                                    &doc_block.indent,
                                    &" ".repeat(doc_block.delimiter.chars().count()),
                                    &content_line_updated,
                                );
                            }
                        }
                    }
                }
            }

            CodeDocBlock::CodeBlock(contents) =>
            // This is code. Simply append it (by definition, indent and
            // delimiter are empty).
            {
                file_contents += contents
            }
        }
    }
    Ok(file_contents)
}

// Transform from source code to `CodeChatForWeb`
// ----------------------------------------------
//
// Given the contents of a file, classify it and (for CodeChat Editor files)
// convert it to the `CodeChatForWeb` format.
pub fn source_to_codechat_for_web(
    // The file's contents.
    file_contents: &str,
    // The file's extension.
    file_ext: &String,
    // True if this file is a TOC.
    _is_toc: bool,
    // True if this file is part of a project.
    _is_project: bool,
) -> TranslationResults {
    // Determine the lexer to use for this file.
    let lexer_name;
    // First, search for a lexer directive in the file contents.
    let lexer = if let Some(captures) = LEXER_DIRECTIVE.captures(file_contents) {
        lexer_name = captures[1].to_string();
        match LEXERS.map_mode_to_lexer.get(&lexer_name) {
            Some(v) => v,
            None => {
                return TranslationResults::Err(format!(
                    "<p>Unknown lexer type {}.</p>",
                    &lexer_name
                ));
            }
        }
    } else {
        // Otherwise, look up the lexer by the file's extension.
        match LEXERS.map_ext_to_lexer_vec.get(file_ext) {
            Some(llc) => llc.first().unwrap(),
            _ => {
                // The file type is unknown; treat it as plain text.
                return TranslationResults::Unknown;
            }
        }
    };

    // Transform the provided file into the `CodeChatForWeb` structure.
    let code_doc_block_arr;
    let codechat_for_web = CodeChatForWeb {
        metadata: SourceFileMetadata {
            mode: lexer.language_lexer.lexer_name.to_string(),
        },
        source: if lexer.language_lexer.lexer_name.as_str() == "markdown" {
            // Document-only files are easy: just encode the contents.
            let html = markdown_to_html(file_contents);
            // TODO: process the HTML.
            CodeMirror {
                doc: html,
                doc_blocks: vec![],
            }
        } else {
            // This is a source file.
            //
            // Create an initially-empty struct; the source code will be
            // translated to this.
            let mut code_mirror = CodeMirror {
                doc: "".to_string(),
                doc_blocks: Vec::new(),
            };

            // Lex the code.
            code_doc_block_arr = source_lexer(file_contents, lexer);

            // Combine all the doc blocks into a single string, separated by a
            // delimiter. Transform this to markdown, then split the transformed
            // content back into the doc blocks they came from. This is
            // necessary to allow [link reference
            // definitions](https://spec.commonmark.org/0.31.2/#link-reference-definitions)
            // between doc blocks to work; for example, `[Link][1]` in one doc
            // block, then `[1]: http:/foo.org` in another doc block requires
            // both to be in the same Markdown document to translate correctly.
            //
            // Walk through the code/doc blocks, ...
            let doc_contents = code_doc_block_arr
                .iter()
                // ...selcting only the doc block contents...
                .filter_map(|cdb| {
                    if let CodeDocBlock::DocBlock(db) = cdb {
                        Some(db.contents.as_str())
                    } else {
                        None
                    }
                })
                // ...then collect them, separated by the doc block separator
                // string.
                .collect::<Vec<_>>()
                .join(DOC_BLOCK_SEPARATOR_STRING);
            let html = markdown_to_html(&doc_contents);
            // Now that we have HTML, process it. TODO.
            //
            // After processing by Markdown, the doc block separator string may
            // be (mostly) unchanged; however, if there's an unterminated fenced
            // code block, then HTML entities replaces angle brackets. Match on
            // either case.
            let mut doc_block_contents_iter = DOC_BLOCK_SEPARATOR_STRING_REGEX.split(&html);

            // Translate each `CodeDocBlock` to its `CodeMirror` equivalent.
            for code_or_doc_block in code_doc_block_arr {
                match code_or_doc_block {
                    CodeDocBlock::CodeBlock(code_string) => code_mirror.doc.push_str(&code_string),
                    CodeDocBlock::DocBlock(doc_block) => {
                        // Create the doc block.
                        //
                        // Get the length of the string in characters (not
                        // bytes, which is what `len()` returns).
                        let len = code_mirror.doc.chars().count();
                        code_mirror.doc_blocks.push((
                            // From
                            len,
                            // To. Make this one line short, which allows
                            // CodeMirror to correctly handle inserts at the
                            // first character of the following code block. Note
                            // that the last doc block could be zero length, so
                            // handle this case.
                            len + max(doc_block.lines, 1) - 1,
                            doc_block.indent.to_string(),
                            doc_block.delimiter.to_string(),
                            // Used the markdown-translated replacement for this
                            // doc block, rather than the original string.
                            doc_block_contents_iter.next().unwrap().to_string(),
                        ));
                        // Append newlines to the document; the doc block will
                        // replace these in the editor. This keeps the line
                        // numbering of non-doc blocks correct.
                        code_mirror.doc.push_str(&"\n".repeat(doc_block.lines));
                    }
                }
            }
            code_mirror
        },
    };

    TranslationResults::CodeChat(codechat_for_web)
}

// Like `source_to_codechat_for_web`, translate a source file to the CodeChat
// Editor client format. This wraps a call to that function with additional
// processing (determine if this is part of a project, encode the output as
// necessary, etc.).
pub fn source_to_codechat_for_web_string(
    // The file's contents.
    file_contents: &str,
    // The path to this file.
    file_path: &Path,
    // True if this file is a TOC.
    is_toc: bool,
) -> (
    // The resulting translation.
    TranslationResultsString,
    // Path to the TOC, if found; otherwise, None.
    Option<PathBuf>,
) {
    // Determine the file's extension, in order to look up a lexer.
    let ext = &file_path
        .extension()
        .unwrap_or_else(|| OsStr::new(""))
        .to_string_lossy();

    // To determine if this source code is part of a project, look for a project
    // file by searching the current directory, then all its parents, for a file
    // named `toc.md`.
    let path_to_toc = find_path_to_toc(file_path);
    let is_project = path_to_toc.is_some();

    (
        match source_to_codechat_for_web(file_contents, &ext.to_string(), is_toc, is_project) {
            TranslationResults::CodeChat(codechat_for_web) => {
                if is_toc {
                    // For the table of contents sidebar, which is pure
                    // markdown, just return the resulting HTML, rather than the
                    // editable CodeChat for web format.
                    TranslationResultsString::Toc(codechat_for_web.source.doc)
                } else {
                    TranslationResultsString::CodeChat(codechat_for_web)
                }
            }
            TranslationResults::Unknown => TranslationResultsString::Unknown,
            TranslationResults::Err(err) => TranslationResultsString::Err(err),
        },
        path_to_toc,
    )
}

/// Convert markdown to HTML. (This assumes the Markdown defined in the
/// CommonMark spec.)
fn markdown_to_html(markdown: &str) -> String {
    let mut options = Options::all();
    // Turndown (which converts HTML back to Markdown) doesn't support smart
    // punctuation.
    options.remove(Options::ENABLE_SMART_PUNCTUATION);
    options.remove(Options::ENABLE_MATH);
    let parser = Parser::new_ext(markdown, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

// Goal: make it easy to update the data structure. We update on every
// load/save, then do some accesses during those processes.
//
// Top-level data structures: a file HashSet<PathBuf, FileAnchor> and an id
// HashMap<id, {Anchor, HashSet<referring\_id>}>. Some FileAnchors in the file
// HashSet are also in a pending load list.
//
// *   To update a file:
//     *   Remove the old file from the file HasHMap. Add an empty FileAnchor to
//         the file HashMap.
//     *   For each id, see if that id already exists.
//         *   If the id exists: if it refers to an id in the old FileAnchor,
//             replace it with the new one. If not, need to perform resolution
//             on this id (we have a non-unique id; how to fix?).
//         *   If the id doesn't exist: create a new one.
//     *   For each hyperlink, see if that id already exists.
//         *   If so, upsert the referring id. Check the metadata on the id to
//             make sure that data is current. If not, add this to the pending
//             hyperlinks list. If the file is missing, delete it from the
//             cache.
//         *   If not, create a new entry in the id HashSet and add the
//             referring id to the HashSet. Add the file to a pending hyperlinks
//             list.
//     *   When the file is processed:
//         *   Look for all entries in the pending file list that refer to the
//             current file and resolve these. Start another task to load in all
//             pending files.
//         *   Look at the old file; remove each id that's still in the id
//             HashMap. If the id was in the HashMap and it also was a
//             Hyperlink, remove that from the HashSet.
// *   To remove a file from the HashMap:
//     *   Remove it from the file HashMap.
//     *   For each hyperlink, remove it from the HashSet of referring links (if
//         that id still exists).
//     *   For each id, remove it from the id HashMap.
// *   To add a file from the HashSet:
//     *   Perform an update with an empty FileAnchor.
//
// Pending hyperlinks list: for each hyperlink,
//
// *   check if the id is now current in the cache. If so, add the referring id
//     to the HashSet then move to the next hyperlink.
// *   check if the file is now current in the cache. If not, load the file and
//     update the cache, then go to step 1.
// *   The id was not found, even in the expected file. Add the hyperlink to a
//     broken links set?
//
// Global operations:
//
// *   Scan all files, then perform add/upsert/removes based on differences with
//     the cache.
//
// Functions:
//
// *   Upsert an Anchor.
// *   Upsert a Hyperlink.
// *   Upsert a file.
// *   Remove a file.
/*x
/// There are two types of files that can serve as an anchor: these are file
/// anchor targets.
enum FileAnchor {
    Plain(PlainFileAnchor),
    Html(HtmlFileAnchor),
}

/// This is the cached metadata for a file that serves as an anchor: perhaps an
/// image, a PDF, or a video.
struct PlainFileAnchor {
    /// A relative path to this file, rooted at the project's TOC.
    path: Rc<PathBuf>,
    /// The globally-unique anchor used to link to this file. It's generated
    /// based on hash of the file's contents, so that each file will have a
    /// unique identifier.
    anchor: String,
    /// Metadata captured when this data was cached. If it disagrees with the
    /// file's current state, then this cached data should be re=generated from
    /// the file.
    file_metadata: Metadata,
}

/// Cached metadata for an HTML file.
struct HtmlFileAnchor {
    /// The file containing this HTML.
    file_anchor: PlainFileAnchor,
    /// The TOC numbering of this file.
    numbering: Vec<Option<u32>>,
    /// The headings in this file.
    headings: Vec<HeadingAnchor>,
    /// Anchors which appear before the first heading.
    pre_anchors: Vec<NonHeadingAnchor>,
}

/// Cached metadata shared by both headings (which are also anchors) and
/// non-heading anchors.
struct AnchorCommon {
    /// The HTML file containing this anchor.
    html_file_anchor: Weak<FileAnchor>,
    /// The globally-unique anchor used to link to this object.
    anchor: String,
    /// The inner HTML of this anchor.
    inner_html: String,
    /// The hyperlink this anchor contains.
    hyperlink: Option<Rc<Hyperlink>>,
}

/// An anchor is defined only in these two places: the anchor source.
enum HtmlAnchor {
    Heading(HeadingAnchor),
    NonHeading(NonHeadingAnchor),
}

/// Cached metadata for a heading (which is always also an anchor).
struct HeadingAnchor {
    anchor_common: AnchorCommon,
    /// The numbering of this heading on the HTML file containing it.
    numbering: Vec<Option<u32>>,
    /// Non-heading anchors which appear after this heading but before the next
    /// heading.
    non_heading_anchors: Vec<NonHeadingAnchor>,
}

/// Cached metadata for a non-heading anchor.
struct NonHeadingAnchor {
    anchor_common: AnchorCommon,
    /// The heading this anchor appears after (unless it appears before the
    /// first heading in this file).
    parent_heading: Option<Weak<HeadingAnchor>>,
    /// A snippet of HTML preceding this anchor.
    pre_snippet: String,
    /// A snippet of HTML following this anchor.
    post_snippet: String,
    /// If this is a numbered item, the name of the numbering group it belongs
    /// to.
    numbering_group: Option<String>,
    /// If this is a numbered item, its number.
    number: u32,
}

/// An anchor can refer to any of these structs: these are all possible anchor
/// targets.
enum Anchor {
    Html(HtmlAnchor),
    File(FileAnchor),
}

/// The metadata for a hyperlink.
struct Hyperlink {
    /// The file this hyperlink refers to.
    file: PathBuf,
    /// The anchor this hyperlink refers to.
    html_anchor: String,
}

/// The value stored in the id HashMap.
struct AnchorVal {
    /// The target anchor this id refers to.
    anchor: Anchor,
    /// All hyperlinks which target this anchor.
    referring_links: Rc<HashSet<String>>,
}

// Given HTML, catalog all link targets and link-like items, ensuring that they
// have a globally unique id.
fn html_analyze(
    file_path: &Path,
    html: &str,
    mut file_map: HashMap<Rc<PathBuf>, Rc<FileAnchor>>,
    mut anchor_map: HashMap<Rc<String>, HashSet<AnchorVal>>,
) -> io::Result<String> {
    // Create the missing anchors:
    //
    // A missing file.
    let missing_html_file_anchor = Rc::new(FileAnchor::Html(HtmlFileAnchor {
        file_anchor: PlainFileAnchor {
            path: Rc::new(PathBuf::new()),
            anchor: "".to_string(),
            // TODO: is there some way to create generic/empty metadata?
            file_metadata: Path::new(".").metadata().unwrap(),
        },
        numbering: Vec::new(),
        headings: Vec::new(),
        pre_anchors: Vec::new(),
    }));
    // Define an anchor in this file.
    let missing_anchor = NonHeadingAnchor {
        anchor_common: AnchorCommon {
            html_file_anchor: Rc::downgrade(&missing_html_file_anchor),
            anchor: "".to_string(),
            hyperlink: None,
            inner_html: "".to_string(),
        },
        parent_heading: None,
        pre_snippet: "".to_string(),
        post_snippet: "".to_string(),
        numbering_group: None,
        number: 0,
    };
    // Add this to the top-level hashes.
    let anchor_val = AnchorVal {
        anchor: Anchor::Html(HtmlAnchor::NonHeading(missing_anchor)),
        referring_links: Rc::new(HashSet::new()),
    };
    //file_map.insert(mfa.file_anchor.path, missing_html_file_anchor);
    //let anchor_val_set: HashSet<AnchorVal> = HashSet::new();
    //anchor_val_set.insert(anchor_val);
    //anchor_map.insert(&mfa.file_anchor.anchor, anchor_val_set);

    Ok("".to_string())
}
*/

// Tests
// -----
#[cfg(test)]
mod tests {
    use std::{path::PathBuf, str::FromStr};

    use predicates::prelude::predicate::str;

    use super::{CodeChatForWeb, CodeMirror, CodeMirrorDocBlocks, SourceFileMetadata};
    use super::{TranslationResults, find_path_to_toc};
    use crate::lexer::{
        CodeDocBlock, DocBlock, compile_lexers, supported_languages::get_language_lexer_vec,
    };
    use crate::processing::{
        code_doc_block_vec_to_source, code_mirror_to_code_doc_blocks, codechat_for_web_to_source,
        source_to_codechat_for_web,
    };
    use crate::test_utils::stringit;

    use crate::prep_test_dir;

    // ### Utilities
    fn build_codechat_for_web(
        mode: &str,
        doc: &str,
        doc_blocks: CodeMirrorDocBlocks,
    ) -> CodeChatForWeb {
        // Wrap the provided parameters in the necessary data structures.
        CodeChatForWeb {
            metadata: SourceFileMetadata {
                mode: mode.to_string(),
            },
            source: CodeMirror {
                doc: doc.to_string(),
                doc_blocks,
            },
        }
    }

    // Provide a way to construct one element of the `CodeMirrorDocBlocks`
    // vector.
    fn build_codemirror_doc_block(
        start: usize,
        end: usize,
        indent: &str,
        delimiter: &str,
        contents: &str,
    ) -> (usize, usize, String, String, String) {
        (
            start,
            end,
            indent.to_string(),
            delimiter.to_string(),
            contents.to_string(),
        )
    }

    fn build_doc_block(indent: &str, delimiter: &str, contents: &str) -> CodeDocBlock {
        CodeDocBlock::DocBlock(DocBlock {
            indent: indent.to_string(),
            delimiter: delimiter.to_string(),
            contents: contents.to_string(),
            lines: 0,
        })
    }

    fn build_code_block(contents: &str) -> CodeDocBlock {
        CodeDocBlock::CodeBlock(contents.to_string())
    }

    fn run_test(mode: &str, doc: &str, doc_blocks: CodeMirrorDocBlocks) -> Vec<CodeDocBlock> {
        let codechat_for_web = build_codechat_for_web(mode, doc, doc_blocks);
        code_mirror_to_code_doc_blocks(&codechat_for_web.source)
    }

    // ### Tests for `codechat_for_web_to_source`
    //
    // Since it just invokes `code_mirror_to_code_doc_blocks` and
    // `code_doc_block_vec_to_source`, both of which have their own set of
    // tests, we just need to do a bit of testing.
    #[test]
    fn test_codechat_for_web_to_source() {
        let codechat_for_web = build_codechat_for_web("python", "", vec![]);
        assert_eq!(
            codechat_for_web_to_source(&codechat_for_web),
            Result::Ok("".to_string())
        );

        let codechat_for_web = build_codechat_for_web("undefined", "", vec![]);
        assert_eq!(
            codechat_for_web_to_source(&codechat_for_web),
            Result::Err("Invalid mode".to_string())
        );
    }

    // ### Tests for `code_mirror_to_code_doc_blocks`
    #[test]
    fn test_codemirror_to_code_doc_blocks_py() {
        // Pass nothing to the function.
        assert_eq!(run_test("python", "", vec![]), vec![]);

        // Pass one code block.
        assert_eq!(
            run_test("python", "Test", vec![]),
            vec![build_code_block("Test")]
        );

        // Pass one doc block.
        assert_eq!(
            run_test(
                "python",
                "\n",
                vec![build_codemirror_doc_block(0, 0, "", "#", "Test")],
            ),
            vec![build_doc_block("", "#", "Test")]
        );

        // Pass one doc block containing Unicode.
        assert_eq!(
            run_test(
                "python",
                "σ\n",
                vec![build_codemirror_doc_block(1, 1, "", "#", "Test")],
            ),
            vec![build_code_block("σ"), build_doc_block("", "#", "Test")]
        );

        // A code block then a doc block
        assert_eq!(
            run_test(
                "python",
                "code\n\n",
                vec![build_codemirror_doc_block(5, 5, "", "#", "doc")],
            ),
            vec![build_code_block("code\n"), build_doc_block("", "#", "doc")]
        );

        // A doc block then a code block
        assert_eq!(
            run_test(
                "python",
                "\ncode\n",
                vec![build_codemirror_doc_block(0, 0, "", "#", "doc")],
            ),
            vec![build_doc_block("", "#", "doc"), build_code_block("code\n")]
        );

        // A code block, then a doc block, then another code block
        assert_eq!(
            run_test(
                "python",
                "\ncode\n\n",
                vec![
                    build_codemirror_doc_block(0, 0, "", "#", "doc 1"),
                    build_codemirror_doc_block(6, 6, "", "#", "doc 2")
                ],
            ),
            vec![
                build_doc_block("", "#", "doc 1"),
                build_code_block("code\n"),
                build_doc_block("", "#", "doc 2")
            ]
        );

        // Empty doc blocks separated by an empty code block
        assert_eq!(
            run_test(
                "python",
                "\n\n\n",
                vec![
                    build_codemirror_doc_block(0, 0, "", "#", ""),
                    build_codemirror_doc_block(2, 2, "", "#", "")
                ],
            ),
            vec![
                build_doc_block("", "#", ""),
                build_code_block("\n"),
                build_doc_block("", "#", "")
            ]
        );
    }

    #[test]
    fn test_codemirror_to_code_doc_blocks_cpp() {
        // Pass an inline comment.
        assert_eq!(
            run_test(
                "c_cpp",
                "\n",
                vec![build_codemirror_doc_block(0, 0, "", "//", "Test")]
            ),
            vec![build_doc_block("", "//", "Test")]
        );

        // Pass a block comment.
        assert_eq!(
            run_test(
                "c_cpp",
                "\n",
                vec![build_codemirror_doc_block(0, 0, "", "/*", "Test")]
            ),
            vec![build_doc_block("", "/*", "Test")]
        );

        // Two back-to-back doc blocks.
        assert_eq!(
            run_test(
                "c_cpp",
                "\n\n",
                vec![
                    build_codemirror_doc_block(0, 0, "", "//", "Test 1"),
                    build_codemirror_doc_block(1, 1, "", "/*", "Test 2")
                ]
            ),
            vec![
                build_doc_block("", "//", "Test 1"),
                build_doc_block("", "/*", "Test 2")
            ]
        );
    }

    // ### Tests for `code_doc_block_vec_to_source`
    //
    // A language with just one inline comment delimiter and no block comments.
    #[test]
    fn test_code_doc_blocks_to_source_py() {
        let llc = compile_lexers(get_language_lexer_vec());
        let py_lexer = llc.map_mode_to_lexer.get(&stringit("python")).unwrap();

        // An empty document.
        assert_eq!(code_doc_block_vec_to_source(&vec![], py_lexer).unwrap(), "");
        // A one-line comment.
        assert_eq!(
            code_doc_block_vec_to_source(&vec![build_doc_block("", "#", "Test")], py_lexer)
                .unwrap(),
            "# Test"
        );
        assert_eq!(
            code_doc_block_vec_to_source(&vec![build_doc_block("", "#", "Test\n")], py_lexer)
                .unwrap(),
            "# Test\n"
        );
        // Check empty doc block lines and multiple lines.
        assert_eq!(
            code_doc_block_vec_to_source(
                &vec![build_doc_block("", "#", "Test 1\n\nTest 2")],
                py_lexer
            )
            .unwrap(),
            "# Test 1\n#\n# Test 2"
        );

        // Repeat the above tests with an indent.
        assert_eq!(
            code_doc_block_vec_to_source(&vec![build_doc_block(" ", "#", "Test")], py_lexer)
                .unwrap(),
            " # Test"
        );
        assert_eq!(
            code_doc_block_vec_to_source(&vec![build_doc_block("  ", "#", "Test\n")], py_lexer)
                .unwrap(),
            "  # Test\n"
        );
        assert_eq!(
            code_doc_block_vec_to_source(
                &vec![build_doc_block("   ", "#", "Test 1\n\nTest 2")],
                py_lexer
            )
            .unwrap(),
            "   # Test 1\n   #\n   # Test 2"
        );

        // Basic code.
        assert_eq!(
            code_doc_block_vec_to_source(&vec![build_code_block("Test")], py_lexer).unwrap(),
            "Test"
        );

        // An incorrect delimiter.
        assert_eq!(
            code_doc_block_vec_to_source(&vec![build_doc_block("", "?", "Test")], py_lexer)
                .unwrap_err(),
            "Unknown comment opening delimiter '?'."
        );

        // Empty doc blocks separated by an empty code block.
        assert_eq!(
            code_doc_block_vec_to_source(
                &vec![
                    build_doc_block("", "#", "\n"),
                    build_code_block("\n"),
                    build_doc_block("", "#", ""),
                ],
                py_lexer
            )
            .unwrap(),
            "#\n\n#"
        );

        assert_eq!(
            code_doc_block_vec_to_source(
                &vec![
                    build_doc_block("", "#", "σ\n"),
                    build_code_block("σ\n"),
                    build_doc_block("", "#", "σ"),
                ],
                py_lexer
            )
            .unwrap(),
            "# σ\nσ\n# σ"
        );
    }

    // A language with just one block comment delimiter and no inline comment
    // delimiters.
    #[test]
    fn test_code_doc_blocks_to_source_css() {
        let llc = compile_lexers(get_language_lexer_vec());
        let css_lexer = llc.map_mode_to_lexer.get(&stringit("css")).unwrap();

        // An empty document.
        assert_eq!(
            code_doc_block_vec_to_source(&vec![], css_lexer).unwrap(),
            ""
        );
        // A one-line comment.
        assert_eq!(
            code_doc_block_vec_to_source(&vec![build_doc_block("", "/*", "Test\n")], css_lexer)
                .unwrap(),
            "/* Test */\n"
        );
        assert_eq!(
            code_doc_block_vec_to_source(&vec![build_doc_block("", "/*", "Test")], css_lexer)
                .unwrap(),
            "/* Test */"
        );
        // Check empty doc block lines and multiple lines.
        assert_eq!(
            code_doc_block_vec_to_source(
                &vec![
                    build_code_block("Test_0\n"),
                    build_doc_block("", "/*", "Test 1\n\nTest 2\n")
                ],
                css_lexer
            )
            .unwrap(),
            r#"Test_0
/* Test 1

   Test 2 */
"#
        );

        // Repeat the above tests with an indent.
        assert_eq!(
            code_doc_block_vec_to_source(&vec![build_doc_block("  ", "/*", "Test\n")], css_lexer)
                .unwrap(),
            "  /* Test */\n"
        );
        assert_eq!(
            code_doc_block_vec_to_source(
                &vec![
                    build_code_block("Test_0\n"),
                    build_doc_block("   ", "/*", "Test 1\n\nTest 2\n")
                ],
                css_lexer
            )
            .unwrap(),
            r#"Test_0
   /* Test 1

      Test 2 */
"#
        );

        // Basic code.
        assert_eq!(
            code_doc_block_vec_to_source(&vec![build_code_block("Test")], css_lexer).unwrap(),
            "Test"
        );

        // An incorrect delimiter.
        assert_eq!(
            code_doc_block_vec_to_source(&vec![build_doc_block("", "?", "Test")], css_lexer)
                .unwrap_err(),
            "Unknown comment opening delimiter '?'."
        );
    }

    // A language with multiple inline and block comment styles.
    #[test]
    fn test_code_doc_blocks_to_source_csharp() {
        let llc = compile_lexers(get_language_lexer_vec());
        let csharp_lexer = llc.map_mode_to_lexer.get(&stringit("csharp")).unwrap();

        // An empty document.
        assert_eq!(
            code_doc_block_vec_to_source(&vec![], csharp_lexer).unwrap(),
            ""
        );

        // An invalid comment.
        assert_eq!(
            code_doc_block_vec_to_source(&vec![build_doc_block("", "?", "Test\n")], csharp_lexer)
                .unwrap_err(),
            "Unknown comment opening delimiter '?'."
        );

        // Inline comments.
        assert_eq!(
            code_doc_block_vec_to_source(&vec![build_doc_block("", "//", "Test\n")], csharp_lexer)
                .unwrap(),
            "// Test\n"
        );
        assert_eq!(
            code_doc_block_vec_to_source(&vec![build_doc_block("", "///", "Test\n")], csharp_lexer)
                .unwrap(),
            "/// Test\n"
        );

        // Block comments.
        assert_eq!(
            code_doc_block_vec_to_source(&vec![build_doc_block("", "/*", "Test\n")], csharp_lexer)
                .unwrap(),
            "/* Test */\n"
        );
        assert_eq!(
            code_doc_block_vec_to_source(&vec![build_doc_block("", "/**", "Test\n")], csharp_lexer)
                .unwrap(),
            "/** Test */\n"
        );
    }

    // ### Tests for `source_to_codechat_for_web`
    #[test]
    fn test_source_to_codechat_for_web_1() {
        // A file with an unknown extension and no lexer, which is classified as
        // a text file.
        assert_eq!(
            source_to_codechat_for_web("", &".xxx".to_string(), false, false),
            TranslationResults::Unknown
        );

        // A file with an invalid lexer specification. Obscure this, so that
        // this file can be successfully lexed by the CodeChat editor.
        let lexer_spec = format!("{}{}", "CodeChat Editor ", "lexer: ");
        assert_eq!(
            source_to_codechat_for_web(
                &format!("{}unknown", lexer_spec),
                &".xxx".to_string(),
                false,
                false,
            ),
            TranslationResults::Err("<p>Unknown lexer type unknown.</p>".to_string())
        );

        // A CodeChat Editor document via filename.
        assert_eq!(
            source_to_codechat_for_web("", &"md".to_string(), false, false),
            TranslationResults::CodeChat(build_codechat_for_web("markdown", "", vec![]))
        );

        // A CodeChat Editor document via lexer specification.
        assert_eq!(
            source_to_codechat_for_web(
                &format!("{}markdown", lexer_spec),
                &"xxx".to_string(),
                false,
                false,
            ),
            TranslationResults::CodeChat(build_codechat_for_web(
                "markdown",
                &format!("<p>{}markdown</p>\n", lexer_spec),
                vec![]
            ))
        );

        // An empty source file.
        assert_eq!(
            source_to_codechat_for_web("", &"js".to_string(), false, false),
            TranslationResults::CodeChat(build_codechat_for_web("javascript", "", vec![]))
        );

        // A zero doc block source file.
        assert_eq!(
            source_to_codechat_for_web("let a = 1;", &"js".to_string(), false, false),
            TranslationResults::CodeChat(build_codechat_for_web(
                "javascript",
                "let a = 1;",
                vec![]
            ))
        );

        // One doc block source files.
        assert_eq!(
            source_to_codechat_for_web("// Test", &"js".to_string(), false, false),
            TranslationResults::CodeChat(build_codechat_for_web(
                "javascript",
                "\n",
                vec![build_codemirror_doc_block(0, 0, "", "//", "<p>Test</p>\n")]
            ))
        );
        assert_eq!(
            source_to_codechat_for_web("let a = 1;\n// Test", &"js".to_string(), false, false,),
            TranslationResults::CodeChat(build_codechat_for_web(
                "javascript",
                "let a = 1;\n\n",
                vec![build_codemirror_doc_block(
                    11,
                    11,
                    "",
                    "//",
                    "<p>Test</p>\n"
                )]
            ))
        );
        assert_eq!(
            source_to_codechat_for_web("// Test\nlet a = 1;", &"js".to_string(), false, false,),
            TranslationResults::CodeChat(build_codechat_for_web(
                "javascript",
                "\nlet a = 1;",
                vec![build_codemirror_doc_block(0, 0, "", "//", "<p>Test</p>\n")]
            ))
        );

        // A two doc block source file. This also tests references in one block
        // to a target in another block.
        assert_eq!(
            source_to_codechat_for_web(
                "// [Link][1]\nlet a = 1;\n/* [1]: http://b.org */",
                &"js".to_string(),
                false,
                false,
            ),
            TranslationResults::CodeChat(build_codechat_for_web(
                "javascript",
                "\nlet a = 1;\n\n",
                vec![
                    build_codemirror_doc_block(
                        0,
                        0,
                        "",
                        "//",
                        "<p><a href=\"http://b.org\">Link</a></p>\n"
                    ),
                    build_codemirror_doc_block(12, 12, "", "/*", "")
                ]
            ))
        );

        // Trigger special cases:
        //
        // *   An empty doc block at the beginning of the file.
        // *   A doc block in the middle of the file
        // *   A doc block with no trailing newline at the end of the file.
        assert_eq!(
            source_to_codechat_for_web("//\n\n//\n\n//", &"cpp".to_string(), false, false),
            TranslationResults::CodeChat(build_codechat_for_web(
                "c_cpp",
                "\n\n\n\n",
                vec![
                    build_codemirror_doc_block(0, 0, "", "//", ""),
                    build_codemirror_doc_block(2, 2, "", "//", ""),
                    build_codemirror_doc_block(4, 4, "", "//", "")
                ]
            ))
        );

        // Test Unicode characters in code.
        assert_eq!(
            source_to_codechat_for_web("; // σ\n//", &"cpp".to_string(), false, false),
            TranslationResults::CodeChat(build_codechat_for_web(
                "c_cpp",
                "; // σ\n",
                vec![build_codemirror_doc_block(7, 7, "", "//", ""),]
            ))
        );

        // Test Unicode characters in strings.
        assert_eq!(
            source_to_codechat_for_web("\"σ\";\n//", &"cpp".to_string(), false, false),
            TranslationResults::CodeChat(build_codechat_for_web(
                "c_cpp",
                "\"σ\";\n",
                vec![build_codemirror_doc_block(5, 5, "", "//", ""),]
            ))
        );

        // Test a fenced code block that's unterminated.
        assert_eq!(
            source_to_codechat_for_web("/* ```\n*/\n//", &"cpp".to_string(), false, false),
            TranslationResults::CodeChat(build_codechat_for_web(
                "c_cpp",
                "\n\n",
                vec![
                    build_codemirror_doc_block(0, 1, "", "/*", "<pre><code>\n\n"),
                    build_codemirror_doc_block(2, 2, "", "//", "\n</code></pre>\n"),
                ]
            ))
        );
    }

    #[test]
    fn test_find_path_to_toc_1() {
        let (temp_dir, test_dir) = prep_test_dir!();

        // Test 1: the TOC is in the same directory as the file.
        let fp = find_path_to_toc(&test_dir.join("1/foo.py"));
        assert_eq!(fp, Some(PathBuf::from_str("toc.md").unwrap()));

        // Test 2: no TOC. (We assume all temp directory parents lack a TOC as
        // well.)
        let fp = find_path_to_toc(&test_dir.join("2/foo.py"));
        assert_eq!(fp, None);

        // Test 3: the TOC is a few levels above the file.
        let fp = find_path_to_toc(&test_dir.join("3/bar/baz/foo.py"));
        assert_eq!(fp, Some(PathBuf::from_str("../../toc.md").unwrap()));

        // Report any errors produced when removing the temporary directory.
        temp_dir.close().unwrap();
    }
}
