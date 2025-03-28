// Sample with various struct types

// Regular struct
pub struct SampleStruct {
    pub field: String,
}

// Tuple struct
pub struct TupleStruct(pub i32, pub i32);

// Unit struct
pub struct UnitStruct;

// Generic struct
pub struct GenericStruct<T> {
    pub field: T,
}

// Struct with attributes
#[derive(Debug)]
pub struct AttributedStruct {
    pub field: String,
}

// Struct with docstring
/// This is a documented struct
pub struct DocumentedStruct {
    pub field: String,
}
