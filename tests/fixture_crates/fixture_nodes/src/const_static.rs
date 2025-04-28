// tests/fixture_crates/fixture_nodes/src/const_static.rs

#![allow(dead_code)] // Prevent warnings about unused items
#![allow(unused_mut)] // Prevent warnings about unused mutable statics

//! Test fixture for parsing const and static items.

// --- Top Level Items ---

/// A top-level private constant with a simple integer type.
const TOP_LEVEL_INT: i32 = 10;

/// A top-level public constant with a boolean type.
pub const TOP_LEVEL_BOOL: bool = true;

// A top-level private static string slice (no doc comment).
static TOP_LEVEL_STR: &str = "hello world";

/// A top-level public mutable static counter.
pub static mut TOP_LEVEL_COUNTER: u32 = 0;

/// A top-level crate-visible static string.
pub(crate) static TOP_LEVEL_CRATE_STATIC: &str = "crate visible";

// --- Type Variations ---

// Constant array (no doc comment).
const ARRAY_CONST: [u8; 3] = [1, 2, 3];

// Static tuple (no doc comment).
static TUPLE_STATIC: (i32, bool) = (5, false);

// A simple struct used for const/static types (no doc comment).
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

// Constant initialized with a basic arithmetic expression (no doc comment).
const EXPR_CONST: i32 = 5 * 2 + 1;

// A const function needed to initialize another const (no doc comment).
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

// --- Inline Module ---

mod inner_mod {
    // Constant visible only within the crate, defined inside an inline module.
    pub(crate) const INNER_CONST: u8 = 1;

    // Static mutable boolean visible only to the parent module (`crate`), defined inside an inline module.
    #[allow(dead_code)] // Allow unused for fixture simplicity
    pub(super) static mut INNER_MUT_STATIC: bool = false;
}

// --- Usage function (to ensure items are used and fixture compiles) ---
#[allow(unused_variables, clippy::let_unit_value)]
pub fn use_all_const_static() {
    // Top Level
    let _int = TOP_LEVEL_INT;
    let _bool = TOP_LEVEL_BOOL;
    let _str = TOP_LEVEL_STR;
    let _crate_static = TOP_LEVEL_CRATE_STATIC;

    // Type Variations
    let _arr = ARRAY_CONST;
    let _tuple = TUPLE_STATIC;
    let _struct = STRUCT_CONST;
    let _aliased = ALIASED_CONST;

    // Initializer Variations
    let _expr = EXPR_CONST;
    let _fn_call = FN_CALL_CONST;

    // Attributes and Docs
    let _doc_attr = doc_attr_const;
    #[cfg(target_os = "linux")]
    let _doc_attr_static = DOC_ATTR_STATIC;

    // Associated Constants
    let _impl_const = Container::IMPL_CONST;
    let _trait_const = <Container as ExampleTrait>::TRAIT_REQ_CONST;

    // Inline Module Items
    let _inner_const = inner_mod::INNER_CONST;

    // Accessing mutable statics requires unsafe block
    unsafe {
        TOP_LEVEL_COUNTER += 1;
        let _counter = TOP_LEVEL_COUNTER;

        inner_mod::INNER_MUT_STATIC = !inner_mod::INNER_MUT_STATIC;
        let _inner_mut = inner_mod::INNER_MUT_STATIC;
    }

    // Println to potentially use some values and avoid unused warnings further
    println!("Used: {}, {}, {}, {}", _int, _bool, _str, _crate_static);
}
