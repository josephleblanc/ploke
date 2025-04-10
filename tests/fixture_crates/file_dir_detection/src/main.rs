// fixture_test_crate/src/main.rs
// Main entry point for file_dir_detection fixture

// Keep the existing complex module structure for deep nesting tests
pub mod example_mod;

// Add a public top-level module declared via file
pub mod top_pub_mod;

// Add a private top-level module declared via file
mod top_priv_mod;

// Add an inline public module
pub mod inline_pub_mod {
    pub fn inline_pub_func() {}

    // Nested inline private module
    mod inline_nested_priv {
        fn inline_nested_priv_func() {}

        // Nested inline public module within private
        pub mod inline_nested_pub_in_priv {
            pub fn deep_inline_pub_func() {}
        }
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

// Main function (optional, but good practice for a binary crate root)
fn main() {
    println!("File Dir Detection Fixture");
    main_pub_func();
    main_priv_func();
    inline_pub_mod::inline_pub_func();
    // inline_priv_mod::inline_priv_func(); // Private
    inline_priv_mod::inline_nested_pub::inline_nested_pub_func(); // Public within private
    top_pub_mod::top_pub_func();
    // top_priv_mod::top_priv_func(); // Private module
    top_priv_mod::nested_pub_in_priv::nested_pub_func(); // Public within private
}
