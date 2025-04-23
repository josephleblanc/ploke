//! Fixture crate designed to test potentially problematic or malformed edge cases
//! for `shortest_public_path` (SPP), focusing on scenarios that might cause
//! infinite loops or panics if not handled robustly.
//!
//! Note: Some code herein might be rejected by `rustc` or cause warnings,
//! but the goal is to test the parser/SPP logic's resilience.
//!
//! # Tested Scenarios:
//!
//! 1.  **Re-export Cycles:**
//!     - Direct cycle: `mod a { pub use crate::b::X; } mod b { pub use crate::a::X; }`
//!     - Indirect cycle: `mod a { pub use crate::b::X; } mod b { pub use crate::c::X; } mod c { pub use crate::a::X; }`
//!     - SPP should detect the cycle (e.g., via depth limit) and return an appropriate error
//!       (like `ModuleTreeError::ReExportChainTooLong`) rather than looping infinitely.
//!
//! 2.  **Self-Referential Re-exports (Potentially Invalid Rust):**
//!     - `pub use self::Item;` where `Item` is defined in the same module. (Likely valid, SPP should handle).
//!     - `pub use Item;` where `Item` is defined in the same module (if allowed by Rust).
//!     - Pathological cases attempting self-reference that might confuse path resolution.
//!
//! 3.  **Ambiguous Re-exports (Potentially Invalid Rust):**
//!     - Scenarios where multiple `pub use` statements might bring items with the same name into the same scope,
//!       potentially leading to ambiguity if not handled correctly by the parser or SPP.
//!       (e.g., `pub use mod_a::Item; pub use mod_b::Item;`) - Rust usually rejects this.
//!
//! 4.  **Deeply Nested Re-exports:**
//!     - Extremely long (but non-cyclic) re-export chains to test the depth limit implementation.

// --- Scenarios moved from valid fixture due to compile errors ---

// Scenario: Re-exporting Restricted Items (Causes E0364, E0603)
mod restricted_vis_malformed {
    pub(crate) fn crate_func() -> u8 { 50 }
    pub(super) fn super_func() -> u8 { 51 } // super is crate here
    mod inner {
        pub(in crate::restricted_vis_malformed) fn in_path_func() -> u8 { 52 }
    }
    // This re-export within the restricted scope might be valid itself,
    // but re-exporting *again* from the root causes issues.
    pub use inner::in_path_func;
}
// These cause E0364/E0603 because the items are not public relative to crate root
// pub use restricted_vis_malformed::crate_func;
// pub use restricted_vis_malformed::super_func;
// pub use restricted_vis_malformed::in_path_func;

// Scenario: Shadowing via Re-export vs Local Definition (Causes E0255)
mod shadowing_malformed {
    pub mod other {
        pub fn shadowed_item() -> u8 { 60 }
    }
    // This combination is invalid in Rust
    // pub use other::shadowed_item; // Re-export
    // pub fn shadowed_item() -> u8 { 61 } // Local definition
}


// --- Original Scenarios for Malformed Fixture ---

// Scenario 1: Re-export Cycles
mod cycle_a {
    // pub use crate::cycle_b::CycleItemB; // Uncomment to create direct cycle
    pub struct CycleItemA;
}
mod cycle_b {
    // pub use crate::cycle_a::CycleItemA; // Uncomment to create direct cycle
    pub struct CycleItemB;
}

// Scenario 2: Self-Referential Re-exports
mod self_ref {
    pub struct SelfItem;
    // pub use self::SelfItem; // This is likely valid but redundant
    // pub use SelfItem; // This might be invalid depending on context
}

// Scenario 3: Ambiguous Re-exports (Rust usually prevents this)
mod ambiguous_a { pub struct AmbiguousItem; }
mod ambiguous_b { pub struct AmbiguousItem; }
// pub use ambiguous_a::AmbiguousItem;
// pub use ambiguous_b::AmbiguousItem; // This causes E0252

// Scenario 4: Deeply Nested Re-exports (Valid, but tests limits)
// (Already present in valid fixture, no need to duplicate unless making it excessively deep)


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
