// Invalid use statements that should be skipped

// Unterminated
use std::

// Malformed glob
use std::collections::*;

// Invalid characters
use $invalid::path;

// Macro-generated (test error handling)
macro_rules! make_use {
    () => { use generated::path; }
}
make_use!();
