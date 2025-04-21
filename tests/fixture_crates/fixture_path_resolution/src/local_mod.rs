//! A standard file-based module.

// Nested module defined in its own file
pub mod nested;

pub fn local_func() -> u8 { 0 }

#[allow(dead_code)] // Allow dead code for fixture clarity
fn private_local_func() {}

// Use item from dependency within this module
use log::warn;

pub fn func_using_dep() {
    warn!("Called func_using_dep in local_mod");
}
