//C:\Users\bj147\Documents\git\CodeChat_Editor\server\target\debug\codechat-editor-server.exe

// Copyright (C) 2023 Bryan A. Jones.
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
use std::{
    borrow::Cow,
    cell::RefCell,
    cmp::max,
    ffi::OsStr,
    iter::Map,
    mem::take,
    ops::Range,
    path::{Path, PathBuf},
    rc::Rc,
    slice::Iter,
};

// ### Third-party
use imara_diff::{
    Algorithm, diff,
    intern::{InternedInput, TokenSource},
    sources::lines_with_terminator,
};
use lazy_static::lazy_static;
use pulldown_cmark::{Options, Parser, html};
use regex::Regex;
use serde::{Deserialize, Serialize};

// ### Local
use crate::lexer::{CodeDocBlock, DocBlock, LEXERS, LanguageLexerCompiled, source_lexer};

// Data structures
// ---------------
//
// ### Translation between a local (traditional) source file and its web-editable, client-side representation
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
    pub source: CodeMirrorDiffable,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum CodeMirrorDiffable {
    Plain(CodeMirror),
    Diff(CodeMirrorDiff),
}

/// <a id="SourceFileMetadata"></a>Metadata about a source file sent along with
/// it both to and from the client. TODO: currently, this is too simple to
/// justify a struct. This allows for future growth -- perhaps the valid types
/// of comment delimiters?
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SourceFileMetadata {
    /// The lexer used to transforms source code into code and doc blocks and
    /// vice versa.
    pub mode: String,
}

type CodeMirrorDocBlockVec = Vec<CodeMirrorDocBlock>;

/// The format used by CodeMirror to serialize/deserialize editor contents.
/// TODO: Link to JS code where this data structure is defined.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CodeMirror {
    /// The document being edited.
    pub doc: String,
    pub doc_blocks: CodeMirrorDocBlockVec,
}

/// A diff of the `CodeMirror` struct.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CodeMirrorDiff {
    /// A diff of the document being edited.
    pub doc: StringDiff,
    pub doc_blocks: Vec<CodeMirrorDocBlockDiff>,
}

/// This defines a doc block for CodeMirror.
#[derive(Clone, Debug, PartialEq)]
pub struct CodeMirrorDocBlock {
    // From -- the starting character this doc block is anchored to.
    pub from: usize,
    // To -- the ending character this doc block is anchored to.
    pub to: usize,
    // Indent.
    pub indent: String,
    // Delimiter.
    pub delimiter: String,
    // Contents.
    pub contents: String,
}

/// Store the difference between the previous and current `CodeMirrorDocBlock`s.
#[derive(Clone, Debug, PartialEq)]
pub struct CodeMirrorDocBlockDiff {
    /// From -- the starting character this doc block is anchored to. In the
    /// JSON encoding, there's little gain from making this an `Option`, since
    /// `undefined` takes more characters than most line numbers.
    pub from: usize,
    /// To -- the ending character this doc block is anchored to. Likewise,
    /// avoid using an `Option` here.
    pub to: usize,
    /// Indent, or None if unchanged. Since the indent may be many characters,
    /// use an `Option` here.
    pub indent: Option<String>,
    /// Delimiter. Again, this is usually too short to merit an `Option`.
    pub delimiter: String,
    /// Contents, as a diff of the previous contents.
    pub contents: Vec<StringDiff>,
}

/// Store the difference between a previous and current string; this is based on
/// [CodeMirror's
/// ChangeSpec](https://codemirror.net/docs/ref/#state.ChangeSpec).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StringDiff {
    /// The index into the previous `CodeMirrorDocBlockVec` of the start of the
    /// change.
    pub from: usize,
    /// The index of the end of the change; defined for deletions and
    /// replacements.
    pub to: Option<usize>,
    /// The text to insert/replace; an empty string indicates deletion.
    pub insert: String,
}

/// Store one element of the difference between previous and current
/// `CodeMirrorDocBlockVec`s.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct CodeMirrorDocBlocksDiff {
    /// The index of the start of the change.
    pub from: usize,
    /// The index of the end of the change; defined for deletions and
    /// replacements.
    pub to: Option<usize>,
    /// The doc blocks to insert/replace; an empty vector indicates deletion.
    pub insert: Vec<CodeMirrorDocBlockDiff>,
}

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
    /// This file is unknown to the CodeChat Editor. It must be viewed raw or
    /// using the simple viewer.
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
    /// <a class="fence-mending-start"></a>If this matches, it means an
    /// unterminated fenced code block. This should be replaced with the
    /// `</code></pre>` terminator.
    static ref DOC_BLOCK_SEPARATOR_BROKEN_FENCE: Regex = Regex::new(concat!(
        // Allow the `.` wildcard to match newlines.
        "(?s)",
        // The first `<CodeChatEditor-fence>` will be munged when a fenced code
        // block isn't closed.
        "&lt;CodeChatEditor-fence&gt;\n",
        // Non-greedy wildcard -- match the first separator, so we don't munch
        // multiple `DOC_BLOCK_SEPARATOR_STRING`s in one replacement.
        ".*?",
        "<CodeChatEditor-separator/>\n")).unwrap();
}

// Use this as a way to end unterminated fenced code blocks and specific types
// of HTML blocks. (The remaining types of HTML blocks are terminated by a blank
// line, which this also provides.)
const DOC_BLOCK_SEPARATOR_STRING: &str = concat!(
    // If an HTML block with specific start conditions (see the [section 4.6 of
    // the commonmark spec](https://spec.commonmark.org/0.31.2/#html-blocks),
    // items 1-5) doesn't have a matching end condition, provide one here.
    // Otherwise, hide these end conditions inside a raw HTML block, so that it
    // doesn't get processed by the Markdown parser. Note that this only
    // supports fenced code blocks with an opening code fence of 23 characters
    // or less (which should cover most cases). To allow more, we'd need to know
    // the length of the opening code fence, which is hard to find. Since
    // CommonMark doesn't care if there are multiple HTML start conditions,
    // abuse this by not closing the fence until the very end of this string.
    r#"
<CodeChatEditor-fence>
</pre></script></style></textarea>-->?>]]>
"#,
    // Likewise, if there's an unterminated fenced code block with \`\`\`
    // characters, then provide the ending fence here. Otherwise, hide the
    // ending fence inside a raw HTML block as before.
    r#"<CodeChatEditor-fence>
```````````````````````
"#,
    // Repeat for the other style of fenced code block.
    r#"<CodeChatEditor-fence>
~~~~~~~~~~~~~~~~~~~~~~~
</CodeChatEditor-fence>
<CodeChatEditor-separator/>

"#
);

// After converting Markdown to HTML, this can be used to split doc blocks
// apart.
const DOC_BLOCK_SEPARATOR_SPLIT_STRING: &str = "<CodeChatEditor-separator/>\n";
// Correctly terminated fenced code blocks produce this, which can be removed
// from the HTML produced by Markdown conversion.
const DOC_BLOCK_SEPARATOR_REMOVE_FENCE: &str = r#"<CodeChatEditor-fence>
</pre></script></style></textarea>-->?>]]>
<CodeChatEditor-fence>
```````````````````````
<CodeChatEditor-fence>
~~~~~~~~~~~~~~~~~~~~~~~
</CodeChatEditor-fence>
"#;
// The replacement string for the `DOC_BLOCK_SEPARATOR_BROKEN_FENCE` regex.
const DOC_BLOCK_SEPARATOR_MENDED_FENCE: &str = "</code></pre>\n<CodeChatEditor-separator/>\n";
// <a class="fence-mending-end"></a>

// Serialization for `CodeMirrorDocBlock`
// --------------------------------------
#[derive(Serialize, Deserialize)]
struct CodeMirrorDocBlockTuple<'a>(
    // from
    usize,
    // to
    usize,
    // indent
    Cow<'a, str>,
    // delimiter
    Cow<'a, str>,
    // contents
    Cow<'a, str>,
);

// Convert the struct to a tuple, then serialize the tuple. This makes the
// resulting JSON more compact.
impl Serialize for CodeMirrorDocBlock {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let tuple = CodeMirrorDocBlockTuple(
            self.from,
            self.to,
            Cow::from(&self.indent),
            Cow::from(&self.delimiter),
            Cow::from(&self.contents),
        );
        tuple.serialize(serializer)
    }
}

// Deserialize the tuple, then convert it to a struct.
impl<'de> Deserialize<'de> for CodeMirrorDocBlock {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let tuple = CodeMirrorDocBlockTuple::deserialize(deserializer)?;
        Ok(CodeMirrorDocBlock {
            from: tuple.0,
            to: tuple.1,
            indent: tuple.2.into_owned(),
            delimiter: tuple.3.into_owned(),
            contents: tuple.4.into_owned(),
        })
    }
}

#[derive(Serialize, Deserialize)]
struct CodeMirrorDocBlockDiffTuple<'a>(
    // from
    usize,
    // to
    usize,
    // indent
    Option<Cow<'a, str>>,
    // delimiter
    Cow<'a, str>,
    // contents
    Vec<StringDiff>,
);

// Convert the struct to a tuple, then serialize the tuple. This makes the
// resulting JSON more compact.
impl Serialize for CodeMirrorDocBlockDiff {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let tuple = CodeMirrorDocBlockDiffTuple(
            self.from,
            self.to,
            self.indent.as_ref().map(Cow::from),
            Cow::from(&self.delimiter),
            self.contents.clone(),
        );
        tuple.serialize(serializer)
    }
}

// Deserialize the tuple, then convert it to a struct.
impl<'de> Deserialize<'de> for CodeMirrorDocBlockDiff {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let tuple = CodeMirrorDocBlockDiffTuple::deserialize(deserializer)?;
        Ok(CodeMirrorDocBlockDiff {
            from: tuple.0,
            to: tuple.1,
            indent: tuple.2.map(|s| s.into_owned()),
            delimiter: tuple.3.into_owned(),
            contents: tuple.4,
        })
    }
}

// Determine if the provided file is part of a project
// ---------------------------------------------------
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
    let CodeMirrorDiffable::Plain(ref code_mirror) = codechat_for_web.source else {
        panic!("No diff!");
    };
    let code_doc_block_vec = code_mirror_to_code_doc_blocks(code_mirror);
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
        let code_contents = &code[code_index..codemirror_doc_block.from];
        if !code_contents.is_empty() {
            // Convert back from a character array to a string.
            let s: String = code_contents.iter().collect();
            code_doc_block_arr.push(CodeDocBlock::CodeBlock(s.to_string()))
        }
        // Append the doc block.
        code_doc_block_arr.push(CodeDocBlock::DocBlock(DocBlock {
            indent: codemirror_doc_block.indent.to_string(),
            delimiter: codemirror_doc_block.delimiter.to_string(),
            contents: codemirror_doc_block.contents.to_string(),
            lines: 0,
        }));
        code_index = codemirror_doc_block.to + 1;
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
            CodeMirrorDiffable::Plain(CodeMirror {
                doc: html,
                doc_blocks: vec![],
            })
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

            // Convert the Markdown to HTML.
            let html = markdown_to_html(&doc_contents);

            // <a class="fence-mending-start"></a>Break it back into doc blocks:
            //
            // 1.  Mend broken fences.
            let html = DOC_BLOCK_SEPARATOR_BROKEN_FENCE
                .replace_all(&html, DOC_BLOCK_SEPARATOR_MENDED_FENCE);
            // 2.  Remove good fences.
            let html = html.replace(DOC_BLOCK_SEPARATOR_REMOVE_FENCE, "");
            // 3.  Split on the separator.
            let mut doc_block_contents_iter = html.split(DOC_BLOCK_SEPARATOR_SPLIT_STRING);
            // <a class="fence-mending-end"></a>

            // Translate each `CodeDocBlock` to its `CodeMirror` equivalent.
            for code_or_doc_block in code_doc_block_arr {
                let source = &mut code_mirror.doc;
                match code_or_doc_block {
                    CodeDocBlock::CodeBlock(code_string) => source.push_str(&code_string),
                    CodeDocBlock::DocBlock(doc_block) => {
                        // Create the doc block.
                        //
                        // Get the length of the string in characters (not
                        // bytes, which is what `len()` returns).
                        let len = source.chars().count();
                        code_mirror.doc_blocks.push(CodeMirrorDocBlock {
                            // From
                            from: len,
                            // To. Make this one line short, which allows
                            // CodeMirror to correctly handle inserts at the
                            // first character of the following code block. Note
                            // that the last doc block could be zero length, so
                            // handle this case.
                            to: len + max(doc_block.lines, 1) - 1,
                            indent: doc_block.indent.to_string(),
                            delimiter: doc_block.delimiter.to_string(),
                            // Used the markdown-translated replacement for this
                            // doc block, rather than the original string.
                            contents: doc_block_contents_iter.next().unwrap().to_string(),
                        });
                        // Append newlines to the document; the doc block will
                        // replace these in the editor. This keeps the line
                        // numbering of non-doc blocks correct.
                        source.push_str(&"\n".repeat(doc_block.lines));
                    }
                }
            }
            CodeMirrorDiffable::Plain(code_mirror)
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
                    let CodeMirrorDiffable::Plain(plain) = codechat_for_web.source else {
                        panic!("No diff!");
                    };
                    TranslationResultsString::Toc(plain.doc)
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

// ### Diff support
//
// This section provides methods to diff the previous and current
// `CodeMirrorDocBlockVec`.  The primary purpose is to fix a visual bug: if the
// entire CodeMirror data structure is overwritten, then CodeMirror loses track
// of the correct vertical scroll bar position, probably because it has build up
// information on the size of each rendered doc block; these correct sizes are
// reset when all data is overrwritten, causing unexpected scrolling. Therefore,
// this approach is to modify only what changed, rather than changing
// everything. As a secondary goal, this hopefully improves overall performance
// by sending less data between the server and the client, in spite of the
// additional computational requirements for compting the diff.
//
// Fundamentally, diffs of a string and diff of this vector require different
// approaches:
//
// *   The `CodeMirrorDocBlock` is a structure, with several fields. In
//     particular, the contents is usually the largest element; the indent can
//     also be large.
// *   It should handle the following common cases well:
//     1.  An update of a code block. This causes the from and to field of all
//         following doc blocks to change, without changing the other fields.
//     2.  An update to the contents of a doc block. For large doc blocks, this
//         is more efficiently stored as a diff rather than the full doc block
//         text.
//     3.  Inserting or deleting a doc block.
//
// The diff algorithm simply looks for equality between elements contained in
// the before and after vectors provided it. However, this requires something
// more fine-grained: the ability to track changes to the `contents` as a first
// priority (common cases 2, 3), then fix up non-`contents` field (common case
// 1).
//
// #### Overall approach
//
// 1.  Use the diff algorithm to find the minimal change set between a before
//     and after `CodeMirrorDocBlocksVec`, which only looks at the `contents`.
//     This avoids "noise" from changes in from/to fields from obscuring changes
//     only to the `contents`.
// 2.  For all before and after blocks whose `contents` were identical, compare
//     the other fields, adding these to the change set, but not attempting to
//     use the diff algorithm.
// 3.  Represent changes to the `contents` as a `StringDiff`.
//
// #### String diff
/// Given two strings, return a list of changes between them.
fn diff_str(before: &str, after: &str) -> Vec<StringDiff> {
    let mut change_spec: Vec<StringDiff> = Vec::new();
    // The previous value of `before.start` and the character index
    // corresponding to `before.start`.
    let mut prev_before_start = 0;
    let mut prev_before_start_chars = 0;
    let input = InternedInput::new(lines_with_terminator(before), lines_with_terminator(after));
    let sink = |before: Range<u32>, after: Range<u32>| {
        let count_before_chars = |lines: Range<u32>| {
            input.before[lines.start as usize..lines.end as usize]
                .iter()
                .map(|&line| input.interner[line].chars().count())
                .sum::<usize>()
        };
        // Sum characters between the last change and this change.
        prev_before_start_chars += count_before_chars(prev_before_start..before.start);
        prev_before_start = before.start;
        // Get the characters in the hunk after this change.
        let hunk_after: Vec<_> = input.after[after.start as usize..after.end as usize]
            .iter()
            .map(|&line| input.interner[line])
            .collect();
        let before_chars = count_before_chars(before.start..before.end);
        change_spec.push(StringDiff {
            from: prev_before_start_chars,
            to: if before_chars != 0 {
                Some(prev_before_start_chars + before_chars)
            } else {
                None
            },
            insert: if hunk_after.is_empty() {
                "".to_string()
            } else {
                hunk_after.into_iter().collect()
            },
        })
    };

    diff(Algorithm::Histogram, &input, sink);
    change_spec
}

// #### Diff support for `CodeMirrorDocBlockVec`
/// We can't simply implement traits for `CodeMirrorDocBlockVec`, since it's not
/// a struct. So, wrap that it in a struct, then implement traits on that
/// struct.
struct CodeMirrorDocBlocksStruct<'a>(&'a CodeMirrorDocBlockVec);

/// Only compare the `contents` of two doc blocks; later, we'll compare the
/// other fields as well.
impl<'a> TokenSource for CodeMirrorDocBlocksStruct<'a> {
    type Token = &'a str;

    type Tokenizer = Map<Iter<'a, CodeMirrorDocBlock>, fn(&'a CodeMirrorDocBlock) -> &'a str>;

    // Ignore the other fields; just use the contents for tokenizing.
    fn tokenize(&self) -> Self::Tokenizer {
        self.0.iter().map(|x| &x.contents)
    }

    fn estimate_tokens(&self) -> u32 {
        self.0.len() as u32
    }
}

/// Given two `CodeMirrorDocBlocks`, return a list of changes between them.
fn diff_code_mirror_doc_blocks(
    before: &CodeMirrorDocBlockVec,
    after: &CodeMirrorDocBlockVec,
) -> Vec<CodeMirrorDocBlocksDiff> {
    let input = InternedInput::new(
        CodeMirrorDocBlocksStruct(before),
        CodeMirrorDocBlocksStruct(after),
    );
    let change_spec: Rc<RefCell<Vec<CodeMirrorDocBlocksDiff>>> = Rc::new(RefCell::new(Vec::new()));
    // This compare all fields, not just the `contents`, of two
    // `CodeMirrorDocBlock`s. It should be applied to every entry that the
    // `diff` function sees as equal.
    let diff_all = |prev_before_range_end: u32,
                    before_range_start: u32,
                    prev_after_range_end: u32,
                    after_range_start: u32| {
        let mut prev_before_range_end = prev_before_range_end;
        let mut prev_after_range_end = prev_after_range_end;

        // First, compare blocks from the previous point until this point. The
        // diff used only compares contents; this checks everything.
        while prev_before_range_end < before_range_start && prev_after_range_end < after_range_start
        {
            // Note that `input[before/after_range.start]` only returns the
            // `contents` (a `&str`), not the full `CodeMirrorDocBlock` (since
            // we only want to compare strings for the first phase of the diff).
            // This is the second phase of the diff -- looking for changes
            // beyond the `contents`. For this, we need the full
            // `CodeMirrorDocBlock`. Fortunately, the indices of
            // `before/after_range` (which refers to only the `contents`) match
            // the same (full) object in `before/after`; simply use these
            // indices to get the full object.
            let prev_before_range_start_val = &before[prev_before_range_end as usize];
            let prev_after_range_start_val = &after[prev_after_range_end as usize];
            // Second phase: if before and after are different, insert a diff.
            if prev_before_range_start_val != prev_after_range_start_val {
                change_spec.borrow_mut().push(CodeMirrorDocBlocksDiff {
                    from: prev_before_range_end as usize,
                    to: Some(prev_before_range_end as usize + 1),
                    insert: vec![CodeMirrorDocBlockDiff {
                        from: prev_after_range_start_val.from,
                        to: prev_after_range_start_val.to,
                        indent: if prev_before_range_start_val.indent
                            == prev_after_range_start_val.indent
                        {
                            None
                        } else {
                            Some(prev_after_range_start_val.indent.clone())
                        },
                        delimiter: prev_after_range_start_val.delimiter.clone(),
                        contents: diff_str(
                            &prev_after_range_start_val.contents,
                            &prev_after_range_start_val.contents,
                        ),
                    }],
                });
            }

            prev_before_range_end += 1;
            prev_after_range_end += 1;
        }
    };

    let mut prev_before_range_end = 0;
    let mut prev_after_range_end = 0;
    let sink = |before_range: Range<u32>, after_range: Range<u32>| {
        diff_all(
            prev_before_range_end,
            before_range.start,
            prev_after_range_end,
            after_range.start,
        );
        // Update the `prev` values so we start processing immediately after
        // this change.
        prev_before_range_end = before_range.end;
        prev_after_range_end = after_range.end;

        // Process the insertions and deletions.
        let mut before_index = before_range.start;
        let mut insert = Vec::new();
        // Values in the `after_index` become either inserts or replacements.
        for after_index in after_range {
            let after_val = &after[after_index as usize];
            // Assume that an insert/delete is a replace; this is the most
            // common case (a minor edit to the text of a doc block). If not,
            // the replace is a bit less efficient than the insert/delete, but
            // still correct.
            if before_index < before_range.end {
                let before_val = &before[before_index as usize];
                insert.push(CodeMirrorDocBlockDiff {
                    from: after_val.from,
                    to: after_val.to,
                    indent: if before_val.indent == after_val.indent {
                        None
                    } else {
                        Some(after_val.indent.clone())
                    },
                    delimiter: after_val.delimiter.clone(),
                    contents: diff_str(&before_val.contents, &after_val.contents),
                });
                before_index += 1;
            } else {
                // Otherwise, this in an insert.
                insert.push(CodeMirrorDocBlockDiff {
                    from: after_val.from,
                    to: after_val.to,
                    indent: Some(after_val.indent.clone()),
                    delimiter: after_val.delimiter.clone(),
                    contents: diff_str("", &after_val.contents),
                });
            }
        }

        // Now, create a diff from the the `before_range` and the `insert`s.
        change_spec.borrow_mut().push(CodeMirrorDocBlocksDiff {
            from: before_range.start as usize,
            to: if before_range.start == before_range.end {
                None
            } else {
                Some(before_range.end as usize)
            },
            insert,
        });
    };

    diff(Algorithm::Histogram, &input, sink);
    diff_all(
        prev_before_range_end,
        before.len() as u32,
        prev_after_range_end,
        after.len() as u32,
    );
    // Extract the underlying vec from the `Rc<RefCell<>>`.
    take(&mut *change_spec.borrow_mut())
}

// Goal: make it easy to update the data structure. We update on every
// load/save, then do some accesses during those processes.
//
// Top-level data structures: a file HashSet<PathBuf, FileAnchor> and an id
// HashMap<id, {Anchor, HashSet<referring\_id>}>. Some FileAnchors in the file
// HashSet are also in a pending load list..
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
mod tests;
