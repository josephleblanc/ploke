//! A nested module within local_mod.

pub fn deep_func() -> u16 { 100 }

fn private_deep_func() {}

// Re-export something from the parent
pub use super::local_func as parent_local_func_reexport;
