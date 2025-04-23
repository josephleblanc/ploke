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
//!     - **Complex Targets:** Includes testing glob re-exports where the target module contains:
//!         - Items defined under mutually exclusive `cfg` attributes.
//!         - Items defined within a submodule accessed via `#[path]`.
//!         - Items with restricted visibility (`pub(crate)`, `pub(super)`, etc.) to ensure the glob doesn't incorrectly elevate their visibility for SPP.
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
//!
//! 8.  **Deep Re-export Chains:**
//!     - Chains significantly longer than typical usage (e.g., 10+ levels) to test depth limits and performance.
//!
//! 9.  **Branching/Converging Re-exports:**
//!     - An item made public via multiple different re-export paths.
//!     - Includes scenarios where some intermediate paths might go through private modules before becoming public again.
//!     - SPP must still find the absolute shortest *public* path.
//!
//! 10. **Multiple Renames in Chain:**
//!     - An item is re-exported multiple times along a chain, with a different `as` rename at each (or multiple) steps.
//!     - SPP needs to correctly identify the final visible name associated with the shortest path.
//!
//! 11. **Nested `#[path]` Attributes:**
//!     - A module declaration uses `#[path]` to include a file, and *that* file contains another module declaration using `#[path]`.
//!     - Testing correct resolution of paths relative to the file containing the declaration.
//!
//! 12. **Mutually Exclusive `cfg` Attributes on Modules:**
//!     - Two module definitions with the same name and path but mutually exclusive `cfg` attributes (e.g., `#[cfg(a)] mod foo;` and `#[cfg(not(a))] mod foo;`).
//!     - Although `NodeId`s should be distinct due to `cfg_bytes`, SPP should find the path (`crate::foo`) if *either* variant is public, reflecting Phase 2 graph structure.
//!
//! 13. **Nested Mutually Exclusive `cfg` Attributes:**
//!     - Similar to #12, but involving nested modules with different `cfg` combinations (e.g., `#[cfg(a)] mod foo { #[cfg(b)] mod bar; }` vs `#[cfg(not(a))] mod foo { #[cfg(c)] mod bar; }`).
//!     - SPP should find the nested path (`crate::foo::bar`) if the modules are syntactically public in *any* valid `cfg` branch combination seen by the parser.
//!
//! 14. **Conflicting Parent/Child `cfg` Attributes:**
//!     - A parent module has a `cfg` attribute, and a child module has a conflicting `cfg` (e.g., `#[cfg(a)] mod foo { #[cfg(not(a))] mod bar; }`).
//!     - SPP should still find the syntactic path (`crate::foo::bar`) if both modules are marked `pub`, even though this path is impossible at compile time. Tests the traversal logic independent of `cfg` evaluation.

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
