//! Fixture for testing edge cases of name shadowing in Rust
//!
//! Covers scenarios where identical names appear in different scopes,
//! testing the parser's ability to correctly track visibility and resolve symbols.
//! Each test case is documented with the Rust Reference section it validates.

/// Case 1: Basic module hierarchy shadowing
/// Tests §3.3 (Name Resolution) - Simple shadowing in nested modules
mod basic_hierarchy {
    pub struct Shadowed;
    pub fn shadowed_fn() -> &'static str { "root" }
    
    pub mod nested {
        pub struct Shadowed;  // Shadows parent's struct
        pub fn shadowed_fn() -> &'static str { "nested" }
        
        pub mod inner {
            pub fn shadowed_fn() -> &'static str { "inner" }
        }
    }
}

/// Case 2: Re-export shadowing
/// Tests §3.3.1 (Use Declarations) - Re-exports creating shadowing
mod reexport_shadow {
    pub mod inner {
        pub fn shadowed() -> &'static str { "inner" }
    }
    pub fn shadowed() -> &'static str { "outer" }
    pub use inner::shadowed;  // Creates shadow of outer function
}

/// Case 3: pub(in path) restricted visibility
/// Tests RFC #3052 (scoped visibility) - Shadowing through restricted visibility
mod restricted_visibility {
    pub(in crate::restricted_visibility) struct Shadowed;
    pub struct Shadowed;  // Allowed since previous is scope-restricted
    
    pub mod nested {
        pub(in crate::restricted_visibility) fn shadowed_fn() -> &'static str { "restricted" }
        pub fn shadowed_fn() -> &'static str { "unrestricted" }
    }
}
