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

// ### Third-party
#[cfg(debug_assertions)]
use clap::ValueEnum;
use clap::{Parser, Subcommand};
use log::LevelFilter;

// ### Local
use code_chat_editor::webserver::{self, IP_ADDRESS};

// ## Data structures
//
// ### Command-line interface
//
// The following defines the command-line interface for the CodeChat Editor.
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
    Serve {
        /// Control logging verbosity.
        #[arg(short, long)]
        log: Option<LevelFilter>,
    },
    /// Start the webserver in a child process then exit.
    Start,
    /// Stop the webserver child process.
    Stop,
}

// ## Code
//
// The following code implements the command-line interface for the CodeChat
// Editor.
impl Cli {
    fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        match &self.command {
            Commands::Serve { log } => {
                #[cfg(debug_assertions)]
                if let Some(TestMode::Sleep) = self.test_mode {
                    // For testing, don't start the server at all.
                    std::thread::sleep(std::time::Duration::from_secs(10));
                    return Ok(());
                }
                webserver::configure_logger(log.unwrap_or(LevelFilter::Info));
                webserver::main(self.port).unwrap();
            }
            Commands::Start => {
                println!("Starting server in background...");
                let current_exe = match env::current_exe() {
                    Ok(exe) => exe,
                    Err(e) => return Err(format!("Failed to get current executable: {e}").into()),
                };
                #[cfg(not(debug_assertions))]
                let mut cmd = Command::new(&current_exe);
                #[cfg(debug_assertions)]
                let mut cmd;
                #[cfg(debug_assertions)]
                match self.test_mode {
                    None => cmd = Command::new(&current_exe),
                    Some(TestMode::NotFound) => cmd = Command::new("nonexistent-command"),
                    Some(TestMode::Sleep) => {
                        cmd = Command::new(&current_exe);
                        cmd.args(["--test-mode", "sleep"]);
                    }
                }
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
