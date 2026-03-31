//! Shared file: `not_dup` loads this via `#[path]`; scan also sees `dup.rs` as a file root.
pub fn only_in_not_dup() -> u32 {
    2
}
