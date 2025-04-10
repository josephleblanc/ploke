// Retain existing structure but add an item

// private parent, public submod (will be deleted later)
// pub mod public_submod_private_parent;
// private parent, private submod (will be deleted later)
// mod very_private_submod;

// Keep the deep nesting
pub mod subsubmod;

// Item directly within example_private_submod
pub fn item_in_example_private_submod() {} // Effectively crate visible
