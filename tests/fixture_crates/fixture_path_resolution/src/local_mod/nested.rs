//! A nested module within local_mod.

pub fn deep_func() -> u16 { 100 }

#[allow(dead_code)] // Allow dead code for fixture clarity
fn private_deep_func() {}

// Re-export something from the parent
pub use super::local_func as parent_local_func_reexport;
