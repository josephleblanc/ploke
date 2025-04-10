// Corresponds to `mod top_priv_mod;` in main.rs

// Nested public module via file (effectively crate visible)
pub mod nested_pub_in_priv;

// Nested private module via file
mod nested_priv_in_priv;

// Item directly in this module (effectively crate visible)
pub fn top_priv_func() {}

// Private item
fn top_priv_priv_func() {}
