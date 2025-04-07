// Base file for testing tracking hash sensitivity.
// Variations will be created manually or by tests later.

pub fn function_one(a: i32) -> i32 {
    // Initial implementation
    a * 2
}

pub struct StructOne {
    pub field_a: String,
    pub field_b: bool,
}

impl StructOne {
    // A method associated with StructOne
    pub fn method_one(&self) -> String {
        self.field_a.clone()
    }
}

// Another function
pub fn function_two() {
    println!("Function two");
}

/*
Variations to create later for testing:

Variation 1 (Whitespace Change):
pub fn function_one( a: i32 ) -> i32 { // Extra spaces
    // Initial implementation

    a * 2

}

Variation 2 (Comment Change):
pub fn function_one(a: i32) -> i32 {
    // Changed comment
    a * 2
}

Variation 3 (Code Change):
pub fn function_one(a: i32) -> i32 {
    // Changed implementation
    a + 10
}

Variation 4 (Struct Field Added):
pub struct StructOne {
    pub field_a: String,
    pub field_b: bool,
    pub field_c: u64, // New field
}

Variation 5 (Method Body Changed):
impl StructOne {
    pub fn method_one(&self) -> String {
        format!("Modified: {}", self.field_a) // Changed body
    }
}

*/
