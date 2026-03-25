//! # Mock Serde
//!
//! A mock implementation of the serde crate structure for testing workspace analysis.
//! This crate emulates the real serde's module organization and re-export patterns.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

// Use #[path] attributes to demonstrate non-standard module paths
// This pattern is used in the real serde crate for conditional compilation
#[macro_use]
#[path = "core/crate_root.rs"]
mod crate_root;

#[macro_use]
#[path = "core/macros.rs"]
mod macros;

// Invoke the crate_root macro to set up the module structure
crate_root!();

// Include the integer128 module normally
mod integer128;

// Re-export derive macros when the derive feature is enabled
#[cfg(feature = "mock_serde_derive")]
extern crate mock_serde_derive;

/// Derive macro available if mock_serde is built with `features = ["derive"]`.
#[cfg(feature = "mock_serde_derive")]
pub use mock_serde_derive::{Deserialize, Serialize};

/// Private module for internal use (demonstrates build script generation)
#[doc(hidden)]
mod private;

// Include the generated private module from build.rs
include!(concat!(env!("OUT_DIR"), "/private.rs"));

#[macro_export]
#[doc(hidden)]
macro_rules! __require_mock_serde_not_mock_serde_core {
    () => {};
}
