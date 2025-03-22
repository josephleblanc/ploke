#[cfg(test)]
#[cfg(feature = "cozo_visitor")]
mod tests {
    use std::any::Any;
    use std::collections::BTreeMap;

    use cozo::{DataValue, ScriptMutability};
    use cozo::{Db, MemStorage};
    use quote::ToTokens;
    use serde_json::Value;
    use syn::parse_quote;
    use syn::visit::Visit;
    use syn::File;
    use syn::ItemFn;
    use syn_parser::parser::visitor_v2::{generate_fn_uuid, CodeVisitorV2, Set, NODES_KEY};

    pub fn test_db() -> Db<MemStorage> {
        let db = Db::new(MemStorage::default()).unwrap();
        // Can't do multiple lines at once. I think we need to `multiple_transactions` method for
        // that, which is streaming and I don't want to mess with it yet.
        db.run_script(
            r#"
            :create nodes {id: Uuid, kind: String, name: String}
            "#,
            Default::default(),
            ScriptMutability::Mutable,
        )
        .expect("Failed to :create nodes");
        db.run_script(
            r#"
            :create relations {source: Uuid, target: Uuid, rel_type: String}"#,
            Default::default(),
            ScriptMutability::Mutable,
        )
        .expect("Failed to :create relations");
        db.run_script(
            r#"
            :create types {id: Uuid, name: String, is_primitive: Bool }"#,
            Default::default(),
            ScriptMutability::Mutable,
        )
        .expect("Failed to :create types");

        println!("{:#?}", db.export_relations(["relations"].iter()));
        println!("{:#?}", db.export_relations(["nodes"].iter()));
        println!("{:#?}", db.export_relations(["types"].iter()));

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
        let nodes = db.export_relations(["nodes"].iter()).unwrap();
        assert!(nodes.iter().any(|(name, rows)| name == "nodes"
            && rows.rows.iter().any(|r| r[1]
                .get_str()
                .expect("Failure parsing string to function")
                == "function"
                && r[2].get_str().expect("Failure parsing string") == "example")));

        // Verify parameter relationship
        let rels = db.export_relations(["relations"].iter()).unwrap();
        assert!(rels.iter().any(|(_, rows)| rows.rows.iter().any(|r| r[2]
            .get_str()
            .expect("failure parsing string to has_param")
            == "has_param"
            && matches!(&r[3], DataValue::Json(j) if j.0["mutable"] == true))));
    }

    #[test]
    fn uuid_determinism() {
        let func_foo_one: ItemFn = parse_quote! { fn foo() {} };
        let func_foo_two: ItemFn = parse_quote! { fn foo() {} };

        assert_eq!(
            func_foo_one.type_id(),
            func_foo_two.type_id(),
            "TypeId of func_foo_one and func_foo_two mismatch (Should never happen, fix test)"
        );

        let id_foo_one = generate_fn_uuid(&func_foo_one);
        let id_foo_two = generate_fn_uuid(&func_foo_two);

        assert_eq!(
            id_foo_one, id_foo_two,
            "UUIDs differ for identical functions: {:?}, {:?}",
            id_foo_one, id_foo_two
        );

        let different: ItemFn = parse_quote! { fn bar() {} };
        let id_different = generate_fn_uuid(&different);
        assert_ne!(
            id_foo_one, id_different,
            "UUIDs match for different functions"
        );
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

        // TODO: Implement scope management
        // Current test is placeholder - actual scope relationships not yet tracked
        assert!(true, "Scope hierarchy tracking not yet implemented");
    }

    #[test]
    pub fn temporal_query_isolates_versions() {
        let db = test_db();

        // Initial version
        let mut v1 = CodeVisitorV2::new(&db);
        let test_target_one = parse_quote! { fn v1() {} };
        v1.visit_item_fn(&test_target_one);
        v1.flush_all();

        // Updated version
        let mut v2 = CodeVisitorV2::new(&db);
        let test_target_two = parse_quote! { fn v2() {} };
        v2.visit_item_fn(&test_target_two);
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

    // 2. **Batch Flush Thresholds**
    #[test]
    pub fn auto_flush_on_batch_limit() {
        let db = test_db();
        let nodes = Set::Nodes(NODES_KEY);
        let mut visitor = CodeVisitorV2 {
            db: &db,
            current_scope: Vec::new(),
            batches: BTreeMap::from([(nodes, Vec::with_capacity(2))]),
            batch_size: 2,
        };

        // Add 3 items - should flush twice
        visitor.batch_push(nodes, vec!["id1".into()]);
        visitor.batch_push(nodes, vec!["id2".into()]);
        visitor.batch_push(nodes, vec!["id3".into()]);

        assert_eq!(
            visitor.batches[&nodes].len(),
            1,
            "Should auto-flush at 2, leaving 1 in buffer"
        );
    }
    // 1. Test type hierarchy relationships
    #[test]
    fn type_dependencies() {
        let db = test_db();
        let mut visitor = CodeVisitorV2::new(&db);

        let func: ItemFn = parse_quote! {
            fn demo(x: Vec<HashMap<String, u32>>) {}
        };

        visitor.visit_item_fn(&func);
        visitor.flush_all();

        // Should create relationships:
        // Function -> Param (x) -> Vec<T> -> HashMap<K,V> -> String -> u32
        let result = db
            .run_script(
                r#"?[depth] := *relations[from, to, 'contains', depth]
           :order -depth
           :limit 1"#,
                Default::default(),
                ScriptMutability::Mutable,
            )
            .unwrap();

        assert!(result.rows[0][0].get_int().unwrap() >= 4);
    }

    // To verify temporal behavior once implemented (per [stored.rst#L977](source/stored.rst)):
    // #[test]
    // fn temporal_type_evolution() {
    //     let db = test_db();
    //
    //     // Initial version
    //     let code_v1 = "struct User { id: u32 }";
    //     // Process v1 code
    //
    //     // Updated version
    //     let code_v2 = "struct User { uuid: Uuid }";
    //     // Process v2
    //
    //     let query = r#"
    //     ?[current_name, prev_name] :=
    //         nodes@'2024-04-01T00:00:00'[id, 'struct', prev_name],
    //         nodes@'2024-04-02T00:00:00'[id, 'struct', current_name],
    //         prev_name != current_name"#;
    //
    //     let result = db.run_script(query, ...).unwrap();
    //     assert!(result.rows.iter().any(|r|
    //         r[0] == "User" && r[1] == "User" // If names changed
    //     ));
    // }
}
