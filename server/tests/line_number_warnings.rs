// The CodeChat Editor, a word processor for software developers.<br>Copyright
// (C) 2024-2026 Bryan A. Jones.
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
#![allow(clippy::too_many_lines)]
//! `line_number_warnings.rs` - test the warnings produced by line number tags
//! ==========================================================================
//!
//! The spec's "Sanitization and dehydration" tests require that a hand-written
//! `l-n` tag logs a warning. The global logger can be set only once per
//! process, and the library's unit tests share one process with tests which
//! start the Server (and so install its logger). Therefore, this test lives in
//! its own test executable, which owns the logger. It's a single test, so
//! nothing else logs concurrently.
//!
//! ### Standard library
use std::{
    path::Path,
    sync::{Mutex, Once},
};

// ### Local
use code_chat_editor::processing::source_to_codechat_for_web;

// Capture warnings
// ----------------
static WARNINGS: Mutex<Vec<String>> = Mutex::new(Vec::new());

struct WarningCapture;

impl log::Log for WarningCapture {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Warn
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            WARNINGS.lock().unwrap().push(record.args().to_string());
        }
    }

    fn flush(&self) {}
}

// Translate a Markdown file, returning the warnings it logged.
fn warnings_for(markdown: &str) -> Vec<String> {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        log::set_logger(&WarningCapture).unwrap();
        log::set_max_level(log::LevelFilter::Warn);
    });
    WARNINGS.lock().unwrap().clear();
    source_to_codechat_for_web(markdown, Path::new("test.md"), 0.0, false, None).unwrap();
    std::mem::take(&mut *WARNINGS.lock().unwrap())
}

#[test]
fn test_line_number_tags_hand_written_warnings() {
    // Hand-written start and end tags warn, in inline HTML and in an HTML
    // block.
    for (markdown, count) in [
        ("a <l-n>5</l-n> b\n", 2),
        ("<l-n>\n", 1),
        ("<div>\n</l-n>\n</div>\n", 1),
    ] {
        let warnings = warnings_for(markdown);
        assert_eq!(warnings.len(), count, "{markdown:?}: {warnings:?}");
        for warning in &warnings {
            assert!(warning.contains("reserved"), "{warning}");
        }
    }
    // A code span doesn't, nor does a tag which merely begins with `l-n`.
    for markdown in ["`<l-n>`\n", "`</l-n>`\n", "<l-nx>a</l-nx>\n"] {
        assert_eq!(warnings_for(markdown), Vec::<String>::new(), "{markdown:?}");
    }
}
