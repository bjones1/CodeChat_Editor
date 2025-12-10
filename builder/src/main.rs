// Copyright (C) 2025 Bryan A. Jones.
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
/// `main.rs` -- Entrypoint for the CodeChat Editor Builder
/// =======================================================
///
/// This code uses [dist](https://opensource.axo.dev/cargo-dist/book/) as a part
/// of the release process. To update the `./release.yaml` file this tool
/// creates:
///
/// 1.  Edit `server/dist-workspace.toml`: change `allow-dirty` to `[]`.
/// 2.  Run `dist init` and accept the defaults, then run `dist generate`.
/// 3.  Review changes to `./release.yaml`, reapplying hand edits.
/// 4.  Revert the changes to `server/dist-workspace.toml`.
/// 5.  Test
// Imports
// -------
//
// ### Standard library
use std::{
    env,
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

// ### Third-party
use clap::{Parser, Subcommand};
use cmd_lib::run_cmd;
use current_platform::CURRENT_PLATFORM;
use dunce::canonicalize;
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
    /// Run formatters and linters.
    Flint {
        /// Check only; don't modify files.
        #[arg(short, long, default_value_t = false)]
        check: bool,
    },
    /// Run lints and tests.
    Test,
    /// Build everything.
    Build,
    /// Build the Client.
    ClientBuild(TypeScriptBuildOptions),
    /// Build the extensions.
    ExtBuild(TypeScriptBuildOptions),
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

#[derive(Parser)]
struct TypeScriptBuildOptions {
    /// True to build for distribution, instead of development.
    #[arg(short, long, default_value_t = false)]
    dist: bool,
    /// True to skip checks for TypeScript errors in the Client.
    #[arg(short, long, default_value_t = false)]
    skip_check_errors: bool,
}

// Constants
// ---------
static VSCODE_PATH: &str = "../extensions/VSCode";
static CLIENT_PATH: &str = "../client";
static BUILDER_PATH: &str = "../builder";
static NAPI_TARGET: &str = "NAPI_TARGET";

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
        Err(io::Error::other("pnpm exit code indicates failure."))
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
        Err(io::Error::other(format!(
            "Copy process return code {exit_code} indicates failure."
        )))
    } else {
        Ok(())
    }
}

fn copy_file<P: AsRef<Path> + std::fmt::Debug>(src: P, dest: P) -> io::Result<()> {
    println!("copy {src:?} -> {dest:?}");
    fs::copy(src, dest).map(|_| ())
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
    let re = Regex::new(search_regex.as_ref())
        .map_err(|err| io::Error::other(format!("Error in search regex {search_regex}: {err}")))?;
    let file_contents_replaced = re.replace(&file_contents, replace_string.as_ref());
    assert_ne!(
        file_contents, file_contents_replaced,
        "No replacements made in {path}."
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
fn patch_client_libs() -> io::Result<()> {
    // Apply a the fixes described in [issue
    // 27](https://github.com/bjones1/CodeChat_Editor/issues/27).
    patch_file(
        "
        selectionNotFocus = this.view.state.facet(editable) ? focused : hasSelection(this.dom, this.view.observer.selectionRange)",
        "        let selectionNotFocus = !focused && !(this.view.state.facet(editable) || this.dom.tabIndex > -1) &&
            hasSelection(this.dom, this.view.observer.selectionRange) && !(activeElt && this.dom.contains(activeElt));",
        &format!("{CLIENT_PATH}/node_modules/@codemirror/view/dist/index.js")
    )?;
    // In [older
    // releases](https://www.tiny.cloud/docs/tinymce/5/6.0-upcoming-changes/#options),
    // TinyMCE allowed users to change `whitespace_elements`; the whitespace
    // inside these isn't removed by TinyMCE. However, this was removed in v6.0.
    // Therefore, manually patch TinyMCE instead.
    patch_file(
        " wc-mermaid graphviz-graph",
        "const whitespaceElementsMap = createLookupTable('whitespace_elements', 'pre script noscript style textarea video audio iframe object code",
        &format!("{CLIENT_PATH}/node_modules/tinymce/tinymce.js"),
    )?;

    // Copy across the parts of MathJax that are needed, since bundling it is
    // difficult.
    remove_dir_all_if_exists(format!("{CLIENT_PATH}/static/mathjax"))?;
    for subdir in ["a11y", "adaptors", "input", "output", "sre", "ui"] {
        quick_copy_dir(
            format!("{CLIENT_PATH}/node_modules/mathjax/{subdir}/"),
            format!("{CLIENT_PATH}/static/mathjax/{subdir}"),
            None,
        )?;
    }
    quick_copy_dir(
        format!("{CLIENT_PATH}/node_modules/mathjax/"),
        format!("{CLIENT_PATH}/static/mathjax"),
        Some("tex-chtml.js".to_string()),
    )?;
    quick_copy_dir(
        format!("{CLIENT_PATH}/node_modules/@mathjax/mathjax-newcm-font/chtml/"),
        format!("{CLIENT_PATH}/static/mathjax-newcm-font/chtml"),
        None,
    )?;

    Ok(())
}

fn run_install(dev: bool) -> io::Result<()> {
    if dev {
        run_script("npm", &["install", "-g", "pnpm@latest-10"], ".", true)?;
    }
    // See [the client manifest](../../client/package.json5) for an explanation
    // of `--no-frozen-lockfile`.
    run_script("pnpm", &["install"], CLIENT_PATH, true)?;
    patch_client_libs()?;
    run_script("pnpm", &["install"], VSCODE_PATH, true)?;
    run_cmd!(
        info "Builder: cargo fetch";
        cargo fetch --manifest-path=$BUILDER_PATH/Cargo.toml;
        info "VSCode extension: cargo fetch";
        cargo fetch --manifest-path=$VSCODE_PATH/Cargo.toml;
        info "cargo fetch";
        cargo fetch;
    )?;
    if dev {
        // Install the cargo binstall binary, taken from the
        // [docs](https://docs.rs/crate/cargo-binstall/1.15.5).
        #[cfg(windows)]
        run_cmd! {
            pwsh -Command "Set-ExecutionPolicy Unrestricted -Scope Process; iex (iwr 'https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.ps1').Content";
        }?;
        #[cfg(not(windows))]
        // The original command had `'=https'`, but single quotes confused
        // `cmd_lib` and aren't needed to quote this. Note that `//` in the URL
        // is a comment in Rust, so it must be [enclosed in
        // quotes](https://github.com/rust-shell-script/rust_cmd_lib/issues/88).
        run_cmd! {
            curl -L --proto =https --tlsv1.2 -sSf "https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh" | bash;
        }?;

        // Installing `cargo-dist` using `cargo binstall` fails intermittently
        // on MacOS. Try another approach.
        #[cfg(target_os = "macos")]
        run_cmd!(
            curl --proto =https --tlsv1.2 -LsSf "https://github.com/axodotdev/cargo-dist/releases/latest/download/cargo-dist-installer.sh" | sh;

        )?;
        #[cfg(not(target_os = "macos"))]
        run_cmd!(
            info "cargo binstall cargo-dist";
            cargo binstall cargo-dist --no-confirm;
        )?;

        run_cmd!(
            info "cargo binstall cargo-outdated";
            cargo binstall cargo-outdated --no-confirm;
            info "cargo binstall cargo-sort";
            cargo binstall cargo-sort --no-confirm;
        )?;
    }
    Ok(())
}

fn run_update() -> io::Result<()> {
    run_script("pnpm", &["update"], CLIENT_PATH, true)?;
    patch_client_libs()?;
    run_script("pnpm", &["update"], VSCODE_PATH, true)?;
    run_cmd!(
        info "Builder: cargo update";
        cargo update --manifest-path=$BUILDER_PATH/Cargo.toml;
        info "VSCoe extension: cargo update";
        cargo update --manifest-path=$VSCODE_PATH/Cargo.toml;
        info "cargo update";
        cargo update;
    )?;
    // Simply display outdated dependencies, but don't consider them an error.
    run_script("pnpm", &["outdated"], CLIENT_PATH, false)?;
    run_script("pnpm", &["outdated"], VSCODE_PATH, false)?;
    run_cmd!(
        info "Builder: cargo outdated";
        cargo outdated --manifest-path=$BUILDER_PATH/Cargo.toml;
        info "VSCode extension: cargo outdated";
        cargo outdated --manifest-path=$VSCODE_PATH/Cargo.toml;
        info "cargo outdated";
        cargo outdated;
    )?;
    Ok(())
}

fn run_format_and_lint(check_only: bool) -> io::Result<()> {
    // The `-D warnings` flag causes clippy to return a non-zero exit status if
    // it issues warnings.
    let (clippy_check_only, check, eslint_check) = if check_only {
        ("-Dwarnings", "--check", "")
    } else {
        ("", "", "--fix")
    };
    run_cmd!(
        info "cargo clippy and fmt";
        cargo clippy --all-targets --all-features --tests -- $clippy_check_only;
        cargo fmt --all $check;
        info "Builder: cargo clippy and fmt";
        cargo clippy --all-targets --all-features --tests --manifest-path=$BUILDER_PATH/Cargo.toml -- $clippy_check_only;
        cargo fmt --all $check --manifest-path=$BUILDER_PATH/Cargo.toml;
        info "VSCode extension: cargo clippy and fmt";
        cargo clippy --all-targets --all-features --tests --manifest-path=$VSCODE_PATH/Cargo.toml -- $clippy_check_only;
        cargo fmt --all $check --manifest-path=$VSCODE_PATH/Cargo.toml;
        info "cargo sort";
        cargo sort $check;
        cd $BUILDER_PATH;
        info "Builder: cargo sort";
        cargo sort $check;
        cd $VSCODE_PATH;
        info "VSCode extension: cargo sort";
        cargo sort $check;
    )?;
    let mut eslint_args = vec!["eslint", "src"];
    if !eslint_check.is_empty() {
        eslint_args.push(eslint_check)
    }
    run_script("npx", &eslint_args, CLIENT_PATH, true)?;
    run_script("npx", &eslint_args, VSCODE_PATH, true)
}

fn run_test() -> io::Result<()> {
    run_format_and_lint(true)?;
    run_build()?;
    // Verify that compiling for release produces no errors.
    run_cmd!(
        cd ..;
        info "dist build";
        dist build;
    )?;
    run_cmd!(
        info "Builder: cargo test";
        cargo test --manifest-path=$BUILDER_PATH/Cargo.toml;
        info "VSCode extension: cargo test";
        cargo test --manifest-path=$VSCODE_PATH/Cargo.toml;
        info "cargo test";
        cargo test --features int_tests;
    )?;
    Ok(())
}

fn run_build() -> io::Result<()> {
    run_cmd!(
        info "Builder: cargo build";
        cargo build --manifest-path=$BUILDER_PATH/Cargo.toml;
        info "cargo build";
        cargo build;
    )?;
    // Clean out all bundled files before the rebuild.
    remove_dir_all_if_exists(format!("{CLIENT_PATH}/static/bundled"))?;
    run_client_build(false, false)?;
    run_extensions_build(false, false)?;
    Ok(())
}

// Build the CodeChat Editor Client.
fn run_client_build(
    // True to build for distribution, not development.
    dist: bool,
    // True to skip checking for TypeScript errors; false to perform these
    // checks.
    skip_check_errors: bool,
) -> io::Result<()> {
    // Ensure the JavaScript data structures generated from Rust are up to date.
    run_cmd!(
        info "cargo test export_bindings";
        cargo test export_bindings;
    )?;

    let esbuild = PathBuf::from_slash("node_modules/.bin/esbuild");
    let distflag = if dist { "--minify" } else { "--sourcemap" };

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
        CLIENT_PATH,
        true,
    )?;

    // <a id="#pdf.js>The PDF viewer for use with VSCode. Built it separately,
    // since it's loaded apart from the rest of the Client.
    run_script(
        &esbuild,
        &[
            "src/third-party/pdf.js/viewer.mjs",
            "node_modules/pdfjs-dist/build/pdf.worker.mjs",
            "--bundle",
            "--outdir=./static/bundled",
            distflag,
            "--format=esm",
            "--loader:.png=dataurl",
            "--loader:.svg=dataurl",
            "--loader:.gif=dataurl",
        ],
        CLIENT_PATH,
        true,
    )?;
    // Copy over the cmap (color map?) files, which the bundler doesn't handle.
    quick_copy_dir(
        format!("{CLIENT_PATH}/node_modules/pdfjs-dist/cmaps/"),
        format!("{CLIENT_PATH}/static/bundled/node_modules/pdfjs-dist/cmaps/"),
        None,
    )?;

    // Build the graphviz rendering engine.
    run_script(
        &esbuild,
        &[
            "src/third-party/graphviz-webcomponent/renderer.js",
            "--bundle",
            "--outdir=./static/bundled",
            distflag,
            "--format=esm",
        ],
        CLIENT_PATH,
        true,
    )?;

    // The HashReader isn't bundled; instead, it's used to translate the JSON
    // metafile produced by the main esbuild run to the simpler format used by
    // the CodeChat Editor. TODO: rewrite this in Rust.
    run_script(
        &esbuild,
        &[
            "src/HashReader.mts",
            "--outdir=.",
            "--platform=node",
            "--format=esm",
        ],
        CLIENT_PATH,
        true,
    )?;
    run_script("node", &["HashReader.js"], CLIENT_PATH, true)?;
    // Finally, check the TypeScript with the (slow) TypeScript compiler.
    if !skip_check_errors {
        run_script(
            PathBuf::from_slash("node_modules/.bin/tsc"),
            &["-noEmit"],
            CLIENT_PATH,
            true,
        )?;
    }

    Ok(())
}

// Build the CodeChat Editor extensions.
fn run_extensions_build(
    // True to build for distribution, not development.
    dist: bool,
    // True to skip checking for TypeScript errors; false to perform these
    // checks.
    skip_check_errors: bool,
) -> io::Result<()> {
    let esbuild = PathBuf::from_slash("node_modules/.bin/esbuild");
    let distflag = if dist { "--minify" } else { "--sourcemap" };

    // The NAPI build.
    let mut napi_args = vec!["napi", "build", "--platform", "--output-dir", "src"];
    if dist {
        napi_args.push("--release");
    }
    // See if this is a cross-platform build -- if so, add in the specified target.
    let target;
    if let Ok(tmp) = env::var(NAPI_TARGET) {
        target = tmp;
        napi_args.extend(["--target", &target]);
    }
    run_script("npx", &napi_args, VSCODE_PATH, true)?;

    // Ensure the JavaScript data structures generated from Rust are up to date.
    run_cmd!(
        info "cargo test export_bindings";
        cargo test export_bindings;
    )?;

    // The main build for the extension.
    run_script(
        &esbuild,
        &[
            "src/extension.ts",
            "--platform=node",
            "--format=cjs",
            "--bundle",
            // Don't bundle the VSCode library, since it's built in.
            "--external:vscode",
            "--outdir=./out",
            distflag,
            // The binaries produced by NAPI-RS should be copied over.
            "--loader:.node=copy",
            // Avoid the default of adding hash names to the `.node` file
            // generated.
            "--asset-names=[name]",
        ],
        VSCODE_PATH,
        true,
    )?;
    // Finally, check the TypeScript with the (slow) TypeScript compiler.
    if !skip_check_errors {
        run_script(
            PathBuf::from_slash("node_modules/.bin/tsc"),
            &["-noEmit"],
            VSCODE_PATH,
            true,
        )?;
    }
    Ok(())
}

fn run_change_version(new_version: &String) -> io::Result<()> {
    let cargo_regex = r#"(\r?\nversion = ")[\d.]+("\r?\n)"#;
    let replacement_string = format!("${{1}}{new_version}${{2}}");
    search_and_replace_file("Cargo.toml", cargo_regex, &replacement_string)?;
    search_and_replace_file(
        format!("{VSCODE_PATH}/Cargo.toml"),
        cargo_regex,
        &replacement_string,
    )?;
    search_and_replace_file(
        format!("{VSCODE_PATH}/package.json"),
        r#"(\r?\n    "version": ")[\d.]+(",\r?\n)"#,
        &replacement_string,
    )?;
    search_and_replace_file(
        format!("{CLIENT_PATH}/package.json5"),
        r#"(\r?\n    version: ')[\d.]+(',\r?\n)"#,
        &replacement_string,
    )?;
    Ok(())
}

fn run_prerelease() -> io::Result<()> {
    // Clean out all bundled files before the rebuild.
    remove_dir_all_if_exists(format!("{CLIENT_PATH}/static/bundled"))?;
    run_install(true)?;
    run_client_build(true, false)
}

fn run_postrelease(target: &str) -> io::Result<()> {
    // Copy all the Client static files needed by the embedded Server to the
    // VSCode extension.
    let client_static_dir = format!("{VSCODE_PATH}/static");
    remove_dir_all_if_exists(&client_static_dir)?;
    quick_copy_dir(&format!("{CLIENT_PATH}/static/"), &client_static_dir, None)?;
    copy_file("log4rs.yml", &format!("{VSCODE_PATH}/log4rs.yml"))?;
    copy_file(
        "hashLocations.json",
        &format!("{VSCODE_PATH}/hashLocations.json"),
    )?;

    // Translate from the target triple to VSCE's target parameter.
    let vsce_target = match target {
        "x86_64-pc-windows-msvc" => "win32-x64",
        "x86_64-unknown-linux-gnu" => "linux-x64",
        "x86_64-apple-darwin" => "darwin-x64",
        "aarch64-apple-darwin" => "darwin-arm64",
        _ => panic!("Unsupported platform {target}."),
    };
    // `vsce` will invoke this program's `ext_build`; however, it doesn't provide a way to pass the target when cross-compiling. Use an environment variable instead.
    unsafe {
        env::set_var(NAPI_TARGET, target);
    }
    run_script(
        "npx",
        &[
            "vsce",
            "package",
            // We use esbuild to package; therefore, tell `vsce` not to package.
            "--no-dependencies",
            // Since we include the server as a binary, package for the
            // architecture the binary was build for.
            "--target",
            vsce_target,
        ],
        VSCODE_PATH,
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
            Commands::Flint { check } => run_format_and_lint(*check),
            Commands::Test => run_test(),
            Commands::Build => run_build(),
            Commands::ClientBuild(build_options) => {
                run_client_build(build_options.dist, build_options.skip_check_errors)
            }
            Commands::ExtBuild(build_options) => {
                run_extensions_build(build_options.dist, build_options.skip_check_errors)
            }
            Commands::ChangeVersion { new_version } => run_change_version(new_version),
            Commands::Prerelease => run_prerelease(),
            Commands::Postrelease { target, .. } => run_postrelease(target),
        }
    }
}

fn main() -> io::Result<()> {
    // Change to the `server/` directory, so it can be run from anywhere.
    let mut root_path = PathBuf::from(env::current_exe().unwrap().parent().unwrap());
    root_path.push("../../../server");
    // Use `dunce.canonicalize`, since UNC paths booger up some of the build
    // tools (cargo can't delete the builder's binary, NPM doesn't accept UNC
    // paths.)
    root_path = canonicalize(root_path).unwrap();
    env::set_current_dir(root_path).unwrap();

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
