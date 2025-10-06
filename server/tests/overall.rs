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
/// `overall.rs` - test the overall system
/// ======================================
///
/// These are functional tests of the overall system, performed by attaching a
/// testing IDE to generate commands then observe results, along with a browser
/// tester.
///
/// Some subtulties of this approach: development dependencies aren't available
/// to integration tests. Therefore, this crate's `Cargo.toml` file includes the
/// `int_tests` feature, which enables crates needed only for integration
/// testing, while keeping these out of the final binary when compiling for
/// production. This means that the same crate appears both in
/// `dev-dependencies` and in `dependencies`, so it's available for both unit
/// tests and integration tests. In addition, any code used in integration tests
/// must be gated on the `int_tests` feature, since this code fails to compile
/// without that feature's crates enabled.
// Imports
// -------
#[cfg(feature = "int_tests")]
mod overall_core;
