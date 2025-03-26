use std::fs;
use std::path::Path;
use syn::{parse_file, Item};
use crate::common::parse_fixture;

#[test]
fn test_function_spans() {
    let path = Path::new("tests/fixtures/functions.rs");
    let source = fs::read_to_string(path).expect("Failed to read functions fixture");
    let ast = parse_file(&source).expect("Failed to parse functions fixture");

    for item in ast.items {
        if let Item::Fn(item_fn) = item {
            let (start, end) = item_fn.extract_span_bytes();
            let span_text = &source[start..end];
            
            // Verify the span contains the function signature
            assert!(span_text.starts_with("fn "));
            assert!(span_text.contains(&item_fn.sig.ident.to_string()));
            
            // For specific functions, verify exact spans
            match item_fn.sig.ident.to_string().as_str() {
                "regular_function" => {
                    assert_eq!(
                        span_text.trim(),
                        "pub fn regular_function() {\n    println!(\"Regular function\");\n}"
                    );
                },
                "function_with_params" => {
                    assert_eq!(
                        span_text.trim(),
                        "pub fn function_with_params(x: i32, y: i32) -> i32 {\n    x + y\n}"
                    );
                },
                _ => continue,
            }
        }
    }
}

#[test]
fn test_enum_spans() {
    let path = Path::new("tests/fixtures/enums.rs");
    let source = fs::read_to_string(path).expect("Failed to read enums fixture");
    let ast = parse_file(&source).expect("Failed to parse enums fixture");

    for item in ast.items {
        if let Item::Enum(item_enum) = item {
            let (start, end) = item_enum.extract_span_bytes();
            let span_text = &source[start..end];
            
            // Verify the span contains the enum definition
            assert!(span_text.starts_with("pub enum "));
            assert!(span_text.contains(&item_enum.ident.to_string()));
            
            // For specific enums, verify exact spans
            match item_enum.ident.to_string().as_str() {
                "SampleEnum" => {
                    assert_eq!(
                        span_text.trim(),
                        "pub enum SampleEnum {\n    Variant1,\n    Variant2 { value: i32 },\n    Variant3,\n}"
                    );
                },
                "EnumWithData" => {
                    assert_eq!(
                        span_text.trim(),
                        "pub enum EnumWithData {\n    Variant1(i32),\n    Variant2(String),\n}"
                    );
                },
                _ => continue,
            }
        }
    }
}
