//! This file content corresponds to `logical_path_mod` in lib.rs due to `#[path]`.

pub fn item_in_actual_file() -> bool {
    true
}

mod inner_mod_in_actual_file {
    pub fn inner_func() {}
}
