use ploke_db::NodeType;
use ploke_tui::{
    rag::utils::NodeKind,
    tools::{
        Tool, code_edit::GatCodeEdit, code_item_lookup::CodeItemLookup,
        get_code_edges::CodeItemEdges,
    },
};

fn enum_values<'a>(schema: &'a serde_json::Value, path: &[&str]) -> Vec<&'a str> {
    let mut cursor = schema;
    for key in path {
        cursor = cursor.get(*key).expect("schema path");
    }
    cursor
        .get("enum")
        .and_then(|v| v.as_array())
        .expect("enum values")
        .iter()
        .map(|v| v.as_str().expect("string enum value"))
        .collect()
}

#[test]
fn apply_code_edit_node_type_schema_matches_semantic_targets() {
    let schema = GatCodeEdit::schema();
    let values = enum_values(
        schema,
        &["properties", "edits", "items", "properties", "node_type"],
    );
    let expected: Vec<&str> = NodeType::primary_and_assoc_nodes()
        .iter()
        .map(|node_type| node_type.relation_str())
        .collect();

    assert_eq!(values, expected);
    assert!(values.contains(&"method"));
}

#[test]
fn lookup_tool_node_kind_schema_matches_shared_vocabulary() {
    let expected: Vec<&str> = NodeKind::allowed_values().to_vec();

    for schema in [CodeItemLookup::schema(), CodeItemEdges::schema()] {
        let values = enum_values(schema, &["properties", "node_kind"]);
        assert_eq!(values, expected);
        assert!(values.contains(&"method"));
    }
}
