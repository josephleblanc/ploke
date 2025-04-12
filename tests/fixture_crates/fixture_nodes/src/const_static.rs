// tests/fixture_crates/fixture_nodes/src/const_static.rs

#![allow(dead_code)] // Prevent warnings about unused items
#![allow(unused_mut)] // Prevent warnings about unused mutable statics

//! Test fixture for parsing const and static items.

// --- Top Level Items ---

/// A top-level private constant with a simple integer type.
const TOP_LEVEL_INT: i32 = 10;

/// A top-level public constant with a boolean type.
pub const TOP_LEVEL_BOOL: bool = true;

/// A top-level private static string slice.
static TOP_LEVEL_STR: &str = "hello world";

/// A top-level public mutable static counter.
pub static mut TOP_LEVEL_COUNTER: u32 = 0;

// --- Type Variations ---

/// Constant array.
const ARRAY_CONST: [u8; 3] = [1, 2, 3];

/// Static tuple.
static TUPLE_STATIC: (i32, bool) = (5, false);

/// A simple struct used for const/static types.
struct SimpleStruct {
    x: i32,
    y: bool,
}

/// Constant struct instance.
const STRUCT_CONST: SimpleStruct = SimpleStruct { x: 99, y: true };

/// Type alias used in a constant.
type MyInt = i32;

/// Constant using a type alias.
const ALIASED_CONST: MyInt = -5;

// --- Initializer Variations ---

/// Constant initialized with a basic arithmetic expression.
const EXPR_CONST: i32 = 5 * 2 + 1;

/// A const function needed to initialize another const.
const fn five() -> i32 {
    5
}

/// Constant initialized with a call to a const function.
const FN_CALL_CONST: i32 = five();

// --- Attributes and Docs ---

/// This is a documented constant.
#[deprecated(note = "Use NEW_DOC_ATTR_CONST instead")]
#[allow(non_upper_case_globals, clippy::approx_constant)] // Example of more attributes
pub const doc_attr_const: f64 = 3.14;

/// This is a documented static variable.
#[cfg(target_os = "linux")] // Example attribute
static DOC_ATTR_STATIC: &str = "Linux specific";

// --- Associated Constants (Basic Examples) ---
// While these might be better tested in impl/trait specific tests,
// including basic forms here ensures the ValueNode parsing handles them.

struct Container;

impl Container {
    /// An associated constant within an impl block.
    pub const IMPL_CONST: usize = 1024;

    // Note: Static items are not allowed directly in inherent impls.
    // static IMPL_STATIC: bool = true; // Compile Error
}

trait ExampleTrait {
    /// An associated constant required by a trait.
    const TRAIT_REQ_CONST: bool;
}

impl ExampleTrait for Container {
    /// Implementation of the trait's associated constant.
    const TRAIT_REQ_CONST: bool = true;
}

// --- Main function (optional, makes it runnable) ---
fn main() {
    println!("Constant: {}", TOP_LEVEL_INT);
    // Accessing mutable statics requires unsafe block
    unsafe {
        TOP_LEVEL_COUNTER += 1;
        println!("Static mut: {}", TOP_LEVEL_COUNTER);
    }
}
