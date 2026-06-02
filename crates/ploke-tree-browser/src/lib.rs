//! Compatibility facade for browser playback models.
//!
//! The model and projection code lives in `ploke_tree::browser`. This crate
//! remains only for existing dependency edges while callers move to `ploke-tree`.

pub use ploke_tree::browser::*;
