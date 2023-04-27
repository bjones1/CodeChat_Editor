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
/// <p>TODO: auto-reload when the current file changes on disk. Use <a
///         href="https://docs.rs/notify/latest/notify/">notify</a>.</p>
/// <h2>Imports</h2>
/// <h3>Standard library</h3>
use std::{
    collections::HashMap,
    env,
    ffi::OsStr,
    path::{Path, PathBuf},
};

// <h3>Third-party</h3>
use actix_files;
use actix_web::{
    get,
    http::header,
    http::header::{ContentDisposition, ContentType},
    put, web, App, HttpRequest, HttpResponse, HttpServer, Responder,
};
use lazy_static::lazy_static;
use pulldown_cmark::{html, Options, Parser};
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

// <h3>Local</h3>
use super::lexer::compile_lexers;
use super::lexer::supported_languages::LANGUAGE_LEXER_ARR;
use crate::lexer::{source_lexer, CodeDocBlock, DocBlock, LanguageLexersCompiled};

/// <h2>Data structures</h2>
#[derive(Serialize, Deserialize)]
/// <p><a id="SourceFileMetadata"></a>Metadata about a source file sent along
///     with it both to and from the client.</p>
struct SourceFileMetadata {
    mode: String,
}

#[derive(Serialize, Deserialize)]
/// <p><a id="ClientSourceFile"></a>A simple structure for accepting JSON input
///     to the <code>save_source</code> endpoint. Use a tuple since serdes can
///     auto-generate a deserializer for it.</p>
struct ClientSourceFile {
    metadata: SourceFileMetadata,
    // <p>TODO: implement a serdes deserializer that would convert this directly
    //     to a CodeDocBlock?</p>
    code_doc_block_arr: Vec<(String, Option<String>, String)>,
}

#[derive(Serialize)]
/// <p><a id="LexedSourceFile"></a>Define the structure of JSON responses when
///     sending a source file from the <code>/fs</code> endpoint.</p>
struct LexedSourceFile {
    metadata: SourceFileMetadata,
    code_doc_block_arr: Vec<CodeDocBlock>,
}

/// <p>This defines the structure of JSON responses from the
///     <code>save_source</code> endpoint.</p>
#[derive(Serialize)]
struct ErrorResponse {
    success: bool,
    message: String,
}

// <h2>Globals</h2>
lazy_static! {
    /// <p>Matches a bare drive letter.</p>
    static ref DRIVE_LETTER_REGEX: Regex = Regex::new("^[a-zA-Z]:$").unwrap();
    /// <p>Match the lexer directive in a source file.</p>
    static ref LEXER_DIRECTIVE: Regex = Regex::new(r#"CodeChat Editor lexer: (\w+)"#).unwrap();
}

/// <h2>Save endpoint</h2>
#[put("/fs/{path:.*}")]
/// <p>The Save button in the CodeChat Editor Client posts to this endpoint with
///     the path of the file to save.</p>
async fn save_source(
    // <p>The path to save this file to.</p>
    encoded_path: web::Path<String>,
    // <p>The file to save plus metadata, stored in the
    //     <code>ClientSourceFile</code></p>
    client_source_file: web::Json<ClientSourceFile>,
    // <p>Lexer info, needed to transform the <code>ClientSourceFile</code> into
    //     source code.</p>
    language_lexers_compiled: web::Data<LanguageLexersCompiled<'_>>,
) -> impl Responder {

    // <p>Takes the source file and the lexer and saves the source as a string.
    // </p>
    let (file_contents, _) = save_source_as_string(client_source_file, language_lexers_compiled);

    // <h3>Save file</h3>
    // <p>Save this string to a file. Add a leading slash for Linux: this
    //     changes from&nbsp;<code>foo/bar.c</code> to <code>/foo/bar.c</code>.
    //     Windows already starts with a drive letter, such as
    //     <code>C:\foo\bar.c</code>, so no changes are needed.</p>
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

    // <p>This function takes in a file with code and doc blocks and outputs a
    //     string of the contents for testing.</p>
fn save_source_as_string(
    // <p>The file to save plus metadata, stored in the
    //     <code>ClientSourceFile</code></p>
    client_source_file: web::Json<ClientSourceFile>,
    // <p>Lexer info, needed to transform the <code>ClientSourceFile</code> into
    //     source code.</p>
    language_lexers_compiled: web::Data<LanguageLexersCompiled<'_>>,
) -> (String, impl Responder) {
    // <p>Given the mode, find the lexer.</p>
    let lexer = match language_lexers_compiled
        .map_mode_to_lexer
        .get(client_source_file.metadata.mode.as_str())
    {
        Some(v) => v,
        None => return (String::new(), save_source_response(false, "Invalid mode")),
    };

    // <p>Turn this back into code and doc blocks by filling in any missing
    //     comment delimiters.</p>
    let inline_comment = lexer.language_lexer.inline_comment_delim_arr.first();
    let block_comment = lexer.language_lexer.block_comment_delim_arr.first();
    let mut code_doc_block_vec: Vec<CodeDocBlock> = Vec::new();
    let some_empty = Some("".to_string());
    for cdb in &client_source_file.code_doc_block_arr {
        let is_code_block = cdb.0.is_empty() && cdb.1 == some_empty;
        code_doc_block_vec.push(if is_code_block {
            CodeDocBlock::CodeBlock(cdb.2.to_string())
        } else {
            CodeDocBlock::DocBlock(DocBlock {
                indent: cdb.0.to_string(),
                // <p>If no delimiter is provided, use an inline comment (if
                //     available), then a block comment.</p>
                delimiter: match &cdb.1 {
                    // <p>The delimiter was provided. Simply use that.</p>
                    Some(v) => v.to_string(),
                    // <p>No delimiter was provided -- fill one in.</p>
                    None => {
                        if let Some(ic) = inline_comment {
                            ic.to_string()
                        } else if let Some(bc) = block_comment {
                            bc.opening.to_string()
                        } else {
                            return (String::new(), save_source_response(
                                false,
                                "Neither inline nor block comments are defined for this language.",
                            ));
                        }
                    }
                },
                contents: cdb.2.to_string(),
            })
        });
    }

    // <p>Turn this vec of code/doc blocks into a string of source code.</p>
    let mut file_contents = String::new();
    for code_doc_block in code_doc_block_vec {
        match code_doc_block {
            CodeDocBlock::DocBlock(doc_block) => {
                // <p>Append a doc block, adding a space between the opening
                //     delimiter and the contents when necessary.</p>
                let mut append_doc_block = |indent: &str, delimiter: &str, contents: &str| {
                    file_contents += indent;
                    file_contents += delimiter;
                    // <p>Add a space between the delimiter and comment body,
                    //     unless the comment was a newline or we're at the end
                    //     of the file.</p>
                    if contents.is_empty() || contents == "\n" {
                        // <p>Nothing to append in this case.</p>
                    } else {
                        // <p>Put a space between the delimiter and the
                        //     contents.</p>
                        file_contents += " ";
                    }
                    file_contents += contents;
                };

                let is_inline_delim = lexer
                    .language_lexer
                    .inline_comment_delim_arr
                    .contains(&doc_block.delimiter.as_str());

                // <p>Build a comment based on the type of the delimiter.</p>
                if is_inline_delim {
                    // <p>Split the contents into a series of lines, adding the
                    //     inline comment delimiter to each line.</p>
                    for content_line in doc_block.contents.split_inclusive('\n') {
                        append_doc_block(&doc_block.indent, &doc_block.delimiter, content_line);
                    }
                } else {
                    // <p>Determine the closing comment delimiter matching the
                    //     provided opening delimiter.</p>
                    let block_comment_closing_delimiter = match lexer
                        .language_lexer
                        .block_comment_delim_arr
                        .iter()
                        .position(|bc| bc.opening == doc_block.delimiter)
                    {
                        Some(index) => lexer.language_lexer.block_comment_delim_arr[index].closing,
                        None => {
                            return (String::new(), save_source_response(
                                false,
                                &format!(
                                    "Unknown block comment opening delimiter '{}'.",
                                    doc_block.delimiter
                                ),
                            ))
                        }
                    };
                    // <p>Produce the resulting block comment. They should
                    //     always end with a newline.</p>
                    assert!(&doc_block.contents.ends_with('\n'));
                    append_doc_block(
                        &doc_block.indent,
                        &doc_block.delimiter,
                        // <p>Omit the newline, so we can instead put on the
                        //     closing delimiter, then the newline.</p>
                        &doc_block.contents[..&doc_block.contents.len() - 1],
                    );
                    file_contents = file_contents + " " + block_comment_closing_delimiter + "\n";
                }
            }
            CodeDocBlock::CodeBlock(contents) =>
            // <p>This is code. Simply append it (by definition, indent and
            //     delimiter are empty).</p>
            {
                file_contents += &contents
            }
        }
    }
    return (file_contents, save_source_response(false,""))
}

/// <p>A convenience method to fill out then return the
///     <code>ErrorResponse</code> struct from the <code>save_source</code>
///     endpoint.</p>
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

/// <h2>Load endpoints</h2>
/// <p>Redirect from the root of the filesystem to the actual root path on this
///     OS.</p>
async fn _root_fs_redirect() -> impl Responder {
    HttpResponse::TemporaryRedirect()
        .insert_header((header::LOCATION, "/fs/"))
        .finish()
}

/// <p>The load endpoint: dispatch to support functions which serve either a
///     directory listing, a CodeChat Editor file, or a normal file.</p>
#[get("/fs/{path:.*}")]
async fn serve_fs(
    req: HttpRequest,
    language_lexers_compiled: web::Data<LanguageLexersCompiled<'_>>,
    orig_path: web::Path<String>,
) -> impl Responder {
    let mut fixed_path = orig_path.to_string();
    #[cfg(target_os = "windows")]
    // <p>On Windows, a path of <code>drive_letter:</code> needs a
    //     <code>/</code> appended.</p>
    if DRIVE_LETTER_REGEX.is_match(&fixed_path) {
        fixed_path += "/";
    } else if fixed_path.is_empty() {
        // <p>If there's no drive letter yet, we will always use
        //     <code>dir_listing</code> to select a drive.</p>
        return dir_listing("", Path::new("")).await;
    }
    // <p>All other cases (for example, <code>C:\a\path\to\file.txt</code>) are
    //     OK.</p>

    // <p>For Linux/OS X, prepend a slash, so that
    //     <code>a/path/to/file.txt</code> becomes
    //     <code>/a/path/to/file.txt</code>.</p>
    #[cfg(not(target_os = "windows"))]
    let fixed_path = "/".to_string() + &fixed_path;

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

    // <p>It's not a directory or a file...we give up. For simplicity, don't
    //     handle symbolic links.</p>
    html_not_found(&format!(
        "<p>The requested path <code>{}</code> is not a directory or a file.</p>",
        path_display(&canon_path)
    ))
}

/// <h3>Directory browser</h3>
/// <p>Create a web page listing all files and subdirectories of the provided
///     directory.</p>
async fn dir_listing(web_path: &str, dir_path: &Path) -> HttpResponse {
    // <p>Special case on Windows: list drive letters.</p>
    #[cfg(target_os = "windows")]
    if dir_path == Path::new("") {
        // <p>List drive letters in Windows</p>
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
            "<li><a href='/fs/{}{}{}'>{}</a></li>\n",
            web_path,
            // <p>If this is a raw drive letter, then the path already ends with
            //     a slash, such as <code>C:/</code>. Don't add a second slash
            //     in this case. Otherwise, add a slash to make
            //     <code>C:/foo</code> into <code>C:/foo/</code>.</p>
            // <p>Likewise, the Linux root path of <code>/</code> already ends
            //     with a slash, while all other paths such a <code>/foo</code>
            //     don't. To detect this, look for an empty
            //     <code>web_path</code>.</p>
            if web_path.ends_with('/') || web_path.is_empty() {
                ""
            } else {
                "/"
            },
            encoded_dir,
            dir_name
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

    // <p>Get the <code>mode</code> and <code>test</code> query parameters.</p>
    let empty_string = "".to_string();
    let query_params = web::Query::<HashMap<String, String>>::from_query(req.query_string());
    let (mode, is_test_mode) = match query_params {
        Ok(query) => (
            query.get("mode").unwrap_or(&empty_string).clone(),
            query.get("test").is_some(),
        ),
        Err(_err) => (empty_string, false),
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
    
    // <p>Categorize the file:</p>
    // <ul>
    //     <li>A binary file (meaning we can't read the contents as UTF-8): just
    //         serve it raw. Assume this is an image/video/etc.</li>
    //     <li>A text file - first determine the type. Based on the type:
    //         <ul>
    //             <li>If it's an unknown type (such as a source file we don't
    //                 know or a plain text file): just serve it raw.</li>
    //             <li>If the client requested a table of contents, then serve
    //                 it wrapped in a CodeChat TOC.</li>
    //             <li>If it's Markdown or CCHTML, serve it wrapped in a
    //                 CodeChat Document Editor.</li>
    //             <li>Otherwise, it must be a recognized file type. Serve it
    //                 wrapped in a CodeChat Editor.</li>
    //         </ul>
    //     </li>
    // </ul>
    if let Err(_err) = read_ret {
        // <p>TODO: make a better decision, don't duplicate code. The file type
        //     is unknown. Serve it raw, assuming it's an image/video/etc.</p>
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

    // <p>The TOC is a simplified web page requiring no additional processing.
    //     The script ensures that all hyperlinks target the enclosing page, not
    //     just the iframe containing this page.</p>
    if mode == "toc" {
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
            name, file_contents
        ));
    }

    // <p>Determine the lexer to use for this file.</p>
    let ace_mode;
    // <p>First, search for a lexer directive in the file contents.</p>
    let lexer = if let Some(captures) = LEXER_DIRECTIVE.captures(&file_contents) {
        ace_mode = captures[1].to_string();
        match language_lexers_compiled
            .map_mode_to_lexer
            .get(&ace_mode.as_ref())
        {
            Some(v) => v,
            None => return html_not_found(&format!("<p>Unknown lexer type {}.</p>", &ace_mode)),
        }
    } else {
        // <p>Otherwise, look up the lexer by the file's extension.</p>
        if let Some(llc) = language_lexers_compiled
            .map_ext_to_lexer_vec
            .get(ext.as_ref())
        {
            llc.first().unwrap()
        } else {
            // <p>The file type is unknown. Serve it raw, assuming it's an
            //     image/video/etc.</p>
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
    };

    // <p>Lex the code and put it in a JSON structure.</p>
    let mut code_doc_block_arr = if lexer.language_lexer.ace_mode == "codechat-html" {
        vec![CodeDocBlock::CodeBlock(file_contents)]
    } else {
        source_lexer(&file_contents, lexer)
    };

    // <p>Convert doc blocks from Markdown to HTML</p>
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    for code_doc_block in &mut code_doc_block_arr {
        if let CodeDocBlock::DocBlock(ref mut doc_block) = code_doc_block {
            let parser = Parser::new_ext(&doc_block.contents, options);
            let mut html_output = String::new();
            html::push_html(&mut html_output, parser);
            doc_block.contents = html_output;
        }
    }

    let lexed_source_file = LexedSourceFile {
        metadata: SourceFileMetadata {
            mode: lexer.language_lexer.ace_mode.to_string(),
        },
        code_doc_block_arr,
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

    // <p>For project files, add in the sidebar.</p>
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

    // <p>Add in content when testing.</p>
    let testing_src = if is_test_mode {
        r#"
        <link rel="stylesheet" href="https://unpkg.com/mocha/mocha.css" />
        <script src="https://unpkg.com/mocha/mocha.js"></script>
        "#
    } else {
        ""
    };

    // <p>Build and return the webpage.</p>
    HttpResponse::Ok().body(format!(
        r##"<!DOCTYPE html>
<html lang="en">
    <head>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1">
        <title>{} - The CodeChat Editor</title>

        <link rel="stylesheet" href="/static/webpack/CodeChatEditor.css">
        <script type="module">
            import {{ page_init, on_keydown, on_save }} from "/static/webpack/CodeChatEditor{}.js"
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
"##, name, if is_test_mode { "-test" } else { "" }, lexed_source_file_string, testing_src, sidebar_css, sidebar_iframe, name, dir
    ))
}

// <h2>Utilities</h2>
// <p>Given a <code>Path</code>, transform it into a displayable string.</p>
fn path_display(p: &Path) -> String {
    let path_orig = p.to_string_lossy();
    if cfg!(windows) {
        path_orig[4..].to_string()
    } else {
        path_orig.to_string()
    }
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
        // <p>Get the path to this executable. Assume that static files for the
        //     webserver are located relative to it.</p>
        let exe_path = env::current_exe().unwrap();
        let exe_dir = exe_path.parent().unwrap();
        let mut client_static_path = PathBuf::from(exe_dir);
        client_static_path.push("../../../client/static");
        client_static_path = client_static_path.canonicalize().unwrap();

        // <p>Start the server.</p>
        App::new()   
            .app_data(web::Data::new(compile_lexers(LANGUAGE_LEXER_ARR)))
            // <p>Serve static files per the <a
            //         href="https://actix.rs/docs/static-files">docs</a>.</p>
            .service(actix_files::Files::new(
                "/static",
                client_static_path.as_os_str(),
            ))
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

// <h2>&nbsp;</h2>
// <h2>Tests</h2>
// <p>As mentioned in the lexer.rs tests, Rust <a
//         href="https://doc.rust-lang.org/book/ch11-03-test-organization.html">almost
//         mandates</a> putting tests in the same file as the source. Here's
//     some <a
//         href="http://xion.io/post/code/rust-unit-test-placement.html">good
//         information</a> on how to put tests in another file, for future
//     refactoring reference.</p>
// <p>&nbsp;</p>
#[cfg(test)]

// <h3>Save Endpoint Testing</h3>
mod tests {
    use actix_web::web::{
        Data,
    };
    use crate::webserver::{
        compile_lexers,
        LANGUAGE_LEXER_ARR,
        ClientSourceFile,
        SourceFileMetadata,
    };
    use crate::webserver::save_source_as_string;

    #[test]
    fn test_save_endpoint() 
    {
        assert_eq!(1, 1); 

        // <p><strong>Pass nothing to the function.</strong></p>
        let test_source_file = ClientSourceFile{
            metadata: SourceFileMetadata {
                mode: "python".to_string()
            },
            code_doc_block_arr: vec![
                ("".to_string(),Some("".to_string()),"".to_string()),
            ] 
        }; 
        let llc = Data::new(compile_lexers(LANGUAGE_LEXER_ARR)); 
        let (file_contents, _) = save_source_as_string(actix_web::web::Json(test_source_file), llc);
        assert_eq!(file_contents, "");
        
// <p style="padding-left: 40px;"><strong>Pass without comment
//         delimiter<br></strong></p>
        let test_source_file = ClientSourceFile{
            metadata: SourceFileMetadata {
                mode: "python".to_string()
            },
            code_doc_block_arr: vec![
                ("".to_string(),Some("".to_string()),"Test".to_string()),
            ] 
        }; 
        let llc = Data::new(compile_lexers(LANGUAGE_LEXER_ARR)); 
        let (file_contents, _) = save_source_as_string(actix_web::web::Json(test_source_file), llc);
        assert_eq!(file_contents, "Test");
//
        // <p><strong>Pass only an indent</strong></p>
        let test_source_file = ClientSourceFile{
            metadata: SourceFileMetadata {
                mode: "python".to_string()
            },
            code_doc_block_arr: vec![
                (" ".to_string(),Some("".to_string()),"".to_string()),
            ] 
        }; 
        let llc = Data::new(compile_lexers(LANGUAGE_LEXER_ARR)); 
        let (file_contents, _) = save_source_as_string(actix_web::web::Json(test_source_file), llc);
        assert_eq!(file_contents, " ");
        
        
        // <p><strong>Pass a test comment.</strong></p>
        let test_source_file = ClientSourceFile{
            metadata: SourceFileMetadata {
                mode: "python".to_string()
            },
            code_doc_block_arr: vec![
                ("".to_string(),Some("#".to_string()),"Test".to_string()),
            ] 
        }; 
        let llc = Data::new(compile_lexers(LANGUAGE_LEXER_ARR)); 
        let (file_contents, _) = save_source_as_string(actix_web::web::Json(test_source_file), llc);
        assert_eq!(file_contents, "# Test");
    
        
        // <p><strong>Pass a block comment</strong></p>
        let test_source_file = ClientSourceFile{
            metadata: SourceFileMetadata {
                mode: "python".to_string()
            },
            code_doc_block_arr: vec![
                ("".to_string(),Some("".to_string()),"/* This is a block comment */".to_string()),
            ] 
        }; 
        let llc = Data::new(compile_lexers(LANGUAGE_LEXER_ARR)); 
        let (file_contents, _) = save_source_as_string(actix_web::web::Json(test_source_file), llc);
        assert_eq!(file_contents, "This is a block comment");
    
    
        // <p><strong>Pass an inline comment</strong></p>
        let test_source_file = ClientSourceFile{
            metadata: SourceFileMetadata {
                mode: "python".to_string()
            },
            code_doc_block_arr: vec![
                ("".to_string(),Some("".to_string()),"This is some code // with an inline comment".to_string()),
            ] 
        }; 
        let llc = Data::new(compile_lexers(LANGUAGE_LEXER_ARR)); 
        let (file_contents, _) = save_source_as_string(actix_web::web::Json(test_source_file), llc);
        assert_eq!(file_contents, "This is some code // with an inline comment");
    }
    
    
}
    
    
    
