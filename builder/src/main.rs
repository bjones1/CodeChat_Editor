// Copyright (C) 2023 Bryan A. Jones.
//
// This file is part of the CodeChat Editor. The CodeChat Editor is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, either
// version 3 of the License, or (at your option) any later version.
//
// The CodeChat Editor is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along with
// the CodeChat Editor. If not, see
// [http://www.gnu.org/licenses](http://www.gnu.org/licenses).
//
// `main.rs` -- Entrypoint for the CodeChat Editor Builder
// =======================================================
//
// This code uses [dist](https://opensource.axo.dev/cargo-dist/book/) as a part
// of the release process. To update the `./release.yaml` file this tool
// creates:
//
// 1.  Edit `server/dist-workspace.toml`: change `allow-dirty` to `[]`.
// 2.  Run `dist init` and accept the defaults, then run `dist generate`.
// 3.  Review changes to `./release.yaml`, reapplying hand edits.
// 4.  Revert the changes to `server/dist-workspace.toml`.
// 5.  Test
//
// Imports
// -------
//
// ### Standard library
use std::{
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

// ### Third-party
use clap::{Parser, Subcommand};
use cmd_lib::run_cmd;
use current_platform::CURRENT_PLATFORM;
use path_slash::PathBufExt;
use regex::Regex;

// ### Local
//
// None
//
// Data structures
// ---------------
//
// The following defines the command-line interface for the CodeChat Editor.
#[derive(Parser)]
#[command(name = "The CodeChat Editor Server", version, about, long_about=None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Install all dependencies.
    Install {
        /// True to install developer-only dependencies.
        #[arg(short, long, default_value_t = false)]
        dev: bool,
    },
    /// Update all dependencies.
    Update,
    /// Run lints and tests.
    Test,
    /// Build everything.
    Build,
    /// Build the Client.
    ClientBuild {
        /// True to build for distribution, instead of development.
        #[arg(short, long, default_value_t = false)]
        dist: bool,
    },
    /// Change the version for the client, server, and extensions.
    ChangeVersion {
        /// The new version number, such as "0.1.1".
        new_version: String,
    },
    /// Steps to run before `cargo dist build`.
    Prerelease,
    /// Steps to run after `cargo dist build`. This builds and publishes a
    /// VSCode release.
    Postrelease {
        /// Receives a target triple, such as `x86_64-pc-windows-msvc`. We can't
        /// always infer this, since `dist` cross-compiles the server on OS X,
        /// while this program isn't cross-compiled.
        #[arg(short, long, default_value_t = CURRENT_PLATFORM.to_string())]
        target: String,
        /// The CI build passes this. We don't use it, but must receive it to
        /// avoid an error.
        #[arg(short, long)]
        artifacts: Option<String>,
    },
}

// Code
// ----
//
// ### Utilities
//
// These functions are called by the build support functions.
/// On Windows, scripts must be run from a shell; on Linux and OS X, scripts are
/// directly executable. This function runs a script regardless of OS.
fn run_script<T: AsRef<Path>, A: AsRef<OsStr>, P: AsRef<Path> + std::fmt::Display>(
    // The script to run.
    script: T,
    // Arguments to pass.
    args: &[A],
    // The directory to run the script in.
    dir: P,
    // True to report errors based on the process' exit code; false to ignore
    // the code.
    check_exit_code: bool,
) -> io::Result<()> {
    let script = OsStr::new(script.as_ref());
    let mut process;
    if cfg!(windows) {
        process = Command::new("cmd");
        process.arg("/c").arg(script);
    } else {
        process = Command::new(script);
    };
    process.args(args).current_dir(&dir);
    // A bit crude, but displays the command being run.
    println!("{dir}: {process:#?}");
    let exit_code = process.status()?.code();

    if exit_code == Some(0) || (exit_code.is_some() && !check_exit_code) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "npm exit code indicates failure.",
        ))
    }
}

/// Quickly synchronize the `src` directory with the `dest` directory, by
/// copying files and removing anything in `dest` not in `src`. It uses OS
/// programs (`robocopy`/`rsync`) to accomplish this. Very important: the `src`
/// **must** end with a `/`, otherwise the Windows and Linux copies aren't
/// identical.
fn quick_copy_dir<P: AsRef<Path>>(src: P, dest: P, files: Option<P>) -> io::Result<()> {
    assert!(src.as_ref().to_string_lossy().ends_with('/'));
    let mut copy_process;
    let src = OsStr::new(src.as_ref());
    let dest = OsStr::new(dest.as_ref());
    #[cfg(windows)]
    {
        // From `robocopy /?`:
        //
        // /MIR MIRror a directory tree (equivalent to /E plus /PURGE).
        //
        // /MT Do multi-threaded copies with n threads (default 8).
        //
        // /NFL No File List - don't log file names.
        //
        // /NDL : No Directory List - don't log directory names.
        //
        // /NJH : No Job Header.
        //
        // /NJS : No Job Summary.
        //
        // /NP : No Progress - don't display percentage copied.
        //
        // /NS : No Size - don't log file sizes.
        //
        // /NC : No Class - don't log file classes.
        copy_process = Command::new("robocopy");
        copy_process
            .args([
                "/MIR", "/MT", "/NFL", "/NDL", "/NJH", "/NJS", "/NP", "/NS", "/NC",
            ])
            .arg(src)
            .arg(dest);
        // Robocopy expects the files to copy after the dest.
        if let Some(files_) = &files {
            copy_process.arg(OsStr::new(files_.as_ref()));
        }
    }
    #[cfg(not(windows))]
    {
        // Create the dest directory, since old CI OSes don't support `rsync
        // --mkpath`.
        run_script("mkdir", &["-p", dest.to_str().unwrap()], "./", true)?;
        let mut tmp;
        let src_combined = match files.as_ref() {
            Some(files_) => {
                tmp = src.to_os_string();
                tmp.push(OsStr::new(files_.as_ref()));
                tmp.as_os_str()
            }
            None => src,
        };

        // Use bash to perform globbing, since rsync doesn't do this.
        copy_process = Command::new("bash");
        copy_process.args([
            "-c",
            format!(
                "rsync --archive --delete {} {}",
                &src_combined.to_str().unwrap(),
                &dest.to_str().unwrap()
            )
            .as_str(),
        ]);
    }

    // Print the command, in case this produces and error or takes a while.
    println!("{:#?}", &copy_process);

    // Check for errors.
    let exit_code = copy_process
        .status()?
        .code()
        .expect("Copy process terminated by signal");
    // Per [these
    // docs](https://learn.microsoft.com/en-us/troubleshoot/windows-server/backup-and-storage/return-codes-used-robocopy-utility),
    // check the return code.
    if cfg!(windows) && exit_code >= 8 || !cfg!(windows) && exit_code != 0 {
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("Copy process return code {exit_code} indicates failure."),
        ))
    } else {
        Ok(())
    }
}

fn remove_dir_all_if_exists<P: AsRef<Path> + std::fmt::Display>(path: P) -> io::Result<()> {
    if Path::new(path.as_ref()).try_exists().unwrap() {
        fs::remove_dir_all(path.as_ref())?;
    }

    Ok(())
}

fn search_and_replace_file<
    P: AsRef<Path> + std::fmt::Display,
    S1: AsRef<str> + std::fmt::Display,
    S2: AsRef<str>,
>(
    path: P,
    search_regex: S1,
    replace_string: S2,
) -> io::Result<()> {
    let file_contents = fs::read_to_string(&path)?;
    let re = Regex::new(search_regex.as_ref()).map_err(|err| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("Error in search regex {search_regex}: {err}"),
        )
    })?;
    let file_contents_replaced = re.replace(&file_contents, replace_string.as_ref());
    assert_ne!(
        file_contents, file_contents_replaced,
        "No replacements made."
    );
    fs::write(&path, file_contents_replaced.as_bytes())
}

// Core routines
// -------------
//
// These functions simplify common build-focused development tasks and support
// CI builds.
/// Apply the provided patch to a file.
fn patch_file(patch: &str, before_patch: &str, file_path: &str) -> io::Result<()> {
    let file_path = Path::new(file_path);
    let file_contents = fs::read_to_string(file_path)?;
    if !file_contents.contains(patch) {
        let patch_loc = file_contents
            .find(before_patch)
            .expect("Patch location not found.")
            + before_patch.len();
        let patched_file_contents = format!(
            "{}{patch}{}",
            &file_contents[..patch_loc],
            &file_contents[patch_loc..]
        );
        fs::write(file_path, &patched_file_contents)?;
    }
    Ok(())
}
/// After updating files in the client's Node files, perform some fix-ups.
fn patch_client_npm() -> io::Result<()> {
    // Apply a the fixes described in [issue
    // 27](https://github.com/bjones1/CodeChat_Editor/issues/27).
    patch_file(
        "
        selectionNotFocus = this.view.state.facet(editable) ? focused : hasSelection(this.dom, this.view.observer.selectionRange)",
        "        let selectionNotFocus = !focused && !(this.view.state.facet(editable) || this.dom.tabIndex > -1) &&
            hasSelection(this.dom, this.view.observer.selectionRange) && !(activeElt && this.dom.contains(activeElt));",
        "../client/node_modules/@codemirror/view/dist/index.js"
    )?;
    // In [older
    // releases](https://www.tiny.cloud/docs/tinymce/5/6.0-upcoming-changes/#options),
    // TinyMCE allowed users to change `whitespace_elements`; the whitespace
    // inside these isn't removed by TinyMCE. However, this was removed in v6.0.
    // Therefore, manually patch TinyMCE instead.
    patch_file(
        " wc-mermaid",
        "const whitespaceElementsMap = createLookupTable('whitespace_elements', 'pre script noscript style textarea video audio iframe object code",
        "../client/node_modules/tinymce/tinymce.js",
    )?;

    // Copy across the parts of MathJax that are needed, since bundling it is
    // difficult.
    quick_copy_dir(
        "../client/node_modules/mathjax/",
        "../client/static/mathjax",
        None,
    )?;
    quick_copy_dir(
        "../client/node_modules/mathjax-modern-font/chtml/",
        "../client/static/mathjax-modern-font/chtml",
        None,
    )?;
    // Copy over the graphviz files needed.
    quick_copy_dir(
        "../client/node_modules/graphviz-webcomponent/dist/",
        "../client/static/graphviz-webcomponent",
        Some("renderer.min.js*"),
    )?;

    Ok(())
}

fn run_install(dev: bool) -> io::Result<()> {
    run_script("npm", &["install"], "../client", true)?;
    patch_client_npm()?;
    run_script("npm", &["install"], "../extensions/VSCode", true)?;
    run_cmd!(
        cargo fetch --manifest-path=../builder/Cargo.toml;
        cargo fetch;
    )?;
    if dev {
        // If the dist install reports an error, perhaps it's already installed.
        if run_cmd!(cargo install --locked cargo-dist;).is_err() {
            run_cmd!(dist --version;)?;
        }
        run_cmd!(
            cargo install --locked cargo-outdated;
            cargo install cargo-sort;
        )?;
    }
    Ok(())
}

fn run_update() -> io::Result<()> {
    run_script("npm", &["update"], "../client", true)?;
    patch_client_npm()?;
    run_script("npm", &["update"], "../extensions/VSCode", true)?;
    run_cmd!(
        cargo update --manifest-path=../builder/Cargo.toml;
        cargo update;
    )?;
    // Simply display outdated dependencies, but don't considert them an error.
    run_script("npm", &["outdated"], "../client", false)?;
    run_script("npm", &["outdated"], "../extensions/VSCode", false)?;
    run_cmd!(
        cargo outdated --manifest-path=../builder/Cargo.toml;
        cargo outdated;
    )?;
    Ok(())
}

fn run_test() -> io::Result<()> {
    // On Windows, `cargo sort --check` fails since it default to LF, not CRLF,
    // line endings. Work around this by changing this setting only on Windows.
    // See the [cargo sort config
    // docs](https://github.com/DevinR528/cargo-sort?tab=readme-ov-file#config)
    // and the [related
    // issue](https://github.com/DevinR528/cargo-sort/issues/85).
    //
    // However, this still fails: `cargo sort` uses [inconsistent line
    // endings](https://github.com/DevinR528/cargo-sort/issues/86).
    /***
    #[cfg(windows)]
    {
        fs::write("tomlfmt.toml", "crlf = true")
            .map_err(|err| -> String { format!("Unable to write tomlfmt.toml: {err}") })?;
    }
     */
    // The `-D warnings` flag causes clippy to return a non-zero exit status if
    // it issues warnings.
    run_cmd!(
        cargo clippy --all-targets -- -D warnings;
        cargo fmt --check;
        cargo clippy --all-targets --manifest-path=../builder/Cargo.toml -- -D warnings;
        cargo fmt --check --manifest-path=../builder/Cargo.toml;
    )?;
    // `cargo sort` produces false positives under Windows. Ignore for now. See
    // the above comments. It also doesn't support the
    #[cfg(not(windows))]
    run_cmd!(
        cargo sort --check;
        cd ../builder;
        cargo sort --check;
    )?;
    run_build()?;
    // Verify that compiling for release produces no errors.
    run_cmd!(dist build;)?;
    run_cmd!(
        cargo test --manifest-path=../builder/Cargo.toml;
        cargo test;
    )?;
    Ok(())
}

fn run_build() -> io::Result<()> {
    // Clean out all bundled files before the rebuild.
    remove_dir_all_if_exists("../client/static/bundled")?;
    run_client_build(false)?;
    run_script("npm", &["run", "compile"], "../extensions/VSCode", true)?;
    run_cmd!(
        cargo build --manifest-path=../builder/Cargo.toml;
        cargo build;
    )?;
    Ok(())
}

// Build the NPM Client.
fn run_client_build(
    // True to build for distribution, not development.
    dist: bool,
) -> io::Result<()> {
    let esbuild = PathBuf::from_slash("node_modules/.bin/esbuild");
    let distflag = if dist { "--minify" } else { "--sourcemap" };
    // This makes the program work from either the `server/` or `client/` directories.
    let rel_path = "../client";

    // The main build for the Client.
    run_script(
        &esbuild,
        &[
            "src/CodeChatEditorFramework.mts",
            "src/CodeChatEditor.mts",
            "src/CodeChatEditor-test.mts",
            "src/css/CodeChatEditorProject.css",
            "src/css/CodeChatEditor.css",
            "--bundle",
            "--outdir=./static/bundled",
            distflag,
            "--format=esm",
            "--splitting",
            "--metafile=meta.json",
            "--entry-names=[dir]/[name]-[hash]",
        ],
        rel_path,
        true,
    )?;
    // <a id="#pdf.js></a>The PDF viewer for use with VSCode. Built it separately, since it's loaded apart from the rest of the Client.
    run_script(
        &esbuild,
        &[
            "src/pdf.js/viewer.mjs",
            "node_modules/pdfjs-dist/build/pdf.worker.mjs",
            "--bundle",
            "--outdir=./static/bundled",
            distflag,
            "--format=esm",
            "--loader:.png=dataurl",
            "--loader:.svg=dataurl",
            "--loader:.gif=dataurl",
        ],
        rel_path,
        true,
    )?;
    // Copy over the cmap (color map?) files, which the bundler doesn't handle.
    quick_copy_dir(
        format!("{rel_path}/node_modules/pdfjs-dist/cmaps/"),
        format!("{rel_path}/static/bundled/node_modules/pdfjs-dist/cmaps/"),
        None,
    )?;
    // The HashReader isn't bundled; instead, it's used to translate the JSON metafile produced by the main esbuild run to the simpler format used by the CodeChat Editor. TODO: rewrite this in Rust.
    run_script(
        &esbuild,
        &[
            "src/HashReader.mts",
            "--outdir=.",
            "--platform=node",
            "--format=esm",
        ],
        rel_path,
        true,
    )?;
    run_script("node", &["HashReader.js"], rel_path, true)?;
    // Finally, check the TypeScript with the (slow) TypeScript compiler.
    run_script(
        PathBuf::from_slash("node_modules/.bin/tsc"),
        &["-noEmit"],
        rel_path,
        true,
    )?;
    Ok(())
}

fn run_change_version(new_version: &String) -> io::Result<()> {
    let replacement_string = format!("${{1}}{new_version}${{2}}");
    search_and_replace_file(
        "Cargo.toml",
        r#"(\r?\nversion = ")[\d.]+("\r?\n)"#,
        &replacement_string,
    )?;
    let json_search_regex = r#"(\r?\n    "version": ")[\d.]+(",\r?\n)"#;
    search_and_replace_file(
        "../client/package.json",
        json_search_regex,
        &replacement_string,
    )?;
    search_and_replace_file(
        "../extensions/VSCode/package.json",
        json_search_regex,
        &replacement_string,
    )?;
    Ok(())
}

fn run_prerelease() -> io::Result<()> {
    // Clean out all bundled files before the rebuild.
    remove_dir_all_if_exists("../client/static/bundled")?;
    run_install(true)?;
    run_script("npm", &["run", "dist"], "../client", true)?;
    Ok(())
}

fn run_postrelease(target: &str) -> io::Result<()> {
    let server_dir = "../extensions/VSCode/server";
    // Only clean the `server/` directory if it exists.
    remove_dir_all_if_exists(server_dir)?;

    // Translate from the target triple to VSCE's target parameter.
    let vsce_target = match target {
        "x86_64-pc-windows-msvc" => "win32-x64",
        "x86_64-unknown-linux-gnu" => "linux-x64",
        "x86_64-apple-darwin" => "darwin-x64",
        "aarch64-apple-darwin" => "darwin-arm64",
        _ => panic!("Unsupported platform {target}."),
    };

    let src_name = format!("codechat-editor-server-{target}");
    quick_copy_dir(
        format!("target/distrib/{src_name}/").as_str(),
        "../extensions/VSCode/server",
        None,
    )?;
    run_script(
        "npx",
        &["vsce", "package", "--target", vsce_target],
        "../extensions/VSCode",
        true,
    )?;

    Ok(())
}

// CLI implementation
// ------------------
//
// The following code implements the command-line interface for the CodeChat
// Editor.
impl Cli {
    fn run(self) -> io::Result<()> {
        match &self.command {
            Commands::Install { dev } => run_install(*dev),
            Commands::Update => run_update(),
            Commands::Test => run_test(),
            Commands::Build => run_build(),
            Commands::ClientBuild { dist } => run_client_build(*dist),
            Commands::ChangeVersion { new_version } => run_change_version(new_version),
            Commands::Prerelease => run_prerelease(),
            Commands::Postrelease { target, .. } => run_postrelease(target),
        }
    }
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();
    cli.run()?;

    Ok(())
}

#[cfg(test)]
mod test {
    use super::Cli;
    use clap::CommandFactory;

    // This is recommended in the
    // [docs](https://docs.rs/clap/latest/clap/_derive/_tutorial/chapter_4/index.html).
    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
    }
}
