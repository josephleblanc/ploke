// This file contains deliberate syntax errors for testing parser error handling.
// It is not expected to compile.

pub fn correct_function(x: i32) -> i32 {
    x + 1
}

// Missing semicolon
let y = 5

// Mismatched types (semantic error, syn might parse but later stages fail)
pub fn type_mismatch() -> String {
    return 10;
}

// Invalid token
pub fn invalid_token() {
    let z = &; // Ampersand requires something after it
}

// Unclosed brace
pub struct UnclosedStruct {
    field: i32,
// Missing closing brace

// Unterminated string literal
// pub fn unterminated_string() {
//     let s = "hello world;
// }

// Invalid attribute syntax
// #[derive(Debug Clone)] // Missing comma
// pub struct InvalidAttr;
