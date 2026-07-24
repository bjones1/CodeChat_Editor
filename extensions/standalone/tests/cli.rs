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
/// `cli.rs` - Test the CLI interface
/// =================================
// Imports
// -------
//
// ### Standard library
#[cfg(not(target_os = "macos"))]
use std::{thread::sleep, time::Duration};

// ### Third-party
use actix_web::{App, HttpResponse, HttpServer, web};
use assert_cmd::Command;
use predicates::{prelude::predicate, str::contains};

// ### Local
use test_utils::prep_test_dir;
use tokio::task::spawn_blocking;

// Support functions
// -----------------
//
// The lint on using `cargo_bin` doesn't apply, since this is only available for
// integration tests per the
// [docs](https://docs.rs/assert_cmd/latest/assert_cmd/cargo/macro.cargo_bin_cmd.html).
// Text of the warning:
//
// ```
// warning: use of deprecated associated function `assert_cmd::Command::cargo_bin`:
//   incompatible with a custom cargo build-dir, see instead `cargo::cargo_bin_cmd!`
// ```
#[allow(deprecated)]
fn get_server() -> Command {
    Command::cargo_bin(assert_cmd::pkg_name!()).unwrap()
}

// Tests
// -----
#[test]
fn test_start_not_found() {
    let mut cmd = get_server();
    let assert = cmd.args(["--test-mode", "not-found", "start"]).assert();
    assert
        .failure()
        .stderr(predicate::str::contains("Failed to start server"));
}

#[test]
fn test_start_no_start() {
    let assert = get_server()
        .args(["--test-mode", "sleep", "--port", "8081", "start"])
        .assert();
    assert
        .failure()
        .stderr(contains("Server failed to start after 5 seconds."));
}

#[actix_web::test]
async fn test_start_no_response() {
    // Run a dummy webserver that doesn't respond to the `/stop` endpoint.
    actix_rt::spawn(async move {
        HttpServer::new(App::new)
            .bind(("127.0.0.1", 8082))
            .unwrap()
            .run()
            .await
            .unwrap();
    });
    // The test must be run in a separate thread to avoid blocking the main
    // thread; otherwise, the webserver will not respond.
    let test = spawn_blocking(move || {
        let assert = get_server()
            .args(["--test-mode", "sleep", "--port", "8082", "start"])
            .assert();
        assert
            .failure()
            .stderr(contains("Server failed to start after 5 seconds."))
            .stderr(contains("status code = 404"));
    });
    test.await.unwrap();
}

// ### `stop` subcommand
//
// Stopping when no server is listening should report a connection failure.
#[test]
fn test_stop_no_server() {
    let assert = get_server()
        // Use a port that nothing is listening on.
        .args(["--port", "8083", "stop"])
        .assert();
    assert.failure().stderr(contains("Failed to stop server"));
}

// A server that responds to `/stop` with the expected 204 causes `stop` to
// succeed.
#[actix_web::test]
async fn test_stop_success() {
    actix_rt::spawn(async move {
        HttpServer::new(|| {
            App::new().route(
                "/stop",
                web::get().to(|| async { HttpResponse::NoContent().finish() }),
            )
        })
        .bind(("127.0.0.1", 8084))
        .unwrap()
        .run()
        .await
        .unwrap();
    });
    let test = spawn_blocking(move || {
        let assert = get_server().args(["--port", "8084", "stop"]).assert();
        assert.success();
    });
    test.await.unwrap();
}

// A server that responds to `/stop` with an unexpected status code causes
// `stop` to report the unexpected response.
#[actix_web::test]
async fn test_stop_unexpected_response() {
    actix_rt::spawn(async move {
        HttpServer::new(|| {
            App::new().route(
                "/stop",
                web::get().to(|| async { HttpResponse::Ok().body("nope") }),
            )
        })
        .bind(("127.0.0.1", 8085))
        .unwrap()
        .run()
        .await
        .unwrap();
    });
    let test = spawn_blocking(move || {
        let assert = get_server().args(["--port", "8085", "stop"]).assert();
        assert
            .failure()
            .stderr(contains("Unexpected response from server"))
            .stderr(contains("status code = 200"));
    });
    test.await.unwrap();
}

// ### Argument parsing
//
// An out-of-range port is rejected by the `port_in_range` validator.
#[test]
fn test_port_out_of_range() {
    let assert = get_server().args(["--port", "0", "serve"]).assert();
    assert.failure().stderr(contains("port not in range"));
}

// A non-numeric port is rejected.
#[test]
fn test_port_not_a_number() {
    let assert = get_server().args(["--port", "abc", "serve"]).assert();
    assert.failure().stderr(contains("isn't a port number"));
}

// Test startup outside the repo path. For some reason, this fails
// intermittently on Mac. Ignore these failures.
#[cfg(not(target_os = "macos"))]
#[test]
fn test_other_path() {
    let (temp_dir, test_dir) = prep_test_dir!();

    // Start the server. Calling `output()` causes the program to hang; call
    // `status()` instead. Since the `assert_cmd` crates doesn't offer this, use
    // the std lib instead.
    std::process::Command::new(get_server().get_program())
        .args(["--port", "8083", "start"])
        .current_dir(&test_dir)
        .status()
        .expect("failed to start server");

    // Stop it.
    get_server()
        .args(["--port", "8083", "stop"])
        .current_dir(&test_dir)
        .assert()
        .success();

    // Wait for the server to exit, since it locks the temp\_dir.
    sleep(Duration::from_secs(3));

    // Report any errors produced when removing the temporary directory.
    temp_dir.close().unwrap();
}
