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

You should have received a copy of the GNU General Public License along with the
CodeChat Editor. If not, see
[http://www.gnu.org/licenses/](http://www.gnu.org/licenses/).

Implementation
==============

### System architecture

<graphviz-graph graph="digraph {
    bgcolor = transparent;
    compound = true;
    node [shape = box];
    subgraph cluster_text_editor {
        label = &quot;Text editor/IDE&quot;
        source_code [label = &quot;Source\ncode&quot;, style = dashed];
        CodeChat_plugin [label = &quot;CodeChat\nEditor plugin&quot;];
    }
    subgraph cluster_server {
        label = <CodeChat Editor Server>;
        websocket_server [label = &quot;Websocket\nserver&quot;];
        web_server [label = &quot;Web\nserver&quot;];
    }
    subgraph cluster_client_framework {
        label = &quot;CodeChat Editor Client framework&quot;
        subgraph cluster_client {
            label = &quot;CodeChat Editor Client\n(Editor/Viewer/Simple Viewer)&quot;
            rendered_code [label = &quot;Rendered code&quot;, style = dashed];
        }
    }
    CodeChat_plugin -> websocket_server [label = &quot;websocket&quot;, dir = both];
    websocket_server -> rendered_code [label = &quot;websocket&quot;, dir = both, lhead = cluster_client_framework];
    web_server -> rendered_code [label = &quot;HTTP&quot;, dir = both, lhead = cluster_client ];
    }"></graphviz-graph>

<a id="an-implementation"></a>Architecture
------------------------------------------

### Client/server partitioning

Doc blocks consist of Markdown augmented with custom HTML elements which provide
authoring support. Some of these elements depend only on information in the
current page. For example, a GraphViz graph tag transforms the graph provided in
its tag into an SVG; it doesn't need information from other files. Other
elements, such as a cross-reference tag, depend on information from other pages
(in this case, the page containing the referenced item). The client lacks the
ability to access other files, while the server has direct access to these
files. Therefore, the overall strategy is:

*   On page load, the server transforms custom tags which depend on information
    from other pages into tags which include this information. For example, a
    cross-reference tag might be transformed into a hyperlink whose link text
    comes from the cross-reference on another page.
*   The client them defines a set of [Web
    Components](https://developer.mozilla.org/en-US/docs/Web/Web_Components)
    which implement custom tags which only need local information. For example,
    a GraphViz custom tag renders graphs based on a description of the graph
    inside the tag.
*   On save, the client sends its text back to the server, which de-transforms
    custom tags which depend on information from other pages. If de-transforms
    disagree with the provided text, then re-load the updated text after the
    save is complete. For example, after inserting an auto-titled link, the
    auto-titled text is missing; a save/reload fixes this.

### Page processing pipeline

On load:

*   Classify the file; input are mutable global state (which, if present,
    indicates this is a project build), if the file is a TOC, the file's binary
    data, and the file's path. Output of the classification: binary, raw text, a
    CodeChat document (a Markdown file), or a CodeChat file. The load processing
    pipelines For CodeChat files:
*   (CodeChat files only) Run pre-parse hooks: they receive source code, file
    metadata. Examples: code formatters. Skip if cache is up to date.
*   (CodeChat files only) Lex the file into code and doc blocks.
*   Run post-parse hooks: they receive an array of code and doc blocks.
    *   Transform Markdown to HTML.
*   Run HTML hooks:
    *   Update the cache for the current file only if the current file's cache
        is stale. To do this, walk the DOM of each doc block. The hook specifies
        which tags it wants, and the tree walker calls the hook when it
        encounters these. If this requires adding/changing anything (anchors,
        for example), mark the document as dirty.
    *   Update tags whose contents depend on data from other files. Hooks work
        the same as the cache updates, but have a different role. They're always
        run, while the cache update is skipped when the cache is current.
*   Determine next/prev/up hyperlinks based on this file's location in the TOC.
*   Transform the code and doc blocks into CodeMirror's format.

We want a clean separate between the webserver and the processing pipeline. The
webserver should provide I/O between the local file system and the client, but
do little processing. The processing pipeline should not perform I/O. Therefore:

*   On load, the webserver receives a request for a file. It should gather
    and pass the following to the page processor:
    *   The loaded file as text (or an Err result).
    *   The global state (empty if this isn't a project build).
    *   The pathname of the file.
    *   If this file should be processed as a TOC or not.
*   The page processor returns:
    *   An Enum with the file's contents:

On save:

*   Transform the CodeMirror format back to code and doc blocks.
*   Run HTML hooks:
    *   Update the cache for the current file. Mark the file as "dirty" (reload
        needed) if any changes are made.
    *   Check tags whose contents depend on data from other files; if the
        contents differ, mark the file as dirty.
    *   Transform HTML to Markdown.
*   Run post-parse hooks; mark the file as dirty if any changes are made.
*   De-lex the file into source code.
*   Run pre-parse hooks; mark the file as dirty if any changes are made.
*   Save the file to disk.
*   If dirty, re-load the file.

#### HTML to Markdown transformation

Currently, Turndown translates HTML to Markdown, then Prettier word-wraps the
result. This has several problems:

*   There are several bugs/open issues in Turndown; however, this package is no
    longer maintained.
*   Turndown doesn't have a good way to deal with raw HTML intermingled with
    Markdown; since raw HTML can change the meaning of HTML through styles, this
    is hard to avoid. But it still produces ugly results.
*   Prettier translates setext-style headings to ATX headings, which I don't
    like.
*   Because both packages are written in Javascript, they run in the browser.
    However, we need to run processing at the HTML level on the server first,
    requiring some round trips between client and sever in the future.

To build Turndown, simply execute `npm run build` or `npm run test`.

### IDE/editor integration

The IDE extension client (IDE for short) and the CodeChat Editor Client (or
Editor for short) exchange messages with each other, mediated by the CodeChat
Server. The Server forwards messages from one client to the other, translating
as necessary (for example, between source code and the Editor format).

### Editor-overlay filesystem

When the Client displays a file provided by the IDE, that file may not exist in
the filesystem (a newly-created document), the IDE's content may be newer than
the filesystem content (an unsaved file), or the file may exist only in the
filesystem (for examples, images referenced by a file). The Client loads files
by sending HTTP requests to the Server with a URL which includes the path to the
desired file. Therefore, the Server must first ask the IDE if it has the
requested file; if so, it must deliver the IDE's file contents; if not, it must
load thee requested file from the filesystem. This process -- fetching from the
IDE if possible, then falling back to the filesystem -- defines the
editor-overlay filesystem.

#### Network interfaces between the Client, Server, and IDE

*   The startup phase loads the Client framework into a browser:\
    ![Startup
    diagram](https://www.plantuml.com/plantuml/svg/PP3HIiOW583lVOfpwIwY-u4ng62ZHR7P0rYUOkJKZcUDthwPiVh_tOZ8vtS-RH8RucLsGYaOV_OHb1BTpIrSNC68z8bKmqD4ZrPs5lLNn4gKyqniO0q3fiMn79ac_xRHTmVYsatekUNPxLIhxti814z3NvtFEmfpNww0PmfhGW9PbF1APiOrqFk9CB_1XH05_8x-Rs-rVWJ2ZmKJoyl4XgUNaW7mrrtkxNIAmIVSSMlOL0Az5Sssv0_y1W00)
*   If the current file in the IDE changes (including the initial startup, when
    the change is from no file to the current file), or a link is followed in
    the Client's iframe:\
    ![Load
    diagram](https://www.plantuml.com/plantuml/svg/hLBDpjCm4BpdAVRO7E01Se1F-W21gA1gMkucte2HOnjxGzMtnpzHGfo8X8eUqiJUdPcTdIT7p5BVoSBuVz48mnJ1XpTlPzyrsbzePqVFKg2YWibO3L8pxg0L4Wk81ozU3IKLFFVM-fTt_l9GanNgMmKdHjz1jz0neLwQU-cxjF5GxT05N3Tz5rw40ouitGk8ltJjuGDB1LV36KsmZLRahrq63R0GTKPdj79u-FmnLA3YHGQUrQ1qE1JCfysQrgQzde-Peh-f2LecqEHz1UylbnDO_DcZeqCEc8f63KUlRoR01Bj9Jms1VmAV-FFEEEIghMNS_V0LjeHS4FiQBKcmqu2Z-e7bJxnKKQvsxTlkqgjikL9h4rEmprNN6wCjUSeqlL3pBEqoUucJceDfzPB08mqvtThCpBnLYct_Do5Ys7EPIjC_Ilsa5PR_GHIqLdVnpTqTOJU8LBn8po1tZAANEMOHcEBnW86n-WSsj3ETUSmsA8Hxc25LG2qwuy6-2BoXBOkjh488wslBRZEHZyxcBNpoZxwJlm40)
*   If the current file's contents in the IDE are edited:\
    ![Edit
    diagram](https://www.plantuml.com/plantuml/svg/XT1DQiCm40NWlKunItlH2tXH36vBInCI_7C09NeE0cNaIEFa-ed1OCVaPp_l6zxBe-WW_T6flwzl-lYa2k6Ca57J6Ir8AWcM3nanBhJtB629gT9EQAqjKsiTo4Q2iQ9t3ef6OA0APy7oXeABkBVOosklw4C0ouzr4zgKA_BjpANnVDxfjwwt573g4ILP9Xw-6XEnynoVDc2Zfb-t6JCgbudDVwfoi1c6lW80)
*   If the current file's contents in the Client are edited, the Client sends
    the IDE an `Update` with the revised contents.
*   When the PC goes to sleep then wakes up, the IDE client and the Editor
    client both reconnect to the websocket URL containing their assigned ID.
*   If the Editor client or the IDE client are closed, they close their
    websocket, which send a `Close` message to the other websocket, causes it to
    also close and ending the session.
*   If the server is stopped (or crashes), both clients shut down after several
    reconnect retries.

Note: to edit these diagrams, paste the URL into the [PlantUML web
server](https://www.plantuml.com/plantuml/uml), click Decode URL, edit, then
copy and paste the SVG URL back to this file.

#### Message IDs

The message system connects the IDE, Server, and Client; all three can serve as
the source or destination for a message. Any message sent should produce a
Response message in return. Therefore, we need globally unique IDs for each
message. To achieve this, the Server uses IDs that are multiples of 3 (0, 3, 6,
...), the Client multiples of 3 + 1 (1, 4, 7, ...) and the IDE multiples of 3 +
2 (2, 5, 8, ...). A double-precision floating point number (the standard
[numeric
type](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Data_structures#number_type)
in JavaScript) has a 53-bit mantissa, meaning IDs won't wrap around for a very
long time.

#### Architecture

Clients always come in pairs: one IDE client is always paired with one CodeChat
Editor client. The server uses a set of queues to decouple websocket protocol
activity from the core processing needed to translate source code between a
CodeChat Editor Client and an IDE client. Specifically, one task handles the
receive and transmit function for the websocket:

*   The task sends a periodic ping to the CodeChat Editor Client or the IDE
    client, then waits for a pong, closing the connection if the pong isn't
    received in a timely manner. This helps detect a broken websocket connection
    produced when a computer is put to sleep then wakes back up.
*   Likewise, the task responds to a ping message from the CodeChat Editor
    Client by sending a pong in response.
*   It tracks messages sent and produces an error message if a sent message
    isn't acknowledged within a timeout window.
*   If the websocket is closed without warning, the websocket stores the
    relevant data so that it can resume when the client reconnects to it.
*   If the websocket is closed purposefully (for example, by closing a CodeChat
    Editor Client tab in a web browser), the receive task detects this and shuts
    down the websocket along with the associated IDE client tasks.

To decouple these low-level websocket details from high-level processing (such
as translating between source code and its web equivalent), the websocket tasks
enqueue all high-level messages to the processing task; they listen to any
enqueued messages in the client or ide queue, passing these on via the websocket
connection. The following diagram illustrates this approach:

<graphviz-graph graph="digraph {
    ccc -> client_task [ label = &quot;websocket&quot; dir = &quot;both&quot; ]
    ccc -> http_task [ label = &quot;HTTP\nrequest/response&quot; dir = &quot;both&quot;]
    client_task -> from_client
    http_task -> http_to_client
    http_to_client -> processing
    processing -> http_from_client
    http_from_client -> http_task
    from_client -> processing
    processing -> to_client
    to_client -> client_task
    ide -> ide_task [ label = &quot;websocket&quot; dir = &quot;both&quot; ]
    ide_task -> from_ide
    from_ide -> processing
    processing -> to_ide
    to_ide -> ide_task
    { rank = same; client_task; http_task }
    { rank = same; to_client; from_client; http_from_client; http_to_client }
    { rank = same; to_ide; from_ide }
    { rank = max; ide }
    ccc [ label = &quot;CodeChat Editor\nClient&quot;]
    client_task [ label = &quot;Client websocket\ntask&quot;]
    http_task [ label = &quot;HTTP endpoint&quot;]
    from_client [ label = &quot;queue from client&quot; shape=&quot;rectangle&quot;]
    processing [ label = &quot;Processing task&quot; ]
    to_client [ label = &quot;queue to client&quot; shape=&quot;rectangle&quot;]
    http_to_client [ label = &quot;http queue to client&quot; shape = &quot;rectangle&quot;]
    http_from_client [ label = &quot;oneshot from client&quot; shape = &quot;box&quot;]
    ide [ label = &quot;CodeChat Editor\nIDE plugin&quot;]
    ide_task [ label = &quot;IDE websocket\ntask&quot; ]
    from_ide [ label = &quot;queue from IDE&quot; shape=&quot;rectangle&quot; ]
    to_ide [ label = &quot;queue to IDE&quot; shape=&quot;rectangle&quot; ]
    }"></graphviz-graph>

The queues use multiple-sender, single receiver (mpsc) types; hence, a single
task in the diagram receives data from a queue, while multiple tasks send data
to a queue. When the IDE processing task writes updated content to the file
being edited, it notifies the file watcher to ignore the next file update (hence
the dotted arrow).

The exception to this pattern is the HTTP endpoint. This endpoint is invoked
with each HTTP request, rather than operating as a single, long-running task. It
sends the request to the processing task using an mpsc queue; this request
includes a one-shot channel which enables the request to return a response to
this specific request instance. The endpoint then returns the provided response.

Simplest non-IDE integration: the file watcher.

*   On startup, it sends the current file to the CodeChat Editor.
*   It uses a file watcher to send update commands when the current file
    changes.
*   It writes a file to disk when it receives an update command.
*   It closes the editor if the file is deleted or moved.

Simplest IDE integration:

*   On startup, it sends the current file to the CodeChat Editor.
*   It sends update commands if edits are made in the IDE, when scrolling, or
    when the active editor changes.
*   It updates the IDE contents or opens a new file when it receives a update
    command.

More complex IDE integration: everything that the simple IDE does, plus the
ability to toggle between the IDE's editor and the CodeChat Editor.

Build system
------------

The app needs build support because of complexity:

*   The client's NPM libraries need patching and some partial copying.
*   After building a release for a platform, client/server binaries must be
    copied to the VSCode extension, then a release published for that platform.

So, this project contains Rust code to automate this process -- see the
[builder](../builder/Cargo.toml).

Future work
-----------

### Table of contents

*   While the TOC file must be placed in the root of the project, it will be
    served alongside pages served from subdirectories. Therefore, place this in
    an iframe to avoid regenerating it for every page.
*   The TOC is just HTML. Numbered sections are expressed as nested ordered
    lists, with links to each section inside these lists.
*   All numbering is stored internally as a number, instead of the
    corresponding marker (I, II, III, etc.). This allows styles to customize
    numbering easily.
    *   Given an `a` element in the TOC, looking through its parents provides
        the section number. Given an array of section numbers, use CSS to style
        all the headings. Implement numbering using CSS variables, which makes
        it easy for a style sheet to include or exclude section numbers:

        `:root {`\
         
        `--section-counter-reset: s1 4 s2 5;`\
         
        `--section-counter-content: counter(s1, numeric) '-' counter(s2,
        numeric);`\
        `}`

        `h1::before {`\
         
        `counter-reset: var(--section-counter-reset);`\
         
        `content: var(--section-counter-content);`\
        `}`

### Cached state

When hydrating a page (transforming all custom tags into standard HTML) on the
server, all needed hydration data should be stored in the cache.

#### Data and format

What we need to know:

*   To generate the TOC, we need a way to find the linked file, then get a
    list of all its headings.
    *   Problem: files can be moved. Better would be an invariant anchor, stored
        in the file, which doesn't change. It would make sense to link to the
        only \<h1> element...but there may not be one, or it may not be at the
        top of the file. The easy solution would be an anchor tag at the
        beginning of the file...but this would break shell scripts, for example.
        Another is including an anchor tag somewhere in each document, but need
        authors to understand what it is (and not delete it). Another
        possibility is to link to any anchor in the file with a special query
        identifying it as link to the underlying file.
*   To auto-title a link, need to look up an anchor and get its location (page
    number, section number) and title.
*   For back links, need to look up all links to the given anchor, then get the
    location and title of each link.
*   To generate the TOC containing all anchors, we need a list of all anchors on
    a given page.

Therefore, the cache must contain a `FileAnchor`, an enum of:

*   A `PlainFileAnchor` (a non-HTML file -- an image, PDF, etc.). Generate an ID
    based on a checksum of the file. Basically, this provides some way to find
    the (unmodified) file if it's moved/renamed. Cache data: |path, ID, file's
    metadata|.
*   An `HtmlFileAnchor` (an HTML file). Store an ID as a comment in it
    somewhere, probably at the end of the file. Cache data: |path, ID,
    file's metadata|, TOC numbering, a vector of `HeadingAnchor`s, a vector
    of `NonHeadingAnchor`s:
    *   A `HeadingAnchor` in an HTML file: |weak ref to the containing
        `HtmlFileAnchor`, ID, anchor's inner HTML, optional hyperlink|,
        numbering on this page, a vector of `NonHeadingAnchors` contained in
        this heading.
    *   A `NonHeadingAnchor` in an HTML file: |weak ref to the containing
        `HtmlFileAnchor`, ID, anchor's inner HTML, optional hyperlink|, optional
        parent heading, snippet of surrounding text, numbering group, number.

A `Hyperlink` consists of a path and ID the link refers to.\
An `HtmlAnchor` is an enum of `HeadingAnchor` and `NonHeadingAnchor`.\
An `Anchor` is an enum of a `FileAnchor` and an `HtmlAnchor`.

Globals:

*   A map of `PathBuf`s to `FileAnchors`.
*   A map of IDs to (`Anchor`, set of IDs of referring links)

How to keep the sets of referring links up to date? If a link is deleted, we
won't know until that file is removed. To fix, add a validate() function that
looks up each link and drops anything that doesn't exist.

How to deal with a link to an anchor not in the cache? Need a stub for that
until the actual anchor is found. Therefore, have a stub/nonexistent file that
holds all missing anchors. This file contains a single non-heading anchor, which
has an empty id.

How to represent TOC numbering?  In the end, it has to becomes a series of
digits. I'd like to allow headings mixed with ordered lists, since this seems a
reasonable/common way to express the TOC. But these could easily be assembled in
a lot of weird ways. If a make a simplifying assumption -- put headings,
followed by ordered lists -- then this is easier. But, what to do if headings
appear in ordered lists? My goal is to produce something sensible from a wide
range of valid inputs. Options are: always treat headings as a higher level of
organization than lists; or treat them equivalently. The second option is
simplest, and would seem to be what the author is saying.

#### Editing and cached state

Edits to the document of any cached items cause the browser to push this edit to
the server; the server then propagates the edits to open windows and updates its
state. If a modified file is closed without saving it (meaning the cached state
for this file is now invalid), then all that cache state must be flushed. To
flush, simply set the date/time stamp of that file to something old/invalid.

### TOC custom tag

Options:

*   Path to linked file
*   Depth of numbering

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

*   Files/directories to process/ignore
*   Header/footer info (name, version, copyright, etc.)
*   The programming language, markup language, and spellchecker language for
    each source file.
*   Text wrap width when saving.
*   Visual styling (theme/style sheets, color, fonts, size of TOC sidebar,
    location of sidebar, etc.).
*   HTML `<head>` modifications: CSS/JS to add to all pages/a set of pages.
*   Depth of headings to include in the page-local TOC.
*   Auto-reload if modified externally or not
*   Tabs vs spaces; newline type
*   Substitutions

### <a id="core-developmnt-priorities"></a>Core development priorities

1.  Bug fixes
2.  Book support

### <a id="next-steps"></a>Next steps

1.  Refactor the webserver to pull out the processing step (converting source
    code to code/doc blocks). Run this in a separate thread -- see the [Tokio
    docs](https://docs.rs/tokio/latest/tokio/#cpu-bound-tasks-and-blocking-code)
    on how to await a task running in another thread.
2.  Implement caching for all anchors/headings.
3.  Implement author support: TOC, auto-titled links.
4.  Implement a good GUI for inserting hyperlinks.
5.  Better support for template literals.
6.  Decide how to handle nested block comments.
7.  Define the architecture for IDE extensions/plug-ins. Goal: minimize
    extension/plug-in complexity.
8.  Define desired UI behavior. Priority: auto-reload; dirty document detection;
    auto-backup.
9.  Propose visual styling, dark mode, etc.

### To do

1.  Open the TOC as a single-file edit? If not, at least hide the sidebar, since
    that's redundant.

### Open questions

*   I'd like to be able to wrap a heading and associated content in a
    `<section>` tag. This is hard to do -- if a heading appears in the middle of
    an indented comment, then need special processing (close the section, then
    the indent, then restart a new indent and section). In addition, it requires
    that code is nested inside doc blocks, which is tricky. However, I would
    like to do this.
*   How to handle images/videos/PDFs/etc. when file are moved? Currently, we
    expect the user to move them as well. There's not an easy way to tag them
    with an unique ID, then refer to them using that ID than I can think of.
*   Config file format: I really like and prefer Python's strictyaml. Basically,
    I want something that includes type validation and allows comments within
    the config file. Perhaps JSON with a pre-parse step to discard comments then
    [JSON Typedef](https://jsontypedef.com/)? Possibly, vlang can do this
    somewhat, since it wants to decode JSON into a V struct.)

Organization
------------

### Client

As shown in the figure below, the CodeChat Editor Client starts with
`client/package.json`, which tells
[NPM](https://en.wikipedia.org/wiki/Npm_\(software\)) which JavaScript libraries
are used in this project. Running `npm update` copies these libraries and all
their dependencies to the `client/node_modules` directory. The CodeChat Editor
Client source code (see [CodeChatEditor.mts](../client/src/CodeChatEditor.mts))
imports these libraries.

Next, [esbuild](https://esbuild.github.io/) analyzes the CodeChat Editor client
based by transforming any [TypeScript](https://www.typescriptlang.org/) into
JavaScript then packaging all dependencies (JavaScript, CSS, images, etc.) into
a smaller set of files. At a user's request, the CodeChat Editor Server
generates HTML which creates an editor around the user-requested file. This HTML
loads the packaged dependencies to create the CodeChat Editor Client webpage.

<graphviz-graph graph="digraph {
    JS_lib [label = &quot;JavaScript libraries&quot;]
    &quot;package.json&quot; -> JS_lib [label = &quot;npm update&quot;]
    JS_lib -> esbuild;
    CCE_source [label = &quot;CodeChat Editor\nClient source&quot;]
    JS_lib -> CCE_source [label = &quot;imports&quot;]
    CCE_source -> esbuild
    esbuild -> &quot;Bundled JavaScript&quot;
    esbuild -> &quot;Bundle metadata&quot;
    &quot;Bundle metadata&quot; -> &quot;HashReader.mts&quot;
    &quot;HashReader.mts&quot; -> server_HTML
    CCE_webpage [label = &quot;CodeChat Editor\nClient webpage&quot;]
    &quot;Bundled JavaScript&quot; -> CCE_webpage
    server_HTML [label = &quot;CodeChat Editor\nServer-generated\nHTML&quot;]
    server_HTML -> CCE_webpage
    }"></graphviz-graph>

Note: to edit these diagrams, use an [HTML entity
encoder/decoder](https://mothereff.in/html-entities) and a Graphviz editor such
as [Edotor](https://edotor.net/).

However, esbuild's code splitting doesn't work with dynamic imports -- the
splitter always picks Node-style default imports, while the Ace editor expects
Babel-style imports.

TODO: GUIs using TinyMCE. See the [how-to
guide](https://www.tiny.cloud/docs/tinymce/6/dialog-components/#panel-components).

Code style
----------

JavaScript functions are a
[disaster](https://dmitripavlutin.com/differences-between-arrow-and-regular-functions/).
Therefore, we use only arrow functions for this codebase.

Other than that, follow the [MDN style
guide](https://developer.mozilla.org/en-US/docs/MDN/Writing_guidelines/Writing_style_guide/Code_style_guide/JavaScript).

Client modes
------------

The CodeChat Editor client supports four modes:

*   Edit:
    *   Document only: just TinyMCE to edit a pure Markdown file.
    *   Usual: the usual CodeMirror + TinyMCE editor
*   View:
    *   For a ReadTheDocs / browsing experience: clicking on links navigates to
        them immediately, instead of bringing up a context menu. Still use
        CodeMirror for syntax highlighting, collapsing, etc.
*   <a id="Client-simple-viewer"></a>Simple viewer:
    *   For text or binary files that aren't supported by the editor. In project
        mode, this displays the TOC on the left and the file contents in the
        main area; otherwise, it's only the main area. See: \<gather here>.

Misc
----

Eventually, provide a read-only mode with possible auth (restrict who can view)
using JWTs; see [one
approach](https://auth0.com/blog/build-an-api-in-rust-with-jwt-authentication-using-actix-web/).

A better approach to make macros accessible where they're defined, instead of at
the crate root: see
[SO](https://stackoverflow.com/questions/26731243/how-do-i-use-a-macro-across-module-files/67140319#67140319).

When using VSCode with Rust, set `"rust-analyzer.cargo.targetDir": true`. See
[this issue](https://github.com/rust-lang/rust-analyzer/issues/17807).