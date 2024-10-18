// Copyright (C) 2023 Bryan A. Jones.
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
/// # `lib.rs` -- Define library modules for the CodeChat Editor Server
///
/// TODO: Add the ability to use
/// [plugins](https://zicklag.github.io/rust-tutorials/rust-plugins.html).
pub mod lexer;
pub mod processing;
pub mod webserver;

#[cfg(test)]
pub mod test_utils;
#[cfg(test)]
pub mod testing_logger;
