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
