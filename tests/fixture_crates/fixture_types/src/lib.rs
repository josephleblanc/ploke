#![allow(dead_code, unused_variables)]

use std::fmt::Debug;

mod func;

// Basic named type (defined elsewhere or primitive)
pub type NamedType = i32;
pub type QualifiedPath = std::collections::HashMap<String, i32>;

// Tuple type
pub type Point = (i32, i32);
pub fn process_tuple(p: Point) -> i32 {
    p.0 + p.1
}

// Slice type
pub fn process_slice(s: &[u8]) -> usize {
    s.len()
}

// Array type
pub type Buffer = [u8; 1024];
pub fn process_array(a: Buffer) -> u8 {
    a[0]
}

// Reference types
pub fn process_ref(r: &String) -> usize {
    r.len()
}

pub fn process_mut_ref(r: &mut String) {
    r.push_str(" modified");
}

// Raw pointer types
fn process_const_ptr(p: *const i32) -> i32 {
    unsafe { *p }
}
fn process_mut_ptr(p: *mut i32) {
    unsafe {
        *p = 42;
    }
}

// Function pointer type
pub type MathOperation = fn(i32, i32) -> i32;
pub fn apply_op(a: i32, b: i32, op: MathOperation) -> i32 {
    op(a, b)
}

// Trait object type (dyn Trait)
pub trait Drawable {
    fn draw(&self);
}
pub fn draw_object(obj: &dyn Drawable) {
    obj.draw();
}
pub type DrawableRef<'a> = &'a dyn Drawable; // Type alias for trait object

// Impl Trait type (in argument position)
pub fn process_impl_trait_arg(arg: impl Debug) {
    println!("{:?}", arg);
}

// Impl Trait type (in return position)
pub fn create_impl_trait_return() -> impl Debug {
    "Return value implementing Debug"
}

// Never type (!) - stable since 1.41
// pub fn diverges() -> ! {
//     panic!("This function never returns");
// }

// Inferred type (_) - typically in let bindings, not directly as a node type
pub fn inferred_type_example() {
    let x = 5; // Type of x is inferred as i32
    let _y: _ = "hello".to_string(); // Explicit inference placeholder
}

// Parenthesized type
pub type ParenType = (i32); // Equivalent to i32

// Bare function type (less common than fn pointer)
// pub type BareFn = unsafe extern "C" fn(i32) -> i32;

// Placeholder for Macro type (handled by MacroNode)
// macro_rules! my_macro { () => {}; }
// pub type MacroGenerated = my_macro!(); // Not a real type node

// Unknown type (parser fallback) - cannot be directly represented in valid Rust

mod duplicate_names {
    use std::fmt::Debug;

    // Basic named type (defined elsewhere or primitive)
    pub type NamedType = i32;
    pub type QualifiedPath = std::collections::HashMap<String, i32>;

    // Tuple type
    pub type Point = (i32, i32);
    pub fn process_tuple(p: Point) -> i32 {
        p.0 + p.1
    }

    // Slice type
    pub fn process_slice(s: &[u8]) -> usize {
        s.len()
    }

    // Array type
    pub type Buffer = [u8; 1024];
    pub fn process_array(a: Buffer) -> u8 {
        a[0]
    }

    // Reference types
    pub fn process_ref(r: &String) -> usize {
        r.len()
    }
    pub fn process_mut_ref(r: &mut String) {
        r.push_str(" modified");
    }

    // Raw pointer types
    fn process_const_ptr(p: *const i32) -> i32 {
        unsafe { *p }
    }
    fn process_mut_ptr(p: *mut i32) {
        unsafe {
            *p = 42;
        }
    }

    // Function pointer type
    pub type MathOperation = fn(i32, i32) -> i32;
    pub fn apply_op(a: i32, b: i32, op: MathOperation) -> i32 {
        op(a, b)
    }

    // Trait object type (dyn Trait)
    pub trait Drawable {
        fn draw(&self);
    }
    pub fn draw_object(obj: &dyn Drawable) {
        obj.draw();
    }
    pub type DrawableRef<'a> = &'a dyn Drawable; // Type alias for trait object

    // Impl Trait type (in argument position)
    pub fn process_impl_trait_arg(arg: impl Debug) {
        println!("{:?}", arg);
    }

    // Impl Trait type (in return position)
    pub fn create_impl_trait_return() -> impl Debug {
        "Return value implementing Debug"
    }

    // Never type (!) - stable since 1.41
    // pub fn diverges() -> ! {
    //     panic!("This function never returns");
    // }

    // Inferred type (_) - typically in let bindings, not directly as a node type
    pub fn inferred_type_example() {
        let x = 5; // Type of x is inferred as i32
        let _y: _ = "hello".to_string(); // Explicit inference placeholder
    }

    // Parenthesized type
    pub type ParenType = (i32); // Equivalent to i32

    // Bare function type (less common than fn pointer)
    // pub type BareFn = unsafe extern "C" fn(i32) -> i32;

    // Placeholder for Macro type (handled by MacroNode)
    // macro_rules! my_macro { () => {}; }
    // pub type MacroGenerated = my_macro!(); // Not a real type node

    // Unknown type (parser fallback) - cannot be directly represented in valid Rust
    mod duplicate_names {}
}
