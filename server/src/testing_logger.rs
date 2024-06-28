//! This crate supports testing and asserting that appropriate log messages
//! from the `log` crate are generated during tests.
//!
//! Log events are captured in a thread_local variable so this module behaves correctly
//! when tests are run multithreaded.
//!
//! All log levels are captured, but none are sent to any logging system. The
//! test developer should use the `validate()` function in order to check
//! the captured log messages.
//!
//! # Examples
//! ```
//! #[macro_use]
//! extern crate log;
//! use log::Level;
//! extern crate testing_logger;
//!
//! # fn main() { test_something();}
//! # /* Don't put #[test] in code when running doc tests but DO put it in the documentation
//! #[test]
//! # */
//! fn test_something() {
//!     testing_logger::setup();
//!     warn!("Something went wrong with {}", 10);
//!     testing_logger::validate( |captured_logs| {
//!         assert_eq!(captured_logs.len(), 1);
//!         assert_eq!(captured_logs[0].body, "Something went wrong with 10");
//!         assert_eq!(captured_logs[0].level, Level::Warn);
//!     });
//! }
//! ```
//! The target is also captured if you want to validate that.
//! ```
//!
//! # #[macro_use]
//! # extern crate log;
//! # use log::Level;
//! # extern crate testing_logger;
//! # fn main() { test_target();}
//! # /* Don't put #[test] in code when running doc tests but DO put it in the documentation
//! #[test]
//! # */
//! fn test_target() {
//!     testing_logger::setup();
//!     log!(target: "documentation", Level::Trace, "targetted log message");
//!     testing_logger::validate( |captured_logs| {
//!         assert_eq!(captured_logs.len(), 1);
//!         assert_eq!(captured_logs[0].target, "documentation");
//!         assert_eq!(captured_logs[0].body, "targetted log message");
//!         assert_eq!(captured_logs[0].level, Level::Trace);
//!     });
//! }
//! ```

extern crate log;
use log::{Log, Record, Metadata, LevelFilter, Level};
use std::cell::RefCell;
use std::sync::{Once, ONCE_INIT};

/// A captured call to the logging system. A `Vec` of these is passed
/// to the closure supplied to the `validate()` function.
pub struct CapturedLog {
    /// The formatted log message.
    pub body: String,
    /// The level.
    pub level: Level,
    /// The target.
    pub target: String
}

thread_local!(static LOG_RECORDS: RefCell<Vec<CapturedLog>> = RefCell::new(Vec::with_capacity(3)));

struct TestingLogger {}

impl Log for TestingLogger {

    #[allow(unused_variables)]
    fn enabled(&self, metadata: &Metadata) -> bool {
        true // capture all log levels
    }

    fn log(& self, record: &Record) {
        LOG_RECORDS.with( |records| {
            let captured_record = CapturedLog {
                body: format!("{}",record.args()),
                level: record.level(),
                target: record.target().to_string()
            };
            records.borrow_mut().push(captured_record);
        });
    }

    fn flush(&self) {}

}

static FIRST_TEST: Once = ONCE_INIT;

static TEST_LOGGER: TestingLogger = TestingLogger{};

/// Prepare the `testing_logger` to capture log messages for a test.
///
/// Should be called from every test that calls `validate()`, before any calls to the logging system.
/// This function will install an internal `TestingLogger` as the logger if not already done so, and initialise
/// its thread local storage for a new test.
pub fn setup() {
    FIRST_TEST.call_once( || {
        log::set_logger(&TEST_LOGGER).map(|()|
        log::set_max_level(LevelFilter::Trace)).unwrap();
    });
    LOG_RECORDS.with( |records| {
        records.borrow_mut().truncate(0);
    });
}

/// Used to validate any captured log events.
///
/// the `asserter` closure can check the number, body, target and level
/// of captured log events. As a side effect, the records are cleared.
pub fn validate<F>(asserter: F)  where F: Fn(&Vec<CapturedLog>) {
    LOG_RECORDS.with( |records| {
        asserter(&records.borrow());
        records.borrow_mut().truncate(0);
    });
}
