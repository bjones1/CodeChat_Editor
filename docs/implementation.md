Copyright (C) 2023 Bryan A. Jones.

This file is part of the CodeChat Editor.

The CodeChat Editor is free software: you can redistribute it and/or modify it
under the terms of the GNU General Public License as published by the Free
Software Foundation, either version 3 of the License, or (at your option) any
later version.

The CodeChat Editor is distributed in the hope that it will be useful, but
WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more
details.

You should have received a copy of the GNU General Public License along with
the CodeChat Editor. If not, see
[http://www.gnu.org/licenses/](http://www.gnu.org/licenses/).

# Implementation

## <a id="an-implementation"></a>1.4 Architecture

### Doc block markup

- For any markup, must either have:
  - Round-trip capable conversion: from x to HTML (load), then from HTML back
    to x (save).
  - A GUI editor that works on this markup language. I don't know of any
    (except for HTML).
- HTML is simple to implement (already done). However, it's less readable.
- Markdown is very well known, due to GitHub's use of it, and is more readable.
  Anything that can't be translated from HTML from Markdown can simply be left
  as HTML, since Markdown allows HTML as a part of its syntax.

### <span style="color: rgb(0, 0, 0);">Markdown to HTML Conversion Implementation</span>

<span style="color: rgb(0, 0, 0);">Currently, CodeChat only loads and saves doc
blocks in HTML format. This can make a CodeChat-edited script hard to read when
opened in another IDE, due to the HTML markup language being syntax heavy. To
make the output more readable, we propose having CodeChat's main input/output
doc block language to be Markdown rather than HTML. Markdown is a markup
language with lighter syntax that is more intuitive to read.</span>

<span style="color: rgb(0, 0, 0);">Keeping HTML as the markup language may make
the editor harder to use, which could scare away potential users of the
software. We want to mitigate this from occurring by implementing Markdown in
place of HTML.</span>

<p style="padding-left: 40px;"><span style="color: rgb(0, 0, 0);">This implementation will transform files as normal, but instead of the code blocks undergoing the existing pipeline:</span></p>

<p style="padding-left: 80px;"><span style="color: rgb(0, 0, 0);">Load File --&gt; Convert to HTML --&gt; Write Stuff --&gt; Convert to HTML --&gt; Save File,</span></p>

<p style="padding-left: 40px;"><em><span style="color: rgb(0, 0, 0);">we will instead have:</span></em></p>

<p style="padding-left: 80px;">Load File --&gt; <strong>Convert to Markdown </strong>--&gt; Convert to HTML --&gt; Write Stuff --&gt; Convert to HTML --&gt; <strong>Convert to Markdown</strong> --&gt; Save File.</p>

<p style="padding-left: 40px;">To begin, we will be focusing on the <em>first</em> half of the pipeline, converting Markdown to HTML. We can simply convert Markdown to HTML using the pulldown-cmark crate, which should be implemented right after the doc and code blocks have been parsed.</p>

#### CommonMark Info:

- Guide to using Markdown: visit
  [this link](https://www.markdownguide.org/getting-started/)
- Specifications for CommonMark: see [this link](https://spec.commonmark.org/)
- To view CommonMark syntax, visit [this link](https://commonmark.org/help/)

#### Sources and Documentation for pulldown-cmark:

- To download: visit
  [this homepage link](https://crates.io/crates/pulldown-cmark)
- To view documentation for pulldown-cmark, visit
  [this link](https://docs.rs/pulldown-cmark/latest/pulldown_cmark/)
- By default, only CommonMark features are implemented.
  - To use tables, strikethrough text, footnotes, task lists, etc, we must
    enable them in the Options struct. See
    [this link](https://docs.rs/pulldown-cmark/latest/pulldown_cmark/struct.Options.html)
    for details

Once completed, doc blocks written in Markdown can be converted by the editor
into HTML for further use. Then, the second half of the pipeline can be
implemented.

### Lexing

TODO: describe the lexer implementation. Link to the walkthrough, discuss the
language specification, etc.

### Client/server partitioning

Code blocks consist of standard HTML augmented with custom HTML elements which
provide authoring support. Some of these elements depend only on information in
the current page. For example, a GraphViz graph tag transforms the graph
provided in its tag into an SVG; it doesn't  need information from other files.
Other elements, such as a cross-reference tag, depend on information from other
pages (in this case, the page containing the referenced item). The client lacks
the ability to access other files, while the server has direct access to these
files. Therefore, the overall strategy is:

- On page load, the server transforms custom tags which depend on information
  from other pages into tags which include this information. For example, a
  cross-reference tag might be transformed into a hyperlink whose link text
  comes from the cross-reference on another page.
- The client them defines a set of
  [Web Components](https://developer.mozilla.org/en-US/docs/Web/Web_Components)
  which implement custom tags which only need local information. For example, a
  GraphViz custom tag renders graphs based on a description of the graph inside
  the tag.
- The client sends edits (including creating a tag or deleting it) to tags
  which depend on server-side information to the server for transformation.
- On save, the client sends its text back to the server, which de-transforms
  custom tags which depend on information from other pages.

Page processing pipeline

On load:

- Server:
  - Run pre-parse hooks: they receive source code, file metadata. Examples:
    code formatters. Skip if cache is up to date.
  - Parse the file into code and doc blocks.
  - Run post-parse hooks: they receive an array of code and doc blocks. I can't
    think of any sort of processing to do here, though.
  - Update the cache for the current file only if the current file's cache is
    stale. To do this, walk the DOM of each doc block. The hook specifies which
    tags it wants, and the tree walked calls the hook when it encounters these.
  - Update tags whose content depend on data from other files. Hooks work the
    same as the cache updates, but have a different role. They're always run,
    while the cache update is skipped when the cache is current.
  - Determine next/prev/up hyperlinks based on this file's location in the TOC.
- Client:
  - Any edits to items with a specific class are sent to the server for
    processing/rendering.

On save: the same process in reverse.

### Table of contents

- While the TOC file must be placed in the root of the project, it will be
  served alongside pages served from subdirectories. What's the best approach?
  An iframe; otherwise, need to rewrite all URLs (images, links, etc.) which
  sounds hard.
- The TOC is just HTML. Numbered sections are expressed as nested ordered
  lists, with links to each section inside these lists.
- All numbering is stored internally as a number, instead of the corresponding
  marker (I, II, III, etc.). This allows styles to customize numbering easily.
  In addition, while JavaScript can find the index of each item in an ordered
  list, but it can't get the actual marker used (Roman numbers, bullets, or
  things generated by
  [list-style-type](https://developer.mozilla.org/en-US/docs/Web/CSS/list-style-type)).
  There's a
  CSS [::marker](https://developer.mozilla.org/en-US/docs/Web/CSS/::marker)
  selector, but not way to get the rendered text. Even
  [innerText](https://developer.mozilla.org/en-US/docs/Web/API/HTMLElement/innerText)
  doesn't include the marker in the resulting text.
- Old notes:
  - Then, `document.querySelector('[href="https://example.com"]')` finds the
    first instance of the current page's link, which takes care of scrolling
    the TOC.
  - Given the a element in the TOC, looking through its parents provides the
    section number. Given an array of section numbers, use CSS to style all the
    headings. One approach, which makes it easy for a style sheet to include or
    exclude section numbers, by making them CSS variables:\

    `:root {`\
      `--section-counter-reset: s1 4 s2 5;`\
     
    `--section-counter-content: counter(s1, numeric) '-' counter(s2, numeric);`\
    `}`

    `h1::before {`\
      `counter-reset: var(--section-counter-reset);`\
      `content: var(--section-counter-content);`\
    `}`

  - Plan:
    1.  Implement a project finder -- starting at the current directory, ascend
        to the root looking for the project file. If so, return a web page
        which includes the TOC as a sidebar plus some navigation (prev/next/up)
        placeholders. For prev/next, use this:\
        `t = document.createTreeWalker(document.body, NodeFilter.SHOW_ELEMENT, {` \
          `acceptNode(node) {`\
            `return node.nodeName === "A" ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_SKIP;`\
          `}` \
        `});`\
        `t.currentNode = <link corresponding to current page>`\
        `next = t.nextNode();`
    2.  Implement TOC scroll (on the client -- easy there). This means finding
        the first hyperlink to the current page. Given that, it's fairly easy
        to determine prev/next/up and section numbers. Implement all these.

### <a id="combined-code-document-editor"></a>Combined code/document editor

- Need to find some way to allow syntax highlighting, code folding, etc. to
  work across code blocks. Need to have just the code blocks, along with a way
  to map these code back to the resulting document.

### Cached state

When hydrating a page (transforming all custom tags into standard HTML) on the
server, all needed hydration data should be stored in the cache.

#### Data and format

- For each file (including anything linkable -- images, videos, etc.), stored
  as the relative path from the project's root directory to that file:
  - A time/date stamp to determine if this cached data is stale or not.
  - The global TOC numbering.
  - A nested list of headings, represented by their anchor. For headings
    without an anchor, assign one.
- For each anchor:
  - The page it's in, as a path to the file containing this page.
  - The outer HTML of the item it refers to (self/sibling/etc.), for use with
    references.
  - Its numbering within the current page, also for use with references.
  - A list of referring links, represented by their anchor. For links without
    an anchor, assign one.

#### Editing and cached state

Edits to the document of any cached items cause the browser to push this edit
to the server; the server then propagates the edits to open windows and updates
its state. If a modified file is closed without saving it (meaning the cached
state for this file is now invalid), then all that cache state must be flushed.
To flush, simply set the date/time stamp of that file to something old/invalid.

### TOC custom tag

Options:

- Path to linked file
- Depth of numbering

#### Example

<div class="CodeChat-toc mceNonEditable" data-codechat-path="static/css/CodeChatEditor.css" data-codechat-depth=""><p>asdf</p></div>

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

- Files/directories to process/ignore
- Header/footer info (name, version, copyright, etc.)
- The programming language, markup language, and spellchecker language for each
  source file.
- Text wrap width when saving.
- Visual styling (theme/style sheets, color, fonts, size of TOC sidebar,
  location of sidebar, etc.).
- HTML `<head>` modifications: CSS/JS to add to all pages/a set of pages.
- Depth of headings to include in the page-local TOC.
- Auto-reload if modified externally or not
- Tabs vs spaces; newline type
- Substitutions

### <a id="core-developmnt-priorities"></a>Core development priorities

1.  IDE integration
2.  Editor functionality

### <a id="next-steps"></a>Next steps

1.  Implement Markdown. Use
    [pulldown-cmark](https://docs.rs/pulldown-cmark/latest/pulldown_cmark/) to
    transform Markdown to HTML.
    [Turndown](https://github.com/mixmark-io/turndown) has all the features we
    want, but is written in JavaScript.
    [html2md](https://crates.io/crates/html2md) is written in Rust, but doesn't
    support CommonMark or have the feature set we need. Another option is to
    use [Pandoc](https://pandoc.org).
2.  Refactor the webserver to pull out the processing step (converting source
    code to code/doc blocks). Run this in a separate thread -- see the
    [Tokio docs](https://docs.rs/tokio/latest/tokio/#cpu-bound-tasks-and-blocking-code)
    on how to await a task running in another thread.
3.  Implement caching for all anchors/headings.
4.  Implement author support: TOC, auto-titled links.
5.  Implement a good GUI for inserting hyperlinks.
6.  Better support for template literals.
7.  Decide how to handle nested block comments.
8.  Define the architecture for IDE extensions/plug-ins. Goal: minimize
    extension/plug-in complexity.
9.  Define desired UI behavior. Priority: auto-reload; dirty document
    detection; auto-backup.
10. Implement Markdown support.
11. Propose visual styling, dark mode, etc.

### To do

1.  Improve accessibility -- use a `<main>` tag, `<nav>` tags, etc.
2.  Open the TOC as a single-file edit? If not, at least hide the sidebar,
    since that's redundant.

### Open questions

- I'd like to be able to wrap a heading and associated content in a `<section>`
  tag. This is hard to do -- if a heading appears in the middle of an indented
  comment, then need special processing (close the section, then the indent,
  then restart a new indent and section). In addition, it requires that code is
  nested inside doc blocks, which is tricky. However, I would like to do this.
- How to handle images/videos/PDFs/etc. when file are moved? Currently, we
  expect the user to move them as well. There's not an easy way to tag them
  with an unique ID, then refer to them using that ID than I can think of.
- Config file format: I really like and prefer Python's strictyaml. Basically,
  I want something that includes type validation and allows comments within the
  config file. Perhaps JSON with a pre-parse step to discard comments then
  [JSON Typedef](https://jsontypedef.com/)? Possibly, vlang can do this
  somewhat, since it wants to decode JSON into a V struct.)

## 1.5 Organization

### Client

As shown in the figure below, the CodeChat Editor client starts with
`client/webpack/package.json`, which tells
[NPM](<https://en.wikipedia.org/wiki/Npm_(software)>) which JavaScript
libraries are used in this project. Running `npm update` copies these libraries
and all their dependencies to the `client/webpack/node_modules` directory. The
CodeChat Editor client source code (see
[CodeChat-editor.mts](../client/webpack/CodeChat-editor.mts)) imports these
libraries.

Next, [esbuild](https://esbuild.github.io/) analyzes the CodeChat Editor client
based by transforming any [TypeScript](https://www.typescriptlang.org/) into
JavaScript then packaging all dependencies (JavaScript, CSS, images, etc.) into
a smaller set of files. At a user's request, the CodeChat Editor server
generates HTML which creates an editor around the user-requested file. This
HTML loads the packaged dependencies to create the CodeChat Editor webpage.

<graphviz-graph graph="digraph {
    JS_lib [label = &quot;JavaScript libraries&quot;];
    &quot;package.json&quot; -&gt; JS_lib [label = &quot;npm update&quot;];
    JS_lib -&gt; esbuild;
    CCE_source [label = &quot;CodeChat Editor\nclient-side source&quot;];
    JS_lib -&gt; CCE_source [label = &quot;imports&quot;];
    CCE_source -&gt; esbuild;
    esbuild -&gt; &quot;Packaged JavaScript&quot;;
    CCE_webpage [label = &quot;CodeChat Editor\nwebpage&quot;];
    &quot;Packaged JavaScript&quot; -&gt; CCE_webpage;
    server_HTML [label = &quot;CodeChat Editor\nserver-generated\nHTML&quot;];
    server_HTML -&gt; CCE_webpage;
}"></graphviz-graph>

However, esbuild's code splitting doesn't work with dynamic imports -- the
splitter always picks Node-style default imports, while the Ace editor expects
Babel-style imports.

TODO: GUIs using TinyMCE. See the
[how-to guide](https://www.tiny.cloud/docs/tinymce/6/dialog-components/#panel-components).

### System architecture

<graphviz-graph graph="digraph {
    bgcolor = transparent;
    compound = true;
    node [shape = box];
    subgraph cluster_text_editor {
        label = &quot;Text editor/IDE&quot;;
        source_code [label = &quot;Source\ncode&quot;, style = dashed];
        CodeChat_plugin [label = &quot;CodeChat\nEditor plugin&quot;];
    }
    subgraph cluster_server {
        label = &lt;CodeChat Editor Server&gt;;
        websocket_server [label = &quot;Websocket\nserver&quot;];
        web_server [label = &quot;Web\nserver&quot;];
    }
    subgraph cluster_client {
        label = &quot;CodeChat Editor Client&quot;;
        rendered_code [label = &quot;Rendered code&quot;, style = dashed];
        JavaScript;
    }
    CodeChat_plugin -&gt; websocket_server [dir = both];
    websocket_server -&gt; JavaScript [dir = both];
    web_server -&gt; JavaScript [label = &quot;HTTP&quot;, dir = both, lhead = cluster_client];
}"></graphviz-graph>

## Code style

JavaScript functions are a
[disaster](https://dmitripavlutin.com/differences-between-arrow-and-regular-functions/).
Therefore, we use only arrow functions for this codebase.

Other than that, follow the
[MDN style guide](https://developer.mozilla.org/en-US/docs/MDN/Writing_guidelines/Writing_style_guide/Code_style_guide/JavaScript).
