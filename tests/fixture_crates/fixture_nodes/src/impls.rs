// --- Basic Structs and Traits for Impls ---

use std::fmt::Debug;

pub struct SimpleStruct {
    pub data: i32,
}

struct PrivateStruct {
    secret: String,
}

pub trait SimpleTrait {
    fn trait_method(&self) -> i32;
}

trait PrivateTrait {
    fn private_trait_method(&self);
}

pub trait GenericTrait<T> {
    fn generic_trait_method(&self, value: T);
}

pub trait AssocTrait {
    type Output;
    const ID: u32;

    fn create_output(&self) -> Self::Output;
}

// --- Inherent Impls ---

// Basic inherent impl
impl SimpleStruct {
    pub fn new(data: i32) -> Self {
        Self { data }
    }

    fn private_method(&self) -> i32 {
        self.data * 2
    }

    pub fn public_method(&self) -> i32 {
        self.private_method()
    }
}

// Inherent impl for a private struct
impl PrivateStruct {
    fn get_secret_len(&self) -> usize {
        self.secret.len()
    }
}

// Inherent impl with generics
pub struct GenericStruct<T> {
    pub value: T,
}

impl<T> GenericStruct<T> {
    pub fn get_value_ref(&self) -> &T {
        &self.value
    }
}

// Inherent impl with generics and bounds
impl<T: Debug> GenericStruct<T> {
    pub fn print_value(&self) {
        println!("{:?}", self.value);
    }
}

// Inherent impl with lifetimes
impl<'a> GenericStruct<&'a str> {
    fn get_str_len(&self) -> usize {
        self.value.len()
    }
}

// --- Trait Impls ---

// Basic trait impl for struct
impl SimpleTrait for SimpleStruct {
    fn trait_method(&self) -> i32 {
        self.data
    }
}

// Private trait impl for struct
impl PrivateTrait for SimpleStruct {
    fn private_trait_method(&self) {
        println!("Private trait impl: {}", self.data);
    }
}

// Trait impl for generic struct (generic on impl)
impl<T> SimpleTrait for GenericStruct<T> where T: Default + Copy + Into<i32> {
    fn trait_method(&self) -> i32 {
        self.value.into()
    }
}

// Generic trait impl for generic struct
impl<T: Clone> GenericTrait<T> for GenericStruct<T> {
    fn generic_trait_method(&self, value: T) {
        // Method uses T from both trait and struct generics
    }
}

// Trait impl for a specific generic instantiation - REMOVED due to E0119 conflict
// impl SimpleTrait for GenericStruct<String> {
//     fn trait_method(&self) -> i32 {
//         self.value.len() as i32
//     }
// }

// Trait impl for a primitive type
impl SimpleTrait for i32 {
    fn trait_method(&self) -> i32 {
        *self
    }
}

// Trait impl with associated types and consts
impl AssocTrait for SimpleStruct {
    type Output = String;
    const ID: u32 = 123;

    fn create_output(&self) -> Self::Output {
        format!("Data: {}", self.data)
    }
}

// Trait impl with lifetimes
impl<'a> SimpleTrait for &'a SimpleStruct {
    fn trait_method(&self) -> i32 {
        self.data
    }
}

// --- Impls inside modules ---
mod inner {
    use super::{SimpleStruct, SimpleTrait}; // Import necessary items

    // Inherent impl inside module
    impl SimpleStruct {
        pub(super) fn method_in_module(&self) -> i32 {
            self.data + 1
        }
    }

    // Trait impl inside module
    struct InnerStruct;
    impl SimpleTrait for InnerStruct {
        fn trait_method(&self) -> i32 {
            42
        }
    }
}
