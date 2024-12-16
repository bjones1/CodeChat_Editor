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
/// # `main.rs` -- Entrypoint for the CodeChat Editor Server
// ## Imports
//
// ### Standard library
use std::{
    env,
    io::Read,
    process::{Command, Stdio},
    time::SystemTime,
};
#[cfg(debug_assertions)]
use std::{ffi::OsStr, fs, path::Path};

// ### Third-party
#[cfg(debug_assertions)]
use clap::ValueEnum;
use clap::{Args, Parser, Subcommand};
#[cfg(debug_assertions)]
use cmd_lib::run_cmd;
use log::LevelFilter;

// ### Local
use code_chat_editor::webserver::{self, IP_ADDRESS};
// Added for the use of the '**rust_cmd_lib**' library
// [rust_cmd_lib](https://github.com/rust-shell-script/rust_cmd_lib?tab=readme-ov-file)

// ## Data structures
//
// ### Command-line interface
// The following defines the command-line interface for the CodeChat
// Editor.
#[derive(Parser)]
#[command(name = "The CodeChat Editor Server", version, about, long_about=None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Select the port to use for the server.
    #[arg(short, long, default_value_t = 8080)]
    port: u16,

    /// Used for testing only.
    #[cfg(debug_assertions)]
    #[arg(short, long)]
    test_mode: Option<TestMode>,
}

#[cfg(debug_assertions)]
#[derive(Clone, ValueEnum)]
enum TestMode {
    NotFound,
    Sleep,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the webserver.
    Serve(ServeCommand),
    /// Start the webserver in a child process then exit.
    Start,
    /// Stop the webserver child process.
    Stop,
    #[cfg(debug_assertions)]
    /// Install all dependencies.
    Install,
    #[cfg(debug_assertions)]
    /// Update all dependencies.
    Update,
    #[cfg(debug_assertions)]
    /// Build everything.
    Build,
    #[cfg(debug_assertions)]
    /// Steps to run before `cargo dist build`.
    Prerelease,
    #[cfg(debug_assertions)]
    /// Steps to run after `cargo dist build`. This builds a VSCode release,
    /// producing a VSCode `.vsix` file.
    Postrelease,
}

#[derive(Args)]
struct ServeCommand {
    /// Control logging verbosity.
    #[arg(short, long)]
    log: Option<LevelFilter>,
}

// ## Code
//
// ### Build support
//
// #### Utilities
//
// These functions are called by the build support functions.
#[cfg(debug_assertions)]
/// The following function implements the 'Install' command
fn run_script<T: AsRef<OsStr>, P: AsRef<Path> + std::fmt::Display>(
    script: T,
    args: &[T],
    dir: P,
) -> Result<(), Box<dyn std::error::Error>> {
    // On Windows, scripts must be run from a shell; on Linux and OS X, scripts
    // are directly executable.
    let mut tmp;
    let process = if cfg!(windows) {
        tmp = Command::new("cmd");
        tmp.arg("/c").arg(script)
    } else {
        &mut Command::new(script)
    };
    // Runs the 'npm update' command using cmd_lib in the client directory
    // comments cannot be placed in the code below or the commands will not run.
    // 'run_cmd!' puts the text you type into the terminal and it doesn't know
    // how to handle comments.
    let npm_process = process.args(args).current_dir(&dir);
    // A bit crude, but display the command being run.
    println!("{dir}: {npm_process:#?}");
    let exit_code = npm_process.status()?.code();

    if exit_code == Some(0) {
        Ok(())
    } else {
        Err("npm exit code indicates failure".into())
    }
}

#[cfg(debug_assertions)]
fn quick_copy_file<P: AsRef<Path> + std::fmt::Display>(
    src: P,
    dest: P,
) -> Result<(), Box<dyn std::error::Error>> {
    // This is a bit simplistic -- it doesn't check dates/sizes/etc. Better
    // would be to compare metadata.
    if !dest.as_ref().try_exists().unwrap() {
        println!("Copying from {src} to {dest}.");
        // Create the appropriate directories if needed. Ignore errors for
        // simplicity; the copy will produce errors if necessary.
        let _ = fs::create_dir_all(dest.as_ref().parent().unwrap());
        fs::copy(&src, &dest)?;
    }
    Ok(())
}

#[cfg(debug_assertions)]
fn quick_copy_dir<P: AsRef<OsStr>>(src: P, dest: P) -> Result<(), Box<dyn std::error::Error>> {
    let mut os_copy_process;
    let copy_process = if cfg!(windows) {
        // Robocopy copies the contents of the source directory, not the source
        // directory itself. So, append the final path of the source directory
        // to the destination directory.
        let mut robo_dest = Path::new(&dest).to_path_buf();
        robo_dest.push(
            Path::new(&src)
                .file_name()
                .expect("Cannot get parent directory."),
        );
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
        //
        // Robocopy copies the contents of the source directory, not the source
        // directory itself. So, append the final path of the source directory
        // to the destination directory.
        os_copy_process = Command::new("robocopy");
        os_copy_process
            .args([
                "/MIR", "/MT", "/NFL", "/NDL", "/NJH", "/NJS", "/NP", "/NS", "/NC",
            ])
            .arg(src)
            .arg(robo_dest)
    } else {
        os_copy_process = Command::new("rsync");
        os_copy_process
            .args(["--archive", "--delete"])
            .arg(src)
            .arg(dest)
    };

    // Print the command, to help this produces an error.
    println!("{:#?}", &copy_process);

    // Per
    // [these docs](https://learn.microsoft.com/en-us/troubleshoot/windows-server/backup-and-storage/return-codes-used-robocopy-utility),
    // check the return code.
    if copy_process.status()?.code().expect("Error copying") < 8 {
        Ok(())
    } else {
        Err("Copy failed".into())
    }
}

#[cfg(debug_assertions)]
fn remove_dir_all_if_exists<P: AsRef<Path> + std::fmt::Display>(
    path: P,
) -> Result<(), Box<dyn std::error::Error>> {
    if Path::new(path.as_ref()).try_exists().unwrap() {
        fs::remove_dir_all(path.as_ref())?;
    }

    Ok(())
}

// ### Core routines
//
// These functions simplify common build-focused development tasks and support
// CI builds.
#[cfg(debug_assertions)]
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
    quick_copy_dir("../client/node_modules/mathjax/", "../client/static")?;
    quick_copy_dir(
        "../client/node_modules/mathjax-modern-font/chtml",
        "../client/static/mathjax-modern-font",
    )?;
    // Copy over the graphviz files needed.
    quick_copy_file(
        "../client/node_modules/graphviz-webcomponent/dist/renderer.min.js",
        "../client/static/graphviz-webcomponent/renderer.min.js",
    )?;
    quick_copy_file(
        "../client/node_modules/graphviz-webcomponent/dist/renderer.min.js.map",
        "../client/static/graphviz-webcomponent/renderer.min.js.map",
    )?;

    Ok(())
}

#[cfg(debug_assertions)]
fn run_install() -> Result<(), Box<dyn std::error::Error>> {
    run_script("npm", &["install"], "../client")?;
    patch_client_npm()?;
    run_script("npm", &["install"], "../extensions/VSCode")?;
    run_cmd!(cargo fetch)?;
    Ok(())
}

#[cfg(debug_assertions)]
fn run_update() -> Result<(), Box<dyn std::error::Error>> {
    run_script("npm", &["update"], "../client")?;
    patch_client_npm()?;
    run_script("npm", &["update"], "../extensions/VSCode")?;
    run_script("npm", &["outdated"], "../client")?;
    run_script("npm", &["outdated"], "../extensions/VSCode")?;
    run_cmd!(cargo update)?;
    Ok(())
}

#[cfg(debug_assertions)]
fn run_build() -> Result<(), Box<dyn std::error::Error>> {
    // Clean out all bundled files before the rebuild.
    remove_dir_all_if_exists("../client/static/bundled")?;
    run_script("npm", &["run", "build"], "../client")?;
    run_script("npm", &["run", "compile"], "../extensions/VSCode")?;
    run_cmd!(cargo build)?;
    Ok(())
}

#[cfg(debug_assertions)]
fn run_prerelease() -> Result<(), Box<dyn std::error::Error>> {
    // Clean out all bundled files before the rebuild.
    remove_dir_all_if_exists("../client/static/bundled")?;
    run_install()?;
    run_script("npm", &["run", "dist"], "../client")?;

    Ok(())
}

#[cfg(debug_assertions)]
fn run_postrelease() -> Result<(), Box<dyn std::error::Error>> {
    let server_dir = "../extensions/VSCode/server";
    // Only clean the `server/` directory if it exists.
    remove_dir_all_if_exists(server_dir)?;
    let src_prefix = "target/distrib/";
    let src_name_prefix = "codechat-editor-server";

    #[cfg(windows)]
    let (src_name, vsce_target) = (
        format!("{src_name_prefix}-x86_64-pc-windows-msvc"),
        "win32-x64",
    );
    #[cfg(unix)]
    let (src_name, vsce_target) = (
        format!("{src_name_prefix}-x86_64-unknown-linux-gnu"),
        "linux-x64",
    );
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    let (src_name, vsce_target) = (
        format!("{src_name_prefix}-x86_64-apple-darwin"),
        "darwin-x64",
    );
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    let (src_name, vsce_target) = (
        format!("{src_name_prefix}-aarch64-apple-darwin"),
        "darwin-arm64",
    );

    let src = format!("{src_prefix}{src_name}");
    quick_copy_dir(src.as_str(), "../extensions/VSCode")?;
    fs::rename(format!("../extensions/VSCode/{src_name}"), server_dir)?;
    run_script(
        "npx",
        &["vsce", "package", "--target", vsce_target],
        "../extensions/VSCode",
    )?;

    Ok(())
}

// ### CLI implementation
//
// The following code implements the command-line interface for the CodeChat
// Editor.
impl Cli {
    fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        match &self.command {
            Commands::Serve(serve_command) => {
                #[cfg(debug_assertions)]
                if let Some(TestMode::Sleep) = self.test_mode {
                    // For testing, don't start the server at all.
                    std::thread::sleep(std::time::Duration::from_secs(10));
                    return Ok(());
                }
                webserver::configure_logger(serve_command.log.unwrap_or(LevelFilter::Info));
                webserver::main(self.port).unwrap();
            }
            Commands::Start => {
                println!("Starting server in background...");
                let current_exe = match env::current_exe() {
                    Ok(exe) => exe,
                    Err(e) => return Err(format!("Failed to get current executable: {e}").into()),
                };
                // Define here, to avoid lifetime issues.
                #[cfg(debug_assertions)]
                let mut cmd_temp: Command;
                #[cfg(debug_assertions)]
                let cmd = match self.test_mode {
                    None => &mut Command::new(current_exe),
                    Some(TestMode::NotFound) => &mut Command::new("nonexistent-command"),
                    Some(TestMode::Sleep) => {
                        cmd_temp = Command::new(current_exe);
                        cmd_temp.args(["--test-mode", "sleep"])
                    }
                };
                #[cfg(not(debug_assertions))]
                let cmd = &mut Command::new(current_exe);
                let mut process = match cmd
                    .args(["--port", &self.port.to_string(), "serve", "--log", "off"])
                    // Subtle: the default of `stdout(Stdio::inherit())` causes
                    // a parent process to block, since the child process
                    // inherits the parent's stdout. So, use the pipes to avoid
                    // blocking.
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                {
                    Ok(process) => process,
                    Err(e) => {
                        return Err(format!("Failed to start server: {e}").into());
                    }
                };
                // Poll the server to ensure it starts.
                let now = SystemTime::now();
                loop {
                    // Look for a ping/pong response from the server.
                    match minreq::get(format!("http://{IP_ADDRESS}:{}/ping", self.port))
                        .with_timeout(3)
                        .send()
                    {
                        Ok(response) => {
                            let status_code = response.status_code;
                            let body = response.as_str().unwrap_or("Non-text body");
                            if status_code == 200 && body == "pong" {
                                println!("Server started.");
                                return Ok(());
                            } else {
                                eprintln!(
                                    "Unexpected response from server: {body}, status code = {status_code}"
                                );
                            }
                        }
                        Err(err) => {
                            eprintln!("Failed to start server: {err}");
                        }
                    }

                    // Check if the server has exited or failed to start.
                    match process.try_wait() {
                        Ok(Some(status)) => {
                            let mut stdout_buf = String::new();
                            let mut stderr_buf = String::new();
                            let stdout = process.stdout.as_mut().unwrap();
                            let stderr = process.stderr.as_mut().unwrap();
                            stdout.read_to_string(&mut stdout_buf).unwrap();
                            stderr.read_to_string(&mut stderr_buf).unwrap();
                            return Err(format!(
                                "Server failed to start: {status:?}\n{stdout_buf}\n{stderr_buf}"
                            )
                            .into());
                        }
                        Ok(None) => {}
                        Err(e) => return Err(format!("Error starting server: {e}").into()),
                    }
                    // Wait a bit before trying again.
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    // Give up after a few seconds.
                    match now.elapsed() {
                        Ok(elapsed) => {
                            if elapsed.as_secs() > 5 {
                                return Err("Server failed to start after 5 seconds.".into());
                            }
                        }

                        Err(e) => return Err(format!("Error getting elapsed time: {e}").into()),
                    }
                }
            }
            Commands::Stop => {
                println!("Stopping server...");
                // TODO: Use https://crates.io/crates/sysinfo to find the server
                // process and kill it if it doesn't respond to a stop request.
                return match minreq::get(format!("http://{IP_ADDRESS}:{}/stop", self.port))
                    .with_timeout(3)
                    .send()
                {
                    Err(err) => Err(format!("Failed to stop server: {err}").into()),
                    Ok(response) => {
                        let status_code = response.status_code;
                        if status_code == 204 {
                            println!("Server shutting down.");
                            Ok(())
                        } else {
                            Err(format!(
                                "Unexpected response from server: {}, status code = {status_code}",
                                response.as_str().unwrap_or("Non-text body")
                            )
                            .into())
                        }
                    }
                };
            }
            #[cfg(debug_assertions)]
            Commands::Install => return run_install(),
            #[cfg(debug_assertions)]
            Commands::Update => return run_update(),
            #[cfg(debug_assertions)]
            Commands::Build => return run_build(),
            #[cfg(debug_assertions)]
            Commands::Prerelease => return run_prerelease(),
            #[cfg(debug_assertions)]
            Commands::Postrelease => return run_postrelease(),
        }

        Ok(())
    }
}

#[cfg(not(tarpaulin_include))]
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
