// <details>
//     <summary>Copyright (C) 2012-2022 Bryan A. Jones.</summary>
//     <p>This file is part of CodeChat.</p>
//     <p>CodeChat is free software: you can redistribute it and/or modify it
//         under the terms of the GNU General Public License as published by the
//         Free Software Foundation, either version 3 of the License, or (at
//         your option) any later version.</p>
//     <p>CodeChat is distributed in the hope that it will be useful, but
//         WITHOUT ANY WARRANTY; without even the implied warranty of
//         MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
//         General Public License for more details.</p>
//     <p>You should have received a copy of the GNU General Public License
//         along with CodeChat. If not, see <a
//             href="http://www.gnu.org/licenses/">http://www.gnu.org/licenses/</a>.
//     </p>
// </details>
// <h1><code>CodeChatEditor.js</code> &mdash; <strong>JavaScrip</strong>t which
//     implements the client-side portion of the CodeChat Editor</h1>
// <p>The CodeChat Editor provides a simple IDE which allows editing of mixed
//     code and doc blocks.</p>
// <h2>Next steps</h2>
// <ul>
//     <li>Create a new repo or directory for the CodeChat Editor, with NPM and
//         webpack set up. Use TypeScript.</li>
// </ul>
// <h2>Thoughts and ideas</h2>
// <p>Need to write some components:</p>
// <ul>
//     <li>Autotitle: like an a, but takes the link&rsquo;s name from element
//         linked to. Same for figure references, etc.</li>
//     <li>A index tool -- provides links to all instances of the given term,
//         plus a index page with all these terms.</li>
//     <li>Insert the name of the file.</li>
// </ul>
"use strict";

// <h2>UI</h2>
// <h3>DOM ready event</h3>
// <p>This is copied from <a
//         href="https://developer.mozilla.org/en-US/docs/Web/API/Document/DOMContentLoaded_event#checking_whether_loading_is_already_complete">MDN</a>.
// </p>
const on_dom_content_loaded = on_load_func => {
    if (document.readyState === "loading") {
        // <p>Loading hasn't finished yet.</p>
        document.addEventListener("DOMContentLoaded", on_load_func);
    } else {
        // <p><code>DOMContentLoaded</code> has already fired.</p>
        on_load_func();
    }
}


// <p>Emulate an enum. <a
//         href="https://www.30secondsofcode.org/articles/s/javascript-enum">This</a>
//     seems like a simple-enough approach; see also <a
//         href="https://masteringjs.io/tutorials/fundamentals/enum">JavaScript
//         Enums</a> for other options.</p>
const EditorMode = Object.freeze({
    // <p>Display the source code using CodeChat, but disallow editing.</p>
    view: 0,
    // <p>The full CodeChat editor.</p>
    edit: 1,
    // <p>Show only raw source code; ignore doc blocks, treating them also as
    //     code.</p>
    raw: 2
});


// <p>Load code when the DOM is ready.</p>
const page_init = (source_code, ext) => {
    // <p>Get the mode from the page's query parameters. Default to edit using
    //     the <a
    //         href="https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Operators/Nullish_coalescing_operator">nullish
    //         coalescing operator</a>.</p>
    const urlParams = new URLSearchParams(window.location.search);
    const mode = EditorMode[urlParams.get("mode")] ?? EditorMode.edit;
    on_dom_content_loaded(() => open_lp(source_code, ext, mode));
};


// <p>This code instantiates editors/viewers for code and doc blocks.</p>
const make_editors = (
    // <p>A instance of the <code>EditorMode</code> enum.</p>
    editorMode
) => {
    // <p>In view mode, don't use TinyMCE, since we already have HTML. Raw mode
    //     doesn't use TinyMCE at all, or even render doc blocks as HTML.</p>
    if (editorMode === EditorMode.edit) {
        // <p>Instantiate the TinyMCE editor for doc blocks.</p>
        tinymce.init({
            // <p>See the <a
            //         href="https://www.tiny.cloud/docs/ui-components/contextmenu/">contextmenu
            //         docs</a> for the default value. TODO: this doesn't work!
            // </p>
            contextmenu: "align | forecolor backcolor | bold italic underline superscript subscript codeformat | image link lists table",
            // <p>Place the Tiny MCE menu bar at the top of the screen;
            //     otherwise, it floats in front of text, sometimes obscuring
            //     what the user wants to edit. See the <a
            //         href="https://www.tiny.cloud/docs/configure/editor-appearance/#fixed_toolbar_container">docs</a>.
            // </p>
            fixed_toolbar_container: "#CodeChat-menu",
            inline: true,
            // <p>I would like to add to this: noneditable paste textpattern</p>
            plugins: 'advlist anchor charmap directionality emoticons help image link lists media nonbreaking pagebreak quickbars searchreplace table visualblocks visualchars',
            // <p>When true, this still prevents hyperlinks to anchors on the
            //     current page from working correctly. There's an onClick
            //     handler that prevents links in the current page from working
            //     -- need to look into this. See also <a
            //         href="https://github.com/tinymce/tinymce/issues/3836">a
            //         related GitHub issue</a>.</p>
            //readonly: true,
            relative_urls: true,
            selector: '.CodeChat-TinyMCE',
            // <p>This combines the <a
            //         href="https://www.tiny.cloud/blog/tinymce-toolbar/">default
            //         TinyMCE toolbar buttons</a> with a few more from plugins.
            // </p>
            toolbar: 'undo redo | styleselect | bold italic | alignleft aligncenter alignright alignjustify | outdent indent | numlist bullist | ltr rtl | help',

            // <h3>Settings for plugins</h3>
            // <h4><a
            //         href="https://www.tiny.cloud/docs/plugins/opensource/image/">Image</a>
            // </h4>
            image_caption: true,
            image_advtab: true,
            image_title: true,
        });
    }

    // <p>The CodeChat Document Editor doesn't include ACE.</p>
    if (window.ace !== undefined) {
        // <p>Instantiate the Ace editor for code blocks.</p>
        ace.config.set('basePath', 'https://cdnjs.cloudflare.com/ajax/libs/ace/1.9.5');
        for (const ace_tag of document.querySelectorAll(".CodeChat-ACE")) {
            ace.edit(ace_tag, {
                // <p>The leading <code>+</code> converts the line number from a
                //     string (since all HTML attributes are strings) to a
                //     number.</p>
                firstLineNumber: +ace_tag.getAttribute("data-CodeChat-firstLineNumber"),
                // <p>This is distracting, since it highlights one line for each
                //     ACE editor instance on the screen. Better: only show this
                //     if the editor has focus.</p>
                highlightActiveLine: false,
                highlightGutterLine: false,
                maxLines: 1e10,
                mode: `ace/mode/${current_language_lexer[0]}`,
                // <p>TODO: this still allows cursor movement. Need something
                //     that doesn't show an edit cursor / can't be selected;
                //     arrow keys should scroll the display, not move the cursor
                //     around in the editor.</p>
                readOnly: editorMode === EditorMode.view,
                showPrintMargin: false,
                theme: "ace/theme/textmate",
                wrap: true,
            });
        }
    }

    // <p>Set up for editing the indent of doc blocks.</p>
    for (const td of document.querySelectorAll(".CodeChat-doc-indent")) {
        td.addEventListener("beforeinput", doc_block_indent_on_before_input);
        td.addEventListener("input", doc_block_indent_on_input);
    }
};


// <p>After an edit, the editor by default changes some non-breaking spaces into
//     normal spaces. Undo this, since it breaks the layout. This is because
//     normal spaces wrap, while non-breaking spaces don't; we need no wrapping
//     to correctly set the indent.</p>
const doc_block_indent_on_input = event => {
    // <p>Save the current cursor position. Setting <code>innerHTML</code> loses
    //     it.</p>
    const offset = window.getSelection().anchorOffset;
    // <p>Restore the current cursor position -- an offset into the text node
    //     inside this <code>&lt;tr&gt; element.</code></p>
    window.getSelection().setBaseAndExtent(event.currentTarget.childNodes[0], offset, event.currentTarget.childNodes[0], offset);
}


// <p>Store the lexer info for the currently-loaded language.</p>
let current_language_lexer;


// <h3>Doc block indent editor</h3>
// <p>Allow only spaces and delete/backspaces when editing the indent of a doc
//     block.</p>
const doc_block_indent_on_before_input = event => {
    // <p>Only modify the behavior of inserts.</p>
    if (event.data) {
        // <p>Block any insert that's not an insert of spaces. TODO: need to
        //     support tabs.</p>
        if (event.data !== " ".repeat(event.data.length)) {
            event.preventDefault();
        }
    }
}


const open_lp = (source_code, extension, mode) => {
    // <p>See if the first line of the file specifies a lexer.</p>
    const m = source_code.match(/^.*CodeChat-lexer:\s*(\w+)/);
    const lexer_name = m ? m[1] : "";
    let found = false;
    for (current_language_lexer of language_lexers) {
        // <p>If the source code provided a lexer name, match only on that;
        //     otherwise, match based on file extension.</p>
        if ((current_language_lexer[0] === lexer_name) || (!lexer_name && current_language_lexer[1].includes(extension))) {
            found = true;
            break;
        }
    }
    console.assert(found, "Unable to determine which lexer to use for this language.");
    const classified_lines = source_lexer(source_code, ...current_language_lexer);
    const html = classified_source_to_html(classified_lines);

    document.getElementById("CodeChat-body").innerHTML = html;
    // <p>Initialize editors for this new content.</p>
    make_editors(mode);
};


const on_save_as = async on_save_func => {
    // <p>TODO!</p>
    msg = "Save as is not implemented.";
    window.alert(msg);
    throw msg;
};


// <p>Save CodeChat Editor contents.</p>
const on_save = async () => {
    // <p>Pick an inline comment from the current lexer. TODO: support block
    //     comments (CSS, for example, doesn't allow inline comment).</p>
    const inline_comment = current_language_lexer[2][0];
    // <p>This is the data to write &mdash; the source code.</p>
    const source_code = editor_to_source_code(inline_comment);
    await save(source_code);
};


// <p>Per <a
//         href="https://developer.mozilla.org/en-US/docs/Web/API/Navigator/platform#examples">MDN</a>,
//     here's the least bad way to choose between the control key and the
//     command key.</p>
const os_is_osx = (navigator.platform.indexOf("Mac") === 0 || navigator.platform === "iPhone") ? true : false;


// <p>Provide a shortcut of ctrl-s (or command-s) to save the current file.</p>
const on_keydown = (event) => {
    if ((event.key === "s") && ((event.ctrlKey && !os_is_osx) || (event.metaKey && os_is_osx)) && !event.altKey) {
        on_save();
        event.preventDefault();
    }
}


// <p><a id="save"></a>Save the provided contents back to the filesystem, by
//     sending a <code>PUT</code> request to the server. See the <a
//         href="CodeChatEditorServer.v.html#save_file">save_file endpoint</a>.
// </p>
const save = async contents => {
    let response;
    try {
        response = await window.fetch(window.location, {
            method: "PUT",
            body: contents,
        });
    } catch (error) {
        window.alert(`Save failed -- ${error}.`);
        return;
    }
    if (response.ok) {
        const response_body = await response.json()
        if (response_body.success !== true) {
            window.alert("Save failed.");
        }
        return;
    }
    window.alert(`Save failed -- server returned ${response.status}, ${response.statusText}.`);
};


// <h2>Load editor contents from source code</h2>
// <p>This process is split between two functions: first, <a
//         href="#source_lexer">split the source code into code blocks and doc
//         blocks</a>; next, <a href="#classified_source_to_html">transform this
//         into its web-editable form</a>.</p>
// <p>Both the load and save routines need information about the programming
//     language in order to load/save code in that language.</p>
const language_lexers = [
    // <dl>
    //     <dt>IC</dt>
    //     <dd>inline comment</dd>
    //     <dt>Heredoc</dt>
    //     <dd>Here document: an array of <code>[start prefix string, start
    //             body regex, start suffix string, stop prefix string, stop
    //             suffix string]</code>.</dd>
    //     <dt>JS tmpl lit</dt>
    //     <dd>JavaScript template literal: 0 = Language is not JavaScript, 1 =
    //         Language is JavaScript. (2 = inside a template literal should
    //         only be used by the lexer itself).</dd>
    // </dl>
    // <p>C++11 or newer. Don't worry about supporting C or older C++ using
    //     another lexer entry, since the raw string syntax in C++11 and newer
    //     is IMHO so rare we won't encounter it in older code. See the <a
    //         href="https://en.cppreference.com/w/cpp/language/string_literal">C++
    //         string literals docs</a> for the reasoning behind the start body
    //     regex.</p>
    //Language name File extensions     IC      Block comment       Long string     Short str   Heredoc JS tmpl lit
    ["c_cpp",       [".cc", ".cpp"],    ["//"], [["/*", "*/"]],     [],             ['"'],      [['R"', "[^()\\ ]", "(", ")", ""]], 0],
    ["html",        [".html"],          [],     [["<!--", "-->"]],  [],             [],         [],     0],
    ["javascript",  [".js"],            ["//"], [["/*", "*/"]],     [],             ['"', "'"], [],     1],
    ["python",      [".py"],            ["#"],  [],                 ['"""', "'''"], ['"', "'"], [],     0],
    ["verilog",     [".v"],             ["//"], [["/*", "*/"]],     [],             ['"'],      [],     0],
    ["vlang",       [".v"],             ["//"], [["/*", "*/"]],     [],             ['"', "'"], [],     0],
    ["codechat-html", [".cchtml"],      [""],   [],                 [],             [],         [],     0],
];


// <h2>Source lexer</h2>
// <p>Rather than attempt to lex the entire language, this lexer's only goal is
//     to categorize all the source code into code blocks or doc blocks. To do
//     it, it only needs to:</p>
// <ul>
//     <li>Recognize where comments can't be&mdash;inside strings, <a
//             href="https://en.wikipedia.org/wiki/Here_document">here text</a>,
//         or <a
//             href="https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Template_literals">template
//             literals</a>. These are always part of a code block and can never
//         contain a comment or (by implication) a doc block.</li>
//     <li>Outside of these special cases, look for inline or block comments,
//         categorizing everything else as code.</li>
//     <li>After finding either an inline or block comment, determine if this is
//         a doc block.</li>
// </ul>
// <p>It returns a list of <code>indent, string, indent_type</code> where:</p>
// <dl>
//     <dt><code>indent</code></dt>
//     <dd>The indent of a doc block (a string of whitespace), or
//         <code>null</code> for a code block.</dd>
//     <dt><code>string</code></dt>
//     <dd>The classified string; for doc blocks, this does not include the
//         indenting spaces or the inline/block comment prefix/suffix</dd>
//     <dt><code>indent_type</code></dt>
//     <dd>The comment string for a doc block, or "" for a code block.</dd>
// </dl>
const source_lexer = (
    source_code,
    language_name,
    extension_strings,
    inline_comment_strings,
    block_comment_strings,
    long_string_strings,
    short_string_strings,
    here_text_strings,
    template_literals,
) => {
    // <p>A special case -- CodeChat documents are already a single doc block.
    //     Return that.</p>
    if (language_name === "codechat-html") {
        return [["", source_code, ""]];
    }

    // <p>Construct <a
    //         href="https://developer.mozilla.org/en-US/docs/Web/JavaScript/Guide/Regular_Expressions">regex</a>
    //     and associated indices from language information provided.
    //     <strong>This code makes heavy use of regexes -- read the previous
    //         link thoroughly.</strong></p>
    let regex_index = 1;
    let regex_strings = [];
    const regex_builder = (strings) => {
        // <p>Look for a non-empty array. Note that <code>[]</code> is
        //     <code>true</code>.</p>
        if (strings.length) {
            regex_strings.push(
                // <p>Escape any regex characters in these strings.</p>
                strings.map(escapeRegExp).join("|")
            );
            return regex_index++;
        }
        return null;
    }
    // <p>Order these by length of the expected strings, since the regex with an
    //     or expression will match left to right.</p>
    // <p>Include only the opening block comment string (element 0) in the
    //     regex.</p>
    let block_comment_index = regex_builder(block_comment_strings.map(element => element[0]));
    let long_string_index = regex_builder(long_string_strings);
    let inline_comment_index = regex_builder(inline_comment_strings);
    let short_string_index = regex_builder(short_string_strings);
    // <p>Template literals only exist in JavaScript. No other language (that I
    //     know of) allows comments inside these, or nesting of template
    //     literals.</p>
    let template_literal_index = null;
    if (template_literals) {
        // <p>If inside a template literal, look for a nested template literal
        //     (<code>`</code>) or the end of the current expression
        //     (<code>}</code>).</p>
        regex_strings.push(template_literals === 1 ? "`" : "`|}");
        template_literal_index = regex_index++;
    }
    let classify_regex = new RegExp("(" + regex_strings.join(")|(") + ")");

    let classified_source = [];
    // <p>An accumulating array of strings composing the current code block.</p>
    let code_block_array = [];
    while (source_code.length) {
        // <p>Look for either a comment or a no-comment zone.</p>
        const m = source_code.match(classify_regex);
        if (m) {
            // <p>Add everything preceding this match to the current code block.
            // </p>
            code_block_array.push(source_code.substring(0, m.index));
            source_code = source_code.substring(m.index);
            // <p>Figure out which matched.</p>
            if (inline_comment_index && m[inline_comment_index]) {
                // <p>A comment matched.</p>
                const inline_comment_string = m[inline_comment_index];
                // <p>Look at the last line of code by examining the code block
                //     being accumulated.</p>
                let code_block = code_block_array.join("");
                const split_lines = code_block.split(/\n|\r\n|\r/)
                // <p>If there's no matching newline, we're at the beginning of
                //     the uncategorized source code.</p>
                const last_line = split_lines ? split_lines[split_lines.length - 1] : "";

                // <p>Find the end of this comment. No matching newline means
                //     we're at the end of the file. Note that using a negative
                //     lookbehind assertion would make this much simpler:
                //     <code>/(?&lt;!\\)(\n|\r\n|\r)/</code>. However, V doesn't
                //     support this.</p>
                const inline_m = source_code.match(/(\\\r\n|\\\n|\\\r|[^\\\n\r])*(\n|\r\n|\r)/);
                const full_comment = inline_m ? source_code.substring(0, inline_m.index + inline_m[0].length) : source_code;

                // <p>Criteria for doc blocks for an inline comment:</p>
                // <ul>
                //     <li>All characters preceding the comment on the current
                //         line must be whitespace.</li>
                //     <li>Either:
                //         <ul>
                //             <li>The comment is immediately followed by a
                //                 space, or</li>
                //             <li>the comment is followed by a newline or the
                //                 end of file.</li>
                //         </ul>
                //     </li>
                // </ul>
                // <p>Doc block comments have a space after the comment string
                //     or are empty, and only spaces before the comment.</p>
                if ((full_comment.startsWith(inline_comment_string + " ") || full_comment === inline_comment_string + (inline_m ? inline_m[1] : "")) && last_line.match(/^\s*$/)) {
                    // <p>Transition from a code block to this doc block.</p>
                    code_block = code_block.substring(0, code_block.length - last_line.length)
                    if (code_block) {
                        // <p>Save only code blocks with some content.</p>
                        classified_source.push([null, code_block, ""]);
                    }
                    code_block_array = [];
                    // <p>Add this doc block.</p>
                    const has_space_after_comment = full_comment[inline_comment_string.length] === " ";
                    classified_source.push([last_line, full_comment.substring(inline_comment_string.length + (has_space_after_comment ? 1 : 0)), inline_comment_string]);
                } else {
                    // <p>This is still code.</p>
                    code_block_array.push(full_comment);
                }
                // <p>Move to the next block of source code to be lexed.</p>
                source_code = source_code.substring(full_comment.length);
            } else if (block_comment_index && m[block_comment_index]) {
                // <p>TODO!</p>
                const msg = "Block comments not implemented.";
                window.alert(msg);
                throw msg;
            } else if (long_string_index && m[long_string_index]) {
                // <p>A long string. Find the end of it.</p>
                code_block_array.push(m[long_string_index]);
                source_code = source_code.substring(m[long_string_index].length);
                const string_m = source_code.match(m[long_string_index]);
                // <p>Add this to the code block, then move forward. If it's not
                //     found, the quote wasn't properly closed; add the rest of
                //     the code.</p>
                if (string_m) {
                    const index = string_m.index + string_m[0].length;
                    code_block_array.push(source_code.substring(0, index));
                    source_code = source_code.substring(index);
                } else {
                    code_block_array.push(source_code);
                    source_code = "";
                }
            } else if (short_string_index && m[short_string_index]) {
                // <p>A short string. Find the end of it.</p>
                code_block_array.push(m[short_string_index]);
                source_code = source_code.substring(m[short_string_index].length);
                const string_m = source_code.match(
                    // <p>Use <a
                    //         href="https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/String/raw"><code>String.raw</code></a>
                    //     so we don't have to double the number of backslashes
                    //     in this regex. Joining regex literals doesn't work
                    //     &ndash; <code>/.a/ +
                    //         /b/</code> produces the string
                    //     <code>'/.a//b/'</code>, not a regex. The regex is:
                    // </p>
                    // <p>Look for anything that doesn't terminate a string:</p>
                    "(" +
                        // <p>a backslash followed by a newline (in all three
                        //     newline styles);</p>
                        String.raw`\\\r\n|\\\n|\\\r|` +
                        // <p>a backslash followed by any non-newline character
                        //     (note that the <code>.</code> character class <a
                        //         href="https://developer.mozilla.org/en-US/docs/Web/JavaScript/Guide/Regular_Expressions/Character_Classes#types">doesn't
                        //         match newlines</a>; using the <code>s</code>
                        //     or <code>dotAll</code> flag causes it to match <a
                        //         href="https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Lexical_grammar#line_terminators">line
                        //         terminators</a> that we don't recognize, plus
                        //     not match a <code>\r\n</code> sequence);</p>
                        String.raw`\\.|` +
                        // <p>anything that's not a backslash, quote mark, or
                        //     newline.</p>
                        String.raw`[^\\${m[short_string_index]}\n\r]` +
                    // <p>Find as many of these as possible. Therefore, the next
                    //     token will be the end of the string.</p>
                    ")*" +
                    // <p>A string is terminated by either a quote mark or a
                    //     newline. (We can't just put <code>.</code>, because
                    //     one flavor of newline is two characters; in addition,
                    //     that character class doesn't match newlines, as
                    //     stated above.) Terminating strings at a newline helps
                    //     avoid miscategorizing large chunks of code that the
                    //     compiler likewise flags as a syntax error.</p>
                    String.raw`(${m[short_string_index]}|\r\n|\n|\r)`
                );
                if (string_m) {
                    const index = string_m.index + string_m[0].length;
                    code_block_array.push(source_code.substring(0, index));
                    source_code = source_code.substring(index);
                } else {
                    code_block_array.push(source_code);
                    source_code = "";
                }
            } else if (template_literal_index && m[template_literal_index]) {
                // <p>TODO! For now, just assume there's no comments in
                //     here...dangerous!!!</p>
                code_block_array.push(m[template_literal_index]);
                source_code = source_code.substring(m[template_literal_index].length);
            } else {
                console.assert(false);
                debugger;
            }
        } else {
            // <p>The rest of the source code is in the code block.</p>
            code_block_array.push(source_code);
            source_code = "";
        }
    }

    // <p>Include any accumulated code in the classification.</p>
    const code = code_block_array.join("")
    if (code) {
        classified_source.push([null, code, ""]);
    }

    return classified_source;
};


// <h2 id="classified_source_to_html">Convert lexed code into HTML</h2>
const classified_source_to_html = (classified_source) => {
    // <p>An array of strings for the new content of the current HTML page.</p>
    let html = [];

    // <p>Keep track of the current type. Begin with neither comment nor code.
    // </p>
    let current_indent = -2

    // <p>Keep track of the current line number.</p>
    let line = 1

    for (let [indent, source_string, comment_string] of classified_source) {
        // <p><span id="newline-movement">In a code or doc block, omit the last
        //         newline; otherwise, code blocks would show an extra newline
        //         at the end of the block. (Doc blocks ending in a
        //         <code>&lt;pre&gt;</code> tag or something similar would also
        //         have this problem). To do this, remove the newline from the
        //         end of the current line, then prepend it to the beginning of
        //         the next line.</span></p>
        const m = source_string.match(/(\n|\r\n|\r)$/);
        if (m) {
            source_string = source_string.substring(0, m.index);
        }

        // <p>See if there's a change in state.</p>
        if (current_indent !== indent) {
            // <p>Exit the current state.</p>
            _exit_state(current_indent, html)

            // <p>Enter the new state.</p>
            if (indent === null) {
                // <p>Code state: emit the beginning of an ACE editor block.</p>
                html.push(
`
<div class="CodeChat-code">
    <div class="CodeChat-ACE" data-CodeChat-firstLineNumber="${line}">`,
                    escapeHTML(source_string),
                )

            } else {
                // <p>Comment state: emit an opening indent for non-zero
                //     indents; insert a TinyMCE editor.</p>
                // <p><span id="one-row-table">Use a one-row table to lay out a
                //         doc block, so that it aligns properly with a code
                //         block.</span></p>
                html.push(
`<div class="CodeChat-doc">
    <table>
        <tbody>
            <tr>
                <!-- Spaces matching the number of digits in the ACE gutter's line number. TODO: fix this to match the number of digits of the last line of the last code block. Fix ACE to display this number of digits in all gutters. See https://stackoverflow.com/questions/56601362/manually-change-ace-line-numbers. -->
                <td class="CodeChat-ACE-gutter-padding ace_editor">&nbsp;&nbsp;&nbsp</td>
                <td class="CodeChat-ACE-padding"></td>
                <!-- This doc block's indent. TODO: allow paste, but must only allow pasting spaces. -->
                <td class="ace_editor CodeChat-doc-indent" contenteditable onpaste="return false">${indent}</td>
                <td class="CodeChat-TinyMCE-td"><div class="CodeChat-TinyMCE">`,
                    source_string,
                )
            }
        } else {
            // <p><span id="newline-prepend"><a href="#newline-movement">Newline
            //             movement</a>: prepend the newline removed from the
            //         previous line to the current line</span>.</p>
            html.push(m[0], indent === null ? escapeHTML(source_string) : source_string);
        }

        // <p>Update the state.</p>
        current_indent = indent
        // <p>There are an unknown number of newlines in this source string. One
        //     was removed <a href="#newline-movement">here</a>, so include that
        //     in the count.</p>
        line += 1 + (source_string.match(/\n|\r\n|\r/g) || []).length
    }

    // <p>When done, exit the last state.</p>
    _exit_state(current_indent, html)
    return html.join("");
};


// <h3>_exit_state</h3>
// <p>Output text produced when exiting a state. Supports <a
//         href="#_generate_web_editable"><code>_generate_web_editable</code></a>.
// </p>
const _exit_state = (
    // <p>The type (classification) of the last line.</p>
    indent,
    // <p>An array of string to store output in.</p>
    html,
) => {

    if (indent === null) {
        // <p>Close the current code block.</p>
        html.push("</div>\n</div>\n");
    } else if (typeof indent === "string") {
        // <p>Close the current doc block without adding any trailing spaces
        //     &mdash; combining this with the next line would add indentation.
        // </p>
        //</p>
        html.push(
`</td>
            </tr>
        </tbody>
    </table>
</div>
`
        )
    }
}


// <h2>Save editor contents to source code</h2>
// <p>This transforms the current editor contents into source code.</p>
const editor_to_source_code = (
    // <p>A string specifying the comment character(s) for the current
    //     programming language. A space will be added after this string before
    //     appending a line of doc block contents.</p>
    comment_string
) => {
    // <p>Walk through each code and doc block, extracting its contents then
    //     placing it in <code>classified_lines</code>.</p>
    let classified_lines = [];
    for (const code_or_doc_tag of document.querySelectorAll(".CodeChat-ACE, .CodeChat-TinyMCE")) {
        // <p>The type of this block: <code>null</code> for code, or &gt;= 0 for
        //     doc (the value of n specifies the indent in spaces).</p>
        let indent;
        // <p>A string containing all the code/docs in this block.</p>
        let full_string;

        // <p>Get the type of this block and its contents.</p>
        if (code_or_doc_tag.classList.contains("CodeChat-ACE")) {
            indent = null;
            full_string = ace.edit(code_or_doc_tag).getValue();
        } else if (code_or_doc_tag.classList.contains("CodeChat-TinyMCE")) {
            // <p>Get the indent from the previous table cell.</p>
            indent = code_or_doc_tag.parentElement.previousElementSibling.textContent;
            // <p>See <a
            //         href="https://www.tiny.cloud/docs/tinymce/6/apis/tinymce.root/#get"><code>get</code></a>
            //     and <a
            //         href="https://www.tiny.cloud/docs/tinymce/6/apis/tinymce.editor/#getContent"><code>getContent()</code></a>.
            //     Fortunately, it looks like TinyMCE assigns a unique ID if
            //     one's no provided, since it only operates on an ID instead of
            //     the element itself.</p>
            full_string = tinymce.get(code_or_doc_tag.id).getContent();
            // <p>The HTML from TinyMCE is a mess! Wrap at 80 characters,
            //     including the length of the indent and comment string.</p>
            full_string = html_beautify(full_string, { "wrap_line_length": 80 - indent.length - comment_string.length - 1 });
        } else {
            console.assert(false, `Unexpected class for code or doc block ${code_or_doc_tag}.`);
        }

        // <p>Split the <code>full_string</code> into individual lines; each one
        //     corresponds to an element of <code>classified_lines</code>.</p>
        for (const string of full_string.split(/\r?\n/)) {
            classified_lines.push([indent, string + "\n"]);
        }
    }

    // <p>Transform these classified lines into source code.</p>
    let lines = [];
    // <p>If there comment string is empty, assume this is a CodeChat document
    //     (raw HTML/Markdown/etc.), so drop the space after the empty comment
    //     string.</p>
    let space = comment_string === "" ? "" : " ";
    for (const [indent, string] of classified_lines) {
        if (indent === null) {
            // <p>Just dump code out!</p>
            lines.push(string);
        } else {
            // <p>Prefix comments with the indent and the comment string.</p>
            // <p>TODO: allow the use of block comments.</p>
            lines.push(`${indent}${comment_string}${space}${string}`);
        }
    }

    return lines.join("");
};


// <h2>Helper functions</h2>
// <p>Given text, escape it so it formats correctly as HTML. Because the
//     solution at https://stackoverflow.com/a/48054293 transforms newlines into
//     <br>(see
//     https://developer.mozilla.org/en-US/docs/Web/API/HTMLElement/innerText),
//     it's not usable with code. Instead, this is a translation of Python's
//     <code>html.escape</code> function.</p>
const escapeHTML = unsafeText => {
    // <p>Must be done first!</p>
    unsafeText = unsafeText.replaceAll("&", "&amp;")
    unsafeText = unsafeText.replaceAll("<", "&lt;")
    unsafeText = unsafeText.replaceAll(">", "&gt;")
    return unsafeText;
};


// <p>This function comes from the <a
//         href="https://developer.mozilla.org/en-US/docs/Web/JavaScript/Guide/Regular_Expressions#escaping">MDN
//         docs</a>.</p>
const escapeRegExp = string => string.replace(/[.*+?^${}()|[\]\\]/g,
    // <p><code>$&amp;</code> means the whole matched string.</p>
    '\\$&');


// <h2>Unit tests</h2>
// <p>TODO!</p>
const test_source_lexer_1 = () => {
    const python_source_lexer = source_code => source_lexer(source_code, ...language_lexers[3]);
    assert_equals(python_source_lexer(""), []);
    assert_equals(python_source_lexer("\n"), [[null, "\n", ""]]);
    assert_equals(python_source_lexer("\n# Test"), [[null, "\n", ""], ["", "Test", "#"]]);
    assert_equals(python_source_lexer("\n# Test\n"), [[null, "\n", ""], ["", "Test\n", "#"]]);
    assert_equals(python_source_lexer("# Test"), [["", "Test", "#"]]);
    assert_equals(python_source_lexer("# Test\n"), [["", "Test\n", "#"]]);
    assert_equals(python_source_lexer("# Test\n\n"), [["", "Test\n", "#"], [null, "\n", ""]]);
    // <p>Short string with line join.</p>
    assert_equals(python_source_lexer("'\\\n# Test'\n"), [[null, "'\\\n# Test'\n", ""]]);
    assert_equals(python_source_lexer('"\\\n# Test"\n'), [[null, '"\\\n# Test"\n', ""]]);
    // <p>Short string terminated with newline (syntax error) followed by a
    //     comment.</p>
    assert_equals(python_source_lexer("'\\\\\n# Test'\n"), [[null, "'\\\\\n", ""], ["", "Test'\n", "#"]]);
    assert_equals(python_source_lexer('"\\\\\n# Test"\n'), [[null, '"\\\\\n', ""], ["", 'Test"\n', "#"]]);
    // <p>Long string with newlines around comment.</p>
    assert_equals(python_source_lexer('"""\n# Test\n"""'), [[null, '"""\n# Test\n"""', ""]]);
    assert_equals(python_source_lexer("'''\n# Test\n'''"), [[null, "'''\n# Test\n'''", ""]]);
    // <p>Unterminated long strings.</p>
    assert_equals(python_source_lexer('"""\n# Test\n'), [[null, '"""\n# Test\n', ""]]);
    assert_equals(python_source_lexer("'''\n# Test\n"), [[null, "'''\n# Test\n", ""]]);
    // <p>Comments that aren't doc blocks.</p>
    assert_equals(python_source_lexer("  a = 1 # Test"), [[null, "  a = 1 # Test", ""]]);
    assert_equals(python_source_lexer("\n  a = 1 # Test"), [[null, "\n  a = 1 # Test", ""]]);
    assert_equals(python_source_lexer("  a = 1 # Test\n"), [[null, "  a = 1 # Test\n", ""]]);
    // <p>Doc blocks.</p>
    assert_equals(python_source_lexer("   # Test"), [["   ", "Test", "#"]]);
    assert_equals(python_source_lexer("\n   # Test"), [[null, "\n", ""], ["   ", "Test", "#"]]);

    assert_equals(python_source_lexer("   # Test\n"), [["   ", "Test\n", "#"]]);
};


const test_source_lexer = () => {
    test_source_lexer_1();
};


// <p>Woefully inadequate, but enough for testing.</p>
const assert_equals = (a, b) => {
    console.assert(a.length === b.length);
    for (let index = 0; index < a.length; ++index) {
        if (a[index] instanceof Array) {
            console.assert(b[index] instanceof Array);
            assert_equals(a[index], b[index]);
        } else {
            console.assert(a[index] === b[index]);
        }
    }
}


test_source_lexer();
