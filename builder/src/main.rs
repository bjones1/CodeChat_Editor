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
// # `main.rs` -- Entrypoint for the CodeChat Editor Builder
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
// ## Imports
//
// ### Standard library
use std::{ffi::OsStr, fs, path::Path, process::Command};

// ### Third-party
use clap::{Parser, Subcommand};
use cmd_lib::run_cmd;
use current_platform::CURRENT_PLATFORM;

// ### Local
//
// None
//
// ## Data structures
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

// ## Code
//
// ### Utilities
//
// These functions are called by the build support functions.
/// On Windows, scripts must be run from a shell; on Linux and OS X, scripts are
/// directly executable. This function runs a script regardless of OS.
fn run_script<T: AsRef<OsStr>, P: AsRef<Path> + std::fmt::Display>(
    // The script to run.
    script: T,
    // Arguments to pass.
    args: &[T],
    // The directory to run the script in.
    dir: P,
    // True to report errors based on the process' exit code; false to ignore
    // the code.
    check_exit_code: bool,
) -> Result<(), Box<dyn std::error::Error>> {
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
        Err("npm exit code indicates failure".into())
    }
}

/// Quickly synchronize the `src` directory with the `dest` directory, by
/// copying files and removing anything in `dest` not in `src`. It uses OS
/// programs (`robocopy`/`rsync`) to accomplish this. Very important: the `src`
/// **must** end with a `/`, otherwise the Windows and Linux copies aren't
/// identical.
fn quick_copy_dir<P: AsRef<OsStr>>(
    src: P,
    dest: P,
    files: Option<P>,
) -> Result<(), Box<dyn std::error::Error>> {
    assert!(src.as_ref().to_string_lossy().ends_with('/'));
    let mut copy_process;
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
            .arg(&src)
            .arg(&dest);
        // Robocopy expects the files to copy after the dest.
        if let Some(files_) = &files {
            copy_process.arg(files_);
        }
    }
    #[cfg(not(windows))]
    {
        // Create the dest directory, since old CI OSes don't support
        // `rsync --mkpath`.
        run_script(
            "mkdir",
            &["-p", dest.as_ref().to_str().unwrap()],
            "./",
            true,
        )?;
        let mut tmp;
        let src_combined = match files.as_ref() {
            Some(files_) => {
                tmp = src.as_ref().to_os_string();
                tmp.push(files_);
                tmp.as_os_str()
            }
            None => src.as_ref(),
        };

        // Use bash to perform globbing, since rsync doesn't do this.
        copy_process = Command::new("bash");
        copy_process.args([
            "-c",
            format!(
                "rsync --archive --delete {} {}",
                &src_combined.to_str().unwrap(),
                &dest.as_ref().to_str().unwrap()
            )
            .as_str(),
        ]);
    }

    // Print the command, in case this produces and error or takes a while.
    println!("{:#?}", &copy_process);

    // Check for errors.
    let exit_status = copy_process
        .status()
        .map_err(|err| -> String { format!("Error running copy process: {err}") })?;
    let exit_code = exit_status
        .code()
        .expect("Copy process terminated by signal");
    // Per
    // [these docs](https://learn.microsoft.com/en-us/troubleshoot/windows-server/backup-and-storage/return-codes-used-robocopy-utility),
    // check the return code.
    if cfg!(windows) && exit_code >= 8 || !cfg!(windows) && exit_code != 0 {
        Err(format!("Copy process return code {exit_code} indicates failure.").into())
    } else {
        Ok(())
    }
}

fn remove_dir_all_if_exists<P: AsRef<Path> + std::fmt::Display>(
    path: P,
) -> Result<(), Box<dyn std::error::Error>> {
    if Path::new(path.as_ref()).try_exists().unwrap() {
        if let Err(err) = fs::remove_dir_all(path.as_ref()) {
            return Err(format!("Error removing directory tree {path}: {err}").into());
        }
    }

    Ok(())
}

// ## Core routines
//
// These functions simplify common build-focused development tasks and support
// CI builds.
/// After updating files in the client's Node files, perform some fix-ups.
fn patch_client_npm() -> Result<(), Box<dyn std::error::Error>> {
    // Apply a the fixes described in
    // [issue 27](https://github.com/bjones1/CodeChat_Editor/issues/27).
    //
    // Insert this line...
    let patch = "
        selectionNotFocus = this.view.state.facet(editable) ? focused : hasSelection(this.dom, this.view.observer.selectionRange)";
    // After this line.
    let before_path = "        let selectionNotFocus = !focused && !(this.view.state.facet(editable) || this.dom.tabIndex > -1) &&
            hasSelection(this.dom, this.view.observer.selectionRange) && !(activeElt && this.dom.contains(activeElt));";
    // First, see if the patch was applied already.
    let index_js_path = Path::new("../client/node_modules/@codemirror/view/dist/index.js");
    let index_js = fs::read_to_string(index_js_path)?;
    if !index_js.contains(patch) {
        let patch_loc = index_js
            .find(before_path)
            .expect("Patch location not found.")
            + before_path.len();
        let patched_index_js = format!(
            "{}{patch}{}",
            &index_js[..patch_loc],
            &index_js[patch_loc..]
        );
        fs::write(index_js_path, &patched_index_js)?;
    }

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

fn run_install(dev: bool) -> Result<(), Box<dyn std::error::Error>> {
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

fn run_update() -> Result<(), Box<dyn std::error::Error>> {
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

fn run_test() -> Result<(), Box<dyn std::error::Error>> {
    // On Windows, `cargo sort --check` fails since it default to LF, not CRLF,
    // line endings. Work around this by changing this setting only on Windows.
    // See the
    // [cargo sort config docs](https://github.com/DevinR528/cargo-sort?tab=readme-ov-file#config)
    // and the
    // [related issue](https://github.com/DevinR528/cargo-sort/issues/85).
    //
    // However, this still fails: `cargo sort` uses
    // [inconsistent line endings](https://github.com/DevinR528/cargo-sort/issues/86).
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

fn run_build() -> Result<(), Box<dyn std::error::Error>> {
    // Clean out all bundled files before the rebuild.
    remove_dir_all_if_exists("../client/static/bundled")?;
    run_script("npm", &["run", "build"], "../client", true)?;
    run_script("npm", &["run", "compile"], "../extensions/VSCode", true)?;
    run_cmd!(
        cargo build --manifest-path=../builder/Cargo.toml;
        cargo build;
    )?;
    Ok(())
}

fn run_prerelease() -> Result<(), Box<dyn std::error::Error>> {
    // Clean out all bundled files before the rebuild.
    remove_dir_all_if_exists("../client/static/bundled")?;
    run_install(true)?;
    run_script("npm", &["run", "dist"], "../client", true)?;
    Ok(())
}

fn run_postrelease(target: &str) -> Result<(), Box<dyn std::error::Error>> {
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

// ## CLI implementation
//
// The following code implements the command-line interface for the CodeChat
// Editor.
impl Cli {
    fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        match &self.command {
            Commands::Install { dev } => run_install(*dev),
            Commands::Update => run_update(),
            Commands::Test => run_test(),
            Commands::Build => run_build(),
            Commands::Prerelease => run_prerelease(),
            Commands::Postrelease { target, .. } => run_postrelease(target),
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
