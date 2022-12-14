// <!-- CodeChat-lexer: vlang -->
// <details>
//     <summary>License</summary>
//     <p>Copyright (C) 2022 Bryan A. Jones.</p>
//     <p>This file is part of the CodeChat Editor.</p>
//     <p>The CodeChat Editor is free software: you can redistribute it and/or
//         modify it under the terms of the GNU General Public License as
//         published by the Free Software Foundation, either version 3 of the
//         License, or (at your option) any later version.</p>
//     <p>The CodeChat Editor is distributed in the hope that it will be useful,
//         but WITHOUT ANY WARRANTY; without even the implied warranty of
//         MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
//         General Public License for more details.</p>
//     <p>You should have received a copy of the GNU General Public License
//         along with the CodeChat Editor. If not, see <a
//             href="http://www.gnu.org/licenses/">http://www.gnu.org/licenses/</a>.
//     </p>
// </details>
// <h1><code>CodeChatEditorServer.v</code>&mdash;A simple server for the
//     CodeChat Editor</h1>
// <p>This server provides the user the ability to browse the local filesystem;
//     clicking on a CodeChat Editor-supported source file then loads it in the
//     CodeChat Editor. It also enables the CodeChat Editor to save modified
//     files back to the local filesystem.</p>
// <h2>Imports</h2>
import os
import net.urllib
import regex
import vweb

// <h2>Types</h2>
// <p>This stores web server state.</p>
struct App {
	vweb.Context
}

// <p>This defines the JSON file produced by webpack.</p>
struct WebpackJson {
	js []string
	css []string
}


// <h2>Constants</h2>
const (
	codechat_extensions = [
		'.c',
		'.cc',
		'.cpp',
		'.html',
		'.js',
		'.mjs',
		'.json',
		'.py',
		'.toml',
		'.ts',
		'.mts',
		'.v',
		'.yaml',
		'.yml',
		'.cchtml',
	]
)

// <h2>Endpoints</h2>
// <p>Redirect from the root endpoint to the filesystem.</p>
['/']
fn (mut app App) root_redirect() vweb.Result {
	return app.redirect('/fs')
}

// <p>This endpoint serves files from the local filesystem. vweb requires me to
//     declare it twice in order to get an empty path (here) or a path with
//     something after <code>/fs</code> (in the following function).</p>
['/fs']
fn (mut app App) serve_fs_bare() vweb.Result {
	// <p>On Windows, assume the C drive as the root of the filesystem. TODO:
	//     provide some way to list drives / change drives from the HTML GUI.</p>
	if os.user_os() == 'windows' {
		return app.redirect('/fs/${urllib.path_escape('C:')}/')
	}
	return app.serve_fs_('/')
}

['/fs/:path...']
fn (mut app App) serve_fs(path string) vweb.Result {
	return app.serve_fs_(path)
}

// <p>This is used by endpoints when reporting an error.</p>
struct ErrorResponse {
	success bool
	message string
}

// <p><a id="save_file"></a>A <code>PUT</code> to a filename writes the provided
//     data to that file; this is used by the <a
//         href="static/js/CodeChatEditor.js#save">save function</a>.</p>
['/fs/:path...'; put]
fn (mut app App) save_file(path string) vweb.Result {
	// <p>For Unix, restore the leading <code>/</code> to the beginning of the
	//     path.</p>
	fixed_path := (if os.user_os() != 'windows' { '/' } else { '' }) + path
	abs_path := os.abs_path(fixed_path)
	os.write_file(abs_path, app.req.data) or {
		// <p>TODO: Return an ErrorResponse.</p>
		return app.json(ErrorResponse{ success: false, message: 'Unable to write file.' })
	}
	return app.json({
		'success': true
	})
}

// <p>Serve either a directory listing, with special links for CodeChat Editor
//     files, or serve a CodeChat Editor file or a normal file.</p>
fn (mut app App) serve_fs_(path string) vweb.Result {
	// <p>The provided <code>path</code> may need fixing, since it lacks an initial
	//     <code>/</code>.</p>
	mut fixed_path := path
	if os.user_os() == 'windows' {
		// <p>On Windows, a path of <code>drive_letter:</code> needs a <code>/</code>
		//     appended.</p>
		mut regex_drive_letter := regex.regex_opt('^[a-zA-Z]:$') or {
			panic('Regex failed to compile.')
		}
		if regex_drive_letter.matches_string(path) {
			fixed_path += '/'
		}
		// <p>All other cases (for example, <code>C:\a\path\to\file.txt</code>) are
		//     OK.</p>
	} else {
		// <p>For Linux/OS X, prepend a slash, so that <code>a/path/to/file.txt</code>
		//     becomes <code>/a/path/to/file.txt</code>.</p>
		fixed_path = '/' + fixed_path
	}
	// <p>Normalize path as well as making it absolute.</p>
	abs_path := os.abs_path(fixed_path)

	if os.is_dir(abs_path) {
		// <p>Serve a listing of the files and subdirectories in this directory.
		//     Create the text of a web page with this listing.</p>
		mut ret := '<!DOCTYPE html>
<html lang="en">
	<head>
		<meta charset="UTF-8">
		<meta name="viewport" content="width=device-width, initial-scale=1">
		<title>The CodeChat Editor</title>
	</head>
	<body>
		<h1>
			Directory of $abs_path
		</h1>
		<ul>
'

		// <p>List each file/directory with appropriate links.</p>
		mut ls := os.ls(abs_path) or {
			ret += "<p>But it's not valid.</p>"
			[]
		}
		// <p>Sort it case-insensitively; put directoris before files.</p>
		ls.sort_with_compare(fn [abs_path] (a &string, b &string) int {
			// <p>If both a and b aren't directories, sort on that basis.</p>
			a_is_dir := os.is_dir(os.join_path(abs_path, *a))
			b_is_dir := os.is_dir(os.join_path(abs_path, *b))
			if a_is_dir != b_is_dir {
				return if a_is_dir { -1 } else { 1 }
			}
			// <p>Otherwise, sort on the name.</p>
			return compare_strings(a.to_lower(), b.to_lower())
		})
		// <p>Write out HTML for each file/directory.</p>
		for f in ls {
			full_path := os.join_path(abs_path, f)
			if os.is_dir(full_path) {
				// <p>Use an absolute path, instead of a relative path, in case the URL of
				//     this directory doesn't end with a <code>/</code>. Detecting this case
				//     is hard, since vweb removes the trailing <code>/</code> even if it's
				//     there!</p>
				ret += '<li><a href="/fs/$path/${urllib.path_escape(f)}/">$f/</a></li>\n'
			} else {
				extension := os.file_ext(f)
				html_path := '/fs/$path/${urllib.path_escape(f)}'
				// <p>See if it's a CodeChat Editor file.</p>
				if extension in codechat_extensions {
					// <p>Yes. Provide a link to the CodeChat Editor for this file.</p>
					ret += '<li><a href="$html_path" target="_blank">$f</a></li>\n'
				} else {
					// <p>No. Only list the file, but don't link to it.</p>
					ret += '<li>$f</li>\n'
				}
			}
		}
		return app.html(ret + '        </ul>
	</body>
</html>')
	} else if os.is_file(abs_path) {
		ext := os.file_ext(abs_path)
		if ext in codechat_extensions {
			codechat_file_contents := os.read_file(abs_path) or { return app.not_found() }
			// <p>Transform this into a CodeChat Editor webpage.</p>
			return app.html(codechat_editor_html(codechat_file_contents, abs_path, app.query["mode"] == "toc"))
		}
		// <p>It's not a CodeChat Editor file -- just serve the file.</p>
		return app.file(abs_path)
	} else {
		return app.not_found()
	}
}

// <h2>CodeChat Editor support</h2>
// <p>Given the source code for a file and its path, return the HTML to present
//     this in the CodeChat Editor.</p>
fn codechat_editor_html(source_code string, path string, is_toc bool) string {
    mut raw_dir := os.dir(path)
	dir := escape_html(raw_dir)
	name := escape_html(os.base(path))
	ext := os.file_ext(path)

	// <p>The TOC is a simplified web page requiring no additional processing. The
	//     script ensures that all hyperlinks target the enclosing page, not just
	//     the iframe containing this page.</p>
	if is_toc {
		return '<!DOCTYPE html>
<html lang="en">
	<head>
		<meta charset="UTF-8">
		<meta name="viewport" content="width=device-width, initial-scale=1">
		<title>$name - The CodeChat Editor</title>

		<link rel="stylesheet" href="/static/css/CodeChatEditor.css">
		<link rel="stylesheet" href="/static/css/CodeChatEditorSidebar.css">
		<script>
			addEventListener("DOMContentLoaded", (event) => {
				document.querySelectorAll("a").forEach((a_element) => {
					a_element.target = "_parent"
				});
			});
		</script>
	</head>
	<body>
${source_code}
	</body>
</html>
'
	}

    // <p>Look for a project file by searching the current directory, then all
    //     its parents, for a file named <code>toc.cchtml</code>.</p>
    mut is_project := false
	// <p>The number of directories between this file to serve (in
	//     <code>path</code>) and the toc file.</p>
	mut num_dir := 0
	// <p>Using v 0.3.1, on Windows, os.dir("C:\\a_directory") == "C:" (this is
	//     wrong! TODO: as a workaround, need to check C:\ instead) and
	//     os.dir("C:") == "." (nonsensical). But use this as a termination
	//     condition. Linux results are more expected.</p>
	for (raw_dir != ".") {
		project_file := os.join_path(raw_dir, "toc.cchtml")
		if os.is_file(project_file) {
			is_project = true
			break
		}
		// <p>On Linux, we're done if we just checked the root directory.</p>
		if raw_dir == "/" {
			break
		}
		raw_dir = os.dir(raw_dir)
		num_dir += 1
	}
	sidebar_iframe, sidebar_css := if is_project {
		'<iframe src="${"../".repeat(num_dir)}toc.cchtml?mode=toc" id="CodeChat-sidebar"></iframe>',
		'<link rel="stylesheet" href="/static/css/CodeChatEditorProject.css">'
	} else {
		"", ""
	}

	return '<!DOCTYPE html>
<html lang="en">
	<head>
		<meta charset="UTF-8">
		<meta name="viewport" content="width=device-width, initial-scale=1">
		<title>$name - The CodeChat Editor</title>

        <link rel="stylesheet" href="/static/webpack/CodeChatEditor.css">
		<script type="module">
		    import { page_init, on_keydown, on_save, on_save_as } from "/static/webpack/CodeChatEditor.js"
			// <p>Make these accesible on the onxxx handlers below. See <a
			//         href="https://stackoverflow.com/questions/44590393/es6-modules-undefined-onclick-function-after-import">SO</a>.
			// </p>
			window.CodeChatEditor = { on_keydown, on_save, on_save_as };

			page_init(
"${quote_script_string(source_code)}",
"${quote_string(ext)}");
		</script>
		<link rel="stylesheet" href="/static/css/CodeChatEditor.css">
		${sidebar_css}
	</head>
	<body onkeydown="CodeChatEditor.on_keydown(event);">
		${sidebar_iframe}
		<div id="CodeChat-contents">
			<div id="CodeChat-top">
				<div id="CodeChat-filename">
					<p>
						<button disabled onclick="CodeChatEditor.on_save_as(on_save_doc);" id="CodeChat-save-as-button">
							Save as
						</button>
						<button onclick="CodeChatEditor.on_save();" id="CodeChat-save-button">
							<span class="CodeChat-hotkey">S</span>ave
						</button>
						- $name - $dir
					</p>
				</div>
				<div id="CodeChat-menu"></div>
			</div>
			<div id="CodeChat-body"></div>
			<div id="CodeChat-bottom"></div>
		</div>
	</body>
</html>
'
}

// <p>For JavaScript, escape any double quotes and convert newlines, so it's
//     safe to enclose the returned string in double quotes.</p>
fn quote_string(s string) string {
	return s.replace(r'\', r'\\').replace('"', r'\"').replace('\r\n', r'\n').replace('\r',
		r'\n').replace('\n', r'\n')
}

// <p>In addition to quoting strings, also split up an ending
//     <code>&lt;/script&gt;</code> tags, since this string is placed inside a
//     <code>&lt;script&gt;</code> tag.</p>
fn quote_script_string(source_code string) string {
	return (quote_string(source_code).split('</script>')).join('</scr"+"ipt>')
}

// <p>Given text, escape it so it formats correctly as HTML. This is a
//     translation of Python's <code>html.escape</code> function.</p>
fn escape_html(unsafe_text string) string {
	return unsafe_text.replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;')
}

// <h2>Main&mdash;run the webserver</h2>
fn main() {
	mut app := &App{}
	// <p>Serve static files in the&nbsp;<code>../client/static/</code>
	//     subdirectory from the <code>/static</code> endpoint.</p>
	app.mount_static_folder_at(os.resource_abs_path('../client/static'), '/static')
	print('Open http://localhost:8080/ in a browser.\n')
	vweb.run(app, 8080)
}
