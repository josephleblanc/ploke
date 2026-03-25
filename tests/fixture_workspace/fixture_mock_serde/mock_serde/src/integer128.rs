//! 128-bit integer support macro
//!
//! This module provides a macro similar to serde's serde_if_integer128.
//! In real serde, this was used for backward compatibility but is now
/// deprecated as all supported compilers have 128-bit integer support.

#[macro_export]
#[deprecated = "
This macro has no effect on modern Rust versions.
128-bit integers are always supported.
"]
#[doc(hidden)]
macro_rules! mock_serde_if_integer128 {
    ($($tt:tt)*) => {
        $($tt)*
    };
}
