// Retain existing structure but add an item

// Public subdirectory
pub mod example_submod;

// Private subdirectory
mod example_private_submod;

// Public files in same directory (will be deleted later)
// pub mod mod_sibling_one;
// pub mod mod_sibling_two;

// Private file in same directory (will be deleted later)
// mod mod_sibling_private;

// Item directly within example_mod
pub fn item_in_example_mod() {}
