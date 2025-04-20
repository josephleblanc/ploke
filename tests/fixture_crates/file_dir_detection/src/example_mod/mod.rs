// Retain existing structure but add an item

// Public subdirectory
pub mod example_submod;

// Private subdirectory
mod example_private_submod;

pub mod mod_sibling_one;
pub mod mod_sibling_two;

mod mod_sibling_private;

// Item directly within example_mod
pub fn item_in_example_mod() {}
