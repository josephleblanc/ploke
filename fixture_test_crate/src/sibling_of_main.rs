// fixture_test_crate/src/sibling_of_main.rs

// mod second_sibling; // incorrect, unresolved module: cannot import other file here.

// Inner module
mod sibling_outer {
    fn sibling_outer_function() {
        println!("Sibling Outer Function");
    }

    pub fn sibling_public_function() {
        println!("Sibling Public Function")
    }
}

// Public function
pub fn sibling_outer_function() {
    // sibling_inner_function(); // Incorrect, out of scope
    // sibling_outer::sibling_inner_function(); // Incorrect, private
     // Correct use statement, as public function from same file in same context
                       // as `sibling_outer`
    println!("Outer function");
}

pub mod third_sibling {
    // use sibling_outer; // incorrect, not in same context as `sibling_outer`
    use crate::sibling_of_main::sibling_outer; // Correct

    fn third_sibling_function() {
        // sibling_public_function(); // Incorrect, out of scope
        sibling_outer::sibling_public_function(); // correct
    }
}

pub mod fourth_sibling {
    
}

pub fn sibling_test_outer() {
    // outer_function(); // Error E0425
    // outer::outer_function // Incorrect
    crate::outer::outer_function(); // Correct
    super::outer::outer_function(); // Correct
}

fn sibling_test_outer_private() {
    // outer_function(); // Error E0425
}

// outer_function(); // Incorrect. cannot call function outside a function like this.

mod sibling_unrelated {
    pub fn test_outer_sibling_unrelated() {
        // outer_function(); // Error E0425
    }
}
mod sibling_unrelated_with_super_import {
    use super::super::outer::outer_function; // Correct, since we are in sibling of defined
                                             // location (which is main.rs)
                                             // use super::outer::outer_function; // Incorrect `super` since we are in different file

    pub fn test_outer_sibling_unrelated() {
        crate::outer::outer_function(); // Correct, since we are sibling of main.rs
        super::super::outer::outer_function(); // Correct, shadowing import namespace allowed for function
                                               // name
        outer_function(); // Correct, since we imported `super::super::outer::outer_function`
    }
}

mod sibling_unrelated_with_crate_import {
    // Correct import using `crate` since we are in
    // `fixture_test_crate/src/main.rs`
    use crate::outer::outer_function;

    pub fn test_outer_sibling_unrelated() {
        outer_function(); // Correct
                          // outer::outer_function(); // Error E0433 Incorrect, because we have not imported `crate::outer`
    }
}

mod sibling_unrelated_with_short_import {
    // Correct import using `super` since we are in
    // `fixture_test_crate/src/main.rs`
    use crate::outer; //

    pub fn test_outer_sibling_unrelated() {
        // outer_function(); // Error E0425 since we are not providing full path for function which
        // includes function name
        outer::outer_function(); // Correct
        crate::outer::outer_function() // Also correct
    }
}
