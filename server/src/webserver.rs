use std::path::Path;

use actix_files;
use actix_web::{get, http::header, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use lazy_static::lazy_static;
use regex::Regex;
use tokio::fs::{self, DirEntry};
use urlencoding::{self, encode};

lazy_static! {
    /// Matches a bare drive letter.
    static ref DRIVE_LETTER_REGEX: Regex = Regex::new("^[a-zA-Z]:$").unwrap();
}

/// Redirect from the root of the filesystem to the actual root path on this OS.
async fn _root_fs_redirect() -> impl Responder {
    // <p>On Windows, assume the C drive as the root of the filesystem. TODO:
    //     provide some way to list drives / change drives from the HTML GUI.</p>
    #[cfg(target_os = "windows")]
    let redirect_location = urlencoding::encode("C:").into_owned() + &"/";

    // On Linux, redirect to the root of the filesystem.
    #[cfg(not(target_os = "windows"))]
    let redirect_location = "";

    HttpResponse::TemporaryRedirect()
        .insert_header((header::LOCATION, "/fs/".to_string() + &redirect_location))
        .finish()
}

/// <p>Serve either a directory listing, with special links for CodeChat Editor
///     files, or serve a CodeChat Editor file or a normal file.</p>
#[get("/fs/{path:.*}")]
async fn serve_fs(req: HttpRequest) -> impl Responder {
    let encoded_path = req.match_info().get("path").unwrap();
    let mut fixed_path = urlencoding::decode(encoded_path).expect("UTF-8");

    #[cfg(target_os = "windows")]
    {
        // <p>On Windows, a path of <code>drive_letter:</code> needs a <code>/</code>
        //     appended.</p>
        if DRIVE_LETTER_REGEX.is_match(&fixed_path) {
            fixed_path += "/";
        }
        // <p>All other cases (for example, <code>C:\a\path\to\file.txt</code>) are
        //     OK.</p>
    }

    // <p>For Linux/OS X, prepend a slash, so that <code>a/path/to/file.txt</code>
    //     becomes <code>/a/path/to/file.txt</code>.</p>
    #[cfg(not(target_os = "windows"))]
    let mut fixed_path = "/".to_string() + fixed_path;

    // On Windows, the returned path starts with <code>\\?\</code> per the <a href="https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file#win32-file-namespaces">docs</a>. Handle any <a href="https://doc.rust-lang.org/std/fs/fn.canonicalize.html#errors">errors</a>.
    let canon_path = match Path::new(fixed_path.as_ref()).canonicalize() {
        Ok(p) => p,
        Err(err) => {
            return html_not_found(&format!(
                "<p>The requested path <code>{}</code> is not valid: {}.</p>",
                fixed_path, err
            ))
        }
    };
    if canon_path.is_dir() {
        return dir_listing(&encoded_path, &canon_path).await;
    } else if canon_path.is_file() {
        return serve_file(&canon_path).await;
    }

    // It's not a directory or a file...we give up. TODO: remove the odd prefix.
    html_not_found(&format!(
        "<p>The requested path <code>{}</code> is not a directory or a file.</p>",
        path_display(&canon_path)
    ))
}

/// Create a web page listing all files and subdirectories of the provided directory.
async fn dir_listing(web_path: &str, dir_path: &Path) -> HttpResponse {
    // <p>List each file/directory with appropriate links.</p>
    let mut unwrapped_read_dir = match fs::read_dir(dir_path).await {
        Ok(p) => p,
        Err(err) => {
            return html_not_found(&format!(
                "<p>Unable to list the directory {}: {}/</p>",
                path_display(&dir_path),
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
    let file_name_ord = |a: &DirEntry, b: &DirEntry| {
        a.file_name()
            .into_string()
            .unwrap()
            .to_lowercase()
            .partial_cmp(&b.file_name().into_string().unwrap().to_lowercase())
            .unwrap()
    };
    #[cfg(not(target_os = "windows"))]
    let file_name_ord =
        |a: &DirEntry, b: &DirEntry| a.file_name().partial_cmp(&b.file_name()).unwrap();
    files.sort_by(file_name_ord);
    dirs.sort_by(file_name_ord);

    // Put this on the resulting webpage.
    let mut dir_html = String::new();
    for dir in dirs {
        let dir_name = dir.file_name().into_string().unwrap();
        let encoded_dir = encode(&dir_name);
        dir_html += &format!(
            "<li><a href='/fs/{}/{}'>{}</a></li>\n",
            web_path, encoded_dir, dir_name
        );
    }
    let mut file_html = String::new();
    for file in files {
        file_html += &format!("<li>{}</li>\n", file.file_name().into_string().unwrap());
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

    return HttpResponse::Ok().body(body);
}

async fn serve_file(file_path: &Path) -> HttpResponse {
    return HttpResponse::Ok().body(format!("TODO: serve file {}.", file_path.display()));
}

fn path_display(p: &Path) -> String {
    let path_orig = p.to_string_lossy();
    #[cfg(target_os = "windows")]
    return path_orig[4..].to_string();
    #[cfg(not(target_os = "windows"))]
    path_orig
}

fn html_not_found(msg: &str) -> HttpResponse {
    HttpResponse::NotFound().body(html_wrapper(msg))
}

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

#[actix_web::main]
pub async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            // Serve static files per the <a href="https://actix.rs/docs/static-files">docs</a>.
            .service(actix_files::Files::new("/static", "../client/static"))
            // This endpoint serves the filesystem.
            .service(serve_fs)
            // Reroute to the filesystem for typical user-requested URLs.
            .route("/", web::get().to(_root_fs_redirect))
            .route("/fs", web::get().to(_root_fs_redirect))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
