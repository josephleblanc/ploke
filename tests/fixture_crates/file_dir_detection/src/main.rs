// fixture_test_crate/src/main.rs
#![allow(unused)] // Add file-level attribute
//! This is the crate root doc comment.

// fixture_test_crate/src/main.rs
// Main entry point for file_dir_detection fixture

use std::path::Path; // Add a use statement

// Keep the existing complex module structure for deep nesting tests
pub mod example_mod;

// Add a public top-level module declared via file
pub mod top_pub_mod;

// Add a private top-level module declared via file
mod top_priv_mod;

// pub mod second_sibling;

// Add a crate-visible top-level module declared via file
pub(crate) mod crate_visible_mod;

// Add a module declared via #[path] attribute
#[path = "custom_path/real_file.rs"]
pub mod logical_name;

/// An inline public module doc comment.
#[cfg(test)] // Add module-level attribute
pub mod inline_pub_mod {
    use std::collections::HashMap; // Add use statement inside module

    pub fn inline_pub_func() {}

    // Function with a name duplicated elsewhere
    pub fn duplicate_name() -> u8 {
        1
    }

    // Nested inline private module
    mod inline_nested_priv {
        fn inline_nested_priv_func() {}

        // Nested inline public module within private
        pub mod inline_nested_pub_in_priv {
            pub fn deep_inline_pub_func() {}
        }
    }

    // Nested inline super-visible module
    pub(super) mod super_visible_inline {
        pub fn super_visible_func() {}
    }
}

// Add an inline private module
mod inline_priv_mod {
    fn inline_priv_func() {}

    // Nested inline public module within private
    pub mod inline_nested_pub {
        pub fn inline_nested_pub_func() {}
    }
}

// A top-level public function
pub fn main_pub_func() {}

// A top-level private function
fn main_priv_func() {}

// Re-export
pub use crate::top_pub_mod::top_pub_func as reexported_func;

// Function with a name duplicated elsewhere
pub fn duplicate_name() -> u8 {
    0
}

// Main function (optional, but good practice for a binary crate root)
fn main() {
    println!("File Dir Detection Fixture");
    main_pub_func();
    main_priv_func();
    #[cfg(test)] // Add module-level attribute
    inline_pub_mod::inline_pub_func();
    // inline_priv_mod::inline_priv_func(); // Private
    inline_priv_mod::inline_nested_pub::inline_nested_pub_func(); // Public within private
    top_pub_mod::top_pub_func();
    // top_priv_mod::top_priv_func(); // Private module function, but module is private
    top_priv_mod::nested_pub_in_priv::nested_pub_func(); // Public function in public mod within private mod
    crate_visible_mod::crate_vis_func(); // Crate visible module's function
    logical_name::item_in_real_file(); // Item from module declared via #[path]

    // Demonstrate name duplication is valid
    let _val0 = duplicate_name(); // Calls crate::duplicate_name
    #[cfg(test)] // Add module-level attribute
    let _val1 = inline_pub_mod::duplicate_name(); // Calls crate::inline_pub_mod::duplicate_name
    let _val2 = top_pub_mod::duplicate_name(); // Calls crate::top_pub_mod::duplicate_name

    // Access item in super-visible module
    #[cfg(test)] // Add module-level attribute
    inline_pub_mod::super_visible_inline::super_visible_func();
}
