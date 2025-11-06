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
/// `test.rs` -- Unit tests for the vscode interface
/// ================================================
// Imports
// -------
use std::{
    path::{MAIN_SEPARATOR_STR, PathBuf},
    thread::{self, sleep},
    time::Duration,
};

use assert_cmd::Command;
use assertables::{assert_ends_with, assert_not_contains, assert_starts_with};

use super::{path_to_url, url_to_path};
use crate::ide::{filewatcher::FILEWATCHER_PATH_PREFIX, vscode::tests::IP_PORT};
use crate::prep_test_dir;

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
fn test_url_to_path() {
    let (temp_dir, test_dir) = prep_test_dir!();

    // Test a non-existent path.
    assert_eq!(
        url_to_path(
            &format!(
                "http://127.0.0.1:{IP_PORT}/fw/fsc/dummy_connection_id/{}path%20spaces/foo.py",
                if cfg!(windows) { "C:/" } else { "" }
            ),
            FILEWATCHER_PATH_PREFIX
        ),
        Ok(PathBuf::from(format!(
            "{}path spaces{MAIN_SEPARATOR_STR}foo.py",
            if cfg!(windows) { "C:\\" } else { "/" }
        ),))
    );

    // Test a path with a backslash in it.
    assert_eq!(
        url_to_path(
            &format!(
                "http://127.0.0.1:{IP_PORT}/fw/fsc/dummy_connection_id/{}foo%5Cbar.py",
                if cfg!(windows) { "C:/" } else { "" }
            ),
            FILEWATCHER_PATH_PREFIX
        ),
        Ok(PathBuf::from(format!(
            "{}foo%5Cbar.py",
            if cfg!(windows) { "C:\\" } else { "/" }
        ),))
    );

    // Test an actual path.
    let test_dir_str = test_dir.to_str().unwrap();
    assert_eq!(
        url_to_path(
            &format!(
                "http://127.0.0.1:{IP_PORT}/fw/fsc/dummy_connection_id/{test_dir_str}/test%20spaces.py"
            ),
            FILEWATCHER_PATH_PREFIX
        )
        .unwrap()
        .canonicalize()
        .unwrap(),
        PathBuf::from(format!("{test_dir_str}{MAIN_SEPARATOR_STR}test spaces.py"))
            .canonicalize()
            .unwrap()
    );

    // Report any errors produced when removing the temporary directory.
    temp_dir.close().unwrap();
}

#[test]
fn test_path_to_url() {
    let (temp_dir, test_dir) = prep_test_dir!();

    let mut file_path = test_dir.clone();
    file_path.push("test spaces.py");
    let url = path_to_url("/a/b", Some("conn1"), &file_path);
    assert_starts_with!(url, "/a/b/conn1/");
    assert_ends_with!(url, "test_path_to_url/test%20spaces.py");
    // There shouldn't be a double forward slash in the name.
    assert_not_contains!(url, "//");
    // Report any errors produced when removing the temporary directory.
    temp_dir.close().unwrap();
}

// Test startup outside the repo path.
#[test]
fn test_other_path() {
    let (temp_dir, test_dir) = prep_test_dir!();

    // Start the server.
    let test_dir1 = test_dir.clone();
    let handle = thread::spawn(move || {
        get_server()
            .args(["--port", "8083", "start"])
            .current_dir(&test_dir1)
            .assert()
            .success();
    });
    // The server waits for up to 3 seconds for a ping to work. Add some extra
    // time for starting the process.
    sleep(Duration::from_millis(6000));
    get_server()
        .args(["--port", "8083", "stop"])
        .current_dir(&test_dir)
        .assert()
        .success();
    handle.join().unwrap();

    // Report any errors produced when removing the temporary directory.
    temp_dir.close().unwrap();
}
