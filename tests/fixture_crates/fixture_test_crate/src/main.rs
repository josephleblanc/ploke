// fixture_test_crate/src/main.rs
// #![allow(unused_variables, dead_code, unused_imports, unexpected_cfgs)]

mod example_mod;
mod second_sibling;
mod sibling_of_main;

mod outer {
    // Inner module
    mod inner {
        fn inner_function() {
            println!("Inner function");
        }
    }

    // Public function in outer module
    pub fn outer_function() {
        println!("Outer function");
    }
}

mod a {
    pub mod b {
        pub fn public_func() {}
    }
    mod c {
        fn private_func() {}
    }
    mod d {
        pub fn public_func_in_private_mod() {}
    }
    fn test_func() {
        // public_func(); // incorrect, not found E0425
        // private_func(); // incorrect, not found E0425
        // public_func_in_private_mod; // incorrect, not found E0425
    }
    fn other_test_func() {
        use crate::a::b::public_func;
        public_func(); // correct

        // use crate::a::c::private_func; // incorrect, is private E0603
    }
    fn final_test_func() {
        use crate::a::d::public_func_in_private_mod;
        public_func_in_private_mod(); // correct
    }
    fn temp_test_func() {
        use crate::a::b::public_func;
        // use crate::a::c::private_func; // E0603 Error, private function
        use crate::a::d::public_func_in_private_mod;
    }
}
pub fn test_pub_in_priv() {
    // use a::d; // Incorrect, E0603 module is private
    // use a::d::private_func; // Incorrect, E0603 module is private
    use a::b::public_func;
    public_func(); // Correct
}

pub fn test_outer() {
    // outer_function(); // Error E0425
}

fn test_outer_private() {
    // outer_function(); // Error E0425
}

// outer_function(); // Incorrect. cannot call function outside a function like this.

mod unrelated {
    pub fn test_outer_unrelated() {
        // outer_function(); // Error E0425
    }
}
mod unrelated_with_super_import {
    use super::outer::outer_function; // Correct import using `super` since we are in same file

    pub fn test_outer_unrelated() {
        outer_function(); // Correct
        crate::outer::outer_function(); // Correct
        super::outer::outer_function(); // Correct, shadowing import namespace allowed for function
                                        // calls
    }
}

mod unrelated_with_crate_import {
    // Correct import using `crate` since we are in
    // `fixture_test_crate/src/main.rs`
    use crate::outer::outer_function;

    pub fn test_outer_unrelated() {
        outer_function(); // Correct
                          // outer::outer_function(); // Error E0433 Incorrect, because we have not imported `crate::outer`
    }
}

mod unrelated_with_short_import {
    // Correct import using `super` since we are in
    // `fixture_test_crate/src/main.rs`
    use crate::outer; //

    pub fn test_outer_unrelated() {
        // outer_function(); // Error E0425 since we are not providing full path for function which
        // includes function name
        outer::outer_function(); // Correct
        crate::outer::outer_function() // Also correct
    }
}

mod unrelated_with_long_import {
    // use fixture_test_crate; // Incorrect, unresolved import
    // use fixture_test_crate::outer; // Incorrect, unresolved import
    // use fixture_test_crate::outer::outer_function; // Incorrect, unresolved import
    // use crate::super::outer; // Incorrect, `super` must be first term in import
    // use super::super::outer::outer_function; // Incorrect, too many leading `super` keywords
    // use super::crate; // Incorrect No `crate` found in root.
    // use super::unrelated_with_long_import::some_func; // Incorrect, defined multiple times.
    use super::unrelated_with_long_import; // Correct (but useless?)
    pub fn some_func() {}
}

fn main() {
    use crate::outer::outer_function; // Correct import
    outer_function(); // Correct

    println!("Hello, world!");
}
