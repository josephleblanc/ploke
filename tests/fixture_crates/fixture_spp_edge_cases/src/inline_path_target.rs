//! This file is targeted by the `#[path]` attribute on the inline module `inline_path_mod` in lib.rs.

// This should be shadowed by the definition inside the inline module block.
pub fn shadow_me() -> u8 {
    0
}

// This should be accessible via the inline module's path.
pub fn item_only_in_inline_target() -> u8 {
    20
}
