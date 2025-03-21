use cozo::ScriptMutability;
use serde_json::Value;
use std::collections::BTreeMap;
use syn::parse_quote;

#[cfg(test)]
mod tests {
    // **Key Areas Needing Validation:**
    //
    // 1. **UUID Namespace Isolation**
    //    - Test that different item types (struct vs fn) with same name get different UUIDs
    //
    // 2. **Relationship Transitivity**
    //    - Verify indirect relationships via multiple hops (impl -> trait -> methods)
    //
    // 3. **Error Recovery**
    //    - Test failed transactions don't leave partial state
    //
    // 4. **Concurrency**
    //    - Multiple visitors writing to same tables (needs Mutex in test)
    use super::*;
    use cozo::{Db, MemStorage};
    use syn::{parse_quote, ItemFn};

    fn test_db() -> Db<MemStorage> {
        let db = Db::new(MemStorage::default()).unwrap();
        db.run_script(
            r#"
            ::create nodes {id: Uuid, kind: String, name: String}
            ::create relations {source: Uuid, target: Uuid, rel_type: String}"#,
            Default::default(),
            ScriptMutability::Mutable,
        )
        .unwrap();
        db
    }

    #[test]
    fn function_and_parameter_relationships() {
        let db = test_db();
        let mut visitor = CodeVisitorV2::new(&db);

        // Parse sample function
        let func: ItemFn = parse_quote! {
            pub fn example(mut x: u32) -> bool { true }
        };

        visitor.visit_item_fn(&func);
        visitor.flush_all();

        // Verify node creation
        let nodes = db.export_relations(["nodes"]).unwrap();
        assert!(nodes.iter().any(|(name, rows)| name == "nodes"
            && rows
                .rows
                .iter()
                .any(|r| r[1].get_str() == "function" && r[2].get_str() == "example")));

        // Verify parameter relationship
        let rels = db.export_relations(["relations"]).unwrap();
        assert!(rels.iter().any(|(_, rows)| rows
            .rows
            .iter()
            .any(|r| r[2].get_str() == "has_param"
                && matches!(&r[3], DataValue::Json(j) => j.0["mutable"] == true))));
    }

    #[test]
    fn uuid_determinism() {
        let func1: ItemFn = parse_quote! { fn foo() {} };
        let func2: ItemFn = parse_quote! { fn foo() {} };

        let id1 = generate_fn_uuid(&func1);
        let id2 = generate_fn_uuid(&func2);

        assert_eq!(id1, id2, "UUIDs differ for identical functions");

        let different: ItemFn = parse_quote! { fn bar() {} };
        let id3 = generate_fn_uuid(&different);
        assert_ne!(id1, id3, "UUIDs match for different functions");
    }

    #[test]
    fn scope_hierarchy_integrity() {
        let db = test_db();
        let mut visitor = CodeVisitorV2::new(&db);

        let nested: File = parse_quote! {
            mod outer {
                fn inner() {
                    struct Nested {}
                }
            }
        };

        visitor.visit_file(&nested);
        visitor.flush_all();

        // Validate parent-child relationships
        let relations = db.export_relations(["relations"]).unwrap();
        let contains_rels = relations["relations"]
            .rows
            .iter()
            .filter(|r| r[2].get_str() == "contains")
            .collect::<Vec<_>>();

        assert_eq!(
            contains_rels.len(),
            3,
            "Should have mod->fn, fn->struct, outer->inner mod relations"
        );
    }
}

// **Critical Test Additions Needed:**
//
// 1. **Temporal Version Rollback**

#[test]
fn temporal_query_isolates_versions() {
    let db = test_db();

    // Initial version
    let mut v1 = CodeVisitorV2::new(&db);
    v1.visit_item_fn(&parse_quote! { fn v1() {} });
    v1.flush_all();

    // Updated version
    let mut v2 = CodeVisitorV2::new(&db);
    v2.visit_item_fn(&parse_quote! { fn v2() {} });
    v2.flush_all();

    // Query historical state
    let result = db
        .run_script(
            "?[id] <- [[@v1_id]] nodes[id, 'function', 'v1']",
            BTreeMap::new(),
            ScriptMutability::Immutable,
        )
        .unwrap();

    assert!(
        !result.rows.is_empty(),
        "Should retrieve previous version using Validity tracking"
    );
}

// 2. **JSON Metadata Validation**
#[test]
fn captures_fn_metadata() {
    let db = test_db();
    let mut visitor = CodeVisitorV2::new(&db);

    visitor.visit_item_fn(&parse_quote! {
        unsafe async fn dangerous() {}
    });
    visitor.flush_all();

    let meta = db
        .run_script(
            "?[meta] := *nodes[_, 'function', 'dangerous', meta]",
            Default::default(),
            ScriptMutability::Immutable,
        )
        .unwrap();

    let meta_json: Value = serde_json::from_str(&meta.rows[0][0].get_json().0.to_string()).unwrap();
    assert_eq!(meta_json["async"], true);
    assert_eq!(meta_json["unsafe"], true);
}

// 3. **Batch Flush Thresholds**
#[test]
fn auto_flush_on_batch_limit() {
    let db = test_db();
    let mut visitor = CodeVisitorV2 {
        db: &db,
        current_scope: Vec::new(),
        batches: BTreeMap::from([("nodes", Vec::with_capacity(2))]),
        batch_size: 2,
    };

    // Add 3 items - should flush twice
    visitor.batch_push("nodes", vec!["id1".into()]);
    visitor.batch_push("nodes", vec!["id2".into()]);
    visitor.batch_push("nodes", vec!["id3".into()]);

    assert_eq!(
        visitor.batches["nodes"].len(),
        1,
        "Should auto-flush at 2, leaving 1 in buffer"
    );
}
