/// Copyright (C) 2022 Bryan A. Jones.
///
/// This file is part of the CodeChat Editor.
///
/// The CodeChat Editor is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
///
/// The CodeChat Editor is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
///
/// You should have received a copy of the GNU General Public License along with the CodeChat Editor. If not, see [http://www.gnu.org/licenses/](http://www.gnu.org/licenses/).
///
/// `webserver.rs` — Serve CodeChat Editor Client webpages
/// ======================================================
///
/// TODO: auto-reload when the current file changes on disk. Use [notify](https://docs.rs/notify/latest/notify/).
///
/// Imports
/// -------
///
/// ### Standard library
use std::{
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
use pulldown_cmark::{html, Options, Parser};
use regex::{Regex};
use fancy_regex::{Captures};
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
use super::lexer::compile_lexers;
use super::lexer::supported_languages::LANGUAGE_LEXER_ARR;
use crate::lexer::{source_lexer, CodeDocBlock, DocBlock, LanguageLexersCompiled};

/// Data structures
/// ---------------
#[derive(Serialize, Deserialize)]
/// Metadata about a source file sent along with it both to and from the client.
struct SourceFileMetadata {
    mode: String,
}

#[derive(Serialize, Deserialize)]
/// A simple structure for accepting JSON input to the `save_source` endpoint. Use a tuple since serdes can auto-generate a deserializer for it.
struct ClientSourceFile {
    metadata: SourceFileMetadata,
    // TODO: implement a serdes deserializer that would convert this directly to a CodeDocBlock?
    code_doc_block_arr: Vec<(String, Option<String>, String)>,
}

#[derive(Serialize)]
/// Define the structure of JSON responses when sending a source file from the `/fs` endpoint.
struct LexedSourceFile {
    metadata: SourceFileMetadata,
    code_doc_block_arr: Vec<CodeDocBlock>,
}

/// This defines the structure of JSON responses from the `save_source` endpoint.
#[derive(Serialize)]
struct ErrorResponse {
    success: bool,
    message: String,
}

// Globals
// -------
lazy_static! {
    /// Matches a bare drive letter.
    static ref DRIVE_LETTER_REGEX: Regex = Regex::new("^[a-zA-Z]:$").unwrap();
    /// Match the lexer directive in a source file.
    static ref LEXER_DIRECTIVE: Regex = Regex::new(r#"CodeChat Editor lexer: (\w+)"#).unwrap();
    // Matches all < and > symbols not within backticks. This allows the HTML to escape only the necessary symbols
    static ref ESCAPE_HTML_REGEX: fancy_regex::Regex = fancy_regex::Regex::new("[<>](?![^`]*`)").unwrap();
}

/// Save endpoint
/// -------------
#[put("/fs/{path:.*}")]
/// The Save button in the CodeChat Editor Client posts to this endpoint with the path of the file to save.
async fn save_source(
    // The path to save this file to.
    encoded_path: web::Path<String>,
    // The file to save plus metadata, stored in the `ClientSourceFile`
    client_source_file: web::Json<ClientSourceFile>,
    // Lexer info, needed to transform the `ClientSourceFile` into source code.
    language_lexers_compiled: web::Data<LanguageLexersCompiled<'_>>,
) -> impl Responder {
    // Given the mode, find the lexer.
    let lexer = match language_lexers_compiled
        .map_mode_to_lexer
        .get(client_source_file.metadata.mode.as_str())
    {
        Some(v) => v,
        None => return save_source_response(false, "Invalid mode"),
    };

    // Turn this back into code and doc blocks by filling in any missing comment delimiters.
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
                // If no delimiter is provided, use an inline comment (if available), then a block comment.
                delimiter: match &cdb.1 {
                    // The delimiter was provided. Simply use that.
                    Some(v) => v.to_string(),
                    // No delimiter was provided -- fill one in.
                    None => {
                        if let Some(ic) = inline_comment {
                            ic.to_string()
                        } else if let Some(bc) = block_comment {
                            bc.opening.to_string()
                        } else {
                            return save_source_response(
                                false,
                                "Neither inline nor block comments are defined for this language.",
                            );
                        }
                    }
                },
                contents: cdb.2.to_string(),
            })
        });
    }

    // Turn this vec of code/doc blocks into a string of source code.
    let mut file_contents = String::new();
    for code_doc_block in code_doc_block_vec {
        match code_doc_block {
            CodeDocBlock::DocBlock(doc_block) => {
                // Append a doc block, adding a space between the opening delimiter and the contents when necessary.
                let mut append_doc_block = |indent: &str, delimiter: &str, contents: &str| {
                    file_contents += indent;
                    file_contents += delimiter;
                    // Add a space between the delimiter and comment body, unless the comment was a newline or we’re at the end of the file.
                    if contents.is_empty() || contents == "\n" {
                        // Nothing to append in this case.
                    } else {
                        // Put a space between the delimiter and the contents.
                        file_contents += " ";
                    }
                    file_contents += contents;
                };

                let is_inline_delim = lexer
                    .language_lexer
                    .inline_comment_delim_arr
                    .contains(&doc_block.delimiter.as_str());

                // Build a comment based on the type of the delimiter.
                if is_inline_delim {
                    // Split the contents into a series of lines, adding the inline comment delimiter to each line.
                    for content_line in doc_block.contents.split_inclusive('\n') {
                        append_doc_block(&doc_block.indent, &doc_block.delimiter, content_line);
                    }
                } else {
                    // Determine the closing comment delimiter matching the provided opening delimiter.
                    let block_comment_closing_delimiter = match lexer
                        .language_lexer
                        .block_comment_delim_arr
                        .iter()
                        .position(|bc| bc.opening == doc_block.delimiter)
                    {
                        Some(index) => lexer.language_lexer.block_comment_delim_arr[index].closing,
                        None => {
                            return save_source_response(
                                false,
                                &format!(
                                    "Unknown block comment opening delimiter '{}'.",
                                    doc_block.delimiter
                                ),
                            )
                        }
                    };
                    // Produce the resulting block comment. They should always end with a newline.
                    assert!(&doc_block.contents.ends_with('\n'));
                    append_doc_block(
                        &doc_block.indent,
                        &doc_block.delimiter,
                        // Omit the newline, so we can instead put on the closing delimiter, then the newline.
                        &doc_block.contents[..&doc_block.contents.len() - 1],
                    );
                    file_contents = file_contents + " " + block_comment_closing_delimiter + "\n";
                }
            }
            CodeDocBlock::CodeBlock(contents) =>
            // This is code. Simply append it (by definition, indent and delimiter are empty).
            {
                file_contents += &contents
            }
        }
    }

    // Save this string to a file. Add a leading slash for Linux: this changes from `foo/bar.c` to `/foo/bar.c`. Windows already starts with a drive letter, such as `C:\foo\bar.c`, so no changes are needed.
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

/// A convenience method to fill out then return the `ErrorResponse` struct from the `save_source` endpoint.
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

/// Load endpoints
/// --------------
///
/// Redirect from the root of the filesystem to the actual root path on this OS.
async fn _root_fs_redirect() -> impl Responder {
    HttpResponse::TemporaryRedirect()
        .insert_header((header::LOCATION, "/fs/"))
        .finish()
}

/// The load endpoint: dispatch to support functions which serve either a directory listing, a CodeChat Editor file, or a normal file.
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
        // If there’s no drive letter yet, we will always use `dir_listing` to select a drive.
        return dir_listing("", Path::new("")).await;
    }
    // All other cases (for example, `C:\a\path\to\file.txt`) are OK.

    // For Linux/OS X, prepend a slash, so that `a/path/to/file.txt` becomes `/a/path/to/file.txt`.
    #[cfg(not(target_os = "windows"))]
    let fixed_path = "/".to_string() + &fixed_path;

    // On Windows, the returned path starts with `\\?\` per the [docs](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file#win32-file-namespaces). Handle any [errors](https://doc.rust-lang.org/std/fs/fn.canonicalize.html#errors).
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

    // It’s not a directory or a file…we give up. For simplicity, don’t handle symbolic links.
    html_not_found(&format!(
        "<p>The requested path <code>{}</code> is not a directory or a file.</p>",
        path_display(&canon_path)
    ))
}

/// ### Directory browser
///
/// Create a web page listing all files and subdirectories of the provided directory.
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
    // Sort them – case-insensitive on Windows, normally on Linux/OS X.
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
            // If this is a raw drive letter, then the path already ends with a slash, such as `C:/`. Don’t add a second slash in this case. Otherwise, add a slash to make `C:/foo` into `C:/foo/`.
            //
            // Likewise, the Linux root path of `/` already ends with a slash, while all other paths such a `/foo` don’t. To detect this, look for an empty `web_path`.
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

    HttpResponse::Ok().body(body)
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
    // *   A binary file (meaning we can’t read the contents as UTF-8): just serve it raw. Assume this is an image/video/etc.
    // *   A text file - first determine the type. Based on the type:
    //     *   If it’s an unknown type (such as a source file we don’t know or a plain text file): just serve it raw.
    //     *   If the client requested a table of contents, then serve it wrapped in a CodeChat TOC.
    //     *   If it’s Markdown or CCHTML, serve it wrapped in a CodeChat Document Editor.
    //     *   Otherwise, it must be a recognized file type. Serve it wrapped in a CodeChat Editor.
    if let Err(_err) = read_ret {
        // TODO: make a better decision, don’t duplicate code. The file type is unknown. Serve it raw, assuming it’s an image/video/etc.
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

    // The TOC is a simplified web page requiring no additional processing. The script ensures that all hyperlinks target the enclosing page, not just the iframe containing this page.
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

    // Determine the lexer to use for this file.
    let ace_mode;
    // First, search for a lexer directive in the file contents.
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
        // Otherwise, look up the lexer by the file’s extension.
        if let Some(llc) = language_lexers_compiled
            .map_ext_to_lexer_vec
            .get(ext.as_ref())
        {
            llc.first().unwrap()
        } else {
            // The file type is unknown. Serve it raw, assuming it’s an image/video/etc.
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

    // Lex the code and put it in a JSON structure.
    let mut code_doc_block_arr = if lexer.language_lexer.ace_mode == "codechat-html" {
        vec![CodeDocBlock::CodeBlock(file_contents)]
    } else {
        source_lexer(&file_contents, lexer)
    };

    // Read non-standard markdown conversions from [markdown\_conversions.json](../../client/src/markdown_conversions.json) file
    #[derive(Debug, Deserialize)]
    struct Conversion {
        conversions: Vec<Entry>
    }
    #[derive(Debug, Deserialize)]
    struct Entry {
        open_tag: String,
        close_tag: String,
        markdown: char,
    }
    let mut file = File::open("../client/src/markdown_conversions.json").await.unwrap();
    let mut buff = String::new();
    file.read_to_string(&mut buff).await.unwrap();
    let conversion: Conversion = serde_json::from_str(&buff).unwrap();

    for code_doc_block in &mut code_doc_block_arr {
        if let CodeDocBlock::DocBlock(ref mut doc_block) = code_doc_block {
            
            // For each all document block contents, iterate through each character to look for new Markdown conversions. If symbol is found, insert corresponding HTML tag.
            let mut output_contents = Vec::new();
            let mut tag_open = false;
            let doc_block_lines = doc_block.contents.lines();
            
            for line in doc_block_lines {
                let line_escaped = &ESCAPE_HTML_REGEX.replace_all(line, |cap: &Captures| {
                    match &cap[0] {
                        ">" => "&gt;",
                        "<" => "&lt;",
                        _ => panic!("Escape HTML Regex replace failed"),
                    }.to_string()
                }).to_string();
                let doc_block_contents = line_escaped.chars().collect::<Vec<char>>();
                for i in 0..doc_block_contents.len() {
                    let mut replaced = false;
                    for entry in &conversion.conversions {
                        // Check if character matches a markdown conversion.
                        if doc_block_contents[i] == entry.markdown {
                            let mut convert_to_html = false;

                            if tag_open {
                                for tag_char in entry.close_tag.chars() {
                                    output_contents.push(tag_char);
                                }
                                tag_open = false;
                                replaced = true;
                            } 
                            else {
                                // Check to see if characters need converted to HTML. Only convert when the text has the correct format & not just anytime the symbol appears.
                                for j in i..doc_block_contents.len() {
                                    if doc_block_contents[j] == entry.markdown {
                                        convert_to_html = true;
                                    }
                                }
                                // This is in place to ensure that subscript isn't converting '~~' so that strikethrough still works.
                                if i == 0 {
                                    if doc_block_contents[i+1] == entry.markdown {
                                            convert_to_html = false;
                                    }  
                                }
                                else if i == doc_block_contents.len() - 1 {
                                    if doc_block_contents[i-1] == entry.markdown {
                                        convert_to_html = false
                                    }
                                }
                                else {
                                    if doc_block_contents[i+1] == entry.markdown || doc_block_contents[i-1] == entry.markdown {
                                        convert_to_html = false;
                                    }
                                }
                                if convert_to_html {
                                    for tag_char in entry.open_tag.chars() {
                                        output_contents.push(tag_char);
                                    }
                                    tag_open = true;
                                    replaced = true;
                                }
                            }
                        } 
                    }
                    if !replaced {
                        {
                            output_contents.push(doc_block_contents[i]);
                        }
                    }
                }
                output_contents.push('\n');
            }
            let output: String = output_contents.into_iter().map(|i| i.to_string()).collect::<String>();
           
            // Convert doc blocks from Markdown to HTML
            let mut options = Options::all();
            options.remove(Options::ENABLE_SMART_PUNCTUATION);
            let parser = Parser::new_ext(&output, options);
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
    // Look for any script tags and prevent these from causing problems.
    let lexed_source_file_string = lexed_source_file_string.replace("</script>", "<\\/script>");

    // Look for a project file by searching the current directory, then all its parents, for a file named `toc.cchtml`.
    let mut is_project = false;
    // The number of directories between this file to serve (in `path`) and the toc file.
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

// Utilities
// ---------
//
// Given a `Path`, transform it into a displayable string.
fn path_display(p: &Path) -> String {
    let path_orig = p.to_string_lossy();
    if cfg!(windows) {
        path_orig[4..].to_string()
    } else {
        path_orig.to_string()
    }
}

// Return a Not Found (404) errors with the provided HTML body.
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

// Given text, escape it so it formats correctly as HTML. This is a translation of Python’s `html.escape` function.
fn escape_html(unsafe_text: &str) -> String {
    unsafe_text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// Webserver startup
// -----------------
#[actix_web::main]
pub async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        // Get the path to this executable. Assume that static files for the webserver are located relative to it.
        let exe_path = env::current_exe().unwrap();
        let exe_dir = exe_path.parent().unwrap();
        let mut client_static_path = PathBuf::from(exe_dir);
        client_static_path.push("../../../client/static");
        client_static_path = client_static_path.canonicalize().unwrap();

        // Start the server.
        App::new()
            .app_data(web::Data::new(compile_lexers(LANGUAGE_LEXER_ARR)))
            // Serve static files per the [docs](https://actix.rs/docs/static-files).
            .service(actix_files::Files::new(
                "/static",
                client_static_path.as_os_str(),
            ))
            // This endpoint serves the filesystem.
            .service(serve_fs)
            .service(save_source)
            // Reroute to the filesystem for typical user-requested URLs.
            .route("/", web::get().to(_root_fs_redirect))
            .route("/fs", web::get().to(_root_fs_redirect))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
