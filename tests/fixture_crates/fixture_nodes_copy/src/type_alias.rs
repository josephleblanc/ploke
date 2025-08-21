// Basic public alias for a primitive type
pub type SimpleId = u64;

// Private alias (inherited visibility) for a primitive type
type InternalCounter = i32;

// Crate-visible alias for a standard library generic type
pub(crate) type CrateBuffer = Vec<u8>;

/// Documented public alias for a tuple type
pub type Point = (i32, i32);

// Public alias with a single generic parameter
pub type GenericContainer<T> = Vec<T>;

// Public alias with a generic parameter and a trait bound
pub type DisplayableContainer<T: std::fmt::Display> = Vec<T>;

// Public alias with multiple generic parameters and complex type
pub type Mapping<K, V> = std::collections::HashMap<K, V>;

// Private alias for a function pointer type
type MathOperation = fn(i32, i32) -> i32;

// Public alias with an attribute
#[deprecated(note = "Use NewId instead")]
pub type OldId = String;

// Public alias referencing another type alias in the same scope
pub type IdAlias = SimpleId;

// Public alias with a where clause
pub type ComplexGeneric<T>
where
    T: Clone + Send + 'static,
= Option<T>;

// Module to test visibility and path interactions
mod inner {
    // Inherited visibility within the private module `inner`
    type InnerSecret = bool;

    // Public within the private module `inner`
    pub type InnerPublic = f64;

    // Alias using a type from the outer scope
    pub(super) type OuterPoint = super::Point;
}

// Using an inner type alias from a private module (effectively private)
type UseInner = inner::InnerPublic;

// Using a type alias defined with super visibility
type UseOuterPoint = inner::OuterPoint;

// Alias involving a reference
pub type StrSlice<'a> = &'a str;

// Alias involving a mutable reference
pub type MutStrSlice<'a> = &'a mut str;

// Alias involving a raw pointer
type ConstRawPtr = *const u8;

// Alias involving a mutable raw pointer
type MutRawPtr = *mut u8;

// Alias for an array type
pub type ByteArray = [u8; 256];

// Alias for a slice type (less common, usually references are used)
// type ByteSlice = [u8]; // Slices must be used behind a pointer, so this isn't typically aliased directly

// Alias for `Self` (only valid within impl blocks, cannot be tested here directly)
// Example: struct Example; impl Example { type This = Self; }

// Alias for `dyn Trait`
pub type DynDrawable = dyn std::fmt::Debug;

// Alias for `impl Trait` (only valid in specific positions, not as a standalone type alias)
// Example: fn example() -> impl std::fmt::Debug { ... }
