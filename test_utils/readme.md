`readme.md` - Overview of test utilities
================================================================================

These test utilities are used both in unit tests and in integration tests.
Integration tests can't access code inside a `#[cfg(test)]` configuration
predicate that's in the library being compiled; unit tests can't access code in
the `test/` subdirectory. Therefore, these utilities are placed in a separate
library (this one) then included in the `dev-dependencies` to solve both these
problems.
