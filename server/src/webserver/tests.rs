// Copyright (C) 2023 Bryan A. Jones.
//
// This file is part of the CodeChat Editor. The CodeChat Editor is free
// software: you can redistribute it and/or modify it under the terms of the
// GNU General Public License as published by the Free Software Foundation,
// either version 3 of the License, or (at your option) any later version.
//
// The CodeChat Editor is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY
// or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for
// more details.
//
// You should have received a copy of the GNU General Public License along with
// the CodeChat Editor. If not, see
// [http://www.gnu.org/licenses](http://www.gnu.org/licenses).
//
/// # `test.rs` -- Unit tests for the webserver
// ## Imports
use std::{
    env::{current_dir, set_current_dir},
    path::{self, PathBuf, MAIN_SEPARATOR_STR},
    thread::{self, sleep},
    time::Duration,
};

use assert_cmd::Command;
use assertables::{assert_ends_with, assert_starts_with};

use super::{filewatcher::FILEWATCHER_PATH_PREFIX, path_to_url, url_to_path};
use crate::prep_test_dir;

// ## Constants
/// The default port on which the server listens for incoming connections.
pub const IP_PORT: u16 = 8080;

// ## Support functions
fn get_server() -> Command {
    Command::cargo_bin("codechat-editor-server").unwrap()
}

// ## Tests
#[test]
fn test_url_to_path() {
    assert_eq!(
        url_to_path(
            "http://127.0.0.1:8080/fw/fsc/dummy_connection_id/path%20spaces/foo.py",
            FILEWATCHER_PATH_PREFIX
        ),
        Ok(path::absolute(PathBuf::from(format!(
            "path spaces{MAIN_SEPARATOR_STR}foo.py"
        )))
        .unwrap())
    );
}

#[test]
fn test_path_to_url() {
    let (temp_dir, test_dir) = prep_test_dir!();

    let mut file_path = test_dir.clone();
    file_path.push("test spaces.py");
    let url = path_to_url("/a/b", "conn1", &file_path);
    assert_starts_with!(url, "/a/b/conn1/");
    assert_ends_with!(url, "test_path_to_url/test%20spaces.py");
    // Report any errors produced when removing the temporary directory.
    temp_dir.close().unwrap();
}

// Test startup outside the repo path.
#[test]
fn test_other_path() {
    let (temp_dir, test_dir) = prep_test_dir!();

    let old_dir = current_dir().unwrap();
    set_current_dir(&test_dir).unwrap();

    // Start the server.
    let handle = thread::spawn(move || {
        get_server()
            .args(["--port", "8083", "start"])
            .assert()
            .success();
    });
    sleep(Duration::from_millis(1000));
    get_server()
        .args(["--port", "8083", "stop"])
        .assert()
        .success();
    handle.join().unwrap();

    // Restore the original directory, so the temporary directory can be removed.
    set_current_dir(&old_dir).unwrap();

    // Report any errors produced when removing the temporary directory.
    temp_dir.close().unwrap();
}
