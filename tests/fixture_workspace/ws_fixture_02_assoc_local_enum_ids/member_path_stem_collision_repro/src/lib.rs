mod collision;

/// Ensures both the inline `dup` module and the `#[path]`-backed `not_dup` module are reachable.
#[doc(hidden)]
pub fn fixture_path_stem_smoke() -> (u32, u32) {
    (
        collision::dup::only_in_inline_dup(),
        collision::not_dup::only_in_not_dup(),
    )
}
