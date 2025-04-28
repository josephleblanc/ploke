//! This file exists outside the `fixture_path_resolution/src` directory
//! but within the workspace's `tests/fixture_crates` area.
//! It's used to test `#[path]` pointing outside the crate source.

pub fn function_in_common_file() -> &'static str {
    "Hello from common_file.rs!"
}
