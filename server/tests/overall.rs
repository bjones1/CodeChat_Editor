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
//
// This whole binary is test code, so relax the pedantic Clippy lints that
// `Cargo.toml`'s `[lints.clippy]` enables for production code.
#![allow(
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    clippy::float_cmp
)]
/// `overall.rs` - test the overall system
/// =======================================
///
/// This file combines all the `overall_*` test modules into a single test
/// executable, so that a single instance of the web driver server is shared by
/// all of them. See `overall/common/mod.rs` for the shared test harness.
///
/// To run these tests, execute `cargo test --test overall
/// <optional_test_name>` in the `server/` directory.
#[path = "overall/common/mod.rs"]
mod common;
#[path = "overall/overall_1.rs"]
mod overall_1;
#[path = "overall/overall_2.rs"]
mod overall_2;
#[path = "overall/overall_3.rs"]
mod overall_3;
#[path = "overall/overall_4.rs"]
mod overall_4;
#[path = "overall/overall_5.rs"]
mod overall_5;
