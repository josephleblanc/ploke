// Sample with various enum types

// Regular enum
pub enum SampleEnum1 {
    Variant1,
    Variant2,
}

// Enum with data
pub enum EnumWithData {
    Variant1(i32),
    Variant2(String),
}

// Enum with docstring
/// This is a documented enum
pub enum DocumentedEnum {
    Variant1,
    Variant2,
}
// Sample with various enum types

// Regular enum
pub enum SampleEnum {
    Variant1,
    Variant2 { value: i32 },
    Variant3,
}
