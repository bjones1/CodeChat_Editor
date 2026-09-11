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
//! `processing.rs` -- Transform source code to its web-editable equivalent and
//! back
//! ===========================================================================
// Modules
// -------
pub mod cache;

// Imports
// -------
//
// ### Standard library
use std::{
    borrow::Cow,
    cell::RefCell,
    cmp::{max, min},
    collections::HashMap,
    collections::HashSet,
    ffi::OsStr,
    io,
    iter::Map,
    mem,
    ops::Range,
    path::{Path, PathBuf},
    rc::Rc,
    slice::Iter,
    string::FromUtf8Error,
    sync::LazyLock,
    sync::{Arc, Mutex},
};

// ### Third-party
use ammonia::Builder;
use dprint_plugin_markdown::{
    FormatError,
    configuration::{
        Configuration, ConfigurationBuilder, EmphasisKind, HeadingKind, ListUnorderedMarker,
        StrongKind, TextWrap,
    },
    format_text,
};
use htmd::{
    HtmlToMarkdown,
    options::{BrStyle, LinkStyle, TranslationMode},
};
use html5ever::{
    Attribute, LocalName, Namespace, ParseOpts, QualName, parse_document, serialize,
    serialize::{SerializeOpts, TraversalScope},
    tendril::TendrilSink,
    tree_builder::TreeBuilderOpts,
};
use imara_diff::{Algorithm, Diff, Hunk, InternedInput, TokenSource};
use markup5ever_rcdom::{Node, NodeData, RcDom, SerializableHandle};
use minify_html;
use path_slash::PathBufExt as _;
use phf::phf_map;
use pulldown_cmark::{Options, Parser, html};
use regex::Regex;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ### Local
use crate::{
    lexer::{
        CodeDocBlock, DocBlock, LEXERS, LanguageLexerCompiled, source_lexer,
        supported_languages::MARKDOWN_MODE,
    },
    processing::cache::{FileFacts, FragmentFact, IdResolution, TargetFact},
};
use cache::{Cache, CacheMap};

// Data structures
// ---------------
//
// ### Translation between a local (traditional) source file and its web-editable, client-side representation
//
// There are three ways that a source file is represented:
//
// 1. As traditional source code, in a plain text file.
// 2. As a alternating series of code and doc blocks, produced by the lexer. See
//    `lexer.rs\CodeDocBlock`.
// 3. As a CodeMirror data structure, which consists of a single block of text,
//    to which are attached doc blocks at specific character offsets.
//
// The lexer translates between items 1 and 2; `processing.rs` translates
// between 2 and 3. The following data structures define the format for item 3.

/// <a id="LexedSourceFile"></a>Define the JSON data structure used to represent
/// a source file in a web-editable format.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TS)]
#[ts(export)]
pub struct CodeChatForWeb {
    pub metadata: SourceFileMetadata,
    /// The version number after accepting this update.
    pub version: f64,
    pub source: CodeMirrorDiffable,
}

/// Provide two options for sending CodeMirror data -- as the full contents
/// (`Plain`), or as a diff of the existing contents (`Diff`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TS)]
#[ts(export)]
pub enum CodeMirrorDiffable {
    Plain(CodeMirror),
    Diff(CodeMirrorDiff),
}

/// <a id="SourceFileMetadata"></a>Metadata about a source file sent along with
/// it both to and from the client. TODO: currently, this is too simple to
/// justify a struct. This allows for future growth -- perhaps the valid types
/// of comment delimiters?
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TS)]
pub struct SourceFileMetadata {
    /// The lexer used to transforms source code into code and doc blocks and
    /// vice versa.
    pub mode: String,
}

pub type CodeMirrorDocBlockVec = Vec<CodeMirrorDocBlock>;

/// The format used by CodeMirror to serialize/deserialize editor contents.
/// TODO: Link to JS code where this data structure is defined.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TS)]
pub struct CodeMirror {
    /// The document being edited.
    pub doc: String,
    pub doc_blocks: CodeMirrorDocBlockVec,
}

/// A diff of the `CodeMirror` struct.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TS)]
pub struct CodeMirrorDiff {
    /// The version number from which this diff was produced.
    pub version: f64,
    /// A diff of the document being edited.
    pub doc: Vec<StringDiff>,
    pub doc_blocks: Vec<CodeMirrorDocBlockTransaction>,
}

/// A transaction produced by the diff of the `CodeMirror` struct.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TS)]
pub enum CodeMirrorDocBlockTransaction {
    Add(CodeMirrorDocBlock),
    Update(CodeMirrorDocBlockUpdate),
    Delete(CodeMirrorDocBlockDelete),
}

/// This defines a doc block for CodeMirror.
#[derive(Clone, Debug, PartialEq, TS)]
// Serde replaces this struct with a tuple for coding efficiency -- see
// `CodeMirrorDocBlockTuple`.
#[ts(as = "CodeMirrorDocBlockTuple")]
pub struct CodeMirrorDocBlock {
    /// The starting character this doc block is anchored to, measured in UTF-16
    /// code units. `to` is measured the same way.
    pub from: usize,
    /// The ending character this doc block is anchored to.
    pub to: usize,
    /// Indent.
    pub indent: String,
    /// Delimiter.
    pub delimiter: String,
    /// Contents.
    pub contents: String,
}

/// Store the difference between the previous and current `CodeMirrorDocBlock`s.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TS)]
#[ts(optional_fields)]
pub struct CodeMirrorDocBlockUpdate {
    /// The starting character this doc block is anchored to before this update.
    /// Like `CodeMirrorDocBlock`, units for this, `from_update`, and `to` are
    /// in UTF-16 code units.
    pub from: usize,
    /// The starting character this doc block is anchored to after this update.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_new: Option<usize>,
    /// The ending character this doc block is anchored to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<usize>,
    /// `None` if the indent is unchanged. Since the indent may be many
    /// characters, use an `Option` here.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indent: Option<String>,
    /// Delimiter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delimiter: Option<String>,
    /// Contents, as a diff of the previous contents.
    pub contents: Vec<StringDiff>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TS)]
pub struct CodeMirrorDocBlockDelete {
    pub from: usize,
}

/// Store the difference between a previous and current string; this is based on
/// [CodeMirror's ChangeSpec](https://codemirror.net/docs/ref/#state.ChangeSpec).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, optional_fields)]
pub struct StringDiff {
    /// The index of the start of the change, in UTF-16 code units.
    pub from: usize,
    /// The index of the end of the change; defined for deletions and
    /// replacements, in UTF-16 code units. See the
    /// [skip serializing field docs](https://serde.rs/attr-skip-serializing.html);
    /// this must be excluded from the JSON output if it's `None` to avoid
    /// CodeMirror errors.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<usize>,
    /// The text to insert/replace; an empty string indicates deletion.
    pub insert: String,
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
/// Match the lexer directive in a source file.
static LEXER_DIRECTIVE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"CodeChat Editor lexer: (\w+)").unwrap());
/// If this matches, it means an unterminated fenced code block. This should be
/// replaced with the `</code></pre>` terminator.
static DOC_BLOCK_SEPARATOR_BROKEN_FENCE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        // Allow the `.` wildcard to match newlines.
        "(?s)",
        // The first `<CodeChatEditor-fence>` will be munged when a fenced code
        // block isn't closed.
        "&lt;CodeChatEditor-fence&gt;\n",
        // Non-greedy wildcard -- match the first separator, so we don't munch
        // multiple `DOC_BLOCK_SEPARATOR_STRING`s in one replacement.
        ".*?",
        r"<CodeChatEditor-separator>(\d+)</CodeChatEditor-separator>\n"
    ))
    .unwrap()
});
/// After converting Markdown to HTML, this can be used to split doc blocks
/// apart. Since this is post hydration, the element names are normalized to
/// lower case.
static DOC_BLOCK_SEPARATOR_SPLIT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<codechateditor-separator>\d+</codechateditor-separator>").unwrap()
});
/// Match a valid
/// [CSS identifier](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Values/ident),
/// which all cached ids must be. This is a slight simplification of the CSS
/// grammar: escape sequences aren't recognized, and all code points above
/// U+0080 are accepted.
static CSS_IDENTIFIER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:--|-?[_a-zA-Z\x{0080}-\x{10FFFF}])[-_a-zA-Z0-9\x{0080}-\x{10FFFF}]*$").unwrap()
});

// Use this as a way to end unterminated fenced code blocks and specific types
// of HTML blocks. (The remaining types of HTML blocks are terminated by a blank
// line, which this also provides.)
const DOC_BLOCK_SEPARATOR_STRING: &str = concat!(
    // If an HTML block with specific start conditions (see the
    // [section 4.6 of the commonmark spec](https://spec.commonmark.org/0.31.2/#html-blocks),
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
<CodeChatEditor-separator>{}</CodeChatEditor-separator>

"#
);

// Correctly terminated fenced code blocks produce this, which can be removed
// from the HTML produced by Markdown conversion.
const DOC_BLOCK_SEPARATOR_REMOVE_FENCE: &str = r"<CodeChatEditor-fence>
</pre></script></style></textarea>-->?>]]>
<CodeChatEditor-fence>
```````````````````````
<CodeChatEditor-fence>
~~~~~~~~~~~~~~~~~~~~~~~
</CodeChatEditor-fence>
";
// The replacement string for the `DOC_BLOCK_SEPARATOR_BROKEN_FENCE` regex. It
// relies on the first capture group in that regex (`$1`) containing the index,
// which it replaces here.
const DOC_BLOCK_SEPARATOR_MENDED_FENCE: &str =
    "</code></pre>\n<CodeChatEditor-separator>$1</CodeChatEditor-separator>\n";
// The value of an `id` attribute which requests that the cache assign an id;
// see `Auto-assignment of ids` in `cache.rs`.
const AUTO_ID: &str = "*";
// The column at which to word wrap doc blocks.
const WORD_WRAP_COLUMN: usize = 80;
// The minimum width for doc block word wrap, since large indents may leave
// little space for word wrapping.
const WORD_WRAP_MIN_WIDTH: usize = 40;

// Serialization for `CodeMirrorDocBlock`
// --------------------------------------
#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
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

// Determine if the provided file is part of a project
// ---------------------------------------------------
#[must_use]
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

#[derive(Debug, thiserror::Error)]
pub enum CodechatForWebToSourceError {
    #[error("invalid lexer {0}")]
    InvalidLexer(String),
    #[error("doc blocks not allowed in Markdown documents")]
    DocBlocksNotAllowed,
    #[error("TODO: diffs not supported")]
    TodoDiff,
    #[error("unable to convert from HTML to Markdown: {0}")]
    HtmlToMarkdownFailed(#[from] HtmlToMarkdownWrappedError),
    #[error("unable to translate CodeChat to source: {0}")]
    CannotTranslateCodeChat(#[from] CodeDocBlockVecToSourceError),
    #[error("unable to parse HTML {0}")]
    ParseFailed(#[from] io::Error),
}

// Transform `CodeChatForWeb` to source code
// -----------------------------------------
/// This function takes in a source file in web-editable format (the
/// `CodeChatForWeb` struct) and transforms it into source code.
pub fn codechat_for_web_to_source(
    // The file to save plus metadata, stored in the `LexedSourceFile`
    codechat_for_web: &CodeChatForWeb,
) -> Result<String, CodechatForWebToSourceError> {
    let lexer_name = &codechat_for_web.metadata.mode;
    // Given the mode, find the lexer.
    let Some(lexer) = LEXERS.map_mode_to_lexer.get(lexer_name) else {
        return Err(CodechatForWebToSourceError::InvalidLexer(
            lexer_name.clone(),
        ));
    };

    // Extract the plain (not diffed) CodeMirror contents.
    let CodeMirrorDiffable::Plain(ref code_mirror) = codechat_for_web.source else {
        return Err(CodechatForWebToSourceError::TodoDiff);
    };

    // If this is a Markdown-only document, handle this special case.
    if *lexer.language_lexer.lexer_name == MARKDOWN_MODE {
        // There should be no doc blocks.
        if !code_mirror.doc_blocks.is_empty() {
            return Err(CodechatForWebToSourceError::DocBlocksNotAllowed);
        }
        // Translate the HTML document to Markdown.
        let converter = HtmlToMarkdownWrapped::new();
        let tree = html_to_dom(&code_mirror.doc, None)?;
        dehydrating_walk_node(&tree);
        return converter
            .convert(&tree)
            .map_err(CodechatForWebToSourceError::HtmlToMarkdownFailed);
    }
    let code_doc_block_vec_html = code_mirror_to_code_doc_blocks(code_mirror);
    let code_doc_block_vec = doc_block_html_to_markdown(code_doc_block_vec_html, None)
        .map_err(CodechatForWebToSourceError::HtmlToMarkdownFailed)?;
    code_doc_block_vec_to_source(&code_doc_block_vec, lexer)
        .map_err(CodechatForWebToSourceError::CannotTranslateCodeChat)
}

/// Return the byte index of `s[utf_16_index]`, where the indexing operation is
/// in UTF-16 code units.
#[must_use]
pub fn byte_index_of(s: &str, utf_16_index: usize) -> usize {
    let mut byte_index = 0;
    let mut current_index = 0;
    for c in s.chars() {
        if current_index >= utf_16_index {
            return byte_index;
        }
        current_index += c.len_utf16();
        byte_index += c.len_utf8();
    }
    // This index refers to the end of the string -- return that.
    s.len()
}

/// Translate from CodeMirror to CodeDocBlocks.
fn code_mirror_to_code_doc_blocks(code_mirror: &CodeMirror) -> Vec<CodeDocBlock> {
    let doc_blocks = &code_mirror.doc_blocks;
    // Translate between UTF-16 code units (the `from` and `to` provided by
    // CodeMirror) and byte indexes (which Rust uses). Keep track of the current
    // byte index/UTF-16 index; we always move forward from that location.
    let mut byte_index: usize = 0;
    let mut utf16_index: usize = 0;
    let mut code_doc_block_arr: Vec<CodeDocBlock> = Vec::new();

    // Walk through each doc block, inserting the previous code block followed
    // by the doc block.
    for codemirror_doc_block in doc_blocks {
        // Translate `from`. Use a checked subtraction, since a malformed (for
        // example, out-of-order or overlapping) `doc_blocks` array sent by the
        // Client could otherwise cause this to underflow and wrap around to a
        // huge value, producing a confusing out-of-bounds slice panic below
        // instead of a clear error at the point of the actual problem.
        let byte_index_prev = byte_index;
        byte_index += byte_index_of(
            &code_mirror.doc[byte_index..],
            codemirror_doc_block
                .from
                .checked_sub(utf16_index)
                .expect("doc_blocks must be sorted, with non-overlapping from/to ranges"),
        );
        utf16_index = codemirror_doc_block.from;
        // Append the code block, unless it's empty.
        let code_contents = &code_mirror.doc[byte_index_prev..byte_index];
        if !code_contents.is_empty() {
            code_doc_block_arr.push(CodeDocBlock::CodeBlock(code_contents.to_string()));
        }
        // Append the doc block.
        code_doc_block_arr.push(CodeDocBlock::DocBlock(DocBlock {
            indent: codemirror_doc_block.indent.clone(),
            delimiter: codemirror_doc_block.delimiter.clone(),
            contents: codemirror_doc_block.contents.clone(),
            lines: 0,
        }));
        let byte_index_prev = byte_index;
        // Translate `to`.
        byte_index += byte_index_of(
            &code_mirror.doc[byte_index..],
            codemirror_doc_block
                .to
                .checked_sub(utf16_index)
                .expect("doc_blocks must be sorted, with non-overlapping from/to ranges"),
        );
        utf16_index = codemirror_doc_block.to;
        // Verify that everything between `from` and `to` is newlines.
        for char in code_mirror.doc[byte_index_prev..byte_index].chars() {
            assert_eq!(char, '\n');
        }
    }

    // See if there's a code block after the last doc block.
    let code_contents = &code_mirror.doc[byte_index..];
    if !code_contents.is_empty() {
        code_doc_block_arr.push(CodeDocBlock::CodeBlock(code_contents.to_string()));
    }

    code_doc_block_arr
}

/// This converts HTML to Markdown then word wraps the result.
struct HtmlToMarkdownWrapped {
    html_to_markdown: HtmlToMarkdown,
    word_wrap_config: Configuration,
}

#[derive(Debug, thiserror::Error)]
pub enum HtmlToMarkdownWrappedError {
    #[error("unable to convert from HTML to markdown")]
    HtmlToMarkdownFailed(#[from] std::io::Error),
    #[error("unable to word wrap Markdown")]
    WordWrapFailed(#[from] FormatError),
    #[error("line width exceeds u32::MAX")]
    LineWidthOverflow(#[from] std::num::TryFromIntError),
}

impl HtmlToMarkdownWrapped {
    fn new() -> Self {
        HtmlToMarkdownWrapped {
            // Most of the options don't need to be specified here, since the
            // line wrapper will override them.
            html_to_markdown: HtmlToMarkdown::builder()
                .options(htmd::options::Options {
                    link_style: LinkStyle::Inlined,
                    translation_mode: TranslationMode::Faithful,
                    // Note that this is ignored in Faithful mode.
                    br_style: BrStyle::Backslash,
                    ..Default::default()
                })
                .build(),
            // TODO: numbered list formatting should be improved in the dprint
            // library.
            word_wrap_config: ConfigurationBuilder::new()
                .emphasis_kind(EmphasisKind::Asterisks)
                .strong_kind(StrongKind::Asterisks)
                .list_unordered_marker(ListUnorderedMarker::Asterisks)
                .text_wrap(TextWrap::Always)
                .heading_kind(HeadingKind::Setext)
                .code_block_preserve_blank_lines(true)
                .code_block_preserve_indentation(true)
                .build(),
        }
    }
    fn set_line_width(&mut self, line_width: usize) -> Result<(), HtmlToMarkdownWrappedError> {
        self.word_wrap_config.line_width = u32::try_from(line_width)?;
        Ok(())
    }

    /// Convert one item in a stream of HTML to markdown. The HTML must be start
    /// at the root, not continue a previous incomplete section of the DOM.
    fn next(&self, tree: &Rc<Node>) -> Result<String, HtmlToMarkdownWrappedError> {
        let converted = self.html_to_markdown.tree_to_markdown(tree);
        Ok(
            format_text(&converted, &self.word_wrap_config, |_, _, _| Ok(None))?
                // A return value of `None` means the text was unchanged or
                // ignored (by an
                // [ignoreFileDirective](https://dprint.dev/plugins/markdown/config/)).
                // Simply return the unchanged text in this case.
                .unwrap_or(converted),
        )
    }

    fn last(&self) -> Result<String, HtmlToMarkdownWrappedError> {
        let converted = self.html_to_markdown.finalize_conversion();
        Ok(
            format_text(&converted, &self.word_wrap_config, |_, _, _| Ok(None))?
                .unwrap_or(converted),
        )
    }

    /// Convert HTML to markdown.
    fn convert(&self, tree: &Rc<Node>) -> Result<String, HtmlToMarkdownWrappedError> {
        let mut converted = self.next(tree)?;
        converted.push_str(&self.last()?);
        Ok(converted)
    }
}

// Transform HTML in doc blocks to Markdown.
pub fn doc_block_html_to_markdown(
    mut code_doc_block_vec: Vec<CodeDocBlock>,
    // If provided, the index of each successive node in the DOM, ending with
    // the offset in UTF-16 characters within the last node (which must be a
    // text node) at which a marker character will be inserted.
    //
    // For this reason, when provided, this function must called with a vec
    // containing only one doc block.
    dom_location: Option<&(Vec<usize>, usize)>,
) -> Result<Vec<CodeDocBlock>, HtmlToMarkdownWrappedError> {
    let mut converter = HtmlToMarkdownWrapped::new();
    let mut last_doc_block_index = None;
    // Only perform marker insertions to a length 1 vec.
    assert!(dom_location.is_none() || code_doc_block_vec.len() == 1);
    for (index, code_doc_block) in &mut code_doc_block_vec.iter_mut().enumerate() {
        if let CodeDocBlock::DocBlock(doc_block) = code_doc_block {
            last_doc_block_index = Some(index);
            let tree = html_to_dom(&doc_block.contents, dom_location)?;
            dehydrating_walk_node(&tree);

            // Calculate the total delimiter width: the delimiter width plus the
            // extra space after it. Special case: an empty delimiter means
            // we're wrapping a Markdown document to insert a marker, so don't
            // add the extra space.
            let delimiter_width = doc_block.delimiter.chars().count();
            let total_delimiter_width = if delimiter_width > 0 {
                delimiter_width + 1
            } else {
                0
            };
            // Compute a line wrap width based on the current indent. Set a
            // minimum of half the line wrap width, to prevent ridiculous
            // wrapping with large indents.
            converter.set_line_width(max(
                WORD_WRAP_MIN_WIDTH,
                // Use `min` to avoid overflow with unsigned subtraction.
                WORD_WRAP_COLUMN
                    - min(
                        total_delimiter_width + doc_block.indent.chars().count(),
                        WORD_WRAP_COLUMN,
                    ),
            ))?;
            doc_block.contents = converter.next(&tree)?;
        }
    }

    // Append the finalized conversion to the last doc block.
    if let Some(last_doc_block_index) = last_doc_block_index {
        let CodeDocBlock::DocBlock(ref mut last_doc_block) =
            code_doc_block_vec[last_doc_block_index]
        else {
            unreachable!();
        };
        let last = converter.last()?;
        last_doc_block.contents.push_str(&last);
    }

    Ok(code_doc_block_vec)
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum CodeDocBlockVecToSourceError {
    #[error("unknown comment opening delimiter '{0}'")]
    UnknownCommentOpeningDelimiter(String),
}

// Turn this vec of CodeDocBlocks into a string of source code.
fn code_doc_block_vec_to_source(
    code_doc_block_vec: &[CodeDocBlock],
    lexer: &LanguageLexerCompiled,
) -> Result<String, CodeDocBlockVecToSourceError> {
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
                    // does, and is also the subject of a
                    // [Rust bug report](https://github.com/rust-lang/rust/issues/111457).
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
                            return Err(
                                CodeDocBlockVecToSourceError::UnknownCommentOpeningDelimiter(
                                    doc_block.delimiter.clone(),
                                ),
                            );
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
                            // * If this line is just a newline, include just
                            //   the newline.
                            if *content_line == "\n" {
                                append_doc_block("", "", "\n");
                            // * Otherwise, include spaces in place of the
                            //   delimiter.
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
                file_contents += contents;
            }
        }
    }
    Ok(file_contents)
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum SourceToCodeChatForWebError {
    #[error("unknown lexer {0}")]
    UnknownLexer(String),
    // Since we want `PartialEq`, we can't use `#[from] io::Error`; instead,
    // convert the IO error to a string.
    #[error("unable to parse HTML {0}")]
    ParseFailed(String),
    #[error("no lexer for this file")]
    NoLexer,
    #[error("encoding error {0}")]
    EncodeFailed(#[from] FromUtf8Error),
}

// Transform from source code to `CodeChatForWeb`
// ----------------------------------------------
//
// Given the contents of a file, classify it and (for CodeChat Editor files)
// convert it to the `CodeChatForWeb` format.
#[allow(clippy::too_many_lines)]
pub fn source_to_codechat_for_web(
    // The file's contents.
    file_contents: &str,
    // The file's extension.
    file_path: &Path,
    // The version of this file.
    version: f64,
    // True if this file is a TOC.
    _is_toc: bool,
    // If provided, the cache for this project; otherwise, this file is not in a
    // project.
    cache: Option<Arc<Mutex<Cache>>>,
) -> Result<CodeChatForWeb, SourceToCodeChatForWebError> {
    // Determine the file's extension, in order to look up a lexer.
    let file_ext = &file_path
        .extension()
        .unwrap_or_else(|| OsStr::new(""))
        .to_string_lossy()
        .to_string();

    // Determine the lexer to use for this file.
    let lexer_name;
    // First, search for a lexer directive in the file contents.
    let lexer = if let Some(captures) = LEXER_DIRECTIVE.captures(file_contents) {
        lexer_name = captures[1].to_string();
        match LEXERS.map_mode_to_lexer.get(&lexer_name) {
            Some(v) => v,
            None => {
                return Err(SourceToCodeChatForWebError::UnknownLexer(lexer_name));
            }
        }
    } else {
        // Otherwise, look up the lexer by the file's extension.
        match LEXERS.map_ext_to_lexer_vec.get(file_ext) {
            Some(llc) => llc.first().unwrap(),
            _ => {
                // The file type is unknown; we can't lex it.
                return Err(SourceToCodeChatForWebError::NoLexer);
            }
        }
    };

    // Transform the provided file into the `CodeChatForWeb` structure.
    let cache = if let Some(project_cache) = cache {
        project_cache
    } else {
        // A non-project file uses a throwaway cache whose "project" is the
        // file's directory: only references within this file resolve.
        Arc::new(Mutex::new(Cache::new(
            file_path
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .to_path_buf(),
        )))
    };
    let code_doc_block_arr;
    let codechat_for_web = CodeChatForWeb {
        metadata: SourceFileMetadata {
            mode: lexer.language_lexer.lexer_name.to_string(),
        },
        version,
        source: if lexer.language_lexer.lexer_name.as_str() == MARKDOWN_MODE {
            // Document-only files are easy: just encode the contents. Fragments
            // aren't supported in Markdown documents; `hydrate_html` reports
            // them as errors.
            let dry_html = markdown_to_html(file_contents);
            let html = hydrate_html(&dry_html, file_path, &cache)
                .map_err(|e| SourceToCodeChatForWebError::ParseFailed(e.to_string()))?;
            let html = minify(&html)?;
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
                doc: String::new(),
                doc_blocks: Vec::new(),
            };

            // Lex the code.
            code_doc_block_arr = source_lexer(file_contents, lexer);

            // Combine all the doc blocks into a single string, separated by a
            // delimiter. Transform this to markdown, then split the transformed
            // content back into the doc blocks they came from. This is
            // necessary to allow
            // [link reference definitions](https://spec.commonmark.org/0.31.2/#link-reference-definitions)
            // between doc blocks to work; for example, `[Link][1]` in one doc
            // block, then `[1]: http:/foo.org` in another doc block requires
            // both to be in the same Markdown document to translate correctly.
            //
            // Walk through the code/doc blocks, ...
            let doc_contents = code_doc_block_arr
                .iter()
                .enumerate()
                // ...selecting only the doc block contents...
                .filter_map(|(index, cdb)| {
                    if let CodeDocBlock::DocBlock(db) = cdb {
                        Some((index, db.contents.as_str()))
                    } else {
                        None
                    }
                })
                // Precede each doc block with the separator string; the
                // separator contains the index of this doc block in the vec of
                // code/doc blocks. The separator appears before *every* doc
                // block (including the first), so the DOM walk always knows the
                // current doc block index and empty doc blocks stay aligned
                // with their separators.
                .fold(String::new(), |mut acc: String, x: (usize, &str)| {
                    acc.push_str(&DOC_BLOCK_SEPARATOR_STRING.replace("{}", &x.0.to_string()));
                    acc.push_str(x.1);
                    acc
                });

            // Convert the Markdown to HTML.
            let html = markdown_to_html(&doc_contents);

            // Break it back into doc blocks:
            //
            // 1. Mend broken fences.
            let html = DOC_BLOCK_SEPARATOR_BROKEN_FENCE
                .replace_all(&html, DOC_BLOCK_SEPARATOR_MENDED_FENCE);
            // 2. Remove good fences.
            let html = html.replace(DOC_BLOCK_SEPARATOR_REMOVE_FENCE, "");
            // 3. Hydrate the cleaned HTML: commit this file's facts to the
            //    cache, then patch cross-references and fragment backlinks.
            let (dom, walk_context) = hydrate_dom(&html, file_path, &cache, false)
                .map_err(|e| SourceToCodeChatForWebError::ParseFailed(e.to_string()))?;
            // 4. Serialize and split on the separator, giving each doc block's
            //    hydrated HTML -- the form in which fragment contents are
            //    stored. The piece before the first separator isn't a doc
            //    block; discard it.
            let intermediate_html = dom_to_html(&dom)
                .map_err(|e| SourceToCodeChatForWebError::ParseFailed(e.to_string()))?;
            let mut chunk_iter = DOC_BLOCK_SEPARATOR_SPLIT_REGEX.split(&intermediate_html);
            chunk_iter.next();
            // Pair the index of each doc block in `code_doc_block_arr` with its
            // hydrated HTML.
            let mut doc_block_html: HashMap<usize, &str> = HashMap::new();
            for (index, code_doc_block) in code_doc_block_arr.iter().enumerate() {
                if matches!(code_doc_block, CodeDocBlock::DocBlock(_))
                    && let Some(chunk) = chunk_iter.next()
                {
                    doc_block_html.insert(index, chunk);
                }
            }
            // 5. Store each fragment's content in the cache, which marks the
            //    files containing gather elements listing changed fragments as
            //    outdated.
            store_fragment_contents(
                &walk_context,
                Some((&code_doc_block_arr, &doc_block_html)),
                &cache,
            );
            // 6. Hydrate gather lists, now that every fragment's content --
            //    including those defined in this file -- is in the cache.
            hydrate_gathers(&walk_context, &cache)
                .map_err(|e| SourceToCodeChatForWebError::ParseFailed(e.to_string()))?;
            // 7. Serialize the fully-hydrated DOM and split it into the final
            //    doc block contents, again discarding the piece before the
            //    first separator.
            let html = dom_to_html(&dom)
                .map_err(|e| SourceToCodeChatForWebError::ParseFailed(e.to_string()))?;
            let mut doc_block_contents_iter: regex::Split<'_, '_> =
                DOC_BLOCK_SEPARATOR_SPLIT_REGEX.split(&html);
            doc_block_contents_iter.next();

            // Translate each `CodeDocBlock` to its `CodeMirror` equivalent.
            let mut len = len_utf16(&code_mirror.doc);
            for code_or_doc_block in code_doc_block_arr {
                let source = &mut code_mirror.doc;
                match code_or_doc_block {
                    CodeDocBlock::CodeBlock(code_string) => {
                        source.push_str(&code_string);
                        len += len_utf16(&code_string);
                    }
                    CodeDocBlock::DocBlock(doc_block) => {
                        // Create the doc block.
                        code_mirror.doc_blocks.push(CodeMirrorDocBlock {
                            from: len,
                            // To. Note that the last doc block could be zero
                            // length, so handle this case.
                            to: len + max(doc_block.lines, 1),
                            indent: doc_block.indent.clone(),
                            delimiter: doc_block.delimiter.clone(),
                            // Used the markdown-translated replacement for this
                            // doc block, rather than the original string.
                            contents: minify(doc_block_contents_iter.next().unwrap())?,
                        });
                        // Append newlines to the document; the doc block will
                        // replace these in the editor. This keeps the line
                        // numbering of non-doc blocks correct.
                        source.push_str(&"\n".repeat(doc_block.lines));
                        len += doc_block.lines;
                    }
                }
            }
            CodeMirrorDiffable::Plain(code_mirror)
        },
    };

    Ok(codechat_for_web)
}

// Options for a spec-compliant minifier.
static MINIFY_OPTIONS: LazyLock<minify_html::Cfg> = LazyLock::new(|| {
    let mut cfg = minify_html::Cfg::new();
    cfg.allow_noncompliant_unquoted_attribute_values = false;
    cfg.allow_optimal_entities = false;
    cfg.allow_removing_spaces_between_attributes = false;
    cfg.keep_comments = true;
    cfg.keep_html_and_head_opening_tags = true;
    cfg.minify_doctype = false;
    let mut override_whitespace: HashSet<Vec<u8>> = HashSet::new();
    override_whitespace.insert(b"wc-mermaid".to_vec());
    override_whitespace.insert(b"graphviz-graph".to_vec());
    cfg.override_whitespace = override_whitespace;
    cfg
});

// A static config for Ammonia.
static AMMONIA_OPTIONS: LazyLock<Builder> = LazyLock::new(|| {
    let mut b = Builder::default();
    // Add custom tags produced during hydration, plus `input` (task list
    // checkboxes produced by pulldown-cmark) and `iframe` (embedded media
    // inserted via TinyMCE), neither of which Ammonia allows by default.
    b.add_tags(&[
        "wc-mermaid",
        "graphviz-graph",
        "xref",
        "fragment",
        "input",
        "iframe",
    ])
    // Allow any element to be assigned an ID and to be a gather element.
    .add_generic_attributes(&["id", "data-gather"])
    // This allows math produced by pulldown-cmark and updated by the hydration
    // code, plus hydration error messages.
    .add_allowed_classes(
        "span",
        &[
            "math",
            "math-inline",
            "math-display",
            "mceNonEditable",
            "cc-error",
            // The line number preceding each line of a code block in a rendered
            // fragment; see `render_fragment_content`.
            "cc-line-number",
        ],
    )
    // Classes produced by gather-element hydration. The `cc-gather` class may
    // appear on any element with an `id` and `data-gather`; Ammonia only
    // supports per-tag class allowlists, so list the elements which plausibly
    // serve as gather elements.
    .add_allowed_classes("h1", &["cc-gather"])
    .add_allowed_classes("h2", &["cc-gather"])
    .add_allowed_classes("h3", &["cc-gather"])
    .add_allowed_classes("h4", &["cc-gather"])
    .add_allowed_classes("h5", &["cc-gather"])
    .add_allowed_classes("h6", &["cc-gather"])
    .add_allowed_classes("p", &["cc-gather", "cc-gather-item-link"])
    // The `cc-fragment-*` classes lay out one doc block of a rendered fragment:
    // its source indent, then its contents. See `render_fragment_content`.
    .add_allowed_classes(
        "div",
        &[
            "cc-gather",
            "cc-gather-items",
            "cc-fragment-doc",
            "cc-fragment-doc-contents",
        ],
    )
    // The doc block indents and the code blocks of a rendered fragment; see
    // `render_fragment_content`. Listing these here rather than allowing a
    // `class` attribute on any `pre` (as `code` below does) keeps a doc block's
    // hand-written `<pre>` from claiming the layout these name.
    .add_allowed_classes("pre", &["cc-fragment-indent", "cc-fragment-code"])
    // The gather-items list is generated content, marked non-editable.
    .add_tag_attributes("div", &["contenteditable"])
    // `code` tags can have `class=language-*`. Since Ammonia doesn't support a
    // regex like this, just allow anything.
    .add_tag_attributes("code", &["class"])
    // Task list checkboxes are rendered as `<input type="checkbox" checked>`.
    .add_tag_attributes("input", &["type", "checked", "disabled"])
    // Allow the attributes TinyMCE/the IDE place on embedded `<iframe>`s.
    .add_tag_attributes(
        "iframe",
        &["width", "height", "src", "allowfullscreen", "frameborder"],
    )
    .add_tag_attributes("xref", &["contenteditable", "ref"])
    .add_tag_attributes("fragment", &["contenteditable", "id", "following"])
    // Keep HTML comments, which Ammonia strips by default.
    .strip_comments(false)
    // For now, don't change this. We can't tell if the user included this
    // manually and it should not be stripped without some extra work (perhaps
    // adding custom attributes?).
    .link_rel(None);
    b
});

// Clean HTML for storage as a `Target`'s inner HTML, per the spec in
// `cache.rs`: only the
// [permitted content for an `<a>` element](https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/a#technical_summary)
// is allowed (phrasing content, excluding interactive content), since this HTML
// becomes the text of a hydrated cross-reference link. Disallowed tags are
// stripped but their text is kept; `id` attributes are stripped because they
// aren't in the allowlist, preventing duplicate ids when the HTML is rendered
// in a referencing file.
static CLEAN_TARGET_HTML: LazyLock<Builder> = LazyLock::new(|| {
    let mut b = Builder::default();
    b.tags(HashSet::from([
        "abbr", "b", "bdi", "bdo", "br", "cite", "code", "data", "dfn", "em", "i", "img", "kbd",
        "mark", "q", "rp", "rt", "ruby", "s", "samp", "small", "span", "strong", "sub", "sup",
        "time", "u", "var", "wbr",
    ]))
    // Keep hydrated math legible in link text.
    .add_allowed_classes(
        "span",
        &["math", "math-inline", "math-display", "mceNonEditable"],
    )
    .link_rel(None);
    b
});

// Clean HTML for storage as a `Fragment`'s content, per the spec in `cache.rs`:
// like `AMMONIA_OPTIONS`, but `id` and `data-gather` attributes are stripped
// (they aren't in any allowlist) to prevent duplicate ids when the content is
// rendered in a gather list, and `<fragment>` elements are removed *along with
// their contents* -- backlink hydration is excluded from fragment content (see
// layer 4 in the `cache.rs` design), which is what keeps a gather element and
// the fragments it lists from outdating each other forever.
static CLEAN_FRAGMENT_HTML: LazyLock<Builder> = LazyLock::new(|| {
    let mut b = Builder::default();
    b.add_tags(&["wc-mermaid", "graphviz-graph", "xref", "input", "iframe"])
        .clean_content_tags(HashSet::from(["fragment"]))
        .add_allowed_classes(
            "span",
            &[
                "math",
                "math-inline",
                "math-display",
                "mceNonEditable",
                "cc-error",
            ],
        )
        .add_tag_attributes("code", &["class"])
        .add_tag_attributes("input", &["type", "checked", "disabled"])
        .add_tag_attributes(
            "iframe",
            &["width", "height", "src", "allowfullscreen", "frameborder"],
        )
        .add_tag_attributes("xref", &["contenteditable", "ref"])
        .strip_comments(false)
        .link_rel(None);
    b
});

/// A spec-compliant minifier.
pub fn minify(html: &str) -> Result<String, FromUtf8Error> {
    let clean_html = AMMONIA_OPTIONS.clean(html).to_string();
    String::from_utf8(minify_html::minify(clean_html.as_bytes(), &MINIFY_OPTIONS))
}

// Compute the length of the provided string in UTF16 characters.
fn len_utf16(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
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
    // The version to assign to this file.
    version: f64,
    // True if this file is a TOC.
    is_toc: bool,
    // The map of Caches.
    cache: &CacheMap,
) -> Result<
    (
        // The resulting translation.
        TranslationResultsString,
        // Path to the TOC, if found; otherwise, None.
        Option<PathBuf>,
    ),
    SourceToCodeChatForWebError,
> {
    // To determine if this source code is part of a project, look for a project
    // file by searching the current directory, then all its parents, for a file
    // named `toc.md`.
    let path_to_toc = find_path_to_toc(file_path);
    let cache: Option<Arc<Mutex<Cache>>> = path_to_toc.as_ref().map(|path_to_toc| {
        // The project root is the directory containing `toc.md`. `path_to_toc`
        // is relative to the file's directory; canonicalize the combination so
        // that every file in a project keys the same cache, and different
        // projects key different caches.
        let toc_path = file_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path_to_toc);
        let root = toc_path
            .parent()
            .map_or_else(PathBuf::new, Path::to_path_buf);
        let root = dunce::canonicalize(&root).unwrap_or(root);
        cache
            .lock()
            .unwrap()
            .entry(root.clone())
            .or_insert_with(|| Arc::new(Mutex::new(Cache::new(root.clone()))))
            .clone()
    });

    Ok((
        match source_to_codechat_for_web(file_contents, file_path, version, is_toc, cache) {
            Err(err) => {
                // The no lexer error means we should treat this file type as
                // unknown.
                if err == SourceToCodeChatForWebError::NoLexer {
                    TranslationResultsString::Unknown
                } else {
                    // Otherwise, this is an unhandleable error.
                    return Err(err);
                }
            }
            Ok(codechat_for_web) => {
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
        },
        path_to_toc,
    ))
}

/// Convert markdown to HTML. (This assumes the Markdown defined in the
/// CommonMark spec.)
fn markdown_to_html(markdown: &str) -> String {
    let mut options = Options::all();
    // htmd (which converts HTML back to Markdown) doesn't support smart
    // punctuation.
    options.remove(Options::ENABLE_SMART_PUNCTUATION);
    let parser = Parser::new_ext(markdown, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

// Mark the current cursor position with a
// [private use area code point](https://en.wikipedia.org/wiki/Private_Use_Areas),
// something that's unlike to appear in source code.
pub const UNICODE_CURSOR_MARKER: char = '\u{E83B}';

/// Use html5ever to parse a string containing HTML to a DOM tree.
fn html_to_dom(
    html: &str,
    // See the same parameter from `doc_block_html_to_markdown`.
    dom_location: Option<&(Vec<usize>, usize)>,
) -> io::Result<Rc<Node>> {
    let dom = parse_document(
        RcDom::default(),
        ParseOpts {
            tree_builder: TreeBuilderOpts {
                scripting_enabled: true,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .from_utf8()
    .read_from(&mut html.as_bytes())?;

    if let Some((dom_path, dom_offset)) = dom_location {
        // Each element in `dom_offsets` is the index of a node in the `dom`.
        // Take the first index, then descend into the indicated node. Repeat
        // this process until the last node, which should be a text node. The
        // last index is the UTF-16 offset with the text contents to insert a
        // `UNICODE_CURSOR_MARKER` character. Any failures (index exceeds number
        // of nodes, etc.) use an approximation where possible.
        let mut current_node = get_dom_body(&dom.document);
        'outer: for &offset in dom_path {
            let next_node = {
                let children = current_node.children.borrow();
                if offset < children.len() {
                    children[offset].clone()
                } else if let Some(last) = children.last() {
                    last.clone()
                } else {
                    break 'outer;
                }
            };
            current_node = next_node;
        }
        // Insert the cursor marker at the given character offset within the
        // text node.
        if let NodeData::Text { contents } = &current_node.data {
            let mut text = contents.borrow().to_string();
            // Convert the character offset into a byte offset.
            let byte_offset = byte_index_of(&text, *dom_offset);
            text.insert(byte_offset, UNICODE_CURSOR_MARKER);
            *contents.borrow_mut() = text.into();
        }
    }

    Ok(dom.document)
}

// A framework to transform HTML by parsing it to a DOM tree, walking the tree,
// then serializing the tree back to an HTML string.
pub fn transform_html<T: FnOnce(&Rc<Node>)>(html: &str, transform: T) -> io::Result<String> {
    let tree = html_to_dom(html, None)?;
    transform(&tree);
    dom_to_html(&tree)
}

// Transform a DOM tree back to an HTML string.
pub fn dom_to_html(dom: &Rc<Node>) -> io::Result<String> {
    // Serialize the transformed DOM back to a string.
    let so = SerializeOpts {
        // Don't include the body node in the output.
        traversal_scope: TraversalScope::ChildrenOnly(None),
        ..Default::default()
    };
    let mut bytes = vec![];
    serialize(&mut bytes, &SerializableHandle::from(get_dom_body(dom)), so)?;
    let html_out = String::from_utf8(bytes).map_err(io::Error::other)?;

    Ok(html_out)
}

/// Serialize a node's children back to an HTML string -- the node's inner HTML.
/// This defines the contents of a target or gather element stored in the cache.
fn node_inner_html(node: &Rc<Node>) -> io::Result<String> {
    let so = SerializeOpts {
        // Serialize only the node's children, not the node itself.
        traversal_scope: TraversalScope::ChildrenOnly(None),
        ..Default::default()
    };
    let mut bytes = vec![];
    serialize(&mut bytes, &SerializableHandle::from(node.clone()), so)?;
    String::from_utf8(bytes).map_err(io::Error::other)
}

/// Get the body element from a top-level DOM.
fn get_dom_body(document: &Rc<Node>) -> Rc<Node> {
    // HTML is:
    //
    // ```html
    // <html>   <-- element 0
    //  <head>...</head>
    //  <body>...</body>  <-- element 1
    // </html>
    // ```
    document.children.borrow()[0].children.borrow()[1].clone()
}

// HTML produced from Markdown needs additional processing, termed hydration:
//
// * Transform math, Mermaid, GraphViz, etc. nodes.
// * Hydrate cross-references, fragments, and gather elements from the project
//   cache.
// * (Eventually) record document structure information.
// * (Eventually) fill in autocomplete fields.
//
// Hydration is layered (see the `Design` section in `cache.rs`); the layers
// which depend on fragment contents (gather lists) can only run after doc
// blocks are finalized, so hydration is split into phases:
//
// 1. `hydrate_dom`: collect facts from the DOM, commit them to the cache, then
//    patch `<xref>` contents and `<fragment>` backlinks.
// 2. `store_fragment_contents`: once each doc block's hydrated HTML is known,
//    store each fragment's content in the cache.
// 3. `hydrate_gathers`: insert each gather element's list of fragment contents,
//    now all present in the cache.
//
// This function runs all three phases for a Markdown document, where phase 2
// needs no doc block data (fragments aren't allowed in Markdown documents, so
// every fragment's content is an error message).
fn hydrate_html(html: &str, file: &Path, cache: &Arc<Mutex<Cache>>) -> io::Result<String> {
    let (dom, walk_context) = hydrate_dom(html, file, cache, true)?;
    store_fragment_contents(&walk_context, None, cache);
    hydrate_gathers(&walk_context, cache)?;
    dom_to_html(&dom)
}

/// Phase 1 of hydration: parse the HTML, walk the DOM (transforming
/// math/Mermaid/GraphViz and collecting cacheable facts), commit the facts to
/// the cache, then patch the content the commit makes available: each
/// `<xref>`'s link and each `<fragment>`'s backlinks (or error messages).
/// Gather lists are *not* inserted here; they depend on fragment contents,
/// which aren't final until doc blocks are (see `hydrate_gathers`).
fn hydrate_dom(
    // The HTML to hydrate.
    html: &str,
    // The file this HTML was produced from.
    file: &Path,
    // The cache for the project containing this file.
    cache: &Arc<Mutex<Cache>>,
    // True when this is a Markdown document, in which fragments aren't allowed.
    is_markdown: bool,
    // The parsed, patched DOM plus the walk results needed by later phases.
) -> io::Result<(Rc<Node>, WalkContext)> {
    let dom = html_to_dom(html, None)?;
    // Read the file's metadata and canonicalize its path before taking the
    // cache lock, so that no I/O happens while the lock is held. The metadata
    // captures the state of the file whose content is being processed; the
    // canonicalization satisfies the cache's requirement that all paths are
    // canonicalized and absolute. (Canonicalization fails for files which don't
    // exist on disk -- such as tests -- in which case the path is used as-is.)
    let metadata = file.metadata().ok();
    let path = dunce::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
    // ### Assign requested ids
    //
    // Replace each `id="*"` with a freshly generated id before facts are
    // collected, so that everything downstream sees a real id. These ids reach
    // the Client but are deliberately kept out of the cache until this file is
    // written and re-read; see `Auto-assignment of ids` in `cache.rs`.
    let mut auto_ids: HashSet<String> = HashSet::new();
    {
        let cache_guard = cache.lock().unwrap();
        assign_auto_ids(&dom, &cache_guard, &mut auto_ids);
    }
    // ### Collect facts
    //
    // Walk the DOM, collecting all cacheable facts -- targets,
    // cross-references, fragments, and gather elements -- without touching the
    // cache. This keeps the non-`Send` DOM types out of the cache and off its
    // critical section; see the design discussion in `cache.rs`.
    let mut walk_context = WalkContext {
        facts: FileFacts::default(),
        path,
        is_markdown,
        auto_ids,
        doc_block_index: 0,
        xrefs: Vec::new(),
        fragments: Vec::new(),
        gathers: Vec::new(),
        gathers_error: Vec::new(),
        gather_block_indices: Vec::new(),
    };
    walk_context = hydrating_walk_node(&dom, walk_context)?;
    // ### Commit facts
    //
    // Apply the collected facts to the cache in a single transaction. This
    // diffs them against the file's previous state: added, deleted, and
    // modified targets and fragments mark the files which depend on them as
    // outdated, this file is linked to every id it references, and duplicate
    // ids are recorded.
    let mut cache_guard = cache.lock().unwrap();
    // TODO: schedule reprocessing for each file in `commit.outdated`.
    let _commit = cache_guard.commit_file(
        &walk_context.path,
        metadata,
        mem::take(&mut walk_context.facts),
    );
    // ### Patch cross-references
    //
    // Layer 2: each `<xref>`'s content depends only on its target's inner HTML,
    // which the commit just placed in the cache.
    for (id, node) in &walk_context.xrefs {
        set_attr(node, "contenteditable", "false");
        let children = xref_content(&cache_guard, id, &walk_context.path)?;
        set_element_children(node, children);
    }
    // ### Patch fragment backlinks
    //
    // Layer 4: each `<fragment>` renders a backlink to every gather element
    // which lists it, all of which the commit just placed in the cache. A
    // fragment in an error state renders the error instead.
    for fragment in &walk_context.fragments {
        set_attr(&fragment.node, "contenteditable", "false");
        // A duplicate id is only detectable after the commit, so it can't be
        // recorded in `FragmentHydration::error` during the walk; unlike that
        // error, it doesn't replace the fragment's cached content (resolving
        // the duplicate elsewhere wouldn't reprocess this file, which would
        // leave stale error text in the cache).
        let duplicate = fragment.error.is_none()
            && matches!(
                cache_guard.resolve_id(&fragment.id),
                IdResolution::Multiple(_)
            );
        let children = if let Some(message) = &fragment.error {
            vec![error_span(message)]
        } else if duplicate {
            vec![error_span(&format!(
                "id \"{}\" is defined more than once",
                fragment.id
            ))]
        } else {
            fragment_backlinks(&cache_guard, &fragment.id, &walk_context.path)?
        };
        set_element_children(&fragment.node, children);
    }
    drop(cache_guard);
    Ok((dom, walk_context))
}

/// Replace every `id="*"` -- a request that the cache assign an id -- with a
/// freshly generated one, recording the ids assigned. Per the spec in
/// `cache.rs`, these ids are placed in the HTML but not recorded in the cache;
/// the fact collection walk uses the recorded set to skip them.
fn assign_auto_ids(
    // The node whose descendants are scanned.
    node: &Rc<Node>,
    // The cache, already locked by the caller; it supplies ids which don't
    // collide with any it knows of.
    cache: &Cache,
    // The ids assigned so far, extended by this call.
    assigned: &mut HashSet<String>,
) {
    for child in node.children.borrow().iter() {
        // An `<xref>` doesn't allow an `id`, so it gets no auto-assigned one
        // either; a `<fragment>`, a target, and a gather element all do.
        if get_node_tag_name(child) != Some("xref")
            && get_attr_value(child, "id").as_deref() == Some(AUTO_ID)
        {
            let id = cache.new_id(assigned);
            set_attr(child, "id", &id);
            assigned.insert(id);
        }
        // Skip content the cache generates, for the reason given in
        // `hydrating_walk_node`.
        let skip_descend = matches!(get_node_tag_name(child), Some("xref" | "fragment"))
            || is_gather_items_div(child);
        if !skip_descend {
            assign_auto_ids(child, cache, assigned);
        }
    }
}

/// The content of one hydrated `<xref>`: a link to its target, or an error
/// message.
fn xref_content(
    // The cache, already locked by the caller.
    cache: &Cache,
    // The id the cross-reference names.
    id: &str,
    // The canonicalized path of the file containing the cross-reference.
    path: &Path,
    // The nodes to place inside the `<xref>` element.
) -> io::Result<Vec<Rc<Node>>> {
    if !is_css_identifier(id) {
        return Ok(vec![error_span(&format!(
            "\"{id}\" is not a valid CSS identifier"
        ))]);
    }
    Ok(match cache.resolve_id(id) {
        IdResolution::Target {
            path: target_path,
            target,
        } => {
            let href = format!("{}#{id}", relative_url(path, target_path));
            vec![new_element(
                "a",
                vec![("href", href)],
                parse_html_fragment(&target.inner_html)?,
            )]
        }
        IdResolution::Fragment { .. } => vec![error_span(&format!(
            "id \"{id}\" names a fragment; a cross-reference must name a target or gather element"
        ))],
        IdResolution::Missing => vec![error_span(&format!("id \"{id}\" not found"))],
        IdResolution::Multiple(_) => vec![error_span(&format!(
            "id \"{id}\" is defined more than once"
        ))],
    })
}

/// The content of one hydrated `<fragment>`: a backlink to each gather element
/// which lists it, e.g. `See <a href="...#bar">Bazzy things</a>, <a
/// href="...#zap">Zappy things</a>`; empty if nothing gathers it.
fn fragment_backlinks(
    // The cache, already locked by the caller.
    cache: &Cache,
    // The fragment's id.
    id: &str,
    // The canonicalized path of the file containing the fragment.
    path: &Path,
    // The nodes to place inside the `<fragment>` element.
) -> io::Result<Vec<Rc<Node>>> {
    let backlinks = cache.gathers_referencing(id);
    if backlinks.is_empty() {
        return Ok(vec![]);
    }
    let mut children = vec![new_text("See ")];
    for (index, backlink) in backlinks.iter().enumerate() {
        if index > 0 {
            children.push(new_text(", "));
        }
        let href = format!("{}#{}", relative_url(path, backlink.path), backlink.id);
        children.push(new_element(
            "a",
            vec![("href", href)],
            parse_html_fragment(&backlink.target.inner_html)?,
        ));
    }
    Ok(children)
}

/// Phase 2 of hydration: store each fragment's content in the cache, which
/// marks the files containing gather elements listing changed fragments as
/// outdated.
fn store_fragment_contents(
    // The walk results from `hydrate_dom`.
    walk_context: &WalkContext,
    // The file's code/doc blocks, paired with a map from the index of each doc
    // block in that slice to its hydrated HTML. `None` for Markdown documents,
    // where every fragment's content is an error message.
    blocks: Option<(&[CodeDocBlock], &HashMap<usize, &str>)>,
    // The cache for the project containing this file.
    cache: &Arc<Mutex<Cache>>,
) {
    let mut cache_guard = cache.lock().unwrap();
    for fragment in &walk_context.fragments {
        if !fragment.cached {
            continue;
        }
        let content = if let Some(message) = &fragment.error {
            error_html(message)
        } else if let Some((code_doc_blocks, chunks)) = blocks {
            render_fragment_content(fragment, walk_context, code_doc_blocks, chunks)
        } else {
            // Unreachable: every fragment in a Markdown document has its
            // `error` set during the walk.
            error_html("fragments are not allowed in Markdown documents")
        };
        // TODO: schedule reprocessing for each outdated file returned here.
        let _outdated =
            cache_guard.update_fragment_content(&walk_context.path, &fragment.id, content);
    }
}

/// Render one fragment's content: the hydrated HTML of the code/doc blocks it
/// encloses, cleaned per the spec in `cache.rs`. Per that spec, the rendering
/// reproduces the layout of the source it came from: each doc block carries the
/// indent it had there, each line of a code block and the first line of each doc
/// block are preceded by their line numbers, and equal indents in the source
/// line up in the rendering.
///
/// ### Why a doc block's indent and a code block are `<pre>`s
///
/// The whitespace in those two *is* the layout, so everything which rewrites
/// this HTML on its way to the screen must be told to leave it alone -- and both
/// TinyMCE and the minifier key whitespace sensitivity on the tag name, with no
/// class- or selector-based equivalent. (`CodeChat-doc-indent` needs no such
/// treatment: the Client builds it outside the TinyMCE editable region, so
/// nothing rewrites it.) Of the tag names both tools treat as
/// whitespace-sensitive by default, `pre` is the one which fits, and each tool
/// extends that treatment to everything nested inside it, covering the line
/// numbers as well.
///
/// The cost of a standard tag name is that a theme's `pre` styling -- a font
/// size, a padding -- would otherwise reach these and shift one relative to the
/// other. The alignment rules in `CodeChatEditor.css` therefore sit outside the
/// cascade layer holding the themes, and begin by discarding what a theme
/// declared; see the comments there. That defense belongs to the cascade rather
/// than to the selectors, so it isn't a specificity race a theme might win.
///
/// ### Renaming either class
///
/// Each of these spells the names out independently -- they cross a Rust/CSS/
/// TypeScript boundary, so no single definition can be shared -- and all must
/// change together:
///
/// 1. This function, which emits them.
/// 2. `AMMONIA_OPTIONS` in this file, which allows only these classes on a
///    `<pre>`.
/// 3. The alignment rules in
///    [CodeChatEditor.css](../../client/src/css/CodeChatEditor.css).
/// 4. The expected HTML in `processing/tests.rs` and the layout tests in
///    [CodeChatEditor-test.mts](../../client/src/CodeChatEditor-test.mts).
fn render_fragment_content(
    // The fragment to render.
    fragment: &FragmentHydration,
    // The walk results, which locate the file's gather elements.
    walk_context: &WalkContext,
    // The file's code/doc blocks.
    code_doc_blocks: &[CodeDocBlock],
    // A map from the index of each doc block in `code_doc_blocks` to its
    // hydrated HTML.
    chunks: &HashMap<usize, &str>,
    // The fragment's content, rendered as HTML.
) -> String {
    if code_doc_blocks.is_empty() {
        return String::new();
    }
    // Clamp `following` to the number of code/doc blocks in the document.
    let end = fragment.end.min(code_doc_blocks.len() - 1);
    // A fragment may not contain a gather element: the gather element's
    // hydrated list isn't part of the source, so including it would nest
    // generated content inside generated content.
    if walk_context
        .gather_block_indices
        .iter()
        .any(|index| (fragment.start..=end).contains(index))
    {
        return error_html("a fragment may not contain a gather element");
    }
    // The number of the source line on which the fragment's first block begins.
    // Line numbers are one-based, matching the editor's gutter; every block
    // before the fragment occupies its full height in the source (a doc block
    // becomes that many blank lines -- see the `"\n".repeat(doc_block.lines)`
    // in `source_to_codechat_for_web`), so summing those heights locates the
    // fragment.
    let mut line = 1 + code_doc_blocks
        .iter()
        .take(fragment.start)
        .map(code_doc_block_lines)
        .sum::<usize>();
    let mut content = String::new();
    for (index, code_doc_block) in code_doc_blocks
        .iter()
        .enumerate()
        .take(end + 1)
        .skip(fragment.start)
    {
        match code_doc_block {
            CodeDocBlock::DocBlock(doc_block) => {
                if let Some(chunk) = chunks.get(&index) {
                    content.push_str("<div class=\"cc-fragment-doc\">");
                    // The indent is always given an element, even when it's
                    // empty: that element also supplies the doc side of the
                    // line-number gutter, which every doc block needs in order
                    // to line up with the code around it. Only the doc block's
                    // first line is numbered -- the block's remaining source
                    // lines have no fixed correspondence to the lines its
                    // rendered contents occupy, since the comment delimiters are
                    // gone and the text reflows.
                    content.push_str(
                        "<pre class=\"cc-fragment-indent\"><span class=\"cc-line-number\">",
                    );
                    content.push_str(&line.to_string());
                    content.push_str("</span>");
                    content.push_str(&htmlize::escape_text(&doc_block.indent));
                    content.push_str("</pre><div class=\"cc-fragment-doc-contents\">");
                    content.push_str(&CLEAN_FRAGMENT_HTML.clean(chunk).to_string());
                    content.push_str("</div></div>");
                }
                line += doc_block.lines;
            }
            CodeDocBlock::CodeBlock(code) => {
                content.push_str("<pre class=\"cc-fragment-code\">");
                // `split_inclusive` keeps the newline ending each line, and
                // yields nothing for an empty code block, so the code is
                // reproduced exactly except for the numbers inserted here. The
                // line number comes first on every line, which also keeps the
                // code from beginning with the newline an HTML parser drops
                // when it directly follows a `<pre>`.
                for source_line in code.split_inclusive('\n') {
                    content.push_str("<span class=\"cc-line-number\">");
                    content.push_str(&line.to_string());
                    content.push_str("</span>");
                    content.push_str(&htmlize::escape_text(source_line));
                    line += 1;
                }
                content.push_str("</pre>");
            }
        }
    }
    content
}

/// The number of source lines a code/doc block occupies, used to compute the
/// line numbers a fragment's code blocks display.
fn code_doc_block_lines(
    // The block to measure.
    code_doc_block: &CodeDocBlock,
    // Its height in source lines.
) -> usize {
    match code_doc_block {
        CodeDocBlock::DocBlock(doc_block) => doc_block.lines,
        // The last line of a file need not end with a newline, so count line
        // endings inclusively rather than counting newlines.
        CodeDocBlock::CodeBlock(code) => code.split_inclusive('\n').count(),
    }
}

/// Phase 3 of hydration: mark each gather element with the `cc-gather` class
/// and insert its list of gathered fragment contents as its next sibling. This
/// must run after `store_fragment_contents`, so that same-file fragments'
/// contents are present in the cache (cross-file contents already are).
fn hydrate_gathers(
    // The walk results from `hydrate_dom`.
    walk_context: &WalkContext,
    // The cache for the project containing this file.
    cache: &Arc<Mutex<Cache>>,
) -> io::Result<()> {
    let cache_guard = cache.lock().unwrap();
    for node in &walk_context.gathers {
        add_class(node, "cc-gather");
        // Build the list of gathered items, in `data-gather` order.
        let gather_ids: Vec<String> = get_attr_value(node, "data-gather")
            .map(|gather_ids| gather_ids.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default();
        let mut items: Vec<Rc<Node>> = Vec::new();
        for id in &gather_ids {
            if !is_css_identifier(id) {
                items.push(error_span(&format!(
                    "\"{id}\" is not a valid CSS identifier"
                )));
                continue;
            }
            match cache_guard.resolve_id(id) {
                IdResolution::Fragment {
                    path: fragment_path,
                    fragment,
                } => {
                    let relative = relative_url(&walk_context.path, fragment_path);
                    let href = format!("{relative}#{id}");
                    // Label an intra-file link with the file's own name, since
                    // the relative path to the same file is empty.
                    let link_text = if relative.is_empty() {
                        fragment_path
                            .file_name()
                            .map_or_else(String::new, |name| name.to_string_lossy().to_string())
                    } else {
                        relative
                    };
                    items.push(new_element(
                        "p",
                        vec![("class", "cc-gather-item-link".to_string())],
                        vec![
                            new_text("From "),
                            new_element("a", vec![("href", href)], vec![new_text(&link_text)]),
                            new_text(":"),
                        ],
                    ));
                    items.extend(parse_html_fragment(&fragment.content)?);
                }
                IdResolution::Target { .. } => items.push(error_span(&format!(
                    "id \"{id}\" names a target, not a fragment"
                ))),
                IdResolution::Missing => items.push(error_span(&format!("id \"{id}\" not found"))),
                IdResolution::Multiple(_) => items.push(error_span(&format!(
                    "id \"{id}\" is defined more than once"
                ))),
            }
        }
        insert_after(node, &gather_items_div(items));
    }
    // Elements with `data-gather` but no usable `id` display an error in place
    // of a gather list.
    for (node, message) in &walk_context.gathers_error {
        insert_after(node, &gather_items_div(vec![error_span(message)]));
    }
    Ok(())
}

/// Build the `<div class="cc-gather-items" contenteditable="false">` which
/// holds a gather element's hydrated list.
fn gather_items_div(children: Vec<Rc<Node>>) -> Rc<Node> {
    new_element(
        "div",
        vec![
            ("class", "cc-gather-items".to_string()),
            ("contenteditable", "false".to_string()),
        ],
        children,
    )
}

// ### Hydration helpers
/// Report whether the given id is a valid CSS identifier; all cached ids must
/// be.
fn is_css_identifier(id: &str) -> bool {
    CSS_IDENTIFIER.is_match(id)
}

/// Render an error message produced during hydration as an HTML string.
fn error_html(message: &str) -> String {
    format!(
        "<span class=\"cc-error\">{}</span>",
        htmlize::escape_text(message)
    )
}

/// Build the DOM node form of a hydration error message.
fn error_span(message: &str) -> Rc<Node> {
    new_element(
        "span",
        vec![("class", "cc-error".to_string())],
        vec![new_text(message)],
    )
}

/// Compute a relative URL (with forward slashes) leading from the directory of
/// `from_file` to `to_file`. Returns an empty string when both name the same
/// file, producing intra-page `#id` links. Both paths must be canonicalized
/// (per the cache's requirements) for the result to be meaningful; references
/// across Windows drives aren't supported.
fn relative_url(from_file: &Path, to_file: &Path) -> String {
    if from_file == to_file {
        return String::new();
    }
    let from_dir: Vec<_> = from_file
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .components()
        .collect();
    let to_components: Vec<_> = to_file.components().collect();
    // Find the common prefix of the two paths, stopping short of `to_file`'s
    // file name.
    let limit = from_dir.len().min(to_components.len().saturating_sub(1));
    let mut common = 0;
    while common < limit && from_dir[common] == to_components[common] {
        common += 1;
    }
    let mut relative = PathBuf::new();
    for _ in common..from_dir.len() {
        relative.push("..");
    }
    for component in &to_components[common..] {
        relative.push(component);
    }
    relative.to_slash_lossy().into_owned()
}

/// Build an element node with the given attributes and children.
fn new_element(
    // The element's tag name.
    tag: &str,
    // The element's attributes, as (name, value) pairs.
    attrs: Vec<(&str, String)>,
    // The element's children.
    children: Vec<Rc<Node>>,
) -> Rc<Node> {
    let node = Node::new(NodeData::Element {
        name: QualName::new(None, Namespace::from(""), LocalName::from(tag)),
        attrs: RefCell::new(
            attrs
                .into_iter()
                .map(|(name, value)| Attribute {
                    name: QualName::new(None, Namespace::from(""), LocalName::from(name)),
                    value: value.into(),
                })
                .collect(),
        ),
        template_contents: RefCell::new(None),
        mathml_annotation_xml_integration_point: false,
    });
    set_element_children(&node, children);
    node
}

/// Build a text node.
fn new_text(text: &str) -> Rc<Node> {
    Node::new(NodeData::Text {
        contents: RefCell::new(text.into()),
    })
}

/// Replace a node's children with the given nodes, updating their parent links.
fn set_element_children(node: &Rc<Node>, children: Vec<Rc<Node>>) {
    for child in &children {
        child.parent.set(Some(Rc::downgrade(node)));
    }
    *node.children.borrow_mut() = children;
}

/// Parse an HTML string into a list of detached nodes, ready to insert into
/// another DOM.
fn parse_html_fragment(html: &str) -> io::Result<Vec<Rc<Node>>> {
    let dom = html_to_dom(html, None)?;
    let body = get_dom_body(&dom);
    let children: Vec<Rc<Node>> = body.children.borrow().clone();
    body.children.borrow_mut().clear();
    for child in &children {
        child.parent.set(None);
    }
    Ok(children)
}

/// Insert `sibling` immediately after `node` in `node`'s parent.
fn insert_after(node: &Rc<Node>, sibling: &Rc<Node>) {
    // Read the parent link non-destructively: `Cell` only supports `take`.
    let parent_weak = node.parent.take();
    node.parent.set(parent_weak.clone());
    let Some(parent) = parent_weak.and_then(|weak| weak.upgrade()) else {
        return;
    };
    let mut children = parent.children.borrow_mut();
    if let Some(position) = children.iter().position(|child| Rc::ptr_eq(child, node)) {
        sibling.parent.set(Some(Rc::downgrade(&parent)));
        children.insert(position + 1, sibling.clone());
    }
}

/// Set (adding if not present) an attribute on an element node.
fn set_attr(node: &Rc<Node>, name: &str, value: &str) {
    if let NodeData::Element { attrs, .. } = &node.data {
        let mut attrs = attrs.borrow_mut();
        if let Some(attr) = attrs.iter_mut().find(|attr| &*attr.name.local == name) {
            attr.value = value.into();
        } else {
            attrs.push(Attribute {
                name: QualName::new(None, Namespace::from(""), LocalName::from(name)),
                value: value.into(),
            });
        }
    }
}

/// Remove an attribute from an element node, if present.
fn remove_attr(node: &Rc<Node>, name: &str) {
    if let NodeData::Element { attrs, .. } = &node.data {
        attrs.borrow_mut().retain(|attr| &*attr.name.local != name);
    }
}

/// Report whether an element node's `class` attribute contains the given class.
fn has_class(node: &Rc<Node>, class: &str) -> bool {
    get_attr_value(node, "class")
        .is_some_and(|classes| classes.split_whitespace().any(|token| token == class))
}

/// Add a class to an element node's `class` attribute.
fn add_class(node: &Rc<Node>, class: &str) {
    if !has_class(node, class) {
        let classes = match get_attr_value(node, "class") {
            Some(existing) if !existing.is_empty() => format!("{existing} {class}"),
            _ => class.to_string(),
        };
        set_attr(node, "class", &classes);
    }
}

/// Remove a class from an element node's `class` attribute, dropping the
/// attribute entirely if no classes remain.
fn remove_class(node: &Rc<Node>, class: &str) {
    if has_class(node, class)
        && let Some(existing) = get_attr_value(node, "class")
    {
        let remaining: Vec<&str> = existing
            .split_whitespace()
            .filter(|token| *token != class)
            .collect();
        if remaining.is_empty() {
            remove_attr(node, "class");
        } else {
            set_attr(node, "class", &remaining.join(" "));
        }
    }
}

/// Report whether this node is a hydrated gather list: `<div
/// class="cc-gather-items">`.
fn is_gather_items_div(node: &Rc<Node>) -> bool {
    get_node_tag_name(node) == Some("div") && has_class(node, "cc-gather-items")
}

// Get the value of an attribute on an element node.
fn get_attr_value(node: &Rc<Node>, attr_name: &str) -> Option<String> {
    if let NodeData::Element { attrs, .. } = &node.data {
        attrs
            .borrow()
            .iter()
            .find(|attr| &*attr.name.local == attr_name)
            .map(|attr| attr.value.to_string())
    } else {
        None
    }
}

/// Gather text from all text children of this node.
fn get_text_content(node: &Rc<Node>) -> String {
    let mut text = String::new();
    for child in node.children.borrow().iter() {
        if let NodeData::Text { contents } = &child.data {
            text.push_str(&contents.borrow());
        }
    }
    text
}

/// This provides the needed context when walking the HTML DOM of all doc
/// blocks.
struct WalkContext {
    /// The cacheable facts collected so far; applied to the cache by
    /// `Cache::commit_file` after the walk completes (which empties this
    /// field).
    facts: FileFacts,
    /// The canonicalized path of the file being hydrated.
    path: PathBuf,
    /// True when hydrating a Markdown document, in which fragments aren't
    /// allowed.
    is_markdown: bool,
    /// The ids `assign_auto_ids` generated for this file's `id="*"` elements.
    /// These are excluded from `facts`: they belong to the cache only after the
    /// file carrying them is written and re-read.
    auto_ids: HashSet<String>,
    /// Each cross-reference found: its destination id and its `<xref>` node,
    /// kept so the generated contents can be patched after the cache commit.
    xrefs: Vec<(String, Rc<Node>)>,
    /// Each fragment found, with everything the later hydration phases need.
    fragments: Vec<FragmentHydration>,
    /// DOM for all gather elements, i.e. targets with a non-empty
    /// `TargetFact::gather_ids`.
    gathers: Vec<Rc<Node>>,
    /// Elements whose `data-gather` can't be hydrated (a missing or invalid
    /// `id`), with the error message to display in place of their gather lists.
    gathers_error: Vec<(Rc<Node>, String)>,
    /// The `doc_block_index` of each gather element; a fragment whose block
    /// range contains one of these is an error.
    gather_block_indices: Vec<usize>,
    /// The current doc block index in the vec of code/doc blocks, based on
    /// parsing the HTML for `codechateditor-separator` elements, which contain
    /// this value.
    doc_block_index: usize,
}

/// Everything the hydration phases after the DOM walk need to know about one
/// `<fragment>` element.
struct FragmentHydration {
    /// The fragment's DOM node.
    node: Rc<Node>,
    /// The fragment's id.
    id: String,
    /// The index of the fragment's first code/doc block; see
    /// `FragmentFact::doc_block_start_index`.
    start: usize,
    /// The index of the fragment's last code/doc block, before clamping to the
    /// number of blocks in the document; see
    /// `FragmentFact::code_doc_block_end_index`.
    end: usize,
    /// When set, the fragment is in an error state detected during the walk
    /// (fragment in a Markdown document, or an unparsable `following`
    /// attribute): the message replaces both the hydrated tag content and the
    /// cached fragment content.
    error: Option<String>,
    /// False when the fragment wasn't recorded in `FileFacts` (its id is
    /// invalid), so no content may be stored for it.
    cached: bool,
}

/// Hydrate the HTML of newly-translated doc blocks.
#[allow(clippy::too_many_lines)]
fn hydrating_walk_node(node: &Rc<Node>, mut walk_context: WalkContext) -> io::Result<WalkContext> {
    for child in node.children.borrow_mut().iter_mut() {
        let possible_replacement_child =
        // Perform replacements of GraphViz and Mermaid graphs:
        //
        // Look for a `<pre>` tag
        if get_node_tag_name(child) == Some("pre")
            // with no attributes
            && let NodeData::Element {
                attrs: ref_child_attrs, ..
            } = &child.data
            && ref_child_attrs.borrow().is_empty()
            // with one `<code>` child
            && let code_children = child.children.borrow()
            && code_children.len() == 1
            && let code_child = code_children.iter().next().unwrap()
            && get_node_tag_name(code_child) == Some("code")
            // with only a `class=language-mermaid/graphviz` attribute
            && let NodeData::Element {
                attrs: ref_code_child_attrs, ..
            } = &code_child.data
            && let code_child_attrs = ref_code_child_attrs.borrow()
            && code_child_attrs.len() == 1
            && let Some(attr) = code_child_attrs.iter().next()
            && *attr.name.local == *"class"
            && let Some(element_name) = CODE_BLOCK_LANGUAGE_TO_CUSTOM_ELEMENT.get(&*attr.value)
            // with only one Text child
            && let text_children = &code_child.children.borrow()
            && text_children.len() == 1
            && let Some(text_child) = text_children.iter().next()
            && let NodeData::Text { .. } = &text_child.data
        {
            // Make the parent node a `element_name` node, with the child's
            // text.
            let wc_mermaid = Node::new(NodeData::Element {
                name: QualName::new(None, Namespace::from(""), LocalName::from(*element_name)),
                attrs: RefCell::new(vec![]),
                template_contents: RefCell::new(None),
                mathml_annotation_xml_integration_point: false,
            });
            wc_mermaid.children.borrow_mut().push(text_child.clone());
            Some(wc_mermaid)
        } else {
            // See if this is a math node to replace; if not, this returns
            // `None`.
            replace_math_node(child, true)
        };

        // Replace the child if we found a replacement.
        if let Some(replacement_child) = possible_replacement_child {
            replacement_child.parent.set(Some(Rc::downgrade(node)));
            *child = replacement_child;
        }

        // Analyze this node for cacheable data.
        if let Some(tag_name) = get_node_tag_name(child) {
            // See if the element has an id/anchor.
            let id = get_attr_value(child, "id");

            // Track doc block index from separator elements.
            if tag_name == "codechateditor-separator"
                && let Ok(index) = get_text_content(child).trim().parse::<usize>()
            {
                walk_context.doc_block_index = index;
            } else if tag_name == "xref" {
                // A cross reference: record the referenced id as a fact, and
                // keep the node so its generated contents can be filled in
                // after the cache commit. Note that this element doesn't allow
                // an `id` attribute, so it's never a target.
                if let Some(ref_id) = get_attr_value(child, "ref") {
                    walk_context.facts.xrefs.push(ref_id.clone());
                    walk_context.xrefs.push((ref_id, child.clone()));
                }
            } else if tag_name == "fragment" {
                // A fragment; without an id it's meaningless, so it's ignored.
                // Its content can't be determined yet -- doc blocks aren't
                // finalized during the walk -- so the caller stores it later
                // via `Cache::update_fragment_content`.
                if let Some(id) = id {
                    // The `following` attribute selects how many code/doc
                    // blocks after the current doc block the fragment encloses;
                    // the default is 1.
                    let (following, following_error) = match get_attr_value(child, "following") {
                        None => (1, None),
                        Some(following) => match following.trim().parse::<usize>() {
                            Ok(count) => (count, None),
                            Err(_) => (
                                0,
                                Some(format!(
                                    "the \"following\" attribute (\"{following}\") must be a whole number"
                                )),
                            ),
                        },
                    };
                    // Determine the fragment's error state, if any; see
                    // `FragmentHydration`. A fragment with an invalid id isn't
                    // cached at all; neither is one whose id was auto-assigned
                    // during this pass, which is otherwise hydrated normally
                    // (nothing can reference an id no file contains yet, so its
                    // backlinks are empty).
                    let (error, cached) = if !is_css_identifier(&id) {
                        (
                            Some(format!("\"{id}\" is not a valid CSS identifier")),
                            false,
                        )
                    } else if walk_context.is_markdown {
                        (
                            Some("fragments are not allowed in Markdown documents".to_string()),
                            true,
                        )
                    } else {
                        (following_error, true)
                    };
                    let cached = cached && !walk_context.auto_ids.contains(&id);
                    if cached {
                        walk_context.facts.fragments.push(FragmentFact {
                            id: id.clone(),
                            line: 0,
                            doc_block_start_index: walk_context.doc_block_index,
                            code_doc_block_end_index: walk_context.doc_block_index + following,
                        });
                    }
                    walk_context.fragments.push(FragmentHydration {
                        node: child.clone(),
                        id,
                        start: walk_context.doc_block_index,
                        end: walk_context.doc_block_index + following,
                        error,
                        cached,
                    });
                }
            } else if let Some(id) = id
                && !id.is_empty()
            {
                // Any other element with an id is a target. A `data-gather`
                // attribute makes that target a gather element; the two are one
                // kind of cached item, since a gather element is also a valid
                // cross-reference destination.
                let gather_ids: Vec<String> = get_attr_value(child, "data-gather")
                    .map(|gather_ids| gather_ids.split_whitespace().map(str::to_string).collect())
                    .unwrap_or_default();
                if is_css_identifier(&id) {
                    if !gather_ids.is_empty() {
                        walk_context.gathers.push(child.clone());
                        walk_context
                            .gather_block_indices
                            .push(walk_context.doc_block_index);
                    }
                    // An id auto-assigned during this pass isn't cached, so no
                    // fact is recorded for it. A gather element is still
                    // hydrated above: its list depends on `data-gather`, not on
                    // its own id.
                    if !walk_context.auto_ids.contains(&id) {
                        walk_context.facts.targets.push(TargetFact {
                            id,
                            // Clean the inner HTML for storage; note that it's
                            // captured before any hydration of descendants, per
                            // the spec in `cache.rs`.
                            inner_html: CLEAN_TARGET_HTML
                                .clean(&node_inner_html(child)?)
                                .to_string(),
                            // Line numbers aren't available until the
                            // pulldown-cmark HTML writer preserves them; see
                            // the TODO in `cache.rs`.
                            line: 0,
                            doc_block_index: walk_context.doc_block_index,
                            gather_ids,
                        });
                    }
                } else if !gather_ids.is_empty() {
                    // An invalid id isn't cached; each reference to it reports
                    // the invalid id itself. A gather element with an invalid
                    // id can't hydrate, so it displays the error.
                    walk_context.gathers_error.push((
                        child.clone(),
                        format!("\"{id}\" is not a valid CSS identifier"),
                    ));
                }
            } else if get_attr_value(child, "data-gather").is_some() {
                // `data-gather` on an element without an id: display an error
                // requesting the missing id, per the spec in `cache.rs`.
                walk_context.gathers_error.push((
                    child.clone(),
                    "a gather element requires an \"id\" attribute".to_string(),
                ));
            }
        }

        // Don't descend into elements whose contents the cache generates
        // (`<xref>`, `<fragment>`, and hydrated gather lists): facts must never
        // be collected from generated content. (Such content only appears here
        // if hand-written source contains it; dehydration removes it from saved
        // files.)
        let skip_descend = matches!(get_node_tag_name(child), Some("xref" | "fragment"))
            || is_gather_items_div(child);
        if !skip_descend {
            walk_context = hydrating_walk_node(child, walk_context)?;
        }
    }

    Ok(walk_context)
}

fn replace_math_node(child: &Rc<Node>, is_hydrate: bool) -> Option<Rc<Node>> {
    // Look for math produced by pulldown-cmark: `<span class="math
    // math-inline>...</span>`; add `\(...\)` inside the span. Perform a similar
    // transformation for display math.
    if get_node_tag_name(child) == Some("span")
            && let NodeData::Element {
                attrs: ref_child_attrs, ..
            } = &child.data
            && let child_attrs = ref_child_attrs.borrow()
            && let child_attrs_len = child_attrs.len()
            && child_attrs_len >= 1
            // Look up the `class` attribute by name, so it may appear in any
            // order relative to other attributes.
            && let Some(class_attr) = child_attrs
                .iter()
                .find(|attr| *attr.name.local == *"class")
            && let attr_value = &class_attr.value
            // with only one Text child
            && let text_children = &child.children.borrow()
            && text_children.len() == 1
            && let Some(text_child) = text_children.iter().next()
            && let NodeData::Text { contents } = &text_child.data
    {
        // Final test: if the class is correct, this is math; otherwise, perform
        // no transformation.
        //
        // Add/remove the `mceNonEditable` class to prevent accidental edits of
        // this span.
        let attr_value_str: &str = attr_value;
        let delim = if is_hydrate {
            // When hydrating, there should only be a `class` attribute.
            if child_attrs_len == 1 {
                match attr_value_str {
                    "math math-inline" => Some(("\\(", "\\)", "math math-inline mceNonEditable")),
                    "math math-display" => Some(("$$", "$$", "math math-display mceNonEditable")),
                    _ => None,
                }
            } else {
                None
            }
        } else {
            if child_attrs_len == 2
                && let Some(contenteditable_attr) = child_attrs
                    .iter()
                    .find(|attr| *attr.name.local == *"contenteditable")
                && contenteditable_attr.value == *"false"
            {
                match attr_value_str {
                    "math math-inline mceNonEditable" => Some(("\\(", "\\)", "math math-inline")),
                    "math math-display mceNonEditable" => Some(("$$", "$$", "math math-display")),
                    _ => None,
                }
            } else {
                None
            }
        };

        // Since we've already borrowed `child`, we can't `borrow_mut` to modify
        // it. Instead, create a new `span` with delimited text and return that.
        if let Some(delim) = delim {
            let contents_str = &*contents.borrow();
            let delimited_text_str = if is_hydrate {
                format!("{}{}{}", delim.0, contents_str, delim.1)
            } else {
                // Only apply the dehydration if the delimiters are correct.
                if !contents_str.starts_with(delim.0) || !contents_str.ends_with(delim.1) {
                    return None;
                }
                // Return the contents without the beginning and ending
                // delimiters.
                contents_str[delim.0.len()..contents_str.len() - delim.1.len()].to_string()
            };
            let delimited_text_node = Node::new(NodeData::Text {
                contents: RefCell::new(delimited_text_str.into()),
            });
            let mut attrs_vec = vec![Attribute {
                name: QualName::new(None, Namespace::from(""), LocalName::from("class")),
                value: delim.2.into(),
            }];
            if is_hydrate {
                attrs_vec.push(Attribute {
                    name: QualName::new(
                        None,
                        Namespace::from(""),
                        LocalName::from("contenteditable"),
                    ),
                    value: "false".into(),
                });
            }
            let span = Node::new(NodeData::Element {
                name: QualName::new(None, Namespace::from(""), LocalName::from("span")),
                attrs: RefCell::new(attrs_vec),
                template_contents: RefCell::new(None),
                mathml_annotation_xml_integration_point: false,
            });
            delimited_text_node.parent.set(Some(Rc::downgrade(&span)));
            span.children.borrow_mut().push(delimited_text_node);
            Some(span)
        } else {
            None
        }
    } else {
        None
    }
}

pub fn remove_tinymce_data(
    parent: &Rc<Node>,
    // The index of the node in parent.children.
    index: usize,
) -> Option<Rc<Node>> {
    let node = parent.children.borrow()[index].clone();
    // Remove TinyMCE temp attributes produced in the raw format.
    if let NodeData::Element { name, attrs, .. } = &node.data {
        // Look for any temporary elements inserted for GUI manipulation.
        if name.local == *"span"
            && attrs
                .borrow()
                .iter()
                .any(|attr| attr.name.local == *"class" && attr.value.starts_with("mce"))
        {
            // Replace this element with its children. First, update the
            // children with the new parent.
            let new_parent = node
                .parent
                .take()
                .unwrap_or_else(|| panic!("Must be non-root node, but saw {:#?}.", node.data));
            for child in node.children.borrow_mut().iter_mut() {
                child.parent.set(Some(new_parent.clone()));
            }

            // Insert the children in place of the node.
            let children: Vec<_> = node.children.borrow().to_vec();
            let no_children = children.is_empty();
            parent.children.borrow_mut().splice(index..=index, children);
            // Process the first child which replaced the current node, since it
            // hasn't been processed yet, then return it as the updated node.
            return if no_children {
                None
            } else {
                // Important: all previous borrows of `parent` must be dropped,
                // since this will re-borrow it.
                remove_tinymce_data(parent, index)
            };
        }
        // If we didn't remove this element, then filter out unwanted
        // attributes.
        //
        // A `contenteditable` attribute is one of these on most elements:
        // TinyMCE's anchor plugin marks every empty named anchor (`<a
        // id="foo"></a>`) non-editable when it parses a doc block, and undoes
        // that only in its serializer -- which the Client bypasses by saving in
        // the raw format (see the note on `docContent` in
        // [CodeChatEditor.mts](../../client/src/CodeChatEditor.mts)). Without
        // this, that attribute is written back to the source file.
        //
        // The exceptions are the elements which legitimately carry it here:
        // `div`, `xref`, and `fragment` are the only tags `AMMONIA_OPTIONS`
        // allows it on, so only they can have received it from the source file;
        // the cache-generated copies on `<xref>` and `<fragment>` are removed
        // by `dehydrating_walk_node`. A math `<span>`'s copy must survive until
        // `replace_math_node` recognizes the span and rebuilds it without one.
        let keeps_contenteditable = matches!(&*name.local, "div" | "span" | "xref" | "fragment");
        attrs.borrow_mut().retain(|attr| {
            !(attr.name.local.starts_with("data-mce-")
                || (attr.name.local == *"class" && attr.value.starts_with("mce-"))
                || (!keeps_contenteditable && attr.name.local == *"contenteditable"))
        });
    }
    Some(node.clone())
}

/// Walk a node, dehydrating it by removing TineMCE temporary attributes,
/// changing math to pulldown-cmark's output, changing graphviz/Mermaid to
/// fenced code blocks, and removing all cache-hydrated content (`<xref>` and
/// `<fragment>` contents, gather lists, and the classes/attributes hydration
/// adds), so that none of it is written back to source.
#[allow(clippy::too_many_lines)]
fn dehydrating_walk_node(node: &Rc<Node>) {
    let mut index = 0;
    // Avoid a `while` loop, since accessing `node.children` requires a borrow
    // held for the body of the loop.
    while index < node.children.borrow().len() {
        // Remove TinyMCE data from the child at `index`; if the child was
        // spliced away (no replacement), the slot is gone; leave \`index\`\`
        // unchanged to process what is now at this position.
        if remove_tinymce_data(node, index).is_none() {
            continue;
        }

        // Remove cache-hydration artifacts.
        {
            let child = node.children.borrow()[index].clone();
            // A hydrated gather list is entirely generated content; remove the
            // node. The slot now holds the next child, so leave `index`
            // unchanged.
            if is_gather_items_div(&child) {
                node.children.borrow_mut().remove(index);
                continue;
            }
            // The contents of these elements are generated by the cache; the
            // `contenteditable` attribute is added during hydration.
            if matches!(get_node_tag_name(&child), Some("xref" | "fragment")) {
                remove_attr(&child, "contenteditable");
                child.children.borrow_mut().clear();
            }
            // Hydration marks gather elements with this class.
            remove_class(&child, "cc-gather");
        }

        // Compute the replacement (if any) inside a block so `borrow_mut` is
        // dropped before the recursive walk, which may itself need `borrow_mut`
        // via `remove_tinymce_data`.
        let child_to_walk = {
            let mut children = node.children.borrow_mut();
            if index >= children.len() {
                break;
            }
            let child = &mut children[index];
            let replacement = if let Some(child_name) = get_node_tag_name(child)
                && let Some(language_name) = CUSTOM_ELEMENT_TO_CODE_BLOCK_LANGUAGE.get(child_name)
                // with no attributes
                && let NodeData::Element {
                    attrs: ref_attrs, ..
                } = &child.data
                && ref_attrs.borrow().is_empty()
                // and only one Text child
                && let text_children = &child.children.borrow()
                && text_children.len() == 1
                && let Some(text_child) = text_children.iter().next()
                && let NodeData::Text { .. } = &text_child.data
            {
                // Create `<pre><code class="from language_name">text_child
                // contents</code></pre>`.
                let pre = Node::new(NodeData::Element {
                    name: QualName::new(None, Namespace::from(""), LocalName::from("pre")),
                    attrs: RefCell::new(vec![]),
                    template_contents: RefCell::new(None),
                    mathml_annotation_xml_integration_point: false,
                });
                let code = Node::new(NodeData::Element {
                    name: QualName::new(None, Namespace::from(""), LocalName::from("code")),
                    attrs: RefCell::new(vec![Attribute {
                        name: QualName::new(None, Namespace::from(""), LocalName::from("class")),
                        value: (*language_name).into(),
                    }]),
                    template_contents: RefCell::new(None),
                    mathml_annotation_xml_integration_point: false,
                });
                code.parent.set(Some(Rc::downgrade(&pre)));
                code.children.borrow_mut().push(text_child.clone());
                pre.children.borrow_mut().push(code);
                Some(pre)
            } else
            // Look for a fenced code block containing `<br>` elements added by
            // TinyMCE, instead of the usual newlines. Translate these to
            // newlines, so that HTML to markdown conversion works.
            //
            // A fenced codeblock is a `<pre>` tag...
            if get_node_tag_name(child) == Some("pre")
                // ...with zero attributes...
                && let NodeData::Element {
                    attrs: ref_pre_attrs, ..
                } = &child.data
                && let pre_attrs = ref_pre_attrs.borrow()
                && pre_attrs.is_empty()
                // ...and exactly one child, ...
                && let pre_children = &child.children.borrow()
                && pre_children.len() == 1
                && let Some(code_child) = pre_children.iter().next()
                // ...which is a `code` tag with either zero attributes or one
                // `class` attribute whose value starts with `language-`, ...
                && get_node_tag_name(code_child) == Some("code")
                && let NodeData::Element {
                    attrs: ref_code_attrs, ..
                } = &code_child.data
                && let code_attrs = ref_code_attrs.borrow()
                && (code_attrs.is_empty()
                    || (code_attrs.len() == 1
                        && code_attrs.iter().next().is_some_and(|attr| {
                            *attr.name.local == *"class"
                                && attr.value.starts_with("language-")
                        })))
                // ...whose children consist only of text nodes and `<br>`
                // elements with no attributes, including at least one `<br>`.
                && let code_children = &code_child.children.borrow()
                && code_children.iter().all(|c| {
                    matches!(c.data, NodeData::Text { .. })
                        || (get_node_tag_name(c) == Some("br")
                            && matches!(&c.data, NodeData::Element { attrs, .. } if attrs.borrow().is_empty()))
                })
                && code_children
                    .iter()
                    .any(|c| get_node_tag_name(c) == Some("br"))
            {
                // Replace all `br` instances with a text node containing a
                // newline, preserving the surrounding `pre`/`code` structure.
                let new_code = Node::new(NodeData::Element {
                    name: QualName::new(None, Namespace::from(""), LocalName::from("code")),
                    attrs: RefCell::new(code_attrs.clone()),
                    template_contents: RefCell::new(None),
                    mathml_annotation_xml_integration_point: false,
                });
                {
                    let mut new_code_children = new_code.children.borrow_mut();
                    for c in code_children.iter() {
                        let new_child = if get_node_tag_name(c) == Some("br") {
                            Node::new(NodeData::Text {
                                contents: RefCell::new("\n".into()),
                            })
                        } else {
                            c.clone()
                        };
                        new_child.parent.set(Some(Rc::downgrade(&new_code)));
                        new_code_children.push(new_child);
                    }
                }
                let new_pre = Node::new(NodeData::Element {
                    name: QualName::new(None, Namespace::from(""), LocalName::from("pre")),
                    attrs: RefCell::new(vec![]),
                    template_contents: RefCell::new(None),
                    mathml_annotation_xml_integration_point: false,
                });
                new_code.parent.set(Some(Rc::downgrade(&new_pre)));
                new_pre.children.borrow_mut().push(new_code);
                Some(new_pre)
            } else {
                replace_math_node(child, false)
            };
            if let Some(replacement_child) = replacement {
                *child = replacement_child;
                None
            } else {
                Some(children[index].clone())
            }
        };
        // `borrow_mut` is now dropped; safe to recurse. Recurse first so that
        // TinyMCE attributes on descendants (e.g. `data-mce-bogus` on a `<br>`)
        // are removed before the `<p><br></p>` check below.
        if let Some(child) = child_to_walk {
            dehydrating_walk_node(&child);
        }
        index += 1;
    }
}

fn get_node_tag_name(node: &Rc<Node>) -> Option<&str> {
    match &node.data {
        NodeData::Document => Some("html"),
        NodeData::Element { name, .. } => Some(&name.local),
        _ => None,
    }
}

// Translate from Markdown class names for code blocks to the appropriate HTML
// custom element.
static CODE_BLOCK_LANGUAGE_TO_CUSTOM_ELEMENT: phf::Map<&'static str, &'static str> = phf_map! {
    "language-mermaid" => "wc-mermaid",
    "language-graphviz" => "graphviz-graph",
};

static CUSTOM_ELEMENT_TO_CODE_BLOCK_LANGUAGE: phf::Map<&'static str, &'static str> = phf_map! {
    "wc-mermaid" => "language-mermaid",
    "graphviz-graph" => "language-graphviz"
};

// ### Diff support
//
// This section provides methods to diff the previous and current
// `CodeMirrorDocBlockVec`. The primary purpose is to fix a visual bug: if the
// entire CodeMirror data structure is overwritten, then CodeMirror loses track
// of the correct vertical scroll bar position, probably because it has build up
// information on the size of each rendered doc block; these correct sizes are
// reset when all data is overwritten, causing unexpected scrolling. Therefore,
// this approach is to modify only what changed, rather than changing
// everything. As a secondary goal, this hopefully improves overall performance
// by sending less data between the server and the client, in spite of the
// additional computational requirements for computing the diff.
//
// Fundamentally, diffs of a string and diff of this vector require different
// approaches:
//
// * The `CodeMirrorDocBlock` is a structure, with several fields. In
//   particular, the contents is usually the largest element; the indent can
//   also be large.
// * It should handle the following common cases well:
//   1. An update of a code block. This causes the from and to field of all
//      following doc blocks to change, without changing the other fields.
//   2. An update to the contents of a doc block. For large doc blocks, this is
//      more efficiently stored as a diff rather than the full doc block text.
//   3. Inserting or deleting a doc block.
//
// The diff algorithm simply looks for equality between elements contained in
// the before and after vectors provided it. However, this requires something
// more fine-grained: the ability to track changes to the `contents` as a first
// priority (common cases 2, 3), then fix up non-`contents` field (common case
// 1).
//
// #### Overall approach
//
// 1. Use the diff algorithm to find the minimal change set between a before and
//    after `CodeMirrorDocBlocksVec`, which only looks at the `contents`. This
//    avoids "noise" from changes in from/to fields from obscuring changes only
//    to the `contents`.
// 2. For all before and after blocks whose `contents` were identical, compare
//    the other fields, adding these to the change set, but not attempting to
//    use the diff algorithm.
// 3. Represent changes to the `contents` as a `StringDiff`.
//
// #### String diff
/// Given two strings, return a list of changes between them.
#[must_use]
pub fn diff_str(before: &str, after: &str) -> Vec<StringDiff> {
    let mut change_spec: Vec<StringDiff> = Vec::new();
    // The previous value of `before.start` and the character index
    // corresponding to `before.start`.
    let mut prev_before_start = 0;
    let mut prev_before_start_chars = 0;
    let input = InternedInput::new(before, after);

    let diff = Diff::compute(Algorithm::Histogram, &input);
    for hunk in diff.hunks() {
        let count_before_chars = |lines: Range<u32>| {
            input.before[lines.start as usize..lines.end as usize]
                .iter()
                .map(|&line| {
                    input.interner[line].chars().fold(
                        // Count offsets into the string in UTF-16 code units,
                        // since the offsets produced are used by the Client
                        // ([JavaScript uses UTF-16](https://developer.mozilla.org/en-US/docs/Glossary/UTF-16#utf-16_in_javascript),
                        // as does
                        // [CodeMirror](https://codemirror.net/docs/guide/#document-offsets))
                        // and VSCode (also JavaScript).
                        0,
                        |acc, e| acc + e.len_utf16(),
                    )
                })
                .sum::<usize>()
        };
        // Sum characters between the last change and this change.
        prev_before_start_chars += count_before_chars(prev_before_start..hunk.before.start);
        prev_before_start = hunk.before.start;
        // Get the characters in the hunk after this change.
        let hunk_after: Vec<_> = input.after[hunk.after.start as usize..hunk.after.end as usize]
            .iter()
            .map(|&line| input.interner[line])
            .collect();
        let before_chars = count_before_chars(hunk.before.start..hunk.before.end);
        change_spec.push(StringDiff {
            from: prev_before_start_chars,
            to: if before_chars != 0 {
                Some(prev_before_start_chars + before_chars)
            } else {
                None
            },
            insert: if hunk_after.is_empty() {
                String::new()
            } else {
                hunk_after.into_iter().collect()
            },
        });
    }

    change_spec
}

// #### Diff support for `CodeMirrorDocBlockVec`
/// We can't simply implement traits for `CodeMirrorDocBlockVec`, since it's not
/// a struct. So, wrap it in a struct, then implement traits on that struct.
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
        u32::try_from(self.0.len()).unwrap_or(u32::MAX)
    }
}

fn none_if_eq<T: PartialEq>(before: &T, after: T) -> Option<T> {
    if before == &after { None } else { Some(after) }
}

fn none_if_eq_ref<T: PartialEq + Clone>(before: &T, after: &T) -> Option<T> {
    if before == after {
        None
    } else {
        Some(after.clone())
    }
}

/// Given two `CodeMirrorDocBlocks`, return a list of changes between them.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn diff_code_mirror_doc_blocks(
    before: &CodeMirrorDocBlockVec,
    after: &CodeMirrorDocBlockVec,
) -> Vec<CodeMirrorDocBlockTransaction> {
    let input = InternedInput::new(
        CodeMirrorDocBlocksStruct(before),
        CodeMirrorDocBlocksStruct(after),
    );
    let mut prev_before_range_end = 0;
    let mut prev_after_range_end = 0;

    // This compare all fields, not just the `contents`, of two
    // `CodeMirrorDocBlock`s. It should be applied to every entry that the
    // `diff` function sees as equal.
    let mut diff_all = |hunk: &Hunk, change_specs: &mut Vec<CodeMirrorDocBlockTransaction>| {
        // First, compare blocks from the previous point until this point. The
        // diff used only compares contents; this checks everything.
        while prev_before_range_end < hunk.before.start && prev_after_range_end < hunk.after.start {
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
            // Second phase: if before and after are different, insert an
            // update.
            if prev_before_range_start_val != prev_after_range_start_val {
                change_specs.push(CodeMirrorDocBlockTransaction::Update(
                    CodeMirrorDocBlockUpdate {
                        from: prev_before_range_start_val.from,
                        from_new: none_if_eq(
                            &prev_before_range_start_val.from,
                            prev_after_range_start_val.from,
                        ),
                        to: none_if_eq(
                            &prev_before_range_start_val.to,
                            prev_after_range_start_val.to,
                        ),
                        indent: none_if_eq_ref(
                            &prev_before_range_start_val.indent,
                            &prev_after_range_start_val.indent,
                        ),
                        delimiter: none_if_eq_ref(
                            &prev_before_range_start_val.delimiter,
                            &prev_after_range_start_val.delimiter,
                        ),
                        contents: diff_str(
                            &prev_before_range_start_val.contents,
                            &prev_after_range_start_val.contents,
                        ),
                    },
                ));
            }

            prev_before_range_end += 1;
            prev_after_range_end += 1;
        }
        prev_before_range_end = hunk.before.end;
        prev_after_range_end = hunk.after.end;
    };

    let mut change_specs = Vec::new();
    let diff = Diff::compute(Algorithm::Histogram, &input);
    for hunk in diff.hunks() {
        diff_all(&hunk, &mut change_specs);
        // Update the `prev` values so we start processing immediately after
        // this change.

        // Process the insertions and deletions.
        let mut before_index = hunk.before.start;
        // Values in the `after_index` become either inserts or replacements.
        for after_index in hunk.after {
            let after_val = &after[after_index as usize];
            // Assume that an insert/delete is a replace; this is the most
            // common case (a minor edit to the text of a doc block). If not,
            // the replace is a bit less efficient than the insert/delete, but
            // still correct.
            if before_index < hunk.before.end {
                let before_val = &before[before_index as usize];
                change_specs.push(CodeMirrorDocBlockTransaction::Update(
                    CodeMirrorDocBlockUpdate {
                        from: before_val.from,
                        from_new: none_if_eq(&before_val.from, after_val.from),
                        to: none_if_eq(&before_val.to, after_val.to),
                        indent: none_if_eq_ref(&before_val.indent, &after_val.indent),
                        delimiter: none_if_eq_ref(&before_val.delimiter, &after_val.delimiter),
                        contents: diff_str(&before_val.contents, &after_val.contents),
                    },
                ));
                before_index += 1;
            } else {
                // Otherwise, this in an insert.
                change_specs.push(CodeMirrorDocBlockTransaction::Add(CodeMirrorDocBlock {
                    from: after_val.from,
                    to: after_val.to,
                    indent: after_val.indent.clone(),
                    delimiter: after_val.delimiter.clone(),
                    contents: after_val.contents.clone(),
                }));
            }
        }

        // Anything left should be deleted.
        for index in before_index..hunk.before.end {
            change_specs.push(CodeMirrorDocBlockTransaction::Delete(
                CodeMirrorDocBlockDelete {
                    from: before[index as usize].from,
                },
            ));
        }
    }

    // Process the last hunk. The end of the before and after ranges (0 here)
    // doesn't matter, since it's not used.
    diff_all(
        &Hunk {
            before: (u32::try_from(before.len()).unwrap_or(u32::MAX)..0),
            after: u32::try_from(after.len()).unwrap_or(u32::MAX)..0,
        },
        &mut change_specs,
    );

    // If two doc blocks immediately follow each other: `# foo\n # bar\n`, for
    // example, and a line is inserted before both, then a problem occurs when
    // applying the change: applying the change to the first block sets its
    // `from` value to the `from` value for the second block. This violates a
    // doc blocks invariant -- each doc block must have a unique `from` value;
    // therefore, these two doc blocks can no longer be distinguished, making it
    // impossible to apply the change to the second doc block. More generally,
    // this can occur with an insert before a series of blocks which immediately
    // follow each other.
    //
    // A similar problem occurs when multiple lines are inserted: the `from` of
    // an earlier doc block can become the same as the `from` of a later doc
    // block. For example, consider three doc blocks starting at lines 10, 15,
    // and 20. Inserting 10 lines makes from first doc block's `from` value
    // change to 20, which again violates the doc blocks invariant.
    //
    // Rather than search for this case (which would be computationally
    // expensive), generalize: inserts (which increase the `from` value of a doc
    // block, possibly making it identical to later `from` values) should be
    // processed from the end of the document toward the beginning, while
    // deletions which decrease the `from` value should be processed from the
    // beginning of the document to its end.
    //
    // Doc block insertions and deletions carry the same challenges as textual
    // insertions and deletions. Insertions must be performed end to beginning,
    // while deletions must be performed beginning to end.
    //
    // Therefore, look for sequences of insertions (adds) or updates where
    // `from_new` > `from` and swap these sequences.
    let mut immediate_sequence_start_index: Option<usize> = None;
    for index in 0..change_specs.len() {
        let is_add = matches!(&change_specs[index], CodeMirrorDocBlockTransaction::Add(_));
        let is_inserted_update = matches!(
            &change_specs[index],
            CodeMirrorDocBlockTransaction::Update(u) if u.from_new.is_some_and(|f| f > u.from)
        );
        if is_add || is_inserted_update {
            // This is an update produced by inserting lines.
            if immediate_sequence_start_index.is_none() {
                // This is the start of the sequence -- mark it.
                immediate_sequence_start_index = Some(index);
            }
        } else {
            // This is not an update produced by an insertion.
            if let Some(prev_index) = immediate_sequence_start_index {
                // This is the end of a sequence. Reverse it.
                change_specs[prev_index..index].reverse();
            }
            // Mark that there's no sequence now.
            immediate_sequence_start_index = None;
        }
    }
    // If a sequence ended at the end of the document, process it.
    if let Some(prev_index) = immediate_sequence_start_index {
        change_specs[prev_index..].reverse();
    }

    change_specs
}

// Tests
// -----
#[cfg(test)]
mod tests;
