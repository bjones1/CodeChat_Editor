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
/// # `webserver.rs` -- Serve CodeChat Editor Client webpages
///
/// TODO: auto-reload when the current file changes on disk. Use
/// [notify](https://docs.rs/notify/latest/notify/).
///
/// ## Imports
///
/// ### Standard library
use std::{
    borrow::Cow,
    collections::HashMap,
    env,
    ffi::OsStr,
    path::{Path, PathBuf},
};

// ### Third-party
use actix_files;
use actix_web::{
    get,
    http::header,
    http::header::{ContentDisposition, ContentType},
    put, web, App, HttpRequest, HttpResponse, HttpServer, Responder,
};
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json;
use tokio::{
    fs::{self, DirEntry, File},
    io::AsyncReadExt,
};
use urlencoding::{self, encode};
#[cfg(target_os = "windows")]
use win_partitions::win_api::get_logical_drive;

// ### Local
use crate::lexer::compile_lexers;
use crate::lexer::supported_languages::LANGUAGE_LEXER_ARR;
use crate::lexer::LanguageLexersCompiled;
use crate::processing::{codechat_for_web_to_source, source_to_codechat_for_web};

/// ## Data structures
///
/// ### Translation between a local (traditional) source file and its web-editable, client-side representation
#[derive(Serialize, Deserialize)]
/// <a id="LexedSourceFile"></a>Define the JSON data structure used to represent
/// a source file in a web-editable format.
pub struct CodeChatForWeb<'a> {
    pub metadata: SourceFileMetadata,
    pub source: CodeMirror<'a>,
}

#[derive(Serialize, Deserialize)]
/// <a id="SourceFileMetadata"></a>Metadata about a source file sent along with
/// it both to and from the client. TODO: currently, this is too simple to
/// justify a struct. This allows for future growth -- perhaps the valid types
/// of comment delimiters?
pub struct SourceFileMetadata {
    pub mode: String,
}

pub type CodeMirrorDocBlocks<'a> = Vec<(
    // From -- the starting character this doc block is anchored to.
    usize,
    // To -- the ending character this doc block is anchored ti.
    usize,
    // Indent. This might be a borrowed reference or an owned reference.
    // When the lexer transforms code and doc blocks into this CodeMirror
    // format, a borrow from those existing doc blocks is more efficient.
    // However, deserialization from JSON requires ownership, since the
    // Actix web framework doesn't provide a place to borrow from. The
    // following variables are clone-on-write for the same reason.
    Cow<'a, String>,
    // delimiter
    Cow<'a, String>,
    // contents
    Cow<'a, String>,
)>;

#[derive(Serialize, Deserialize)]
/// The format used by CodeMirror to serialize/deserialize editor contents.
/// TODO: Link to JS code where this data structure is defined.
pub struct CodeMirror<'a> {
    /// The document being edited.
    pub doc: String,
    /// Doc blocks
    pub doc_blocks: CodeMirrorDocBlocks<'a>,
}

/// This defines the structure of JSON responses returned by the `save_source`
/// endpoint. TODO: Link to where this is used in the JS.
#[derive(Serialize)]
struct ErrorResponse {
    success: bool,
    message: String,
}

/// TODO: A better name for this enum.
pub enum FileType<'a> {
    // Text content, but not a CodeChat file
    Text(String),
    // A CodeChat file; the struct contains the file's contents translated to
    // CodeMirror.
    CodeChat(CodeChatForWeb<'a>),
}

// ## Globals
lazy_static! {
    /// Matches a bare drive letter. Only needed on Windows.
    static ref DRIVE_LETTER_REGEX: Regex = Regex::new("^[a-zA-Z]:$").unwrap();
}

/// ## Save endpoint
#[put("/fs/{path:.*}")]
/// The Save button in the CodeChat Editor Client posts to this endpoint.
async fn save_source<'a>(
    // The path to save this file to. See
    // [Path](https://actix.rs/docs/extractors#path), which extracts parameters
    // from the request's path.
    encoded_path: web::Path<String>,
    // The source file to save, in web format. See
    // [JSON](https://actix.rs/docs/extractors#json), which deserializes the
    // request body into the provided struct (here, `CodeChatForWeb`).
    codechat_for_web: web::Json<CodeChatForWeb<'a>>,
    // Lexer info, needed to transform the `CodeChatForWeb` into source code.
    // See
    // [Data](https://docs.rs/actix-web/4.3.1/actix_web/web/struct.Data.html),
    // which provides access to application-wide data. (TODO: link to where this
    // is defined.)
    language_lexers_compiled: web::Data<LanguageLexersCompiled<'_>>,
) -> impl Responder {
    // Translate from the CodeChatForWeb format to the contents of a source
    // file.
    let file_contents = match codechat_for_web_to_source(
        codechat_for_web.into_inner(),
        language_lexers_compiled.into_inner().as_ref(),
    ) {
        Ok(r) => r,
        Err(message) => return save_source_response(false, &message),
    };

    // Save this string to a file. Add a leading slash for Linux/OS X: this
    // changes from `foo/bar.c` to `/foo/bar.c`. Windows paths already starts
    // with a drive letter, such as `C:\foo\bar.c`, so no changes are needed.
    let save_file_path = if cfg!(windows) {
        "".to_string()
    } else {
        "/".to_string()
    } + &encoded_path;
    match fs::write(save_file_path.to_string(), file_contents).await {
        Ok(v) => v,
        Err(err) => {
            return save_source_response(
                false,
                &format!("Unable to save file {}: {}.", save_file_path, err),
            )
        }
    }

    save_source_response(true, "")
}

/// A convenience method to fill out then return the `ErrorResponse` struct from
/// the `save_source` endpoint.
fn save_source_response(success: bool, message: &str) -> HttpResponse {
    let response = ErrorResponse {
        success,
        message: message.to_string(),
    };
    let body = serde_json::to_string(&response).unwrap();
    if success {
        HttpResponse::Ok()
            .content_type(ContentType::json())
            .body(body)
    } else {
        HttpResponse::UnprocessableEntity()
            .content_type(ContentType::json())
            .body(body)
    }
}

/// ## Load endpoints
///
/// Redirect from the root of the filesystem to the actual root path on this OS.
async fn _root_fs_redirect() -> impl Responder {
    HttpResponse::TemporaryRedirect()
        .insert_header((header::LOCATION, "/fs/"))
        .finish()
}

/// The load endpoint: dispatch to support functions which serve either a
/// directory listing, a CodeChat Editor file, or a normal file.
#[get("/fs/{path:.*}")]
async fn serve_fs(
    req: HttpRequest,
    language_lexers_compiled: web::Data<LanguageLexersCompiled<'_>>,
    orig_path: web::Path<String>,
) -> impl Responder {
    let mut fixed_path = orig_path.to_string();
    #[cfg(target_os = "windows")]
    // On Windows, a path of `drive_letter:` needs a `/` appended.
    if DRIVE_LETTER_REGEX.is_match(&fixed_path) {
        fixed_path += "/";
    } else if fixed_path.is_empty() {
        // If there's no drive letter yet, we will always use `dir_listing` to
        // select a drive.
        return dir_listing("", Path::new("")).await;
    }
    // All other cases (for example, `C:\a\path\to\file.txt`) are OK.

    // For Linux/OS X, prepend a slash, so that `a/path/to/file.txt` becomes
    // `/a/path/to/file.txt`.
    #[cfg(not(target_os = "windows"))]
    let fixed_path = "/".to_string() + &fixed_path;

    // Handle any
    // [errors](https://doc.rust-lang.org/std/fs/fn.canonicalize.html#errors).
    let canon_path = match Path::new(&fixed_path).canonicalize() {
        Ok(p) => p,
        Err(err) => {
            return html_not_found(&format!(
                "<p>The requested path <code>{}</code> is not valid: {}.</p>",
                fixed_path, err
            ))
        }
    };
    if canon_path.is_dir() {
        return dir_listing(orig_path.as_str(), &canon_path).await;
    } else if canon_path.is_file() {
        return serve_file(&canon_path, &req, language_lexers_compiled).await;
    }

    // It's not a directory or a file...we give up. For simplicity, don't handle
    // symbolic links.
    html_not_found(&format!(
        "<p>The requested path <code>{}</code> is not a directory or a file.</p>",
        path_display(&canon_path)
    ))
}

/// ### Directory browser
///
/// Create a web page listing all files and subdirectories of the provided
/// directory.
async fn dir_listing(web_path: &str, dir_path: &Path) -> HttpResponse {
    // Special case on Windows: list drive letters.
    #[cfg(target_os = "windows")]
    if dir_path == Path::new("") {
        // List drive letters in Windows
        let mut drive_html = String::new();
        let logical_drives = match get_logical_drive() {
            Ok(v) => v,
            Err(err) => return html_not_found(&format!("Unable to list drive letters: {}.", err)),
        };
        for drive_letter in logical_drives {
            drive_html.push_str(&format!(
                "<li><a href='/fs/{}:/'>{}:</a></li>\n",
                drive_letter, drive_letter
            ));
        }

        return HttpResponse::Ok().body(html_wrapper(&format!(
            "<h1>Drives</h1>
<ul>
{}
</ul>
",
            drive_html
        )));
    }

    // List each file/directory with appropriate links.
    let mut unwrapped_read_dir = match fs::read_dir(dir_path).await {
        Ok(p) => p,
        Err(err) => {
            return html_not_found(&format!(
                "<p>Unable to list the directory {}: {}/</p>",
                path_display(dir_path),
                err
            ))
        }
    };

    // Get a listing of all files and directories
    let mut files: Vec<DirEntry> = Vec::new();
    let mut dirs: Vec<DirEntry> = Vec::new();
    loop {
        match unwrapped_read_dir.next_entry().await {
            Ok(v) => {
                if let Some(dir_entry) = v {
                    let file_type = match dir_entry.file_type().await {
                        Ok(x) => x,
                        Err(err) => {
                            return html_not_found(&format!(
                                "<p>Unable to determine the type of {}: {}.",
                                path_display(&dir_entry.path()),
                                err
                            ))
                        }
                    };
                    if file_type.is_file() {
                        files.push(dir_entry);
                    } else {
                        // Group symlinks with dirs.
                        dirs.push(dir_entry);
                    }
                } else {
                    break;
                }
            }
            Err(err) => {
                return html_not_found(&format!("<p>Unable to read file in directory: {}.", err))
            }
        };
    }
    // Sort them -- case-insensitive on Windows, normally on Linux/OS X.
    #[cfg(target_os = "windows")]
    let file_name_key = |a: &DirEntry| {
        Ok::<String, std::ffi::OsString>(a.file_name().into_string()?.to_lowercase())
    };
    #[cfg(not(target_os = "windows"))]
    let file_name_key =
        |a: &DirEntry| Ok::<String, std::ffi::OsString>(a.file_name().into_string()?);
    files.sort_unstable_by_key(file_name_key);
    dirs.sort_unstable_by_key(file_name_key);

    // Put this on the resulting webpage. List directories first.
    let mut dir_html = String::new();
    for dir in dirs {
        let dir_name = match dir.file_name().into_string() {
            Ok(v) => v,
            Err(err) => {
                return html_not_found(&format!(
                    "<p>Unable to decode directory name '{:?}' as UTF-8.",
                    err
                ))
            }
        };
        let encoded_dir = encode(&dir_name);
        dir_html += &format!(
            "<li><a href='/fs/{}{}{}'>{}</a></li>\n",
            web_path,
            // If this is a raw drive letter, then the path already ends with a
            // slash, such as `C:/`. Don't add a second slash in this case.
            // Otherwise, add a slash to make `C:/foo` into `C:/foo/`.
            //
            // Likewise, the Linux root path of `/` already ends with a slash,
            // while all other paths such a `/foo` don't. To detect this, look
            // for an empty `web_path`.
            if web_path.ends_with('/') || web_path.is_empty() {
                ""
            } else {
                "/"
            },
            encoded_dir,
            dir_name
        );
    }

    // List files second.
    let mut file_html = String::new();
    for file in files {
        let file_name = match file.file_name().into_string() {
            Ok(v) => v,
            Err(err) => {
                return html_not_found(&format!(
                    "<p>Unable to decode file name {:?} as UTF-8.",
                    err
                ))
            }
        };
        let encoded_file = encode(&file_name);
        file_html += &format!(
            "<li><a href=\"/fs/{}/{}\" target=\"_blank\">{}</a></li>\n",
            web_path, encoded_file, file_name
        );
    }
    let body = format!(
        "<h1>Directory {}</h1>
<h2>Subdirectories</h2>
<ul>
{}
</ul>
<h2>Files</h2>
<ul>
{}
</ul>
",
        path_display(dir_path),
        dir_html,
        file_html
    );

    HttpResponse::Ok().body(html_wrapper(&body))
}

// ### Serve a CodeChat Editor Client webpage
async fn serve_file(
    file_path: &Path,
    req: &HttpRequest,
    language_lexers_compiled: web::Data<LanguageLexersCompiled<'_>>,
) -> HttpResponse {
    let raw_dir = file_path.parent().unwrap();
    // Use a lossy conversion, since this is UI display, not filesystem access.
    let dir = escape_html(path_display(raw_dir).as_str());
    let name = escape_html(&file_path.file_name().unwrap().to_string_lossy());
    let ext = &file_path
        .extension()
        .unwrap_or_else(|| OsStr::new(""))
        .to_string_lossy();

    // Get the `mode` and `test` query parameters.
    let empty_string = "".to_string();
    let query_params = web::Query::<HashMap<String, String>>::from_query(req.query_string());
    let (mode, is_test_mode) = match query_params {
        Ok(query) => (
            query.get("mode").unwrap_or(&empty_string).clone(),
            query.get("test").is_some(),
        ),
        Err(_err) => (empty_string, false),
    };
    let is_toc = mode == "toc";

    // Look for a project file by searching the current directory, then all its
    // parents, for a file named `toc.md`.
    let mut is_project = false;
    // The number of directories between this file to serve (in `path`) and the
    // toc file.
    let mut path_to_toc = PathBuf::new();
    let mut current_dir = file_path.to_path_buf();
    loop {
        let mut project_file = current_dir.clone();
        project_file.push("toc.md");
        if project_file.is_file() {
            path_to_toc.pop();
            path_to_toc.push("toc.md");
            is_project = true;
            break;
        }
        if !current_dir.pop() {
            break;
        }
        path_to_toc.push("../");
    }

    // Read the file.
    let mut file_contents = String::new();
    let read_ret = match File::open(file_path).await {
        Ok(fc) => fc,
        Err(err) => {
            return html_not_found(&format!(
                "<p>Error opening file {}: {}.",
                path_display(file_path),
                err
            ))
        }
    }
    .read_to_string(&mut file_contents)
    .await;

    // Categorize the file:
    //
    // - A binary file (meaning we can't read the contents as UTF-8): just serve
    //   it raw. Assume this is an image/video/etc.
    // - A text file - first determine the type. Based on the type:
    //   - If it's an unknown type (such as a source file we don't know or a
    //     plain text file): just serve it raw.
    //   - If the client requested a table of contents, then serve it wrapped in
    //     a CodeChat TOC.
    //   - If it's Markdown, serve it wrapped in a CodeChat Document Editor.
    //   - Otherwise, it must be a recognized file type. Serve it wrapped in a
    //     CodeChat Editor.
    if let Err(_err) = read_ret {
        // TODO: make a better decision, don't duplicate code. The file type is
        // unknown. Serve it raw, assuming it's an image/video/etc.
        match actix_files::NamedFile::open_async(file_path).await {
            Ok(v) => {
                let res = v
                    .set_content_disposition(ContentDisposition {
                        disposition: header::DispositionType::Inline,
                        parameters: vec![],
                    })
                    .into_response(req);
                return res;
            }
            Err(err) => {
                return html_not_found(&format!(
                    "<p>Error opening file {}: {}.",
                    path_display(file_path),
                    err
                ))
            }
        }
    }

    let file_type_wrapped = source_to_codechat_for_web(
        file_contents,
        ext,
        is_toc,
        language_lexers_compiled.into_inner().as_ref(),
    );
    if let Err(err_string) = file_type_wrapped {
        return html_not_found(&err_string);
    }

    let codechat_for_web = match file_type_wrapped.unwrap() {
        FileType::Text(_text_file_contents) => {
            // The file type is unknown. Serve it raw.
            match actix_files::NamedFile::open_async(file_path).await {
                Ok(v) => {
                    let res = v.into_response(req);
                    return res;
                }
                Err(err) => {
                    return html_not_found(&format!(
                        "<p>Error opening file {}: {}.",
                        path_display(file_path),
                        err
                    ))
                }
            }
        }
        // TODO.
        FileType::CodeChat(html) => html,
    };

    if is_toc {
        // The TOC is a simplified web page requiring no additional processing.
        // The script ensures that all hyperlinks target the enclosing page, not
        // just the iframe containing this page.
        return HttpResponse::Ok().body(format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{} - The CodeChat Editor</title>

<link rel="stylesheet" href="/static/css/CodeChatEditor.css">
<link rel="stylesheet" href="/static/css/CodeChatEditorSidebar.css">
<script>
    addEventListener("DOMContentLoaded", (event) => {{
        document.querySelectorAll("a").forEach((a_element) => {{
            a_element.target = "_parent"
        }});
    }});
</script>
</head>
<body>
{}
</body>
</html>
"#,
            name, codechat_for_web.source.doc
        ));
    }

    let codechat_for_web_json_string = match serde_json::to_string(&codechat_for_web) {
        Ok(v) => v,
        Err(err) => {
            return html_not_found(&format!(
                "<p>Unable to convert code_doc_block_arr to JSON: {}.</p>",
                err
            ))
        }
    }
    // Look for any script tags and prevent these from causing problems.
    .replace("</script>", "<\\/script>");

    // For project files, add in the sidebar.
    let (sidebar_iframe, sidebar_css) = if is_project {
        (
            format!(
                r##"<iframe src="{}?mode=toc" id="CodeChat-sidebar"></iframe>"##,
                path_to_toc.to_string_lossy()
            ),
            r#"<link rel="stylesheet" href="/static/css/CodeChatEditorProject.css">"#,
        )
    } else {
        ("".to_string(), "")
    };

    // Add in content when testing.
    let testing_src = if is_test_mode {
        r#"
        <link rel="stylesheet" href="https://unpkg.com/mocha/mocha.css" />
        <script src="https://unpkg.com/mocha/mocha.js"></script>
        "#
    } else {
        ""
    };

    // Build and return the webpage.
    HttpResponse::Ok().body(format!(
        r##"<!DOCTYPE html>
<html lang="en">
    <head>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1">
        <title>{} - The CodeChat Editor</title>

        <link rel="stylesheet" href="/static/bundled/CodeChatEditor.css">
        <script type="module">
            import {{ page_init, on_keydown, on_save }} from "/static/bundled/CodeChatEditor{}.js"
            // <p>Make these accessible on the onxxx handlers below. See <a
            //         href="https://stackoverflow.com/questions/44590393/es6-modules-undefined-onclick-function-after-import">SO</a>.
            // </p>
            window.CodeChatEditor = {{ on_keydown, on_save }};

            page_init(
{},
);
        </script>
        {}
        {}
    </head>
    <body onkeydown="CodeChatEditor.on_keydown(event);">
        {}
        <div id="CodeChat-contents">
            <div id="CodeChat-top">
                <div id="CodeChat-filename">
                    <p>
                        <button onclick="CodeChatEditor.on_save();" id="CodeChat-save-button">
                            <span class="CodeChat-hotkey">S</span>ave
                        </button>
                        - {} - {}
                    </p>
                </div>
                <div id="CodeChat-menu"></div>
            </div>
            <div id="CodeChat-body"></div>
            <div id="CodeChat-bottom"></div>
            <div id="mocha"></div>
        </div>
    </body>
</html>
"##, name, if is_test_mode { "-test" } else { "" }, codechat_for_web_json_string, testing_src, sidebar_css, sidebar_iframe, name, dir
    ))
}

// ## Utilities
//
// Given a `Path`, transform it into a displayable string.
fn path_display(p: &Path) -> String {
    let path_orig = p.to_string_lossy();
    if cfg!(windows) {
        // On Windows, the returned path starts with `\\?\` per the
        // [docs](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file#win32-file-namespaces).
        path_orig[4..].to_string()
    } else {
        path_orig.to_string()
    }
}

// Return a Not Found (404) error with the provided HTML body.
fn html_not_found(msg: &str) -> HttpResponse {
    HttpResponse::NotFound().body(html_wrapper(msg))
}

// Wrap the provided HTML body in DOCTYPE/html/head tags.
fn html_wrapper(body: &str) -> String {
    format!(
        "<!DOCTYPE html>
<html lang=\"en\">
    <head>
        <meta charset=\"UTF-8\">
        <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">
        <title>The CodeChat Editor</title>
    </head>
    <body>
        {}
    </body>
</html>",
        body
    )
}

// Given text, escape it so it formats correctly as HTML. This is a translation
// of Python's `html.escape` function.
fn escape_html(unsafe_text: &str) -> String {
    unsafe_text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// ## Webserver startup
#[actix_web::main]
pub async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        // Get the path to this executable. Assume that static files for the
        // webserver are located relative to it.
        let exe_path = env::current_exe().unwrap();
        let exe_dir = exe_path.parent().unwrap();
        let mut client_static_path = PathBuf::from(exe_dir);
        client_static_path.push("../../../client/static");
        client_static_path = client_static_path.canonicalize().unwrap();

        // Start the server.
        App::new()
            // Provide data to all endpoints -- the compiler lexers.
            .app_data(web::Data::new(compile_lexers(LANGUAGE_LEXER_ARR)))
            // Serve static files per the
            // [docs](https://actix.rs/docs/static-files).
            .service(actix_files::Files::new(
                "/static",
                client_static_path.as_os_str(),
            ))
            // These endpoints serve the files to/from the filesystem.
            .service(serve_fs)
            .service(save_source)
            // Reroute to the filesystem for typical user-requested URLs.
            .route("/", web::get().to(_root_fs_redirect))
            .route("/fs", web::get().to(_root_fs_redirect))
    })
    .bind(("127.0.0.1", 8081))?
    .run()
    .await
}

// ## Tests
//
// As mentioned in the lexer.rs tests, Rust
// [almost mandates](https://doc.rust-lang.org/book/ch11-03-test-organization.html)
// putting tests in the same file as the source. Here's some
// [good information](http://xion.io/post/code/rust-unit-test-placement.html) on
// how to put tests in another file, for future refactoring reference.
#[cfg(test)]

// ### TODO!
mod tests {}
