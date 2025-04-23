//! Fixture crate designed to test complex edge cases for `shortest_public_path` (SPP).
//! All code herein should be valid, stable Rust that compiles without errors or warnings.
//!
//! # Tested Scenarios:
//!
//! 1.  **Multi-Step Re-export Chains:**
//!     - Chains of varying lengths (e.g., 2-step, 3-step).
//!     - Chains involving renaming (`as`).
//!     - Chains where intermediate links have different visibilities (e.g., `pub` -> `pub(crate)` -> `pub`).
//!     - Multiple valid public paths to the same item via different re-export chains, testing shortest path selection.
//!
//! 2.  **Inline Module with `#[path]` Attribute:**
//!     - An inline module (`mod name { ... }`) annotated with `#[path]`.
//!     - Item shadowing: Items defined *inside* the inline module block should shadow items with the same name in the file specified by `#[path]`. SPP should resolve to the inline item.
//!     - Items *only* present in the `#[path]` file should be accessible via the inline module's logical path.
//!
//! 3.  **One File → Multiple Logical Modules:**
//!     - Multiple `mod` declarations with different names but the same `#[path]` attribute pointing to a single file.
//!     - Testing SPP for items within the shared file, ensuring it can find *a* valid public path if one exists via either logical module.
//!
//! 4.  **Glob Re-exports (`pub use ...::*;`):**
//!     - Re-exporting all public items from another module using `*`.
//!     - Testing SPP for an item brought into scope via the glob re-export. The expected path should be relative to the re-exporting module.
//!
//! 5.  **Re-exporting Items with Restricted Visibility:**
//!     - `pub use` of an item originally defined as `pub(crate)`, `pub(super)`, or `pub(in path)`.
//!     - SPP should correctly return `Err(ItemNotPubliclyAccessible)` as the re-export does not make the item publicly accessible outside the crate.
//!
//! 6.  **Shadowing via Re-exports vs. Local Definitions:**
//!     - A module contains both a local definition (`pub fn item()`) and a re-export of an item with the same name (`pub use other::item;`).
//!     - SPP should resolve to the local definition's path (`Ok(["crate", "module"])`), respecting Rust's shadowing rules.
//!
//! 7.  **Relative Re-exports:**
//!     - Using `pub use self::...` and `pub use super::...`.
//!     - Ensuring SPP correctly constructs paths involving these relative segments.

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
