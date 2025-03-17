#![allow(dead_code, unused_variables)]
//! Sample module for testing the code graph builder
//!
//! This module contains various Rust constructs to test parsing.
/// A sample struct with a generic parameter
/// This docstring tests multi-line documentation
#[derive(Debug)]
pub struct SampleStruct<T> {
    pub field: T,
}

/// First trait for testing trait implementations
pub trait SampleTrait<T> {
    /// Method in trait
    fn trait_method(&self, param: T) -> T;
}

/// Second trait for testing multiple trait implementations
pub trait AnotherTrait<T> {
    /// Another method in trait
    fn another_method(&self, param: &T) -> bool;
}

/// Testing default trait with blanket implementation
pub trait DefaultTrait {
    fn default_method(&self) -> String {
        "Default implementation".to_string()
    }
}

/// Implementation of SampleTrait for SampleStruct
impl<T> SampleTrait<T> for SampleStruct<T>
where
    T: Clone,
{
    fn trait_method(&self, param: T) -> T {
        self.field.clone()
    }
}

/// Implementation of AnotherTrait for SampleStruct
impl<T> AnotherTrait<T> for SampleStruct<T>
where
    T: PartialEq,
{
    fn another_method(&self, param: &T) -> bool {
        &self.field == param
    }
}

// Implementation of DefaultTrait for SampleStruct
impl<T> DefaultTrait for SampleStruct<T> {}

// Direct implementation for SampleStruct
impl<T> SampleStruct<T> {
    /// Constructor method
    pub fn new(field: T) -> Self {
        SampleStruct { field }
    }

    /// Method that uses the field
    pub fn use_field(&self) -> &T {
        &self.field
    }
}

/// A nested struct inside the module
pub struct NestedStruct {
    pub nested_field: i32,
}

/// A public function that takes various parameters
pub fn sample_function<T: Clone>(
    param1: SampleStruct<T>,
    param2: &NestedStruct,
) -> SampleStruct<T> {
    // Create a local variable
    let local_var = param1.field.clone();

    // Construct and return a new struct
    SampleStruct { field: local_var }
}

/// Sample enum with different variant types
#[derive(Debug)]
pub enum SampleEnum<T> {
    Variant1,
    Variant2(T),
}

// Private module for testing visibility
mod private_module {

    struct PrivateStruct {
        private_field: String,
    }

    impl PrivateStruct {
        fn private_method(&self) -> &str {
            &self.private_field
        }
    }

    pub fn public_function_in_private_module() -> &'static str {
        "I'm public but in a private module"
    }

    // Private function
    fn private_function() -> i32 {
        42
    }

    // Private struct
    struct PrivateStruct2 {
        private_field: i32,
    }

    // Private enum
    enum PrivateEnum {
        Variant1,
        Variant2,
    }

    // Private type alias
    type PrivateTypeAlias = i32;

    // Private union
    union PrivateUnion {
        i: i32,
        f: f32,
    }

    // Private trait
    trait PrivateTrait {
        fn private_method(&self) -> i32;
    }

    // Private impl
    impl PrivateTrait for PrivateStruct {
        fn private_method(&self) -> i32 {
            42
        }
    }

    // Private const
    const PRIVATE_CONST: i32 = 10;

    // Private static
    static PRIVATE_STATIC: i32 = 0;

    // Private macro
    #[allow(unused_macros)]
    macro_rules! private_macro {
        () => {
            println!("This is a private macro");
        };
    }
}

// Public module with nested types
pub mod public_module {
    use super::*;

    /// Struct inside a public module
    pub struct ModuleStruct {
        pub module_field: String,
    }

    /// Implementation of a trait from parent module
    impl DefaultTrait for ModuleStruct {
        fn default_method(&self) -> String {
            format!("Custom implementation: {}", self.module_field)
        }
    }

    /// Enum with discriminants
    pub enum ModuleEnum {
        First = 1,
        Second = 2,
    }
}

// Tuple struct
pub struct TupleStruct(pub String, pub i32);

// Unit struct
pub struct UnitStruct;

/// Type alias example
pub type StringVec = Vec<String>;

/// Generic type alias
pub type Result<T> = std::result::Result<T, String>;

/// Union example for memory-efficient storage
#[repr(C)]
pub union IntOrFloat {
    pub i: i32,
    pub f: f32,
}

/// A public constant with documentation
pub const MAX_ITEMS: usize = 100;

/// A private constant
const MIN_ITEMS: usize = 10;

/// A public static variable
pub static GLOBAL_COUNTER: i32 = 0;

/// A mutable static variable
pub static mut MUTABLE_COUNTER: i32 = 0;

/// A simple macro for testing
#[macro_export]
macro_rules! test_macro {
    // Simple pattern with no arguments
    () => {
        println!("Hello from macro!");
    };
    // Pattern with an expression argument
    ($expr:expr) => {
        println!("Expression: {}", $expr);
    };
    // Pattern with multiple arguments
    ($name:ident, $value:expr) => {
        println!("{} = {}", stringify!($name), $value);
    };
}
