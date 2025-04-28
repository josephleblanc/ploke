//! This file contains various function types for testing the parser

// Regular function
pub fn regular_function() {
    println!("Regular function");
}

// Function with parameters
pub fn function_with_params(x: i32, y: i32) -> i32 {
    x + y
}

// Generic function
pub fn generic_function<T>(arg: T) -> T {
    arg
}

// Function with attributes
#[cfg(test)]
pub fn attributed_function() {
    println!("Attributed function");
}

// Function with docstring
/// This is a documented function
pub fn documented_function() {
    println!("Documented function");
}

// Unsafe function
pub unsafe fn unsafe_function() {
    println!("Unsafe function");
}

// Function with lifetime annotations
pub fn lifetime_function<'a>(param: &'a str) -> &'a str {
    param
}

// Private function
fn private_function() {
    println!("Private function");
}

// Function with multiple generic parameters
pub fn multi_generic_function<T, U>(t: T, u: U) -> (T, U) {
    (t, u)
}

// Function with where clause
pub fn where_clause_function<T>(arg: T) -> T 
where 
    T: Clone,
{
    arg.clone()
}

// Async function
pub async fn async_function() {
    println!("Async function");
}

// Function with default parameters (via Option)
pub fn default_params(required: i32, optional: Option<String>) -> String {
    match optional {
        Some(s) => format!("{}: {}", required, s),
        None => format!("{}", required),
    }
}
