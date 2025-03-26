use crate::common::{read_byte_range, verify_span};
use std::path::Path;
use syn::{parse_file, Item};
use syn_parser::parser::utils::ExtractSpan;

#[test]
fn test_function_spans() {
    let path = Path::new("tests/fixtures/functions.rs");
    let ast = parse_file(&std::fs::read_to_string(path).unwrap()).unwrap();

    for item in ast.items {
        if let Item::Fn(item_fn) = item {
            let (start, end) = item_fn.extract_span_bytes();
            let span_text = read_byte_range(path, start, end);
            let ident = item_fn.sig.ident.to_string();

            // Verify the span contains the function signature
            assert!(
                span_text.contains(&format!("fn {}", ident)),
                "Function span for '{}' should contain 'fn {}' in:\n{}",
                ident,
                ident,
                span_text
            );

            // For specific functions, verify exact spans
            match ident.as_str() {
                "regular_function" => {
                    verify_span(
                        &item_fn,
                        path,
                        "pub fn regular_function() {\n    println!(\"Regular function\");\n}",
                    );
                }
                "function_with_params" => {
                    verify_span(
                        &item_fn,
                        path,
                        "pub fn function_with_params(x: i32, y: i32) -> i32 {\n    x + y\n}",
                    );
                }
                _ => continue,
            }
        }
    }
}

#[test]
fn test_enum_spans() {
    let path = Path::new("tests/fixtures/enums.rs");
    let ast = parse_file(&std::fs::read_to_string(path).unwrap()).unwrap();

    for item in ast.items {
        if let Item::Enum(item_enum) = item {
            let (start, end) = item_enum.extract_span_bytes();
            let span_text = read_byte_range(path, start, end);
            let ident = item_enum.ident.to_string();

            // Verify the span contains the enum definition
            assert!(
                span_text.contains(&format!("enum {}", ident)),
                "Enum span for '{}' should contain 'enum {}' in:\n{}",
                ident,
                ident,
                span_text
            );

            // For specific enums, verify exact spans
            match ident.as_str() {
                "SampleEnum" => {
                    verify_span(
                        &item_enum,
                        path,
                        "pub enum SampleEnum {\n    Variant1,\n    Variant2 { value: i32 },\n    Variant3,\n}",
                    );
                }
                "EnumWithData" => {
                    verify_span(
                        &item_enum,
                        path,
                        "pub enum EnumWithData {\n    Variant1(i32),\n    Variant2(String),\n}",
                    );
                }
                _ => continue,
            }
        }
    }
}
