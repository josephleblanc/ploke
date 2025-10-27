//! A new test module to validate file creation and editing capabilities
//! within the ploke fixture crate.

/// A new public struct created via the code creation tool
#[derive(Debug, Clone, PartialEq)]
pub struct NewTestStruct {
    /// A test field with String type
    pub content: String,
    /// A numeric field for testing purposes
    pub priority: u32,
}

impl NewTestStruct {
    /// Create a new instance of NewTestStruct
    pub fn new(content: impl Into<String>, priority: u32) -> Self {
        Self {
            content: content.into(),
            priority,
        }
    }
    
    /// Get the content in uppercase
    pub fn uppercase_content(&self) -> String {
        self.content.to_uppercase()
    }
    
    /// Check if this item has high priority
    pub fn is_high_priority(&self) -> bool {
        self.priority > 50
    }
}

/// A new constant specific to this module
pub const NEW_MODULE_CONSTANT: &str = "hello from new module";

/// A new static item in the module
pub static NEW_MODULE_COUNTER: std::sync::atomic::AtomicU32 = 
    std::sync::atomic::AtomicU32::new(0);

/// A new enum for testing pattern matching
#[derive(Debug)]
pub enum NewTestEnum {
    /// Simple variant with no data
    Empty,
    /// Variant with string data
    WithData(String),
    /// Variant with tuple data
    WithTuple(u32, String),
    /// Nested struct variant
    Complex { 
        label: String, 
        value: i64 
    },
}

impl NewTestEnum {
    /// Get a description of this enum variant
    pub fn describe(&self) -> String {
        match self {
            NewTestEnum::Empty => "Empty variant".to_string(),
            NewTestEnum::WithData(s) => format!("Has data: {}", s),
            NewTestEnum::WithTuple(num, text) => format!("Tuple: {} and {}", num, text),
            NewTestEnum::Complex { label, value } => format!("Complex {}={}", label, value),
        }
    }
}

/// A new trait for testing trait implementation patterns
pub trait NewTestTrait {
    /// Required method that all implementors must provide
    fn process(&self) -> String;
    
    /// Provided method with default implementation
    fn summary(&self) -> &str {
        "default summary"
    }
}

/// Implement the new trait for NewTestStruct
impl NewTestTrait for NewTestStruct {
    fn process(&self) -> String {
        format!("Processing: {} (priority: {})", self.content, self.priority)
    }
    
    fn summary(&self) -> &str {
        if self.is_high_priority() {
            "high priority item"
        } else {
            "regular item"
        }
    }
}

/// Test helper function to demonstrate usage
pub fn demonstrate_new_module() -> (NewTestStruct, NewTestEnum) {
    let counter = NEW_MODULE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    
    let test_struct = NewTestStruct::new(
        format!("Test item #{}", counter),
        75
    );
    
    let test_enum = NewTestEnum::Complex {
        label: "created_via_tool".to_string(),
        value: counter as i64,
    };
    
    (test_struct, test_enum)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_struct_creation() {
        let item = NewTestStruct::new("test", 42);
        assert_eq!(item.content, "test");
        assert_eq!(item.priority, 42);
        assert!(!item.is_high_priority());
    }
    
    #[test]
    fn test_enum_description() {
        let empty = NewTestEnum::Empty;
        assert_eq!(empty.describe(), "Empty variant");
        
        let data = NewTestEnum::WithData("test".to_string());
        assert_eq!(data.describe(), "Has data: test");
    }
    
    #[test]
    fn test_trait_implementation() {
        let test_struct = NewTestStruct::new("hello", 60);
        assert_eq!(test_struct.summary(), "high priority item");
        assert!(test_struct.process().contains("Processing:"));
    }
}