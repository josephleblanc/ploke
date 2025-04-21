//! This file content corresponds to `logical_path_mod` in lib.rs due to `#[path]`.

pub fn item_in_actual_file() -> bool {
    true
}

mod inner_mod_in_actual_file {
    #[allow(dead_code)] // Allow dead code for fixture clarity
    pub fn inner_func() {}
}

// Edge Case: pub(crate) item within a file targeted by #[path]
// Visibility is determined relative to the crate where the #[path]
// attribute *resides*, not the file's physical location.
// So, this function is visible throughout the `fixture_path_resolution` crate.
#[allow(dead_code)]
pub(crate) fn crate_visible_in_actual_file() -> bool {
    true
}
