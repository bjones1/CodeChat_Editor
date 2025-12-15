/// Copyright (C) 2025 Bryan A. Jones.
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
/// `test_macros.rs` -- Reusable macros for testing
/// ===============================================
///
/// Placing this file in the `tests/` directory prevents me from importing it
/// outside that directory tree; the desire was to import this for unit tests in
/// the `src/` directory tree. So, it's instead placed here, then conditionally
/// imported in `lib.rs`.
// Imports
// -------
// None.
//
// Macros
// ------
//
// Extract a known enum variant or fail. More concise than the alternative (`if
// let`, or `let else`). From [SO](https://stackoverflow.com/a/69324393). The
// macro does not handle nested patterns like `Some(Animal(cat))`.
#[macro_export]
macro_rules! cast {
    // For an enum containing a single value (the typical case).
    ($target: expr, $pat: path) => {{
        // The `if let` exploits recent Rust compiler's smart pattern matching.
        // Contrary to other solutions like `into_variant` and friends, this
        // one macro covers all ownership usage like `self`, `&self` and `&mut
        // self`. On the other hand `{into,as,as_mut}_{variant}` solution
        // usually needs 3 \* N method definitions where N is the number of
        // variants.
        if let $pat(a) = $target {
            a
        } else {
            // If the variant and value mismatch, the macro will simply panic
            // and report the expected pattern.
            panic!("mismatch variant when cast to {}", stringify!($pat));
        }
    }};
    // For an enum containing multiple values, return a tuple. I can't figure
    // out how to automatically do this; for now, the caller must provide the
    // correct number of tuple parameters.
    ($target: expr, $pat: path, $( $tup: ident),*) => {{
        if let $pat($($tup,)*) = $target {
            ($($tup,)*)
        } else {
            panic!("mismatch variant when cast to {}", stringify!($pat));
        }
    }};
}

// Get the name (and module path) to the current function. From
// [SO](https://stackoverflow.com/a/40234666).
#[macro_export]
macro_rules! function_name {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        // Since we just called the nested function f, strip this off the end of
        // the returned value.
        name.strip_suffix("::f").unwrap()
    }};
}

// Call `_prep_test_dir` with the correct parameter -- `function_name!()`.
#[macro_export]
macro_rules! prep_test_dir {
    () => {{
        use $crate::function_name;
        use $crate::test_utils::_prep_test_dir;
        _prep_test_dir(function_name!())
    }};
}
