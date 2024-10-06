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
/// # `main.rs` -- Entrypoint for the CodeChat Editor Server
// ## Imports
//
// ### Standard library
//
use std::{
    env,
    process::{exit, Command},
    time::SystemTime,
};

// ### Third-party
use clap::{Args, Parser, Subcommand};
use log::LevelFilter;

// ### Local
use code_chat_editor::webserver::{self, IP_ADDRESS, IP_PORT};

// ## Code
//
// ### Command-line interface
// The following code defines the command-line interface for the CodeChat Editor.
#[derive(Parser)]
#[command(name = "The CodeChat Editor Server", version, about, long_about=None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the webserver.
    Serve(ServeCommand),
    /// Start the webserver in a child process then exit.
    Start,
    /// Stop the webserver child process.
    Stop,
}

#[derive(Args)]
struct ServeCommand {
    /// Control logging verbosity.
    #[arg(short, long)]
    log: Option<LevelFilter>,
}

// ### CLI implementation
// The following code implements the command-line interface for the CodeChat Editor.
impl Cli {
    fn run(self) {
        match &self.command {
            Some(Commands::Serve(serve_commad)) => {
                webserver::configure_logger(serve_commad.log.unwrap_or(LevelFilter::Warn));
                webserver::main().unwrap();
            }
            None | Some(Commands::Start) => {
                println!("Starting server in background...");
                let current_exe = match env::current_exe() {
                    Ok(exe) => exe,
                    Err(e) => {
                        eprintln!("Failed to get current executable: {}", e);
                        exit(1);
                    }
                };
                let mut process = match Command::new(current_exe)
                    .args(["serve", "--log", "off"])
                    .spawn()
                {
                    Ok(process) => process,
                    Err(e) => {
                        eprintln!("Failed to start server: {}", e);
                        exit(1);
                    }
                };
                // Poll the server to ensure it starts.
                let now = SystemTime::now();
                loop {
                    // Look for a ping/pong response from the server.
                    if let Ok(response) =
                        minreq::get(format!("http://{IP_ADDRESS}:{IP_PORT}/ping")).send()
                    {
                        let status_code = response.status_code;
                        let body = response.as_str().unwrap_or("Non-text body");
                        if status_code == 200 && body == "pong" {
                            break;
                        } else {
                            eprintln!(
                                "Unexpected response from server: {body}, status code = {status_code}"
                            );
                        }
                    }

                    // Check if the server has exited or failed to start.
                    match process.try_wait() {
                        Ok(Some(status)) => {
                            eprintln!("Server failed to start: {:?}", status);
                            exit(1);
                        }
                        Ok(None) => {}
                        Err(e) => {
                            eprintln!("Error starting server: {e}");
                            exit(1);
                        }
                    }
                    // Wait a bit before trying again.
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    // Give up after a few seconds.
                    match now.elapsed() {
                        Ok(elapsed) if elapsed.as_secs() > 5 => {
                            eprintln!("Server failed to start after 5 seconds.");
                            exit(1);
                        }
                        Err(e) => {
                            eprintln!("Error getting elapsed time: {e}");
                            exit(1);
                        }
                        _ => {}
                    }
                }
                println!("Server started.");
            }
            Some(Commands::Stop) => {
                println!("Stopping server...");
                // TODO: Use https://crates.io/crates/sysinfo to find the server process and kill it if it doesn't respond to a stop request.
                let err_msg = match minreq::get(format!("http://{IP_ADDRESS}:{IP_PORT}/stop"))
                    .with_timeout(3)
                    .send()
                {
                    Err(err) => format!("Failed to stop server: {err}"),
                    Ok(response) => {
                        let status_code = response.status_code;
                        if status_code == 204 {
                            println!("Server shutting down.");
                            exit(0);
                        } else {
                            format!(
                                "Unexpected response from server: {}, status code = {status_code}",
                                response.as_str().unwrap_or("Non-text body")
                            )
                        }
                    }
                };
                eprintln!("{}", err_msg);
                exit(1);
            }
        }
    }
}

#[cfg(not(tarpaulin_include))]
fn main() {
    let cli = Cli::parse();
    cli.run();
}
