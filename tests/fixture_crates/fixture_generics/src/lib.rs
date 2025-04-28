#![allow(dead_code, unused_variables, unused_lifetimes)]

use std::fmt::Display;

// Struct with type parameter
pub struct GenericStruct<T> {
    field: T,
}

// Struct with lifetime parameter
pub struct LifetimeStruct<'a> {
    reference: &'a str,
}

// Struct with const generic parameter
pub struct ConstGenericStruct<const N: usize> {
    array: [i32; N],
}

// Struct with multiple parameters and bounds
pub struct ComplexGenericStruct<'a, T: Display + Clone, const N: usize> {
    field: T,
    reference: &'a str,
    array: [i32; N],
}

// Function with type parameter and where clause
pub fn generic_function<T>(arg: T) -> T
where
    T: Default,
{
    T::default()
}

// Function with lifetime parameter
pub fn lifetime_function(arg: &str) -> &str {
    arg
}

// Function with const generic parameter
pub fn const_generic_function<const N: usize>(arg: [i32; N]) -> usize {
    N
}

// Trait with associated type and generics
pub trait GenericTrait<'a, T>
where
    T: 'a + Default,
{
    type Output;
    fn process(&'a self, input: T) -> Self::Output;
}

// Impl with generics and where clause
impl<'a, T: Display + Clone + Default, const N: usize> GenericTrait<'a, T>
    for ComplexGenericStruct<'a, T, N>
where
    T: 'a + Send, // Additional bound in impl
{
    type Output = String;
    fn process(&'a self, input: T) -> Self::Output {
        format!(
            "Processing {} with ref '{}' and array size {}",
            input, self.reference, N
        )
    }
}

// Impl for a generic struct
impl<T> GenericStruct<T> {
    pub fn new(field: T) -> Self {
        Self { field }
    }
}
