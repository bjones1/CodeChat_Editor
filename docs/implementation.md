Copyright (C) 2025 Bryan A. Jones.

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
[http://www.gnu.org/licenses/](http://www.gnu.org/licenses/).

Implementation
================================================================================

### System architecture

```graphviz
digraph {
    bgcolor = transparent;
    compound = true;
    node [shape = box];
    subgraph cluster_text_editor {
        label = "Text editor/IDE"
        source_code [label = "Source\ncode", style = dashed];
        CodeChat_plugin [label = "CodeChat\nEditor plugin"];
    }
    subgraph cluster_server {
        label = <CodeChat Editor Server>;
        websocket_server [label = "Websocket\nserver"];
        web_server [label = "Web\nserver"];
    }
    subgraph cluster_client_framework {
        label = "CodeChat Editor Client framework"
        subgraph cluster_client {
            label = "CodeChat Editor Client"
            rendered_code [label = "Rendered document", style = dashed];
        }
    }
    CodeChat_plugin -> websocket_server [label = "NAPI-RS", dir = both, lhead = cluster_server];
    websocket_server -> rendered_code [label = "websocket", dir = both, lhead = cluster_client_framework];
    web_server -> rendered_code [label = "HTTP", dir = both, lhead = cluster_client ];
}
```

Inside the client:

* The Framework exchanges messages with the Server and loads the appropriate
  Client (simple view, PDF view, editor, document-only editor).
* The editor provides basic Client services and handles document-only mode.
* The CodeMirror integration module embeds TinyMCE into CodeMirror, providing
  the primary editing environment.

The entire VSCode interface is contained in the extension, with the NAPI-RS glue
in the corresponding library.

Does this make more sense to place in the TOC? Or is it too wordy there? I think
a diagram as an overview might be helpful. Perhaps the server, client, etc.
should have its of readme files providing some of this.

<a id="an-implementation"></a>Architecture
--------------------------------------------------------------------------------

Overall, the code is something like this:

### Client/server partitioning

Doc blocks consist of Markdown augmented with custom HTML elements which provide
authoring support. Some of these elements depend only on information in the
current page. For example, a GraphViz graph tag transforms the graph provided in
its tag into an SVG; it doesn't need information from other files. Other
elements, such as a cross-reference tag, depend on information from other pages
(in this case, the page containing the referenced item). The client lacks the
ability to access other files, while the server has direct access to these
files. Therefore, the overall strategy is:

* On page load, the server transforms custom tags which depend on information
  from other pages into tags which include this information. For example, a
  cross-reference tag might be transformed into a hyperlink whose link text
  comes from the cross-reference on another page.
* The client them defines a set of
  [Web Components](https://developer.mozilla.org/en-US/docs/Web/Web_Components)
  which implement custom tags which only need local information. For example, a
  GraphViz custom tag renders graphs based on a description of the graph inside
  the tag. Likewise, MathJax interprets anything in matching math delimiters
  (<span class="math math-inline mceNonEditable">...</span>, for example), then
  transforms it back to text before saving.
* On save, the client sends its text back to the server, which de-transforms
  custom tags which depend on information from other pages. If de-transforms
  disagree with the provided text, then re-load the updated text after the save
  is complete. For example, after inserting an auto-titled link, the auto-titled
  text is missing; a save/reload fixes this.

### Page processing pipeline

On load:

* Classify the file; inputs are mutable global state (which, if present,
  indicates this is a project build), if the file is a TOC, the file's binary
  data, and the file's path. Output of the classification: binary, raw text, a
  CodeChat document (a Markdown file), or a CodeChat file. The load processing
  pipelines

For CodeChat files:

* (CodeChat files only) Run pre-parse hooks: they receive source code, file
  metadata. Examples: code formatters. Skip if cache is up to date.
* (CodeChat files only) Lex the file into code and doc blocks.
* Run post-parse hooks: they receive an array of code and doc blocks.
* Transform Markdown to HTML.
* Run HTML hooks:
  * Update the cache for the current file only if the current file's cache is
    stale. To do this, walk the DOM of each doc block. The hook specifies which
    tags it wants, and the tree walker calls the hook when it encounters these.
    If this requires adding/changing anything (anchors, for example), mark the
    document as dirty.
  * Update tags whose contents depend on data from other files. Hooks work the
    same as the cache updates, but have a different role. They're always run,
    while the cache update is skipped when the cache is current.
* Determine next/prev/up hyperlinks based on this file's location in the TOC.
* Transform the code and doc blocks into CodeMirror's format.

We want a clean separate between the webserver and the processing pipeline. The
webserver should provide I/O between the local file system and the client, but
do little processing. The processing pipeline should not perform I/O. Therefore:

* On load, the webserver receives a request for a file. It should gather and
  pass the following to the page processor:
  * The loaded file as text (or an Err result).
  * The global state (empty if this isn't a project build).
  * The pathname of the file.
  * If this file should be processed as a TOC or not.
* The page processor returns:
  * An Enum with the file's contents:

On save:

* Transform the CodeMirror format back to code and doc blocks.
* Run HTML hooks:
  * Update the cache for the current file. Mark the file as "dirty" (reload
    needed) if any changes are made.
  * Check tags whose contents depend on data from other files; if the contents
    differ, mark the file as dirty.
  * Transform HTML to Markdown.
* Run post-parse hooks; mark the file as dirty if any changes are made.
* De-lex the file into source code.
* Run pre-parse hooks; mark the file as dirty if any changes are made.
* Save the file to disk.
* If dirty, re-load the file.

### Table of contents

Ideas:

* Something that reflects the filesystem. Subdirectories are branches, files are
  leaves in the TOC tree. Problems:
  * Subdirectories should have content, such as a readme. Assume a readme file
    titles and provides content for a subdirectory? Or provide a config file
    setting to assign this?
  * I'd like the ability to relocate files/directories. The means a config file
    that tracks this movement.
  * We need ignores.
  * To reorder files in the TOC, need a config file per directory to store this
    ordering.
  * Pro: all files are automatically included, so adding a new file is
    automatic. The hierarchy is mostly defined by the filesystem, which is nice.
    A GUI with drag and drop would make this really simple to maintain.
  * Con: a lot of work/rewrite.
  * So: readme.md provides a title and contents for a subdirectory. A config
    file in each directory specifies ordering of files, titles for non-CodeChat
    files (PDFs, etc.), moves of files/directories from other directories, and
    ignores.
* Use mdbook's idea -- a very specific structure for a toc.md file. Simple, but
  doesn't auto-update as files are added.
* Current TOC isn't immediately useful. Too much flexibility. So, I have to do a
  rewrite regardless.

Another topic: how to reconcile headings in a file with the TOC?

* Separate them -- headings have orthogonal numbering to the TOC. I think this
  is simplest. I just need the right way to display it; mdbook is reasonable in
  this regard. I'll use this.
* Combine them -- H1 is current number, H2 is a subhead, etc. But this means the
  TOC's numbering requires reading the contents of all files referenced by the
  TOC, which could be slow.

Config file format:

* order = \[file\_name\_1, file\_name\_2, ...\]
* move = \[path\_to\_moved\_file\_name\_1, ...\]
* \[title\] file\_name = title\_text
* subdir\_contents = name\_of\_readme\_file.md
* ignore = \[ignore\_1, ...\]. However, putting this in a separate file would be
  easier from an implementation perspective. (ignore crate).

Concerns:

* I don't want to re-read the entire filesystem on an update. I assume I can use
  a file change notification to work around this, re-reading only if file is
  created/deleted, or an ignore file is modified. A central TOC would be faster,
  but there's no way to auto-add new files.
* Reading a bunch of config files faces similar challenges.
* Given all that, I end up with a tree of entries. This needs to become a TOC
  data structure and HTML. TOC data structure is an array, each element
  containing:
  * A path to a file/directory. It's a full path, in order to supporr moves.
  * An optional title for this file/directory.
  * If this is a directory:
    * An optional file which provides the page contents for this subdirectory.
    * An array of TOC entires in this directory.
  * As Markdown: a unorderd list item of \[title \<path to contents file>\](path
    to file)
* For simplicity, I can initially only implement the order key.
* This feels a lot like a autogenerated TOC, which is actually what it is.
  Simpler is let the user write this and perhaps provide a way to regenerate it.
* The mdbook-summary crate does pretty much all this. This sounds like a good
  initial approach.

### Cache data format

The cache stores the location (file name and ID), numbering (of headings and
figures/equations/etc.), and contents (title text or code/doc blocks for tags)
of a target. Targets are HTML anchors (such as headings, figure titles, display
equations, etc.) or tags.

Goals:

* Given a file name and/or ID, retrieve the associated location, numbering, and
  contents.
* Perform a search of the contents of all targets, returning a list of matching
  targets.
* Given a file name and/or ID, provide a list of all targets in the containing
  file.

Cache data structure:

* A hashmap of (Path, target data structure). TBD: think about ownership. I
  think a page is the owner of all targets.
* A hashmap of (ID, target data structure). 

Target data structure:

* Location: the containing page and an `Option<String>` containing the ID, if
  assigned.
* Page numbering: `[Option<i32>, ...]` where each i32 in the list represents the
  number of a H1..6 element (non-TOC) or the numbering of a list item (TOC);
  `None` represents a missing level of the hierarchy (e.g. H1 following by and
  H3, with no H2 between).
* Type: page, heading, link, tag, caption, equation; numbered items (caption,
  equation) also include the current number. Pages include the page data
  structure.
* Contents: either a string of HTML (would prefer Markdown) or a vec of code/doc
  blocks. Page contents are an empty string.

Page data structure:

* Path: the path to this file.
* File info: timestamp, etc. to compare with the filesystem in order to
  determine if this cache entry is up to date or no. An option, in the case that
  the file doesn't exist -- it's the target of a broken link.
* TOC location: `[i32, ...]` gives the numbering of this page in the TOC; if
  it's not in the TOC, this is an empty list.
* Vector of targets on this page.
* (Maybe) first ID on this page.

Pseudocode:

1. Create a hashmap of (file paths to index, list of links depending on this
   file). Initialize it with the current file.
2. For each file in the hashset:
   1. If this is the first file, we already have its DOM. Otherwise, load the
      file from disk and compute the DOM.
   2. Given a file's DOM, first create its page data structure. Pre-existing
      cache data provides the TOC numbering.
   3. For each target in the DOM (non-TOC) / numbered item (TOC), add the
      target's data structure to the page's vector of targets, updating the
      current numbering if this is a numbered item (heading, caption, etc.) and
      inserting the HTML to set its number in the DOM.
   4. If this is the first file: for each link in the DOM, if the link is local
      and autotitled, look for it in the cache. If it's not in the cache or if
      the cache for that file is outdated, add the referring file to the hashset
      of files to update if it's not in the hashmap; append this link to its
      list of dependent links.
   5. For each link in the list of links depending on this file, update it with
      the loaded content.

References:

* Hyperlinks with no link text are auto-titled. Look up

### IDE/editor integration

The IDE extension client (IDE for short) and the CodeChat Editor Client (or
Editor for short) exchange messages with each other, mediated by the CodeChat
Server. The Server forwards messages from one client to the other, translating
as necessary (for example, between source code and the Editor format).

#### Architecture

**Reviewed to here**

Clients always come in pairs: one IDE client is always paired with one CodeChat
Editor client. The server uses a set of queues to decouple websocket protocol
activity from the core processing needed to translate source code between a
CodeChat Editor Client and an IDE client. Specifically, one task handles the
receive and transmit function for the websocket:

* The task sends a periodic ping to the CodeChat Editor Client or the IDE
  client, then waits for a pong, closing the connection if the pong isn't
  received in a timely manner. This helps detect a broken websocket connection
  produced when a computer is put to sleep then wakes back up.
* Likewise, the task responds to a ping message from the CodeChat Editor Client
  by sending a pong in response.
* It tracks messages sent and produces an error message if a sent message isn't
  acknowledged within a timeout window.
* If the websocket is closed without warning, the websocket stores the relevant
  data so that it can resume when the client reconnects to it.
* If the websocket is closed purposefully (for example, by closing a CodeChat
  Editor Client tab in a web browser), the receive task detects this and shuts
  down the websocket along with the associated IDE client tasks.

To decouple these low-level websocket details from high-level processing (such
as translating between source code and its web equivalent), the websocket tasks
enqueue all high-level messages to the processing task; they listen to any
enqueued messages in the client or ide queue, passing these on via the websocket
connection.

Simplest non-IDE integration: the file watcher.

* On startup, it sends the current file to the CodeChat Editor.
* It uses a file watcher to send update commands when the current file changes.
* It writes a file to disk when it receives an update command.
* It closes the editor if the file is deleted or moved.

Simplest IDE integration:

* On startup, it sends the current file to the CodeChat Editor.
* It sends update commands if edits are made in the IDE, when scrolling, or when
  the active editor changes.
* It updates the IDE contents or opens a new file when it receives a update
  command.

More complex IDE integration: everything that the simple IDE does, plus the
ability to toggle between the IDE's editor and the CodeChat Editor.

Build system
--------------------------------------------------------------------------------

The app needs build support because of complexity:

* The client's NPM libraries need patching and some partial copying.
* After building a release for a platform, client/server binaries must be copied
  to the VSCode extension, then a release published for that platform.

So, this project contains Rust code to automate this process -- see the
[builder](../builder/Cargo.toml).

Misc topics
--------------------------------------------------------------------------------

### <a id="Client-simple-viewer"></a>CodeChat Editor Client Viewer Types

The Client supports several classes of files:

* Source files, rendered as intermingled code/doc blocks.
* Document-only files -- these contain only Markdown and typically have a file
  extension of `.md`.
* Unsupported text files -- the CodeChat Editor cannot edit some files, such as
  miscellaneous text files, unsupported languages, images, video, etc. The
  simple viewer displays (without allowing editing) these files as raw text in
  the browser, though wrapped in the appropriate project structure (with a TOC
  on the left).
* PDFs, where a plugin viewer for VSCode provides rendering, since the built-in
  browser doesn't.

### Broken fences (Markdown challenges)

All Markdown blocks are terminated by a blank line followed by unindented
content, except for fenced code blocks and some types of HTML blocks. To ensure
that doc blocks containing an opening fence but no matching closing fence, or a
start HTML tag but no closing tag, are properly closed (instead of affecting the
remainder of the doc blocks), the editor injects closing tags and fences after
each doc block, then repairs them (if needed, due to a missing closing fence) or
removed them. This means that some HTML tags won't be properly closed, since the
closing tags are removed from the HTML. This is fixed by later HTML processing
steps (currently, by TinyMCE), which properly closes tags.

Future work
--------------------------------------------------------------------------------

### Table of contents

* While the TOC file must be placed in the root of the project, it will be
  served alongside pages served from subdirectories. Therefore, place this in an
  iframe to avoid regenerating it for every page.

* The TOC is just Markdown. Numbered sections are expressed as nested ordered
  lists, with links to each section inside these lists.

* All numbering is stored internally as a number, instead of the corresponding
  marker (I, II, III, etc.). This allows styles to customize numbering easily.

  * Given an `a` element in the TOC, looking through its parents provides the
    section number. Given an array of section numbers, use CSS to style all the
    headings. Implement numbering using CSS variables, which makes it easy for a
    style sheet to include or exclude section numbers:

    `:root {` `--section-counter-reset: s1 4 s2 5;` `--section-counter-content:
    counter(s1, numeric) '-' counter(s2, numeric);` `}`

    `h1::before {` `counter-reset: var(--section-counter-reset);` `content:
    var(--section-counter-content);` `}`

#### Example of non-editable text

<div class="CodeChat-toc mceNonEditable" data-codechat-path="static/css/CodeChatEditor.css" data-codechat-depth="">
<p>asdf</p>
</div>

### Numbering

On a page, the local TOC numbering comes only from heading tags. The CSS (which
would adding numbering to selected headings) must be kept in sync with the code
which infers the numbering. A simple value (4, meaning h1-h4 are numbered) is
all the code needs. How to do this?

On the TOC, numbering may come from both heading tags and ordered lists, again
based on the CSS. Numbered headings should be included in the numbering,
followed by ordered list numbering.

### Settings

The settings should be configurable from a nice GUI. I like the VSCode idea --
make it easy to add more config values. Settings should also be available for
plug-ins. Store the config values in a bare JSON file; provide a web-based GUI
with descriptions of each setting.

* Files/directories to process/ignore
* Header/footer info (name, version, copyright, etc.)
* The programming language, markup language, and spellchecker language for each
  source file.
* Text wrap width when saving.
* Visual styling (theme/style sheets, color, fonts, size of TOC sidebar,
  location of sidebar, etc.).
* HTML `<head>` modifications: CSS/JS to add to all pages/a set of pages.
* Depth of headings to include in the page-local TOC.
* Auto-reload if modified externally or not
* Tabs vs spaces; newline type
* Substitutions

<a id="core-developmnt-priorities"></a>Core development priorities
--------------------------------------------------------------------------------

1. Bug fixes
2. Book support

### <a id="next-steps"></a>Next steps

1. Implement caching for all anchors/headings.
2. Implement author support: TOC, auto-titled links.
3. Implement a good GUI for inserting hyperlinks.
4. Better support for template literals.
5. Decide how to handle nested block comments.
6. Define the architecture for IDE extensions/plug-ins. Goal: minimize
   extension/plug-in complexity.
7. Define desired UI behavior. Priority: auto-reload; dirty document detection;
   auto-backup.
8. Propose visual styling, dark mode, etc.

### To do

1. Open the TOC as a single-file edit? If not, at least hide the sidebar, since
   that's redundant.

### Open questions

* I'd like to be able to wrap a heading and associated content in a `<section>`
  tag. This is hard to do -- if a heading appears in the middle of an indented
  comment, then need special processing (close the section, then the indent,
  then restart a new indent and section). In addition, it requires that code is
  nested inside doc blocks, which is tricky. However, I would like to do this.
* How to handle images/videos/PDFs/etc. when file are moved? Currently, we
  expect the user to move them as well. There's not an easy way to tag them with
  an unique ID, then refer to them using that ID than I can think of.
* Config file format: I really like and prefer Python's strictyaml. Basically, I
  want something that includes type validation and allows comments within the
  config file. Perhaps JSON with a pre-parse step to discard comments then
  [JSON Typedef](https://jsontypedef.com/)? Possibly, vlang can do this
  somewhat, since it wants to decode JSON into a V struct.

Organization
--------------------------------------------------------------------------------

### Client

As shown in the figure below, the CodeChat Editor Client starts with
`client/package.json`, which tells
[NPM](https://en.wikipedia.org/wiki/Npm_(software)) which JavaScript libraries
are used in this project. Running `npm update` copies these libraries and all
their dependencies to the `client/node_modules` directory. The CodeChat Editor
Client source code (see [CodeChatEditor.mts](../client/src/CodeChatEditor.mts))
imports these libraries.

Next, [esbuild](https://esbuild.github.io/) analyzes the CodeChat Editor client
based by transforming any [TypeScript](https://www.typescriptlang.org/) into
JavaScript then packaging all dependencies (JavaScript, CSS, images, etc.) into
a smaller set of files. At a user's request, the CodeChat Editor Server
generates HTML which creates an editor around the user-requested file. This HTML
loads the packaged dependencies to create the CodeChat Editor Client webpage.

```graphviz
digraph {
    JS_lib [label = "JavaScript libraries"]
    "package.json" -> JS_lib [label = "npm update"]
    JS_lib -> esbuild;
    CCE_source [label = "CodeChat Editor\nClient source"]
    JS_lib -> CCE_source [label = "imports"]
    CCE_source -> esbuild
    esbuild -> "Bundled JavaScript"
    esbuild -> "Bundle metadata"
    "Bundle metadata" -> "HashReader.mts"
    "HashReader.mts" -> server_HTML
    CCE_webpage [label = "CodeChat Editor\nClient webpage"]
    "Bundled JavaScript" -> CCE_webpage
    server_HTML [label = "CodeChat Editor\nServer-generated\nHTML"]
    server_HTML -> CCE_webpage
}
```

Note: to edit these diagrams, use an
[HTML entity encoder/decoder](https://mothereff.in/html-entities) and a Graphviz
editor such as [Edotor](https://edotor.net/).

TODO: GUIs using TinyMCE. See the
[how-to guide](https://www.tiny.cloud/docs/tinymce/6/dialog-components/#panel-components).

Code style
--------------------------------------------------------------------------------

JavaScript functions are a
[disaster](https://dmitripavlutin.com/differences-between-arrow-and-regular-functions/).
Therefore, we use only arrow functions for this codebase.

Other than that, follow the
[MDN style guide](https://developer.mozilla.org/en-US/docs/MDN/Writing_guidelines/Writing_style_guide/Code_style_guide/JavaScript).

Client modes
--------------------------------------------------------------------------------

The CodeChat Editor client supports four modes:

* Edit:
  * Document only: just TinyMCE to edit a pure Markdown file.
  * Usual: the usual CodeMirror + TinyMCE editor
* View:
  * For a ReadTheDocs / browsing experience: clicking on links navigates to them
    immediately, instead of bringing up a context menu. Still use CodeMirror for
    syntax highlighting, collapsing, etc.
* <a id="Client-simple-viewer"></a>Simple viewer:
  * For text or binary files that aren't supported by the editor. In project
    mode, this displays the TOC on the left and the file contents in the main
    area; otherwise, it's only the main area. See: \<gather here>.

Misc
--------------------------------------------------------------------------------

Eventually, provide a read-only mode with possible auth (restrict who can view)
using JWTs; see
[one approach](https://auth0.com/blog/build-an-api-in-rust-with-jwt-authentication-using-actix-web/).

A better approach to make macros accessible where they're defined, instead of at
the crate root: see
[SO](https://stackoverflow.com/questions/26731243/how-do-i-use-a-macro-across-module-files/67140319#67140319).
