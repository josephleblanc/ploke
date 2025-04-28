//! This file is targeted by multiple `#[path]` attributes in lib.rs
//! to test the "One File -> Multiple Logical Modules" scenario.

pub fn item_in_shared_target() -> u8 {
    3
}

pub(crate) fn crate_item_in_shared_target() -> u8 {
    33
}
