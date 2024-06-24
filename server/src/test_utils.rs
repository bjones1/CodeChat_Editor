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
/// # `test_utils.rs` -- Reusable routines for testing
/// Placing this file in the `tests/` directory prevents me from importing it outside that directory tree; the desire was to import this for unit tests in the `src/` directory tree. So, it's instead placed here, then conditionally imported in `lib.rs`.
//
// ## Imports
//
// ### Standard library
use std::env;
use std::path::PathBuf;
use std::path::MAIN_SEPARATOR_STR;

// ### Third-party
use assert_fs::fixture::PathCopy;
use assert_fs::TempDir;

// ## Code
// Get the name (and module path) to the current function. From [SO](https://stackoverflow.com/a/40234666).
#[macro_export]
macro_rules! function_name {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        // Since we just called the nested function f, strip this off the end of the returned value.
        name.strip_suffix("::f").unwrap()
    }};
}

// Call `_prep_test_dir` with the correct parameter -- `function_name!()`.
#[macro_export]
macro_rules! prep_test_dir {
    () => {{
        use crate::function_name;
        use crate::test_utils::_prep_test_dir;
        _prep_test_dir(function_name!())
    }};
}

// Use the `tests/fixtures` path (relative to the root of this Rust project)
// to store files for testing. A subdirectory tree, named by the module path then name of the test
// function by convention, contains everything needed for this test. Copy
// this over to a temporary directory, then run tests there.
pub fn _prep_test_dir(
    // The name of and Rust path to the test function to prepare files for. Call `prep_test_dir!()` to provide this parameter.
    test_full_name: &str,
) -> (
    // The temporary directory created which stores files to use in testing.
    TempDir,
    // The
    PathBuf,
) {
    // Omit the first element of the full module path (the name of the root module).
    let test_full_name = test_full_name.strip_prefix("code_chat_editor::").unwrap();
    // Get rid of closures in the path.
    let test_path = &test_full_name.replace("::{{closure}}", "");
    // Switch from `::` to a filesystem path separator.
    let test_path = &test_path.replace("::", MAIN_SEPARATOR_STR);

    // First, get the project root directory, based on the
    // [location of the cargo.toml file](https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-crates).
    let root_dir = &env::var("CARGO_MANIFEST_DIR").expect("$CARGO_MANIFEST_DIR");
    let mut source_path = PathBuf::from(root_dir);
    // Append the path for test files.
    source_path.push("tests/fixtures");
    // Then the path for the current module
    source_path.push(test_path);
    // Separate out the name of the function, which is now the last element in this path.
    let source_path_tmp = source_path.clone();
    let test_name = source_path_tmp.file_name().unwrap().to_str().unwrap();
    source_path.pop();

    // For debugging, append
    // [.into_persistent()](https://docs.rs/assert_fs/latest/assert_fs/fixture/struct.TempDir.html#method.into_persistent).
    let temp_dir = TempDir::new().unwrap();
    // Create a temporary directory, then copy everything needed for this
    // test to it. Since the `patterns` parameter is a glob, append `/**` to
    // the directory to copy to get all files/subdirectories.
    if let Err(err) = temp_dir.copy_from(&source_path, &[format!("{test_name}/**")]) {
        panic!(
            "Unable to copy files from {}{MAIN_SEPARATOR_STR}{}: {err}",
            source_path.to_string_lossy(),
            test_name
        );
    }

    // This is a path where testing takes place.
    let test_dir = temp_dir.path().join(test_name);

    (temp_dir, test_dir)
}
