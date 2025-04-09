// Basic public union
pub union IntOrFloat {
    i: i32,
    f: f32,
}

// Private union (inherited visibility)
union SecretData {
    key: u64,
    flag: bool,
}

// Crate-visible union
pub(crate) union CrateUnion {
    ptr: *const u8,
    offset: usize,
}

/// Documented public union
pub union DocumentedUnion {
    data: [u8; 16],
    id: u128,
}

// Public union with generic parameter
pub union GenericUnion<T> {
    value: std::mem::ManuallyDrop<T>, // ManuallyDrop often used in unions
    raw: usize,
}

// Public union with generic parameter and trait bound (less common for unions)
// Note: Bounds on union generics are often complex due to safety.
// This example uses `Copy` which is common for union fields.
pub union GenericUnionWithBound<T: Copy> {
    typed_value: T,
    bytes: [u8; std::mem::size_of::<T>()], // Example using size_of
}

// Public union with an attribute
#[repr(C)]
pub union ReprCUnion {
    integer: i64,
    pointer: *mut std::ffi::c_void,
}

// Union with fields having attributes (less common)
pub union UnionWithFieldAttr {
    #[cfg(target_endian = "big")]
    big_endian_data: u32,
    #[cfg(not(target_endian = "big"))]
    little_endian_data: u32,
    always_present: u8,
}

// Module to test visibility interactions
mod inner {
    // Private union inside private module
    union InnerSecret {
        a: i8,
        b: u8,
    }

    // Public union inside private module
    pub union InnerPublic {
        x: f32,
        y: f32,
    }
}

// Using an inner union (effectively private)
type UseInnerUnion = inner::InnerPublic;
