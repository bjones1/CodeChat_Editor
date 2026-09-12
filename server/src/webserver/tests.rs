// Copyright (C) 2026 Bryan A. Jones.
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
//! `test.rs` -- Unit tests for the vscode interface
//! ================================================
// Imports
// -------
//
// ### Standard library
use std::path::{MAIN_SEPARATOR_STR, PathBuf};
// Only the Windows-only tests below name a path as a literal.
#[cfg(windows)]
use std::path::Path;

// ### Third-party
use assertables::{assert_ends_with, assert_not_contains, assert_starts_with};

// ### Local
use super::{path_to_url, request_path_to_file_path, try_canonicalize, url_to_path};
// `simplify` changes a path only on Windows, so only a Windows-only test
// exercises it.
#[cfg(windows)]
use super::simplify;
use crate::ide::vscode::tests::IP_PORT;
use test_utils::{cast, prep_test_dir};

// Tests
// -----
// An arbitrary set of URL path segments used only as example test data for
// `url_to_path`'s `expected_prefix` parameter -- it matches the filewatcher
// IDE's own prefix (see `extensions/standalone/src/filewatcher.rs`), but any
// prefix would do here.
const PATH_PREFIX: &[&str] = &["fw", "fsc"];
// The same prefix as `PATH_PREFIX`, spelled the way `path_to_url` takes it, and
// an arbitrary connection ID to pair with it.
const URL_PREFIX: &str = "/fw/fsc";
const CONNECTION_ID: &str = "dummy_connection_id";

#[test]
fn test_url_to_path() {
    let (temp_dir, test_dir) = prep_test_dir!();

    // Test a non-existent path.
    assert_eq!(
        cast!(
            url_to_path(
                &format!(
                    "http://127.0.0.1:{IP_PORT}/fw/fsc/dummy_connection_id/{}path%20spaces/foo.py",
                    if cfg!(windows) { "C:/" } else { "" }
                ),
                PATH_PREFIX
            ),
            Ok
        ),
        PathBuf::from(format!(
            "{}path spaces{MAIN_SEPARATOR_STR}foo.py",
            if cfg!(windows) { "C:\\" } else { "/" }
        ),)
    );

    // Test a path with a backslash in it. Windows can't name a file this way,
    // so the encoded backslash must survive decoding as an encoded backslash;
    // on OS X/Linux it's an ordinary character in a file name, so it decodes
    // like any other.
    assert_eq!(
        cast!(
            url_to_path(
                &format!(
                    "http://127.0.0.1:{IP_PORT}/fw/fsc/dummy_connection_id/{}foo%5Cbar.py",
                    if cfg!(windows) { "C:/" } else { "" }
                ),
                PATH_PREFIX
            ),
            Ok
        ),
        PathBuf::from(if cfg!(windows) {
            r"C:\foo%5Cbar.py"
        } else {
            r"/foo\bar.py"
        })
    );

    // Test an actual path.
    let test_dir_str = test_dir.to_str().unwrap();
    assert_eq!(
        url_to_path(
            &format!(
                "http://127.0.0.1:{IP_PORT}/fw/fsc/dummy_connection_id/{test_dir_str}/test%20spaces.py"
            ),
            PATH_PREFIX
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
    // The test directory lies on a drive (Windows) or under the root directory
    // (OS X/Linux), never on a network share, so its URL contains no empty
    // segment -- and therefore no double forward slash.
    assert_not_contains!(url, "//");
    // Report any errors produced when removing the temporary directory.
    temp_dir.close().unwrap();
}

// On Windows, a file on a network share is named by a UNC path, whose prefix
// (`\\server\share`) holds the server and share names in place of a drive
// letter. Spell that prefix in a URL as `//server/share`: the empty first
// segment marks the path as a UNC path, letting `url_to_path` restore it.
#[cfg(windows)]
#[test]
fn test_unc_path_to_url() {
    let url = path_to_url(
        "/fw/fsc",
        Some("dummy_connection_id"),
        Path::new(r"\\a_server\a_share\bar\foo.py"),
    );
    assert_eq!(
        url,
        "/fw/fsc/dummy_connection_id//a_server/a_share/bar/foo.py"
    );
    // Every separator in the URL must be a forward slash; an encoded backslash
    // would mean the server and share names never became path segments.
    assert_not_contains!(url, "%5C");
}

// Every consumer of a URL `path_to_url` produced must recover the path it was
// built from, for every shape of path this platform provides. The Client parses
// a whole URL with `url_to_path`; a filesystem route instead receives only the
// path its match captured, already percent-decoded, and converts that with
// `request_path_to_file_path`. Check both, since a URL which only one of them
// understands reaches the Client as a broken link.
//
// None of these files exist, which is fine: an absolute path which names no
// file passes through `try_canonicalize` unchanged.
#[test]
fn test_path_to_url_round_trip() {
    let file_paths = if cfg!(windows) {
        vec![
            r"C:\Users\test\foo.py",
            r"C:\Users\bar bar\foo.py",
            // A file on a network share.
            r"\\a-server\a-share\bar\foo.py",
            // Characters which are URL syntax must survive encoding.
            r"C:\Users\a#b\c%d\été\foo.py",
        ]
    } else {
        vec![
            "/home/test/foo.py",
            "/home/bar bar/foo.py",
            // Characters which are URL syntax must survive encoding.
            "/home/a#b/c%d/été/foo.py",
            // A backslash names a file here rather than dividing a path.
            r"/home/a\b.py",
        ]
    };
    for file_path in file_paths {
        let file_path = PathBuf::from(file_path);
        let url = path_to_url(URL_PREFIX, Some(CONNECTION_ID), &file_path);
        assert_eq!(
            cast!(
                url_to_path(&format!("http://127.0.0.1:{IP_PORT}{url}"), PATH_PREFIX),
                Ok
            ),
            file_path,
            "round trip of {} through `url_to_path`",
            file_path.display()
        );

        // Reproduce what the route does before it calls
        // `request_path_to_file_path`: drop the prefix and connection ID which
        // the pattern's literal segments match, then percent-decode the rest.
        let captured = url
            .strip_prefix(&format!("{URL_PREFIX}/{CONNECTION_ID}/"))
            .expect("URL should start with the prefix it was given");
        let captured = cast!(urlencoding::decode(captured), Ok);
        assert_eq!(
            cast!(try_canonicalize(&request_path_to_file_path(&captured)), Ok),
            file_path,
            "round trip of {} through `request_path_to_file_path`",
            file_path.display()
        );
    }
}

// The Client fetches a file's contents through a filesystem route, which
// captures the file's path from the URL and percent-decodes it. That conversion
// is separate from `url_to_path`'s, so check it against the same set of paths:
// each captured path must name the file whose URL produced it.
#[test]
fn test_request_path_to_file_path() {
    let cases = if cfg!(windows) {
        vec![
            ("C:/Users/test spaces.py", r"C:\Users\test spaces.py"),
            // The route's match absorbs one of the two leading slashes in a UNC
            // path's `//server/share` spelling.
            (
                "/engr-fs-05.engr.msstate.edu/engr_abet/2023 Cycle/foo.py",
                r"\\engr-fs-05.engr.msstate.edu\engr_abet\2023 Cycle\foo.py",
            ),
        ]
    } else {
        vec![
            ("home/test spaces.py", "/home/test spaces.py"),
            ("home/2023 Cycle/foo.py", "/home/2023 Cycle/foo.py"),
        ]
    };
    for (request_path, file_path) in cases {
        assert_eq!(
            cast!(
                try_canonicalize(&request_path_to_file_path(request_path)),
                Ok
            ),
            PathBuf::from(file_path),
            "conversion of {request_path}"
        );
    }
}

// `canonicalize` returns a file on a network share in verbatim form, which
// names the same file but matches neither the path an IDE sends nor the URL the
// Client requests.
#[cfg(windows)]
#[test]
fn test_simplify() {
    // A path on a network share loses its verbatim prefix.
    assert_eq!(
        simplify(Path::new(
            r"\\?\UNC\engr-fs-05.engr.msstate.edu\engr_abet\2023 Cycle\test.py"
        )),
        Path::new(r"\\engr-fs-05.engr.msstate.edu\engr_abet\2023 Cycle\test.py")
    );
    // A path on a drive is left to `dunce`, and a path which is already
    // simplified passes through unchanged.
    assert_eq!(
        simplify(Path::new(r"\\?\C:\Users\test.py")),
        Path::new(r"C:\Users\test.py")
    );
    assert_eq!(
        simplify(Path::new(r"\\engr-fs-05\engr_abet\test.py")),
        Path::new(r"\\engr-fs-05\engr_abet\test.py")
    );
    // A file name which only the verbatim form can express -- here a reserved
    // DOS device name -- keeps its prefix.
    assert_eq!(
        simplify(Path::new(r"\\?\UNC\engr-fs-05\engr_abet\CON\test.py")),
        Path::new(r"\\?\UNC\engr-fs-05\engr_abet\CON\test.py")
    );
}
