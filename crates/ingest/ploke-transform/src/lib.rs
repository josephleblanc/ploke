#![allow(
    dead_code,
    reason = "Under rapid development and refactoring, warnings annoying"
)]
pub mod error;
pub mod macro_traits;
pub mod schema;
pub mod transform;
pub mod tests;

// -- crate-wide imports --
pub mod utils;
// #[cfg(test)]
// pub(crate) use utils::test_utils;
