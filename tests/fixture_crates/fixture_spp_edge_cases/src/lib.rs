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

// --- Scenario 1: Multi-Step Re-export Chains ---
mod chain_a {
    pub fn item_a() -> u8 { 1 }
    pub(crate) fn crate_item_a() -> u8 { 11 }
}
mod chain_b {
    pub use crate::chain_a::item_a as item_b; // 2-step pub
    pub use crate::chain_a::crate_item_a; // 2-step pub(crate) -> pub use (still crate vis)
}
mod chain_c {
    pub use crate::chain_b::item_b as item_c; // 3-step pub
}
// Re-export at root
pub use chain_c::item_c; // Final public name for item_a
                         // crate_item_a is NOT publicly re-exported

// Alternative path (longer)
mod chain_alt_b {
    pub use crate::chain_a::item_a;
}
mod chain_alt_c {
    pub use crate::chain_alt_b::item_a;
}
mod chain_alt_d {
    pub use crate::chain_alt_c::item_a as item_alt_d;
}
// Re-export at root (longer path)
pub use chain_alt_d::item_alt_d; // SPP should prefer item_c path

// --- Scenario 2: Inline Module with `#[path]` Attribute ---
#[path = "inline_path_target.rs"]
pub mod inline_path_mod {
    // This shadows the function in inline_path_target.rs
    pub fn shadow_me() -> u8 { 2 }

    // This item only exists here
    pub fn item_only_in_inline() -> u8 { 21 }

    // We could try re-exporting from the target file, but SPP needs to handle it
    // pub use super::item_only_in_inline_target; // Needs SPP enhancement
}

// --- Scenario 3: One File -> Multiple Logical Modules ---
#[path = "shared_target.rs"]
pub mod logical_mod_1;

#[path = "shared_target.rs"]
mod logical_mod_2; // Private logical module pointing to the same file

// SPP for item_in_shared_target should resolve via logical_mod_1
// SPP for crate_item_in_shared_target should fail (crate visible)

// --- Scenario 4: Glob Re-exports ---
mod glob_target; // Declare the module directory
pub use glob_target::*; // Glob re-export at the root

// Expected via glob: glob_public_item, glob_sub_path, glob_item_cfg_a / glob_item_cfg_not_a, pub_sub_with_restricted
// Expected access within pub_sub_with_restricted: public_item_here
// NOT expected via glob: glob_crate_item, private_sub, pub_sub_with_restricted::super_visible_item

// --- Scenario 5: Re-exporting Items with Restricted Visibility ---
mod restricted_vis {
    pub(crate) fn crate_func() -> u8 { 50 }
    pub(super) fn super_func() -> u8 { 51 } // super is crate here
    mod inner {
        pub(in crate::restricted_vis) fn in_path_func() -> u8 { 52 }
    }
    pub use inner::in_path_func; // Re-export pub(in path)
}
// Re-exporting them with pub use DOES NOT make them public outside crate
pub use restricted_vis::crate_func;
pub use restricted_vis::super_func;
pub use restricted_vis::in_path_func; // Already re-exported in restricted_vis
                                      // SPP for all these should return Err

// --- Scenario 6: Shadowing via Re-exports vs. Local Definitions ---
mod shadowing {
    pub mod other {
        pub fn shadowed_item() -> u8 { 60 }
    }
    pub use other::shadowed_item; // Re-export

    // Local definition shadows the re-export
    pub fn shadowed_item() -> u8 { 61 }
}
// SPP for shadowing::shadowed_item should resolve to the local one: Ok(["crate", "shadowing"])

// --- Scenario 7: Relative Re-exports ---
mod relative {
    pub fn item_in_relative() -> u8 { 70 }
    pub mod inner {
        pub fn item_in_inner() -> u8 { 71 }
        pub use super::item_in_relative as reexport_super; // pub use super::
    }
    pub use self::inner::item_in_inner as reexport_self; // pub use self::
}
// SPP for reexport_super should be Ok(["crate", "relative", "inner"])
// SPP for reexport_self should be Ok(["crate", "relative"])

// --- Scenario 8: Deep Re-export Chains ---
// (Illustrative - actual implementation might be tedious)
mod deep1 { pub fn deep_item() -> u8 { 80 } }
mod deep2 { pub use crate::deep1::deep_item as item2; }
mod deep3 { pub use crate::deep2::item2 as item3; }
mod deep4 { pub use crate::deep3::item3 as item4; }
mod deep5 { pub use crate::deep4::item4 as item5; }
mod deep6 { pub use crate::deep5::item5 as item6; }
mod deep7 { pub use crate::deep6::item6 as item7; }
mod deep8 { pub use crate::deep7::item7 as item8; }
mod deep9 { pub use crate::deep8::item8 as item9; }
mod deep10 { pub use crate::deep9::item9 as item10; }
mod deep11 { pub use crate::deep10::item10 as item11; }
pub use deep11::item11 as final_deep_item; // 11 steps
                                           // SPP should find Ok(["crate"])

// --- Scenario 9: Branching/Converging Re-exports ---
mod branch_source { pub fn branch_item() -> u8 { 90 } }
mod branch_a { pub use crate::branch_source::branch_item; }
mod branch_b { pub use crate::branch_source::branch_item; }
mod private_intermediate {
    // This path is not public
    pub use crate::branch_a::branch_item;
}
mod branch_c {
    // Re-export from private - this doesn't make branch_item public via this path
    // pub use crate::private_intermediate::branch_item as item_c;
}
// Public paths:
pub use branch_a::branch_item as item_via_a; // Path length 2
pub use branch_b::branch_item as item_via_b; // Path length 2
                                             // SPP should find Ok(["crate"]) via either item_via_a or item_via_b

// --- Scenario 10: Multiple Renames in Chain ---
mod rename_source { pub fn multi_rename_item() -> u8 { 100 } }
mod rename_step1 { pub use crate::rename_source::multi_rename_item as renamed1; }
mod rename_step2 { pub use crate::rename_step1::renamed1 as renamed2; }
pub use rename_step2::renamed2 as final_renamed_item;
// SPP should find Ok(["crate"])

// --- Scenario 11: Nested `#[path]` Attributes ---
#[path = "nested_path_target_1.rs"]
pub mod nested_path_1;
// SPP for item_in_nested_target_1 should be Ok(["crate", "nested_path_1"])
// SPP for item_in_nested_target_2 should be Ok(["crate", "nested_path_1", "nested_target_2"])

// --- Scenario 12 & 13: Mutually Exclusive `cfg` Attributes ---
#[cfg(feature = "cfg_a")]
pub mod cfg_mod {
    pub fn item_in_cfg_a() -> u8 { 120 }
    #[cfg(feature = "cfg_b")]
    pub mod nested_cfg {
        pub fn item_in_cfg_ab() -> u8 { 130 }
    }
}

#[cfg(not(feature = "cfg_a"))]
pub mod cfg_mod { // Same name, different NodeId due to cfg
    pub fn item_in_cfg_not_a() -> u8 { 121 }
    #[cfg(feature = "cfg_c")]
    pub mod nested_cfg { // Same name, different NodeId
        pub fn item_in_cfg_nac() -> u8 { 131 }
    }
}
// SPP for item_in_cfg_a should be Ok(["crate", "cfg_mod"]) (if cfg_a active)
// SPP for item_in_cfg_not_a should be Ok(["crate", "cfg_mod"]) (if cfg_a inactive)
// SPP for item_in_cfg_ab should be Ok(["crate", "cfg_mod", "nested_cfg"]) (if cfg_a & cfg_b active)
// SPP for item_in_cfg_nac should be Ok(["crate", "cfg_mod", "nested_cfg"]) (if not cfg_a & cfg_c active)

// --- Scenario 14: Conflicting Parent/Child `cfg` Attributes ---
#[cfg(feature = "cfg_conflict")]
pub mod conflict_parent {
    #[cfg(not(feature = "cfg_conflict"))]
    pub mod conflict_child {
        // This item can never be compiled
        pub fn impossible_item() -> u8 { 140 }
    }
}
// SPP for impossible_item should find Ok(["crate", "conflict_parent", "conflict_child"])
// based on syntactic visibility, even though it's impossible.

// --- Original add function ---
pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

// --- Keep tests module ---
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
