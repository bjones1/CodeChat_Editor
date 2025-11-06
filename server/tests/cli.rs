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
//
// None.
//
// ### Third-party
use actix_web::{App, HttpServer};
use assert_cmd::Command;
use predicates::{prelude::predicate, str::contains};

// ### Local
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
