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
use log::Level;

// ### Local
use crate::testing_logger;

// ## Macros
// Extract a known enum variant or fail. More concise than the alternative (`if let``, or `let else`). From [SO](https://stackoverflow.com/a/69324393). The macro does not handle nested pattern like `Some(Animal(cat))`.
#[macro_export]
macro_rules! cast {
    ($target: expr, $pat: path) => {
        {
            // The if let exploits recent Rust compiler's smart pattern matching. Contrary to other solutions like `into_variant`` and friends, this one macro covers all ownership usage like `self``, `&self`` and `&mut self``. On the other hand `{into,as,as_mut}_{variant}`` solution usually needs 3 * N method definitions where N is the number of variants.
            if let $pat(a) = $target {
                a
            } else {
                // If the variant and value mismatch, the macro will simply panic and report the expected pattern.
                panic!(
                    "mismatch variant when cast to {}",
                    stringify!($pat));
            }
        }
    };
}

#[macro_export]
macro_rules! cast2 {
    ($target: expr, $pat: path) => {
        {
            // The if let exploits recent Rust compiler's smart pattern matching. Contrary to other solutions like `into_variant`` and friends, this one macro covers all ownership usage like `self``, `&self`` and `&mut self``. On the other hand `{into,as,as_mut}_{variant}`` solution usually needs 3 * N method definitions where N is the number of variants.
            if let $pat(a1, a2) = $target {
                (a1, a2)
            } else {
                // If the variant and value mismatch, the macro will simply panic and report the expected pattern.
                panic!(
                    "mismatch variant when cast to {}",
                    stringify!($pat));
            }
        }
    };
}

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

// ## Code
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
    let root_dir = &env::var("CARGO_MANIFEST_DIR")
        .expect("Environment variable CARGO_MANIFEST_DIR not defined.");
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

// Testing with logs is subtle. If logs won't be examined by unit tests,
// this is straightforward. However, to sometimes simply log data and at
// other times examine logs requires care:
//
// 1.  The global logger can only be configured once. Configuring it for one
//     test for the production logger and for another test using the testing
//     logger doesn't work.
// 2.  Since tests are run by default in multiple threads, the logger used
//     should keep each thread's logs separate.
// 3.  The logger needs to be initialized for all tests and for production,
//     preferably without adding code to each test.
//
// The modified `testing_logger` takes care of items 2 and 3. For item 3, I
// don't have a way to auto-initialize the logger for all tests easily;
// [test-log](https://crates.io/crates/test-log) seems like a possibility,
// but it works only for `env_logger`. While `rstest` offers fixtures, this
// seems like a bit of overkill to call one function for each test.
pub fn configure_testing_logger() {
    testing_logger::setup();
}

pub fn check_logger_errors(num_errors: usize) {
    testing_logger::validate(|captured_logs| {
        let error_logs: Vec<_> = captured_logs
            .iter()
            .filter(|log_entry| log_entry.level == Level::Error)
            .collect();
        if error_logs.len() > num_errors {
            println!(
                "Error(s) in logs: saw {}, expected {num_errors}.",
                error_logs.len()
            );
            assert!(false);
        }
    });
}
