// `not_dup` is defined via #[path] pointing at this directory's dup.rs; the same file is also
// discovered as a scan root for a sibling file module named `dup`, while `mod dup {}` is an
// inline definition at the same path (ADR-025 stem collision + path reindex).
#[path = "dup.rs"]
pub mod not_dup;

pub mod dup {
    pub fn only_in_inline_dup() -> u32 {
        1
    }
}
