//! Minimal eval-internal rewrite slice.
//!
//! This slice covers only the intake/freeze/registration boundary:
//! `RunIntent -> FrozenRunSpec -> RunRegistration`.

pub mod core;
pub mod registry;

pub use core::{FrozenRunSpec, RunIntent, RunStorageRoots};
pub use registry::{RunRegistration, RunRegistrationError};
