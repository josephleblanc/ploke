use crate::common::*;
use syn_parser::parser::types::VisibilityKind;

#[test]
fn test_enum_parsing() {
    let graph = parse_fixture("enums.rs");

    let sample_enum = find_enum_by_name(&graph, "SampleEnum")
        .expect("SampleEnum not found");

    assert_eq!(sample_enum.name, "SampleEnum");
    assert_eq!(sample_enum.visibility, VisibilityKind::Public);
    assert_eq!(sample_enum.variants.len(), 3);
    assert_eq!(sample_enum.variants[0].name, "Variant1");
    assert_eq!(sample_enum.variants[1].name, "Variant2");
    assert_eq!(sample_enum.variants[2].name, "Variant3");

    if let Some(variant2) = sample_enum.variants.iter().find(|v| v.name == "Variant2") {
        assert_eq!(variant2.fields.len(), 1);
        assert_eq!(variant2.fields[0].name, Some("value".to_string()));
    } else {
        panic!("Variant2 not found");
    }
}
