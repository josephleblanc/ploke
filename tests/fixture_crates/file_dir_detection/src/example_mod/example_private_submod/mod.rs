// Retain existing structure but add an item

pub mod public_submod_private_parent;
mod very_private_submod;

// Keep the deep nesting
pub mod subsubmod;

// Item directly within example_private_submod
pub fn item_in_example_private_submod() {} // Effectively crate visible
