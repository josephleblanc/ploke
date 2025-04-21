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

// Item visible only to the parent module (crate root in this case)
#[allow(dead_code)]
pub(super) fn super_visible_func_in_local() -> bool {
    true
}

// Nested private module to test pub(super) from deeper level
mod deeper_private {
    #[allow(dead_code)]
    pub(super) fn super_visible_from_deeper() -> bool {
        // This is visible only within local_mod
        true
    }
}
