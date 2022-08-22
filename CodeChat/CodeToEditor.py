# <details>
#     <summary>Copyright (C) 2012-2022 Bryan A. Jones.</summary>
#     <p>This file is part of CodeChat.</p>
#     <p>CodeChat is free software: you can redistribute it and/or
#         modify it under the terms of the GNU General Public License as
#         published by the Free Software Foundation, either version 3 of
#         the License, or (at your option) any later version.</p>
#     <p>CodeChat is distributed in the hope that it will be useful, but
#         WITHOUT ANY WARRANTY; without even the implied warranty of
#         MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
#         GNU General Public License for more details.</p>
#     <p>You should have received a copy of the GNU General Public
#         License along with CodeChat. If not, see <a
#             href="http://www.gnu.org/licenses/">http://www.gnu.org/licenses/</a>.
#     </p>
# </details>
# <h1>CodeToEditor.py - a module to translate source code to an editable
#     web page</h1>
# <p>This module converts source code into a web page that provides an
#     IDE-like editing environment.The <a href="#API">API</a> lists two
#     functions which convert source code into this web page. It relies
#     on <code>source_lexer</code> to classify the source as code or
#     comment, then <a
#         href="#_generate_web_editable"><code>_generate_web_editable</code></a>
#     to perform the conversion.</p>
# <p>For prototyping, run:</p>
# <p><code>python -c "from CodeChat.CodeToEditor import
#         code_to_web_editable_file as c; c('CodeToEditor.py')"</code>
# </p>
# <p>To view the output, run
#     <code>\Users\bjones\AppData\Roaming\npm\httpserver</code>.</p>
# <p>TODO: Create a table of contents.</p>
# <h2>Next steps</h2>
# <ul>
#     <li>Look at / experiment with a book build process.</li>
#     <li>Port most of this file to JavaScript.</li>
#     <li>Decide on a strategy to move all open code to JS.</li>
#     <li>Create a new repo or directory for the CodeChat Editor, with
#         NPM and webpack set up.</li>
#     <li>Integrate this into IDEs.</li>
# </ul>
# <h2>Thoughts and ideas</h2>
# <p>Provide three modes: syntax highlight the entire file (no LP),
#     view, and edit. But...</p>
# <ul>
#     <li>Editing mostly makes sense in the context of an IDE.</li>
#     <li>There might be a performance hit for the new option, waiting
#         for the JS to hydrate. It would be nice to offer a view-only
#         option that is static HTML.</li>
# </ul>
# <p>Need some sort of book build process. It would:</p>
# <ul>
#     <li>Create a list of all ids in all HTML files in the book.</li>
#     <li>Crossref thingy</li>
#     <li>Global TOC / page order for the book</li>
#     <li>Link checker</li>
#     <li>Need to write some components:
#         <ul>
#             <li>Autotitle: like an a, but takes the link&rsquo;s name
#                 from element linked to. Perhaps this is just an XSLT
#                 transform? But want to work with local links too, so
#                 it&rsquo;s partly JS.</li>
#             <li>A local table of contents</li>
#             <li>A crossref thingy &ndash; giving a single crossref
#                 value produces links to all crossrefs but the one
#                 created.</li>
#             <li>Insert the name of the file. How to do templates with
#                 HTML? Perhaps with XSLT? Need to find a nice/easy way
#                 to theme a book. I&rsquo;d like to use PreTeXt if
#                 possible.<br>How to specify the book structure with
#                 maximum simplicity? Probably like Sphinx &ndash; a
#                 toctree defines structure.</li>
#         </ul>
#     </li>
# </ul>
# <p>Perhaps mdbook for this?</p>
# <p>What components should we support/create? Does TinyMCE allow
#     components?</p>
# <p>For links to LP files: the old system generated a new name
#     (appended .html), so that LP files have a unique name. This seems
#     like a simple solution. (If one file is named foo.c and another is
#     foo.html, this fails. But, I think it&rsquo;s reasonable to
#     document this instead of finding a fix.) This would make static
#     and dynamic work, and seems simple. In the non-project IDE case, a
#     missing .html link would cause it to look for the raw file without
#     the .html extension.</p>
# <h2>Open in JavaScript ideas</h2>
# <p>Is it easier to put together a code vs comment finder, or to work
#     with a lexer? For a code vs comment finder, the only difficult
#     pieces are:</p>
# <ul>
#     <li>Anything that could enclose comment text, making it not a
#         comment:
#         <ul>
#             <li>Strings.
#                 <ul>
#                     <li>Have to deal with escapes, to ensure we find
#                         the actual start/end of a string.</li>
#                     <li>Some strings don&rsquo;t allow embedded
#                         newlines (unless they contain a line
#                         continuation character). Terminate these at a
#                         newline even if there&rsquo;s no closing
#                         quote. The damage is minimum since even if
#                         there&rsquo;s another quote on the next line,
#                         that will be terminated by the next newline.
#                     </li>
#                     <li>Some strings allow embedded newlines.</li>
#                     <li>Python raw strings don&rsquo;t need special
#                         treatment, since (at least in Python) they
#                         still require quoting a quote character.</li>
#                     <li>JavaScript allows nested template literals.
#                         Ick.</li>
#                 </ul>
#             </li>
#             <li>Here text (Bash?) and C++ raw strings.</li>
#             <li>Inline comments that span multiple lines due to line
#                 continuation characters</li>
#             <li>Nested block comments? Treat this as a parse error, to
#                 avoid treating a bunch of code as a doc block.</li>
#         </ul>
#     </li>
# </ul>
# <p>So, for each language, need to specify:</p>
# <ul>
#     <li>Line continuation character. These change the definition of
#         newlines for everything but here text.</li>
#     <li>Inline comment strings.</li>
#     <li>Pairs of block comment strings. Perl, for example, allow
#         several of these.</li>
#     <li>String escape regex.</li>
#     <li>String start/end strings.</li>
#     <li>Template literal strings, embedded expression start/end
#         strings.</li>
#     <li>Here string start prefix, start suffix, stop prefix, stop
#         suffix.</li>
# </ul>
# <p>States:</p>
# <ul>
#     <li>Initial; not inside a string or comment.</li>
#     <li>In string; must record valid string closing strings.</li>
#     <li>In here text; must record the closing here text sequence.</li>
#     <li>In template literal.
#         <ul>
#             <li>In embedded expression inside a JavaScript template
#                 literal. This is the initial state, plus looking for
#                 the close character of the embedded expression.</li>
#         </ul>
#     </li>
#     <li>In block comment. Need way to report an unterminated
#         multi-line comment, and also attempts to nest comments (which
#         might indicate a missing comment terminator). Probably have
#         this transition to an error state, or something like that.
#     </li>
#     <li>In inline comment. Need to look for line continuation
#         characters.</li>
# </ul>
# <p>The only really dangerous thing is an unterminated comment, that
#     causes a bunch of code to be munched when saving. This is what we
#     need to find and complain about.</p>
# <p>Filling this out, to see if it seems reasonable for various
#     languages:</p>
# <h3>Python</h3>
# <ul>
#     <li>Line continuation character = <code>["\\"]</code></li>
#     <li>Inline comment strings = <code>["#"]</code></li>
#     <li>Pairs of block comment strings = <code>[ ]</code></li>
#     <li>String escape regex = <code>(\\\\|[^\\])</code>&nbsp;means
#         either&nbsp; <code>\\</code> or not a <code>\</code>.&nbsp;
#     </li>
#     <li>String start/end strings. <code>[ ['"', ['"', newline]
#             ]</code> means strings start with a double quote and end
#         with either a double quote or a newline. A newline is the
#         regex <code>(\n|\r\n|\r)</code>: either <code>\n</code>,
#         <code>\r\n</code>, or <code>\r</code>. In this string context,
#         both must be preceded by the string escape regex. Outside a
#         string, the newline regex is simpler, since there's no
#         <code>\\</code> sequence to avoid. It's just
#         <code>f"[^\\]{newline}"</code>.</li>
#     <li>Template literal strings, embedded expression start/end
#         strings = <code>[ "", "", "" ]</code>. These are all subject
#         to string escape sequences.</li>
#     <li>Here string start prefix, start suffix, stop prefix, stop
#         suffix = <code>[ "", "", "", "" ]</code></li>
# </ul>
# <p>This is fairly complex&hellip;create a regex for the initial state
#     that looks for the start of an inline comment or the start of a
#     string: <code>/(\s*)#|(""")|(''')|(")|(')/</code> would identify
#     the next comment or quote. It assumes that the initial state
#     always starts at the beginning of a line.</p>
# <h2>Imports</h2>
# <p>These are listed in the order prescribed by <a
#         href="http://www.python.org/dev/peps/pep-0008/#imports">PEP
#         8</a>.</p>
# <h3>Standard library</h3>
import html
from io import StringIO
from pathlib import Path
from textwrap import dedent, indent

# <h3>Third-party imports</h3>
# <p>None.</p>

# <h3>Local application imports</h3>
from .SourceClassifier import source_lexer, get_lexer, _debug_print


# <h2 id="API">API</h2>
# <p>The following routines provide easy access to the core
#     functionality of this module.</p>
# <h3>code_to_web_editable_string</h3>
# <p>This function converts a string containing source code to an
#     editable web page, preserving all indentations of both source code
#     and comments. Code is placed in an ACE editor, while doc blocks go
#     in an HTML editor (TinyMCE).</p>
def code_to_web_editable_string(
    # <p>The code to translate.</p>
    code_str,
    # <p>See options (link is TODO).</p>
    **options,
):

    # <p>Use a StringIO to capture writes into a string.</p>
    output_html = StringIO()
    lexer = get_lexer(code=code_str, **options)
    ast_syntax_error, classified_lines = source_lexer(code_str, lexer)
    if ast_syntax_error:
        output_html.write("# Error\n{}\n".format(ast_syntax_error))
    _generate_web_editable(
        _pygments_to_ace_language(lexer.name), classified_lines, output_html
    )
    return output_html.getvalue()


# <h3>code_to_web_editable_file</h3>
# <p>Convert a source file to a web editable HTML file.</p>
def code_to_web_editable_file(
    # <p>Path to a source code file to process.</p>
    source_path,
    # <p>Path to a destination HTML file to create. It will be overwritten
    #     if it already exists. If not specified, it is
    #     <code><em>source_path</em>.html</code>.</p>
    html_path=None,
    # <p>Encoding to use for the input file. The default of None detects the
    #     encoding of the input file.</p>
    input_encoding="utf-8",
    # <p>Encoding to use for the output file.</p>
    output_encoding="utf-8",
    # <p>See `options `.</p>
    **options,
):

    # <p>Provide a default <code>html_path</code>.</p>
    if not html_path:
        html_path = source_path + ".html"
    with open(source_path, encoding=input_encoding) as fi:
        code_str = fi.read()
    # <p>If not already present, provide the filename of the source to help
    #     in identifying a lexer.</p>
    options.setdefault("filename", source_path)
    html = code_to_web_editable_string(code_str, **options)
    # <p>Patch up the title (replace the first empty title).</p>
    source_path = Path(source_path).resolve()
    html = html.replace(
        "<title></title>",
        f'<title data-CodeChat-filename="{source_path.name}" data-CodeChat-path="{source_path.anchor}">{source_path.name} - The CodeChat Editor</title>',
        1,
    )
    with open(html_path, "w", encoding=output_encoding) as fo:
        fo.write(html)


# <h2>Converting classified code to the web editable format</h2>
# <p>This function maps Pygments language names to ACE language names.
#     TODO: there are lots of missing mappings here!</p>
def _pygments_to_ace_language(pygments_language_name: str) -> str:
    return pygments_language_name.lower()


# <h3 id="_generate_web_editable">_generate_web_editable</h3>
# <p>Generate web editable HTML from the classified code. To do this,
#     create a state machine, where <code>current_type</code> defines
#     the state. Use this to produce the correct HTML prolog/epilog when
#     changing from code to doc blocks or vice versa.</p>
def _generate_web_editable(
    # <p>The name of the language; used by ACE to select a syntax
    #     highlighter.</p>
    language_name,
    # <p>An iterable of <code>(type, string)</code> pairs, one per line.</p>
    classified_lines,
    # <p><span id="out_file">A file-like output to which the HTML is
    #         written.</span></p>
    out_file,
):
    # <p><span id="script-param">Write out the beginning of the web page.
    #         Pass the <code>language_name</code> to the <a
    #             href="CodeChatEditor.js#script-param">script</a>.</span>
    # </p>
    out_file.write(
        dedent(
            f"""\
            <!DOCTYPE html>
            <html lang="en">
                <head>
                    <meta charset="UTF-8">
                    <meta name="viewport" content="width=device-width, initial-scale=1">

                    <title></title>

                    <script src="https://cdnjs.cloudflare.com/ajax/libs/ace/1.9.5/ace.min.js"></script>
                    <script src="https://cdn.tiny.cloud/1/rrqw1m3511pf4ag8c5zao97ad7ymvnhqu6z0995b1v63rqb5/tinymce/6/tinymce.min.js" referrerpolicy="origin"></script>
                    <script src="https://cdnjs.cloudflare.com/ajax/libs/js-beautify/1.14.5/beautify-html.min.js"></script>
                    <script src="CodeChatEditor.js" data-CodeChat-language-name={language_name}></script>

                    <link rel="stylesheet" href="css/CodeChatEditor.css">
                </head>
                <body>
                    <p>
                        <button onclick="on_save_as();">
                            Save as
                        </button>
                        <button disabled onclick="on_save();" id="CodeChat-save-button">
                            Save
                        </button>
                    </p>
        """
        )
    )

    # <p>Keep track of the current type. Begin with neither comment nor
    #     code.</p>
    current_type = -2

    # <p>Keep track of the current line number.</p>
    line = 1

    for type_, string in classified_lines:
        _debug_print(
            "type_ = {}, line = {}, string = {}\n".format(type_, line, [string])
        )

        # <p><span id="newline-movement">In a code or doc block, omit the last
        #         newline; otherwise, code blocks would show an extra newline at
        #         the end of the block. (Doc blocks ending in a
        #         <code>&lt;pre&gt;</code> tag or something similar would also
        #         have this problem). To do this, remove the newline from the
        #         end of the current line, then prepend it to the beginning of
        #         the next line.</span></p>
        assert string[-1] == "\n"
        string = string[:-1]

        # <p>See if there's a change in state.</p>
        if current_type != type_:
            # <p>Exit the current state.</p>
            _exit_state(current_type, out_file)

            # <p>Enter the new state.</p>
            # <p>Code state: emit the beginning of an ACE editor block.</p>
            if type_ == -1:
                out_file.write(
                    indent(
                        dedent(
                            f"""
                            <div class="CodeChat-code">
                                <div class="CodeChat-ACE" data-CodeChat-firstLineNumber="{line}">"""
                        ),
                        # <p><span id="html-indent">Indent by 8 spaces to match earlier HTML
                        #         (we're inside the <code>&lt;html&gt;</code> and
                        #         <code>&lt;body&gt;</code> tags).</span></p>
                        "        ",
                    )
                )
                out_file.write(html.escape(string))

            # <p>Comment state: emit an opening indent for non-zero indents; insert
            #     a TinyMCE editor.</p>
            else:
                # <p><span id="one-row-table">Use a one-row table to lay out a doc
                #         block, so that it aligns properly with a code block.</span>
                # </p>
                out_file.write(
                    indent(
                        dedent(
                            f"""
                            <div class="CodeChat-doc">
                                <table>
                                    <tbody>
                                        <tr>
                                            <!-- Spaces matching the number of digits in the ACE gutter's line number. TODO: fix this to match the number of digits of the last line of the last code block. Fix ACE to display this number of digits in all gutters. See https://stackoverflow.com/questions/56601362/manually-change-ace-line-numbers. -->
                                            <td class="CodeChat-ACE-gutter-padding ace_editor">&nbsp;&nbsp;&nbsp</td>
                                            <td class="CodeChat-ACE-padding"</td>
                                            <!-- This doc block's indent. TODO: allow paste, but must only allow pasting spaces. -->
                                            <td class="ace_editor CodeChat-doc-indent" contenteditable onpaste="return false">{'&nbsp;' * type_}</td>
                                            <td class="CodeChat-TinyMCE-td"><div class="CodeChat-TinyMCE">"""
                        ),
                        "        ",
                    )
                )
                out_file.write(string)

        else:
            # <p><span id="newline-prepend"><a href="#newline-movement">Newline
            #             movement</a>: prepend the newline removed from the
            #         previous line to the current line</span>.</p>
            out_file.write("\n")
            if type_ == -1:
                out_file.write(html.escape(string))
            else:
                out_file.write(string)

        # <p>Update the state.</p>
        current_type = type_
        line += 1

    # <p>When done, exit the last state.</p>
    _exit_state(current_type, out_file)

    out_file.write(
        dedent(
            """
                </body>
            </html>
            """
        )
    )


# <h3>_exit_state</h3>
# <p>Output text produced when exiting a state. Supports <a
#         href="#_generate_web_editable"><code>_generate_web_editable</code></a>.
# </p>
def _exit_state(
    # <p>The type (classification) of the last line.</p>
    type_,
    # <p>See <a href="#out_file">out_file</a>.</p>
    out_file,
):

    # <p>Code or commentary state</p>
    if type_ == -1:
        out_file.write("</div>\n        </div>\n")
    elif type_ >= 0:
        # <p>Close the current doc block without adding any trailing spaces
        #     &mdash; combining this with the next line would add indentation.
        # </p>
        out_file.write("</td>\n")
        # <p>Match the current HTML <a href="#html-indent">indentation</a>.</p>
        out_file.write(
            indent(
                dedent(
                    """\
                                </tr>
                            </tbody>
                        </table>
                    </div>
                    """
                ),
                "        ",
            )
        )
    # <p>Initial state or non-indented comment. Nothing needed.</p>
    else:
        pass
