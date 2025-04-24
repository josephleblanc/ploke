//! Fixture crate designed to test complex edge cases for `shortest_public_path` (SPP).
//! All code herein is valid, stable Rust that passes `cargo check`.
//!
//! NOTE: This test fixture is a copy of `fixture_spp_edge_cases` with all cfg elements commented
//! out. The following list is modified to account for the commenting out of all cfg flags.
//!
//! # Implemented Test Scenarios & Targets:
//!
//! 1.  **Multi-Step Re-export Chains:**
//!     - Target: `item_a` (original), `item_c` (3-step re-export), `item_alt_d` (4-step re-export).
//!     - Tests: Chain traversal, renaming, shortest path selection.
//!
//! 2.  **Inline Module with `#[path]` Attribute:**
//!     - Target Module: `inline_path_mod` (inline, uses `#[path]`).
//!     - Target Items: `shadow_me` (defined inline, shadows file), `item_only_in_inline` (inline only), `item_only_in_inline_target` (file only).
//!     - Tests: SPP resolution to inline definition, access to items from target file via inline module path.
//!
//! 3.  **One File → Multiple Logical Modules:**
//!     - Target File: `shared_target.rs` (contains `item_in_shared_target`, `crate_item_in_shared_target`).
//!     - Logical Modules: `logical_mod_1` (pub), `logical_mod_2` (private), both using `#[path = "shared_target.rs"]`.
//!     - Tests: SPP finding public item via the public logical module, SPP failing for crate-visible item.
//!
//! 4.  **Glob Re-exports (`pub use ...::*;`):**
//!     - Source Module: `glob_target` (re-exported via `pub use glob_target::*;`).
//!     - Target Items:
//!         - `glob_public_item` (should be accessible at crate root).
//!         - `glob_crate_item` (should NOT be accessible at crate root).
//!         - `glob_sub_path::item_in_glob_sub_path` (item in `#[path]` mod, should be accessible via `crate::glob_sub_path::...`).
//!         (commented out) - `glob_item_cfg_a` / `glob_item_cfg_not_a` (items under `cfg`, accessible at crate root if `cfg` matches).
//!         - `pub_sub_with_restricted::public_item_here` (item in public sub-mod, accessible via `crate::pub_sub_with_restricted::...`).
//!         - `pub_sub_with_restricted::super_visible_item` (`pub(super)`, should NOT be accessible at crate root).
//!     - Tests: Correct propagation of items and their visibility through glob re-exports.
//!
//! 5.  **Re-exporting Items with Restricted Visibility:**
//!     - Target Items: `restricted_vis::crate_func`, `restricted_vis::super_func`, `restricted_vis::inner::in_path_func`.
//!     - Note: The invalid `pub use` of these at the root was removed to allow compilation.
//!     - Tests: SPP targeting the *original definitions* should return `Err(ItemNotPubliclyAccessible)`.
//!
//! 6.  **Shadowing via Re-exports vs. Local Definitions:**
//!     - Target Item: `shadowing::shadowed_item`.
//!     - Note: The original scenario involving a `pub use` conflicting with a local `pub fn` is invalid Rust. The fixture now only contains the local definition.
//!     - Tests: SPP correctly resolves to the local definition: `Ok(["crate", "shadowing"])`.
//!
//! 7.  **Relative Re-exports:**
//!     - Target Items: `relative::inner::reexport_super` (`pub use super::...`), `relative::reexport_self` (`pub use self::...`).
//!     - Tests: Correct path construction involving `super` and `self`.
//!
//! 8.  **Deep Re-export Chains:**
//!     - Target Item: `final_deep_item` (re-export of `deep_item` through 11 steps).
//!     - Tests: SPP handling of long chains (potential depth limits).
//!
//! 9.  **Branching/Converging Re-exports:**
//!     - Target Item: `branch_item` (original), `item_via_a` (re-export), `item_via_b` (re-export).
//!     - Tests: SPP selecting one of the shortest public paths (`item_via_a` or `item_via_b`).
//!
//! 10. **Multiple Renames in Chain:**
//!     - Target Item: `final_renamed_item` (re-export of `multi_rename_item` with multiple `as` clauses).
//!     - Tests: SPP correctly handling multiple renames.
//!
//! 11. **Nested `#[path]` Attributes:**
//!     - Target Modules: `nested_path_1`, `nested_path_1::nested_target_2`.
//!     - Target Items: `item_in_nested_target_1`, `item_in_nested_target_2`.
//!     - Tests: Correct SPP resolution through nested `#[path]` declarations.
//!
//! 12. (commented out) **Mutually Exclusive `cfg` Attributes on Modules:**
//!     - Target Modules: `cfg_mod` (two variants based on `feature = "cfg_a"`).
//!     - Target Items: `item_in_cfg_a`, `item_in_cfg_not_a`.
//!     - Tests: SPP finding the path `crate::cfg_mod` regardless of which `cfg` variant is conceptually active (reflecting Phase 2 graph).
//!
//! 13. (commented out) **Nested Mutually Exclusive `cfg` Attributes:**
//!     - Target Modules: `cfg_mod::nested_cfg` (variants based on `cfg_a`/`cfg_b` vs `not cfg_a`/`cfg_c`).
//!     - Target Items: `item_in_cfg_ab`, `item_in_cfg_nac`.
//!     - Tests: SPP finding the path `crate::cfg_mod::nested_cfg` based on syntactic visibility.
//!
//! 14. (commented out) **Conflicting Parent/Child `cfg` Attributes:**
//!     - Target Module: `conflict_parent::conflict_child`.
//!     - Target Item: `impossible_item`.
//!     - Tests: SPP finding the syntactic path `crate::conflict_parent::conflict_child` even though it's impossible at compile time.

// --- Scenario 1: Multi-Step Re-export Chains ---
mod chain_a {
    pub fn item_a() -> u8 {
        1
    }
    pub(crate) fn crate_item_a() -> u8 {
        11
    }
}
mod chain_b {
    pub use crate::chain_a::item_a as item_b; // 2-step pub
                                              // REMOVED INVALID RE-EXPORT: pub use crate::chain_a::crate_item_a;
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
    pub fn shadow_me() -> u8 {
        2
    }

    // This item only exists here
    pub fn item_only_in_inline() -> u8 {
        21
    }

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
#[allow(dead_code)] // Allow unused items if only testing glob mechanism
mod glob_target; // Declare the module directory
#[allow(unused_imports)] // Allow glob import if items aren't used directly
pub use glob_target::*; // Glob re-export at the root

// Expected via glob: glob_public_item, glob_sub_path, glob_item_cfg_a / glob_item_cfg_not_a, pub_sub_with_restricted
// Expected access within pub_sub_with_restricted: public_item_here
// NOT expected via glob: glob_crate_item, private_sub, pub_sub_with_restricted::super_visible_item

// --- Scenario 5: Re-exporting Items with Restricted Visibility ---
mod restricted_vis {
    pub(crate) fn crate_func() -> u8 {
        50
    }
    pub(super) fn super_func() -> u8 {
        51
    } // super is crate here
    mod inner {
        pub(in crate::restricted_vis) fn in_path_func() -> u8 {
            52
        }
    }
    // REMOVED INVALID RE-EXPORT: pub use inner::in_path_func;
}
// REMOVED INVALID RE-EXPORTS at root:
// pub use restricted_vis::crate_func;
// pub use restricted_vis::super_func;
// pub use restricted_vis::in_path_func;
// SPP for the original definitions should return Err

// --- Scenario 6: Shadowing via Re-exports vs. Local Definitions ---
mod shadowing {
    #[allow(dead_code)] // Allow if other module isn't used directly
    pub mod other {
        pub fn shadowed_item() -> u8 {
            60
        }
    }
    // REMOVED INVALID RE-EXPORT causing name collision: pub use other::shadowed_item;

    // Local definition is now the only public one
    pub fn shadowed_item() -> u8 {
        61
    }
}
// SPP for shadowing::shadowed_item should resolve to the local one: Ok(["crate", "shadowing"])

// --- Scenario 7: Relative Re-exports ---
#[allow(unused_imports)] // Allow re-exports if not used directly in lib.rs
mod relative {
    pub fn item_in_relative() -> u8 {
        70
    }
    #[allow(unused_imports)] // Allow re-export if not used directly
    pub mod inner {
        pub fn item_in_inner() -> u8 {
            71
        }
        pub use super::item_in_relative as reexport_super; // pub use super::
    }
    pub use self::inner::item_in_inner as reexport_self; // pub use self::
}
// SPP for reexport_super should be Ok(["crate", "relative", "inner"])
// SPP for reexport_self should be Ok(["crate", "relative"])

// --- Scenario 8: Deep Re-export Chains ---
// (Illustrative - actual implementation might be tedious)
mod deep1 {
    pub fn deep_item() -> u8 {
        80
    }
}
mod deep2 {
    pub use crate::deep1::deep_item as item2;
}
mod deep3 {
    pub use crate::deep2::item2 as item3;
}
mod deep4 {
    pub use crate::deep3::item3 as item4;
}
mod deep5 {
    pub use crate::deep4::item4 as item5;
}
mod deep6 {
    pub use crate::deep5::item5 as item6;
}
mod deep7 {
    pub use crate::deep6::item6 as item7;
}
mod deep8 {
    pub use crate::deep7::item7 as item8;
}
mod deep9 {
    pub use crate::deep8::item8 as item9;
}
mod deep10 {
    pub use crate::deep9::item9 as item10;
}
mod deep11 {
    pub use crate::deep10::item10 as item11;
}

pub use deep11::item11 as final_deep_item; // 11 steps
                                           // SPP should find Ok(["crate"])

// --- Scenario 9: Branching/Converging Re-exports ---
mod branch_source {
    pub fn branch_item() -> u8 {
        90
    }
}
mod branch_a {
    pub use crate::branch_source::branch_item;
}
mod branch_b {
    pub use crate::branch_source::branch_item;
}
#[allow(dead_code)] // Allow unused module
mod private_intermediate {
    // This path is not public
    #[allow(unused_imports)] // Allow re-export if not used directly
    pub use crate::branch_a::branch_item;
}
#[allow(unused_imports)] // Allow unused module
mod branch_c {
    // Re-export from private - this doesn't make branch_item public via this path
    pub use crate::private_intermediate::branch_item as item_c;
}

// Public paths:
// SPP should find Ok(["crate"]) via either item_via_a or item_via_b
pub use branch_a::branch_item as item_via_a; // Path length 2
pub use branch_b::branch_item as item_via_b; // Path length 2

// --- Scenario 10: Multiple Renames in Chain ---
mod rename_source {
    pub fn multi_rename_item() -> u8 {
        100
    }
}
mod rename_step1 {
    pub use crate::rename_source::multi_rename_item as renamed1;
}
mod rename_step2 {
    pub use crate::rename_step1::renamed1 as renamed2;
}
pub use rename_step2::renamed2 as final_renamed_item;
// SPP should find Ok(["crate"])

// --- Scenario 11: Nested `#[path]` Attributes ---
#[path = "nested_path_target_1.rs"]
pub mod nested_path_1;
// SPP for item_in_nested_target_1 should be Ok(["crate", "nested_path_1"])
// SPP for item_in_nested_target_2 should be Ok(["crate", "nested_path_1", "nested_target_2"])

// --- Scenario 12 & 13: Mutually Exclusive `cfg` Attributes ---
// #[cfg(feature = "cfg_a")]
// pub mod cfg_mod {
//     pub fn item_in_cfg_a() -> u8 {
//         120
//     }
//     #[cfg(feature = "cfg_b")]
//     pub mod nested_cfg {
//         pub fn item_in_cfg_ab() -> u8 {
//             130
//         }
//     }
// }

// #[cfg(not(feature = "cfg_a"))]
// pub mod cfg_mod {
//     // Same name, different NodeId due to cfg
//     pub fn item_in_cfg_not_a() -> u8 {
//         121
//     }
//     #[cfg(feature = "cfg_c")]
//     pub mod nested_cfg {
//         // Same name, different NodeId
//         pub fn item_in_cfg_nac() -> u8 {
//             131
//         }
//     }
// }
// SPP for item_in_cfg_a should be Ok(["crate", "cfg_mod"]) (if cfg_a active)
// SPP for item_in_cfg_not_a should be Ok(["crate", "cfg_mod"]) (if cfg_a inactive)
// SPP for item_in_cfg_ab should be Ok(["crate", "cfg_mod", "nested_cfg"]) (if cfg_a & cfg_b active)
// SPP for item_in_cfg_nac should be Ok(["crate", "cfg_mod", "nested_cfg"]) (if not cfg_a & cfg_c active)

// --- Scenario 14: Conflicting Parent/Child `cfg` Attributes ---
// #[cfg(feature = "cfg_conflict")]
// pub mod conflict_parent {
//     #[cfg(not(feature = "cfg_conflict"))]
//     pub mod conflict_child {
//         // This item can never be compiled
//         pub fn impossible_item() -> u8 {
//             140
//         }
//     }
// }
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
