// Corresponds to `pub mod top_pub_mod;` in main.rs

// Nested public module via file
pub mod nested_pub;

// Nested private module via file
mod nested_priv;

// Item directly in this module
pub fn top_pub_func() {}

// Private item
fn top_pub_priv_func() {}
