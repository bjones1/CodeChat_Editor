// <!-- CodeChat-lexer: vlang -->
// <h1><code>CodeChatEditorServer.v</code>&mdash;A simple server for the
//     CodeChat Editor</h1>
// <p>This server provides the user the ability to browse the local
//     filesystem; clicking on a CodeChat Editor-supported source file
//     then loads it in the CodeChat Editor. It also enables the CodeChat
//     Editor to save modified files back to the local filesystem.</p>
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

// <h2>Constants</h2>
const (
	codechat_extensions = [
		'.cc',
		'.cpp',
		'.html',
		'.js',
		'.py',
		'.v',
		'.cchtml',
	]
)

// <h2>Endpoints</h2>
// <p>Redirect from the root endpoint to the filesystem.</p>
['/']
fn (mut app App) root_redirect() vweb.Result {
	return app.redirect('/fs')
}

// <p>This endpoint serves files from the local filesystem. vweb requires
//     me to declare it twice in order to get an empty path (here) or a
//     path with something after <code>/fs</code> (in the following
//     function).</p>
['/fs']
fn (mut app App) serve_fs_bare() vweb.Result {
	// <p>On Windows, assume the C drive as the root of the filesystem. TODO:
	//     provide some way to list drives / change drives from the HTML GUI.
	// </p>
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

// <p><a id="save_file"></a>A <code>PUT</code> to a filename writes the
//     provided data to that file; this is used by the <a
//         href="CodeChatEditor.js#save">save function</a>.</p>
['/fs/:path...'; put]
fn (mut app App) save_file(path string) vweb.Result {
	// <p>For Unix, restore the leading <code>/</code> to the beginning of
	//     the path.</p>
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

// <p>Serve either a directory listing, with special links for CodeChat
//     Editor files, or serve a CodeChat Editor file or a normal file.
// </p>
fn (mut app App) serve_fs_(path string) vweb.Result {
	// <p>The provided <code>path</code> may need fixing, since it lacks an
	//     initial <code>/</code>.</p>
	mut fixed_path := path
	if os.user_os() == 'windows' {
		// <p>On Windows, a path of <code>drive_letter:</code> needs a
		//     <code>/</code> appended.</p>
		mut regex_drive_letter := regex.regex_opt('^[a-zA-Z]:$') or {
			panic('Regex failed to compile.')
		}
		if regex_drive_letter.matches_string(path) {
			fixed_path += '/'
		}
		// <p>All other cases (for example, <code>C:\a\path\to\file.txt</code>)
		//     are OK.</p>
	} else {
		// <p>For Linux/OS X, prepend a slash, so that
		//     <code>a/path/to/file.txt</code> becomes
		//     <code>/a/path/to/file.txt</code>.</p>
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
				// <p>Use an absolute path, instead of a relative path, in case the URL
				//     of this directory doesn't end with a <code>/</code>. Detecting
				//     this case is hard, since vweb removes the trailing <code>/</code>
				//     even if it's there!</p>
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
	} else {
		ext := os.file_ext(abs_path)
		if ext in codechat_extensions {
			codechat_file_contents := os.read_file(abs_path) or { return app.not_found() }
			// <p>Transform this into a CodeChat (Doc?) Editor webpage.</p>
			return app.html(if ext == '.cchtml' {
				codechat_doc_editor_html(codechat_file_contents, abs_path)
			} else {
				codechat_editor_html(codechat_file_contents, abs_path)
			})
		}
		// <p>It's not a CodeChat Editor file -- just serve the file.</p>
		return app.file(abs_path)
	}
}

// <h2>CodeChat Editor support</h2>
// <p>Given the source code for a file and its path, return the HTML to
//     present this in the CodeChat Editor.</p>
fn codechat_editor_html(source_code string, path string) string {
	dir := escape_html(os.dir(path))
	name := escape_html(os.base(path))
	ext := os.file_ext(path)
	return '<!DOCTYPE html>
<html lang="en">
	<head>
		<meta charset="UTF-8">
		<meta name="viewport" content="width=device-width, initial-scale=1">
		<title>$name - The CodeChat Editor</title>

		<script src="https://cdnjs.cloudflare.com/ajax/libs/ace/1.9.5/ace.min.js"></script>
		<script src="https://cdn.tiny.cloud/1/rrqw1m3511pf4ag8c5zao97ad7ymvnhqu6z0995b1v63rqb5/tinymce/6/tinymce.min.js" referrerpolicy="origin"></script>
		<script src="https://cdnjs.cloudflare.com/ajax/libs/js-beautify/1.14.5/beautify-html.min.js"></script>
		<script src="/static/CodeChatEditor.js"></script>
		<script>
			const on_save = on_save_codechat;
			on_dom_content_loaded(() => open_lp(
"${quote_script_string(source_code)}",
"${quote_string(ext)}"));
		</script>

		<link rel="stylesheet" href="/static/css/CodeChatEditor.css">
	</head>
	<body onkeydown="on_keydown(event);">
		<div id="CodeChat-top">
			<div id="CodeChat-filename">
				<p>
					$name - $dir -
					<button disabled onclick="on_save_as(on_save_doc);" id="CodeChat-save-as-button">
						Save as
					</button>
					<button onclick="on_save();" id="CodeChat-save-button">
						<span class="CodeChat-hotkey">S</span>ave
					</button>
				</p>
			</div>
			<div id="CodeChat-menu"></div>
		</div>
		<div id="CodeChat-body">
		</div>
		<div id="CodeChat-bottom">
		</div>
	</body>
</html>
'
}

// <p>Given the source code for a file and its path, return the HTML to
//     present this in the CodeChat Document Editor.</p>
fn codechat_doc_editor_html(source_code string, path string) string {
	dir := escape_html(os.dir(path))
	name := escape_html(os.base(path))
	return '<!DOCTYPE html>
<html lang="en">
	<head>
		<meta charset="UTF-8">
		<meta name="viewport" content="width=device-width, initial-scale=1">
		<title>$name - The CodeChat Editor</title>

		<script src="https://cdn.tiny.cloud/1/rrqw1m3511pf4ag8c5zao97ad7ymvnhqu6z0995b1v63rqb5/tinymce/6/tinymce.min.js" referrerpolicy="origin"></script>
		<script src="https://cdnjs.cloudflare.com/ajax/libs/js-beautify/1.14.5/beautify-html.min.js"></script>
		<script src="/static/CodeChatEditor.js"></script>
		<script>
			const on_save = on_save_doc;
			on_dom_content_loaded(make_editors);
		</script>

		<link rel="stylesheet" href="/static/css/CodeChatEditor.css">
	</head>
	<body onkeydown="on_keydown(event);">
		<div id="CodeChat-top">
			<div id="CodeChat-filename">
				<p>
					$name - $dir -
					<button disabled onclick="on_save_as(on_save_doc);" id="CodeChat-save-as-button">
						Save as
					</button>
					<button onclick="on_save();" id="CodeChat-save-button">
						<span class="CodeChat-hotkey">S</span>ave
					</button>
				</p>
			</div>
			<div id="CodeChat-menu"></div>
		</div>
		<div id="CodeChat-body">
			<div class="CodeChat-TinyMCE">
$source_code
			</div>
		</div>
		<div id="CodeChat-bottom">
		</div>
	</body>
</html>
'
}

// <p>For JavaScript, escape any double quotes and convert newlines, so
//     it's safe to enclose the returned string in double quotes.</p>
fn quote_string(s string) string {
	return s.replace(r'\', r'\\').replace('"', r'\"').replace('\r\n', r'\n').replace('\r',
		r'\n').replace('\n', r'\n')
}

// <p>In addition to quoting strings, also split up an ending
//     <code>&lt;/script&gt;</code> tags, since this string is placed
//     inside a <code>&lt;script&gt;</code> tag.</p>
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
	// <p>Serve static files from the <code>/static</code> endpoint.</p>
	app.mount_static_folder_at(os.resource_abs_path('.'), '/static')
	print('Open http://localhost:8080/ in a browser.\n')
	vweb.run(app, 8080)
}
