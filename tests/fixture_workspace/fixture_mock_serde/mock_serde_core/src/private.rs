//! Private implementation details for mock_serde_core
//!
//! This module contains items that are not part of the public API.

/// Private re-export for use by generated code
#[doc(hidden)]
pub mod __private {
    /// Private marker type
    #[doc(hidden)]
    pub struct Private;

    /// Private helper function
    #[doc(hidden)]
    pub fn private_helper() -> Private {
        Private
    }
}

// Re-export at crate root
#[doc(hidden)]
pub use __private::*;

/// Internal marker trait
#[doc(hidden)]
pub trait Sealed {}

/// Implement Sealed for primitive types
impl Sealed for () {}
impl Sealed for bool {}
impl Sealed for i8 {}
impl Sealed for i16 {}
impl Sealed for i32 {}
impl Sealed for i64 {}
impl Sealed for i128 {}
impl Sealed for u8 {}
impl Sealed for u16 {}
impl Sealed for u32 {}
impl Sealed for u64 {}
impl Sealed for u128 {}
impl Sealed for f32 {}
impl Sealed for f64 {}
