// NOTE: All the following commneted out lines result in errors, verified by rust-analyzer.
// All lines left in are correct.

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

fn scope_check_func() {
    let sample_struct: SampleStruct<usize> = SampleStruct::new(42); // correct
    sample_struct.hidden_method(); // correct
    sample_struct.use_field(); // correct
    sample_struct.public_impl_method(); // correct
    sample_struct.private_impl_method(); // correct
    sample_struct.another_method(&21); // correct
    let x = sample_struct.field; // correct;
}

mod not_child_of_main_dir {
    fn cross_module_scope_check_func() {
        // let sample_struct: SampleStruct<usize> = SampleStruct::new(42); // error
        // sample_struct.hidden_method(); // error
        // sample_struct.use_field(); // error
        // sample_struct.public_impl_method(); // error
        // sample_struct.private_impl_method(); // error
        // sample_struct.another_method(&21); // error
        // let x = sample_struct.field; // error;
    }
}

mod not_child_of_main_dir_with_imports {
    // use crate::SampleStruct; // Incorrect, no `SampleStruct` in main.rs

    fn cross_module_scope_check_func_has_import() {
        // let sample_struct: SampleStruct<usize> = SampleStruct::new(42); // incorrect in src/second_sibling, correct in src/main.rs
        // sample_struct.hidden_method(); // incorrect in src/second_sibling, correct in src/main.rs
        // sample_struct.use_field(); // incorrect in src/second_sibling, correct in src/main.rs
        // sample_struct.public_impl_method(); // incorrect in src/second_sibling, correct in src/main.rs
        // sample_struct.private_impl_method(); // incorrect in src/second_sibling, correct in src/main.rs
        // let x = sample_struct.field; // incorrect in src/second_sibling, correct in src/main.rs

        // sample_struct.another_method(&21); // error even in main.rs, trait not visible
    }

    fn cross_module_with_func_and_trait_import() {
        // use crate::AnotherTrait; // incorrect in src/second_sibling, correct in src/main.rs
        // let sample_struct: SampleStruct<usize> = SampleStruct::new(42); // incorrect in src/second_sibling, correct in src/main.rs
        // sample_struct.another_method(&21); // incorrect in src/second_sibling, correct in src/main.rs
    }

    fn corred_sibling_file_imports() {
        use super::SampleStruct; // correct import in src/second_sibling
        let sample_struct: SampleStruct<usize> = SampleStruct::new(42); // correct in src/second_sibling,
        sample_struct.hidden_method(); // correct in src/second_sibling,
        sample_struct.use_field(); // correct in src/second_sibling,
        sample_struct.public_impl_method(); // correct in src/second_sibling,
        sample_struct.private_impl_method(); // correct in src/second_sibling,
        let x = sample_struct.field; // correct in src/second_sibling,
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

    /// Public method in impl block
    pub fn public_impl_method(&self) {}

    /// Private method in impl block
    fn private_impl_method(&self) {}

    /// Hidden method in impl block
    #[doc(hidden)]
    pub fn hidden_method(&self) {}
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
    

    /// Struct inside a public module
    pub struct ModuleStruct {
        pub module_field: String,
    }

    /// Macro inside a module
    #[macro_export]
    macro_rules! module_macro {
        () => {};
    }
}

// Module hierarchy for testing nested visibility
// Assumes presence of mod.rs file with `pub use::outer` or similar.
mod outer {
    pub mod middle {
        pub mod inner {
            pub fn deep_function() {}
        }

        pub fn middle_function() {}

        // pub(in crate::outer) fn restricted_fn() {} // incorrect in src/second_sibling.rs, correct
        //                                            // in src/main.rs
        // pub(in crate::outer) struct RestrictedStruct; // incorrect in src/second_sibling.rs, correct in src/main.rs
    }

    pub fn outer_function() {}
}

// Module with re-exports
mod intermediate {
    
    
    pub use super::DefaultTrait;

    pub struct ModuleStruct {
        module_field: String,
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

/// Struct with [Visibility] markers in docs
pub struct DocumentedStruct;

/// Inherits visibility from parent
pub struct DocInheritanceStruct;

#[doc(hidden)]
fn hidden_function() {}

/// Type alias example
pub type StringVec = Vec<String>;

// Private type alias
type PrivateTypeAlias = i32;

// Module type alias
pub type ModuleTypeAlias = String;

// Public type alias with generics
pub type GenericAlias<T> = Vec<T>;

// Private type alias in module
mod alias_module {
    pub type ModulePrivateAlias = f64;
}

// Items for attribute visibility tests
#[cfg_attr(public_attr, feature = "public")]
struct ConditionalVisibilityStruct {
    field: String,
}

#[cfg_attr(test, allow(unused))]
#[cfg_attr(public_attr, feature = "public")]
fn multi_attr_function() {}

#[cfg_attr(public_attr, feature = "never_enabled")]
struct ConditionalPrivateStruct {
    field: String,
}

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
