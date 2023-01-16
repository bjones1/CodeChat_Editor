/// <details>
///     <summary>Copyright (C) 2022 Bryan A. Jones.</summary>
///     <p>This file is part of the CodeChat Editor.</p>
///     <p>The CodeChat Editor is free software: you can redistribute it and/or
///         modify it under the terms of the GNU General Public License as
///         published by the Free Software Foundation, either version 3 of the
///         License, or (at your option) any later version.</p>
///     <p>The CodeChat Editor is distributed in the hope that it will be
///         useful, but WITHOUT ANY WARRANTY; without even the implied warranty
///         of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
///         General Public License for more details.</p>
///     <p>You should have received a copy of the GNU General Public License
///         along with the CodeChat Editor. If not, see <a
///             href="http://www.gnu.org/licenses/">http://www.gnu.org/licenses/</a>.
///     </p>
/// </details>
/// <h1><code>webserver.rs</code> &mdash; Serve CodeChat Editor Client webpages
/// </h1>
/// <h2>Imports</h2>
/// <h3>Standard library</h3>
use std::{
    collections::HashMap,
    ffi::OsStr,
    path::{Path, PathBuf},
};

// <h3>Third-party</h3>
use actix_files;
use actix_web::{
    get, http::header, http::header::ContentType, put, web, App, HttpRequest, HttpResponse,
    HttpServer, Responder,
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

// <h3>Local</h3>
use super::lexer::compile_lexers;
use super::lexer::supported_languages::LANGUAGE_LEXER_ARR;
use crate::lexer::{source_lexer, CodeDocBlock, LanguageLexersCompiled};

// <h2>Data structures</h2>
#[derive(Serialize, Deserialize)]
struct SourceFileMetadata {
    mode: String,
}

#[derive(Serialize, Deserialize)]
struct ClientSourceFile {
    metadata: SourceFileMetadata,
    // <p>TODO: implement a serdes deserializer that would convert this directly
    //     to a CodeDocBlock?</p>
    code_doc_block_arr: Vec<(String, Option<String>, String)>,
}

#[derive(Serialize)]
struct LexedSourceFile {
    metadata: SourceFileMetadata,
    code_doc_block_arr: Vec<CodeDocBlock>,
}

#[derive(Serialize)]
struct ErrorResponse {
    success: bool,
    message: String,
}

// <h2>Globals</h2>
lazy_static! {
    /// <p>Matches a bare drive letter.</p>
    static ref DRIVE_LETTER_REGEX: Regex = Regex::new("^[a-zA-Z]:$").unwrap();
}

/// <h2>Endpoints</h2>
#[put("/fs/{path:.*}")]
async fn save_source(
    encoded_path: web::Path<String>,
    client_source_file: web::Json<ClientSourceFile>,
    language_lexers_compiled: web::Data<LanguageLexersCompiled<'_>>,
) -> impl Responder {
    // <p>Given the mode, find the lexer.</p>
    let lexer = match language_lexers_compiled
        .map_mode_to_lexer
        .get(client_source_file.metadata.mode.as_str())
    {
        Some(v) => v,
        None => return save_source_response(false, "Invalid mode"),
    };

    // <p>Turn this back into code and doc blocks</p>
    let inline_comment = lexer.language_lexer.inline_comment_delim_arr.first();
    let block_comment = lexer.language_lexer.block_comment_delim_arr.first();
    let mut code_doc_block_vec: Vec<CodeDocBlock> = Vec::new();
    for cdb in &client_source_file.code_doc_block_arr {
        code_doc_block_vec.push(CodeDocBlock {
            indent: cdb.0.to_string(),
            delimiter: match &cdb.1 {
                Some(v) => v.to_string(),
                // <p>If no delimiter is provided, use an inline comment (if
                //     available), then a block comment.</p>
                None => {
                    if let Some(ic) = inline_comment {
                        ic.to_string()
                    } else {
                        if let Some(bc) = block_comment {
                            bc.opening.to_string()
                        } else {
                            return save_source_response(
                                false,
                                "Neither inline nor block comments are defined for this language.",
                            );
                        }
                    }
                }
            },
            contents: cdb.2.to_string(),
        });
    }

    // <p>Turn this into a string.</p>
    let mut contents = String::new();
    for code_doc_block in code_doc_block_vec {
        contents.push_str(&format!(
            "{}{}{}{}",
            code_doc_block.indent,
            code_doc_block.delimiter,
            if code_doc_block.delimiter == "" {
                ""
            } else {
                " "
            },
            code_doc_block.contents
        ));
    }

    // <p>Save this string to a file</p>
    let save_file_path = if cfg!(windows) {
        "".to_string()
    } else {
        "/".to_string()
    } + &encoded_path;
    match fs::write(save_file_path.to_string(), contents).await {
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

/// <p>Redirect from the root of the filesystem to the actual root path on this
///     OS.</p>
async fn _root_fs_redirect() -> impl Responder {
    // <p>On Windows, assume the C drive as the root of the filesystem. TODO:
    //     provide some way to list drives / change drives from the HTML GUI.
    // </p>
    #[cfg(target_os = "windows")]
    let redirect_location = urlencoding::encode("C:").into_owned() + "/";

    // <p>On Linux, redirect to the root of the filesystem.</p>
    #[cfg(not(target_os = "windows"))]
    let redirect_location = "";

    HttpResponse::TemporaryRedirect()
        .insert_header((header::LOCATION, "/fs/".to_string() + &redirect_location))
        .finish()
}

/// <p>Serve either a directory listing, with special links for CodeChat Editor
///     files, or serve a CodeChat Editor file or a normal file.</p>
#[get("/fs/{path:.*}")]
async fn serve_fs(
    req: HttpRequest,
    language_lexers_compiled: web::Data<LanguageLexersCompiled<'_>>,
    orig_path: web::Path<String>,
) -> impl Responder {
    let mut fixed_path = orig_path.to_string();
    #[cfg(target_os = "windows")]
    {
        // <p>On Windows, a path of <code>drive_letter:</code> needs a
        //     <code>/</code> appended.</p>
        if DRIVE_LETTER_REGEX.is_match(&fixed_path) {
            fixed_path += "/";
        }
        // <p>All other cases (for example, <code>C:\a\path\to\file.txt</code>)
        //     are OK.</p>
    }

    // <p>For Linux/OS X, prepend a slash, so that
    //     <code>a/path/to/file.txt</code> becomes
    //     <code>/a/path/to/file.txt</code>.</p>
    #[cfg(not(target_os = "windows"))]
    let mut fixed_path = "/".to_string() + fixed_path;

    // <p>On Windows, the returned path starts with <code>\\?\</code> per the <a
    //         href="https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file#win32-file-namespaces">docs</a>.
    //     Handle any <a
    //         href="https://doc.rust-lang.org/std/fs/fn.canonicalize.html#errors">errors</a>.
    // </p>
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

    // <p>It's not a directory or a file...we give up. TODO: remove the odd
    //     prefix.</p>
    html_not_found(&format!(
        "<p>The requested path <code>{}</code> is not a directory or a file.</p>",
        path_display(&canon_path)
    ))
}

/// <h3>Directory browser</h3>
/// <p>Create a web page listing all files and subdirectories of the provided
///     directory.</p>
async fn dir_listing(web_path: &str, dir_path: &Path) -> HttpResponse {
    // <p>List each file/directory with appropriate links.</p>
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

    // <p>Get a listing of all files and directories</p>
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
                        // <p>Group symlinks with dirs.</p>
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
    // <p>Sort them -- case-insensitive on Windows, normally on Linux/OS X.</p>
    #[cfg(target_os = "windows")]
    let file_name_key = |a: &DirEntry| {
        Ok::<String, std::ffi::OsString>(a.file_name().into_string()?.to_lowercase())
    };
    #[cfg(not(target_os = "windows"))]
    let file_name_key =
        |a: &DirEntry| Ok::<String, std::ffi::OsString>(a.file_name().into_string()?);
    files.sort_unstable_by_key(file_name_key);
    dirs.sort_unstable_by_key(file_name_key);

    // <p>Put this on the resulting webpage. List directories first.</p>
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
            "<li><a href='/fs/{}/{}'>{}</a></li>\n",
            web_path, encoded_dir, dir_name
        );
    }

    // <p>List files second.</p>
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

    HttpResponse::Ok().body(body)
}

// <h3>Serve a CodeChat Editor Client webpage</h3>
async fn serve_file(
    file_path: &Path,
    req: &HttpRequest,
    language_lexers_compiled: web::Data<LanguageLexersCompiled<'_>>,
) -> HttpResponse {
    let raw_dir = file_path.parent().unwrap();
    // <p>Use a lossy conversion, since this is UI display, not filesystem
    //     access.</p>
    let dir = escape_html(path_display(raw_dir).as_str());
    let name = escape_html(&file_path.file_name().unwrap().to_string_lossy());
    let ext = &file_path
        .extension()
        .unwrap_or_else(|| OsStr::new(""))
        .to_string_lossy();

    // <p>Get the <code>mode</code> query parameter.</p>
    let empty_string = "".to_string();
    let mode = match web::Query::<HashMap<String, String>>::from_query(req.query_string()) {
        Ok(query) => query.get("mode").unwrap_or(&empty_string).clone(),
        Err(_err) => empty_string,
    };

    // <p>Read the file.</p>
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
    if let Err(err) = read_ret {
        return html_not_found(&format!(
            "<p>Error reading file {}: {}</p>",
            path_display(file_path),
            err
        ));
    }

    // <p>The TOC is a simplified web page requiring no additional processing.
    //     The script ensures that all hyperlinks target the enclosing page, not
    //     just the iframe containing this page.</p>
    if mode == "toc" {
        return HttpResponse::Ok().body(format!(
            r##"<!DOCTYPE html>
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
"##,
            name, file_contents
        ));
    }

    // <p>Look for a special tag to specify the lexer.</p>
    let lexer = if ext == "cchtml" {
        language_lexers_compiled
            .map_mode_to_lexer
            .get("codechat-html")
            .unwrap()
    } else if let Some(index) = file_contents.find("CodeChat Editor lexer: ") {
        // <p>TODO: look for newline, space, or EOF, and pick out Ace mode?</p>
        &language_lexers_compiled.language_lexer_compiled_vec[0]
    } else {
        // <p>Otherwise, look up the lexer by the file's extension.</p>
        if let Some(llc) = language_lexers_compiled
            .map_ext_to_lexer_vec
            .get(ext.as_ref())
        {
            llc.first().unwrap()
        } else {
            return html_not_found(&format!(
                "<p>Unknown file type for file {}.</p>",
                path_display(file_path)
            ));
        }
    };

    // <p>Lex the code and put it in a JSON structure.</p>
    let code_doc_block_arr = if lexer.language_lexer.ace_mode == "codechat-html" {
        vec![CodeDocBlock {
            indent: "".to_string(),
            delimiter: "".to_string(),
            contents: file_contents,
        }]
    } else {
        source_lexer(&file_contents, lexer)
    };
    let lexed_source_file = LexedSourceFile {
        metadata: SourceFileMetadata {
            mode: lexer.language_lexer.ace_mode.to_string(),
        },
        code_doc_block_arr: code_doc_block_arr,
    };
    let lexed_source_file_string = match serde_json::to_string(&lexed_source_file) {
        Ok(v) => v,
        Err(err) => {
            return html_not_found(&format!(
                "<p>Unable to convert code_doc_block_arr to JSON: {}.</p>",
                err
            ))
        }
    };
    // <p>Look for any script tags and prevent these from causing problems.</p>
    let lexed_source_file_string = lexed_source_file_string.replace("</script>", "<\\/script>");

    // <p>Look for a project file by searching the current directory, then all
    //     its parents, for a file named <code>toc.cchtml</code>.</p>
    let mut is_project = false;
    // <p>The number of directories between this file to serve (in
    //     <code>path</code>) and the toc file.</p>
    let mut path_to_toc = PathBuf::new();
    let mut current_dir = file_path.to_path_buf();
    loop {
        let mut project_file = current_dir.clone();
        project_file.push("toc.cchtml");
        if project_file.is_file() {
            path_to_toc.pop();
            path_to_toc.push("toc.cchtml");
            is_project = true;
            break;
        }
        if !current_dir.pop() {
            break;
        }
        path_to_toc.push("../");
    }

    let (sidebar_iframe, sidebar_css) = if is_project {
        (
            format!(
                r##"<iframe src="{}?mode=toc" id="CodeChat-sidebar"></iframe>"##,
                path_to_toc.to_string_lossy()
            ),
            r##"<link rel="stylesheet" href="/static/css/CodeChatEditorProject.css">"##.to_string(),
        )
    } else {
        ("".to_string(), "".to_string())
    };

    return HttpResponse::Ok().body(format!(
        r##"<!DOCTYPE html>
<html lang="en">
    <head>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1">
        <title>{} - The CodeChat Editor</title>

        <link rel="stylesheet" href="/static/webpack/CodeChatEditor.css">
        <script type="module">
            import {{ page_init, on_keydown, on_save }} from "/static/webpack/CodeChatEditor.js"
            // <p>Make these accessible on the onxxx handlers below. See <a
            //         href="https://stackoverflow.com/questions/44590393/es6-modules-undefined-onclick-function-after-import">SO</a>.
            // </p>
            window.CodeChatEditor = {{ on_keydown, on_save }};

            page_init(
{},
);
        </script>
        <link rel="stylesheet" href="/static/css/CodeChatEditor.css">
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
        </div>
    </body>
</html>
"##, name, lexed_source_file_string, sidebar_css, sidebar_iframe, name, dir
    ));
}

// <h2>Utilities</h2>
// <p>Given a <code>Path</code>, transform it into a displayable string.</p>
fn path_display(p: &Path) -> String {
    let path_orig = p.to_string_lossy();
    #[cfg(target_os = "windows")]
    return path_orig[4..].to_string();
    #[cfg(not(target_os = "windows"))]
    path_orig
}

// <p>Return a Not Found (404) errors with the provided HTML body.</p>
fn html_not_found(msg: &str) -> HttpResponse {
    HttpResponse::NotFound().body(html_wrapper(msg))
}

// <p>Wrap the provided HTML body in DOCTYPE/html/head tags.</p>
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

// <p>Given text, escape it so it formats correctly as HTML. This is a
//     translation of Python's <code>html.escape</code> function.</p>
fn escape_html(unsafe_text: &str) -> String {
    unsafe_text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// <h2>Webserver startup</h2>
#[actix_web::main]
pub async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .app_data(web::Data::new(compile_lexers(LANGUAGE_LEXER_ARR)))
            // <p>Serve static files per the <a
            //         href="https://actix.rs/docs/static-files">docs</a>.</p>
            .service(actix_files::Files::new("/static", "../client/static"))
            // <p>This endpoint serves the filesystem.</p>
            .service(serve_fs)
            .service(save_source)
            // <p>Reroute to the filesystem for typical user-requested URLs.</p>
            .route("/", web::get().to(_root_fs_redirect))
            .route("/fs", web::get().to(_root_fs_redirect))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
