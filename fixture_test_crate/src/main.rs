mod sibling_of_main;
// fixture_test_crate/src/main.rs

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
        pub fn example_func() {}
    }
    mod c {
        pub fn incorrect_func() {}
    }
    fn test_func() {
        example_func(); // Correct
        incorrect_func();
    }
}

pub fn test_outer() {
    // outer_function(); // Error E0425
}

fn test_outer_private() {
    outer_function(); // Error E0425
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
                                        // name
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
