//! Private module for internal implementation details
//!
//! This module contains items that are not part of the public API
//! but may be used by generated code or doc tests.

#[doc(hidden)]
pub use mock_serde_core::private::__private as serde_core_private;

/// Private marker trait for internal use
#[doc(hidden)]
pub trait PrivateMarker {
    /// Internal method
    #[doc(hidden)]
    fn __private(&self);
}
