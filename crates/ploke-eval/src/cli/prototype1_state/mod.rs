//! Typed prototype1 configuration states.
//!
//! These states are not wired into the live controller yet. They exist so the
//! parent/child seam can be modeled explicitly as move-only configuration
//! transitions before the runtime path is rewritten to use them.

pub(crate) mod c1;
pub(crate) mod c2;
pub(crate) mod c3;
pub(crate) mod event;
pub(crate) mod journal;
