// Corresponds to `pub mod top_pub_mod;` in main.rs

// Nested public module via file
pub mod nested_pub;

// Nested private module via file
mod nested_priv;

// Item directly in this module
pub fn top_pub_func() {}

// Nested module visible only within `top_pub_mod`
pub(in crate::top_pub_mod) mod path_visible_mod;

// Private item
fn top_pub_priv_func() {}

// Function with a name duplicated elsewhere
pub fn duplicate_name() -> u8 { 2 }
