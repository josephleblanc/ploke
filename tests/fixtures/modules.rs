// Sample with module structure

mod outer {
    // Inner module
    mod inner {
        pub fn inner_function() {
            println!("Inner function");
        }
    }

    // Public function in outer module
    pub fn outer_function() {
        println!("Outer function");
    }
}
