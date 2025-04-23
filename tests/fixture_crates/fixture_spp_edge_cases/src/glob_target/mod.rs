//! Target module for glob re-export tests.

// Simple public item
pub fn glob_public_item() -> u8 { 40 }

// Crate-visible item (should NOT be re-exported by glob)
pub(crate) fn glob_crate_item() -> u8 { 41 }

// Item in a submodule accessed via #[path]
#[path = "sub_path.rs"]
pub mod glob_sub_path;

// Items under mutually exclusive cfgs
#[cfg(feature = "glob_feat_a")]
pub fn glob_item_cfg_a() -> u8 { 42 }

#[cfg(not(feature = "glob_feat_a"))]
pub fn glob_item_cfg_not_a() -> u8 { 43 }

// Private module (should not be re-exported)
mod private_sub {
    #[allow(dead_code)]
    pub fn item_in_private() -> u8 { 44 }
}

// Public module containing a restricted item
pub mod pub_sub_with_restricted {
    // This is pub(super), so only visible within glob_target
    // It should NOT be accessible via a glob re-export from lib.rs
    pub(super) fn super_visible_item() -> u8 { 45 }

    // This is public within this module
    pub fn public_item_here() -> u8 { 46 }
}
