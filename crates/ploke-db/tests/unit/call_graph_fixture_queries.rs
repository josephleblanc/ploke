use std::collections::BTreeMap;

use cozo::{DataValue, Db, MemStorage, UuidWrapper};
use ploke_db::{
    CallCallerRow, CallContextCandidate, CallContextOptions, CallContextRelation, CallContextRow,
    CallContextSeed, CallReceiver, CallRelationKind, CallResolutionKind, CallSiteKind,
    CallStatusKind, Database, DbError, ProofCheckerEdgeRow, ProofGraphContextRow, ProofGraphStore,
    ProofInvariantStatus, QueryResult, to_uuid,
};
use ploke_transform::{schema::create_schema_all, transform::transform_parsed_graph};
use uuid::Uuid;

#[test]
fn fixture_context_reads_projected_path_call() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_crate_local_target")?;
    let target = function_id_by_name(&db, "local_target")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(
        row.site.path.as_ref(),
        Some(&path(&["crate", "local_target"]))
    );
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_eq!(row.status.status, CallStatusKind::Resolved);
    assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(row.targets.len(), 1);
    assert_eq!(row.targets[0].target_id, target);
    assert_eq!(row.targets[0].relation, CallRelationKind::Function);
    assert_eq!(row.targets[0].source_kind, CallSiteKind::Path);
    assert_eq!(row.targets[0].target_kind, CallRelationKind::Function);

    Ok(())
}

#[test]
fn fixture_projected_call_sites_have_one_matching_body_edge() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let sites = db.raw_query(
        r#"?[id, owner_id, call_kind] :=
            *call_site { id, owner_id, call_kind @ 'NOW' }"#,
    )?;
    assert!(
        !sites.rows.is_empty(),
        "fixture should project call_site rows"
    );

    for site in &sites.rows {
        let site_id = to_uuid(&site[0])?;
        let owner = to_uuid(&site[1])?;
        let call_kind = data_str(&site[2], "call_site.call_kind");
        let owner_kind = owner_kind_for_call_body_owner(&db, owner)?;
        let edges = body_edges_for_site(&db, site_id)?;

        assert_eq!(
            edges.rows.len(),
            1,
            "call_site {site_id} should have exactly one BodyContainsCall edge; rows: {:#?}",
            edges.rows
        );
        let edge = &edges.rows[0];
        assert_eq!(
            to_uuid(&edge[0])?,
            owner,
            "BodyContainsCall source should match call_site.owner_id for {site_id}"
        );
        assert_eq!(
            to_uuid(&edge[1])?,
            site_id,
            "BodyContainsCall target should match call_site.id for {site_id}"
        );
        assert_eq!(
            data_str(&edge[2], "call_site_edge.source_kind"),
            owner_kind,
            "BodyContainsCall source_kind should match owner family for call_site {site_id}"
        );
        assert_eq!(
            data_str(&edge[3], "call_site_edge.target_kind"),
            call_kind,
            "BodyContainsCall target_kind should match call_site.call_kind for {site_id}"
        );
    }

    Ok(())
}

#[test]
fn fixture_projected_call_site_ids_do_not_overlap_node_or_type_ids() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let overlaps = db.raw_query(
        r#"?[call_site_id, relation] :=
            *call_site { id: call_site_id @ 'NOW' },
            stored_non_call_id[call_site_id, relation]

        stored_non_call_id[id, relation] := *function { id @ 'NOW' }, relation = "function"
        stored_non_call_id[id, relation] := *method { id @ 'NOW' }, relation = "method"
        stored_non_call_id[id, relation] := *const { id @ 'NOW' }, relation = "const"
        stored_non_call_id[id, relation] := *static { id @ 'NOW' }, relation = "static"
        stored_non_call_id[id, relation] := *struct { id @ 'NOW' }, relation = "struct"
        stored_non_call_id[id, relation] := *enum { id @ 'NOW' }, relation = "enum"
        stored_non_call_id[id, relation] := *variant { id @ 'NOW' }, relation = "variant"
        stored_non_call_id[id, relation] := *trait { id @ 'NOW' }, relation = "trait"
        stored_non_call_id[id, relation] := *impl { id @ 'NOW' }, relation = "impl"
        stored_non_call_id[id, relation] := *type_alias { id @ 'NOW' }, relation = "type_alias"
        stored_non_call_id[id, relation] := *union { id @ 'NOW' }, relation = "union"
        stored_non_call_id[id, relation] := *module { id @ 'NOW' }, relation = "module"
        stored_non_call_id[id, relation] := *import { id @ 'NOW' }, relation = "import"
        stored_non_call_id[id, relation] := *macro { id @ 'NOW' }, relation = "macro"
        stored_non_call_id[id, relation] := *field { id @ 'NOW' }, relation = "field"
        stored_non_call_id[id, relation] := *generic_type { id @ 'NOW' }, relation = "generic_type"
        stored_non_call_id[id, relation] := *generic_lifetime { id @ 'NOW' }, relation = "generic_lifetime"
        stored_non_call_id[id, relation] := *generic_const { id @ 'NOW' }, relation = "generic_const"
        stored_non_call_id[id, relation] := *file_mod { owner_id: id @ 'NOW' }, relation = "file_mod"
        stored_non_call_id[id, relation] := *type_use { id @ 'NOW' }, relation = "type_use"
        stored_non_call_id[id, relation] := *named_type { type_id: id @ 'NOW' }, relation = "named_type"
        stored_non_call_id[id, relation] := *reference_type { type_id: id @ 'NOW' }, relation = "reference_type"
        stored_non_call_id[id, relation] := *slice_type { type_id: id @ 'NOW' }, relation = "slice_type"
        stored_non_call_id[id, relation] := *array_type { type_id: id @ 'NOW' }, relation = "array_type"
        stored_non_call_id[id, relation] := *tuple_type { type_id: id @ 'NOW' }, relation = "tuple_type"
        stored_non_call_id[id, relation] := *function_type { type_id: id @ 'NOW' }, relation = "function_type"
        stored_non_call_id[id, relation] := *never_type { type_id: id @ 'NOW' }, relation = "never_type"
        stored_non_call_id[id, relation] := *inferred_type { type_id: id @ 'NOW' }, relation = "inferred_type"
        stored_non_call_id[id, relation] := *raw_pointer_type { type_id: id @ 'NOW' }, relation = "raw_pointer_type"
        stored_non_call_id[id, relation] := *trait_object_type { type_id: id @ 'NOW' }, relation = "trait_object_type"
        stored_non_call_id[id, relation] := *impl_trait_type { type_id: id @ 'NOW' }, relation = "impl_trait_type"
        stored_non_call_id[id, relation] := *trait_bound_type { type_id: id @ 'NOW' }, relation = "trait_bound_type"
        stored_non_call_id[id, relation] := *paren_type { type_id: id @ 'NOW' }, relation = "paren_type"
        stored_non_call_id[id, relation] := *macro_type { type_id: id @ 'NOW' }, relation = "macro_type"
        stored_non_call_id[id, relation] := *unknown_type { type_id: id @ 'NOW' }, relation = "unknown_type""#,
    )?;
    assert!(
        overlaps.rows.is_empty(),
        "call_site ids must remain in the CallId universe; overlaps: {:#?}",
        overlaps.rows
    );

    Ok(())
}

#[test]
fn fixture_projected_call_relations_and_statuses_anchor_to_call_sites() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let sites = db.raw_query(
        r#"?[id, call_kind] :=
            *call_site { id, call_kind @ 'NOW' }"#,
    )?;
    assert!(
        !sites.rows.is_empty(),
        "fixture should project call_site rows"
    );

    for site in &sites.rows {
        let site_id = to_uuid(&site[0])?;
        let call_kind = data_str(&site[1], "call_site.call_kind");
        let statuses = statuses_for_site(&db, site_id)?;
        assert_eq!(
            statuses.rows.len(),
            1,
            "call_site {site_id} should have exactly one call_resolution_status row; rows: {:#?}",
            statuses.rows
        );
        assert_eq!(
            data_str(&statuses.rows[0][0], "call_resolution_status.source_kind"),
            call_kind,
            "call_resolution_status source_kind should match call_site.call_kind for {site_id}"
        );
    }

    let relations = db.raw_query(
        r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
            *call_relation { source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW' }"#,
    )?;
    assert!(
        !relations.rows.is_empty(),
        "fixture should project resolved call_relation rows"
    );

    for row in &relations.rows {
        let site_id = to_uuid(&row[0])?;
        let target = to_uuid(&row[1])?;
        let relation = data_str(&row[2], "call_relation.relation_kind");
        let source = data_str(&row[3], "call_relation.source_kind");
        let target_kind = data_str(&row[4], "call_relation.target_kind");
        let site_kind = call_site_kind_for_site(&db, site_id)?;

        assert_eq!(
            source, site_kind,
            "call_relation source_kind should match call_site.call_kind for {site_id}"
        );
        assert!(
            is_valid_call_relation_family(relation, source, target_kind),
            "unexpected call_relation endpoint family for {site_id} -> {target}: {relation}/{source}/{target_kind}"
        );
        assert!(
            call_target_exists(&db, target, target_kind)?,
            "call_relation target {target} should exist in {target_kind} endpoint relation"
        );
    }

    Ok(())
}

#[test]
fn fixture_projected_call_edge_and_status_rows_have_existing_anchors() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let edges = db.raw_query(
        r#"?[source_id, target_id, source_kind, target_kind] :=
            *call_site_edge {
                source_id,
                target_id,
                relation_kind: "BodyContainsCall",
                source_kind,
                target_kind @ 'NOW'
            }"#,
    )?;
    assert!(
        !edges.rows.is_empty(),
        "fixture should project BodyContainsCall rows"
    );

    for edge in &edges.rows {
        let owner = to_uuid(&edge[0])?;
        let site_id = to_uuid(&edge[1])?;
        let source = data_str(&edge[2], "call_site_edge.source_kind");
        let target = data_str(&edge[3], "call_site_edge.target_kind");
        let owner_kind = owner_kind_for_call_body_owner(&db, owner)?;
        let (site_owner, call_kind) = call_site_owner_and_kind(&db, site_id)?;

        assert_eq!(
            site_owner, owner,
            "BodyContainsCall target {site_id} should be owned by source {owner}"
        );
        assert_eq!(
            source, owner_kind,
            "BodyContainsCall source_kind should match owner family for edge {owner} -> {site_id}"
        );
        assert_eq!(
            target, call_kind,
            "BodyContainsCall target_kind should match call_site.call_kind for {site_id}"
        );
    }

    let statuses = db.raw_query(
        r#"?[source_id, source_kind, status_kind, resolution_kind] :=
            *call_resolution_status {
                source_id,
                source_kind,
                status_kind,
                resolution_kind @ 'NOW'
            }"#,
    )?;
    assert!(
        !statuses.rows.is_empty(),
        "fixture should project call_resolution_status rows"
    );

    for status in &statuses.rows {
        let site_id = to_uuid(&status[0])?;
        let source = data_str(&status[1], "call_resolution_status.source_kind");
        let status_kind = data_str(&status[2], "call_resolution_status.status_kind");
        let resolution = optional_data_str(&status[3], "call_resolution_status.resolution_kind");
        let (_owner, call_kind) = call_site_owner_and_kind(&db, site_id)?;

        assert_eq!(
            source, call_kind,
            "call_resolution_status source_kind should match call_site.call_kind for {site_id}"
        );
        assert_valid_status_shape(site_id, status_kind, resolution);
    }

    Ok(())
}

#[test]
fn fixture_projected_status_rows_match_relation_cardinality() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let sites = db.raw_query(
        r#"?[id, call_kind] :=
            *call_site { id, call_kind @ 'NOW' }"#,
    )?;
    assert!(
        !sites.rows.is_empty(),
        "fixture should project call_site rows"
    );

    let mut resolved_count = 0;
    let mut targetless_count = 0;

    for site in &sites.rows {
        let site_id = to_uuid(&site[0])?;
        let call_kind = data_str(&site[1], "call_site.call_kind");
        let statuses = statuses_for_site(&db, site_id)?;
        assert_eq!(
            statuses.rows.len(),
            1,
            "call_site {site_id} should have exactly one call_resolution_status row; rows: {:#?}",
            statuses.rows
        );

        let status = &statuses.rows[0];
        let source = data_str(&status[0], "call_resolution_status.source_kind");
        let status_kind = data_str(&status[1], "call_resolution_status.status_kind");
        let resolution = optional_data_str(&status[2], "call_resolution_status.resolution_kind");
        let relations = relations_for_site(&db, site_id)?;

        assert_eq!(
            source, call_kind,
            "call_resolution_status source_kind should match call_site.call_kind for {site_id}"
        );
        assert_valid_status_shape(site_id, status_kind, resolution);

        match status_kind {
            "Resolved" => {
                resolved_count += 1;
                assert_eq!(
                    relations.rows.len(),
                    1,
                    "resolved call_site {site_id} should have exactly one call_relation row; rows: {:#?}",
                    relations.rows
                );
            }
            "Unresolved" | "Ambiguous" | "External" | "Unsupported" => {
                targetless_count += 1;
                assert!(
                    relations.rows.is_empty(),
                    "non-resolved call_site {site_id} must not have call_relation rows; rows: {:#?}",
                    relations.rows
                );
            }
            other => panic!("unexpected call status kind {other} for {site_id}"),
        }
    }

    assert!(
        resolved_count > 0,
        "fixture should include resolved call sites"
    );
    assert!(
        targetless_count > 0,
        "fixture should include non-resolved call sites"
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_path_resolution_forms() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let local_target = function_id_by_name(&db, "local_target")?;
    let nested_target =
        function_id_by_name_in_module(&db, &["crate", "local_mod"], "nested_target")?;
    let imported_target =
        function_id_by_name_in_module(&db, &["crate", "import_targets"], "imported_target")?;
    let globbed_target =
        function_id_by_name_in_module(&db, &["crate", "import_targets"], "globbed_target")?;
    let cases: [(&[&str], &str, Vec<String>, Uuid); 11] = [
        (
            &["crate"],
            "call_unqualified_local_target",
            path(&["local_target"]),
            local_target,
        ),
        (
            &["crate", "local_mod"],
            "call_self_nested_target",
            path(&["self", "nested_target"]),
            nested_target,
        ),
        (
            &["crate"],
            "call_crate_module_nested_target",
            path(&["crate", "local_mod", "nested_target"]),
            nested_target,
        ),
        (
            &["crate"],
            "call_self_module_nested_target",
            path(&["self", "local_mod", "nested_target"]),
            nested_target,
        ),
        (
            &["crate", "super_path_scope"],
            "call_super_local_target",
            path(&["super", "local_target"]),
            local_target,
        ),
        (
            &["crate"],
            "call_imported_alias_target",
            path(&["imported_alias"]),
            imported_target,
        ),
        (
            &["crate"],
            "call_glob_imported_target",
            path(&["globbed_target"]),
            globbed_target,
        ),
        (
            &["crate"],
            "call_reexported_target",
            path(&["reexported_target"]),
            imported_target,
        ),
        (
            &["crate"],
            "call_imported_module_target",
            path(&["targets_alias", "globbed_target"]),
            globbed_target,
        ),
        (
            &["crate", "grouped_function_import_scope"],
            "call_grouped_imported_alias_target",
            path(&["grouped_alias"]),
            imported_target,
        ),
        (
            &["crate", "grouped_function_import_scope"],
            "call_grouped_imported_globbed_target",
            path(&["grouped_globbed_alias"]),
            globbed_target,
        ),
    ];

    for (module_path, owner_name, expected_path, target) in cases {
        let owner = function_id_by_name_in_module(&db, module_path, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Path);
        assert_eq!(row.site.path.as_ref(), Some(&expected_path));
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.site.generic_arg_count, Some(0));
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallRelationKind::Function,
        );
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_typed_local_method_call() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_typed_local_instance_method")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(row.site.method.as_deref(), Some("instance_value"));
    assert_eq!(
        row.site.receiver,
        Some(CallReceiver::TypedLocalBinding {
            name: "value".to_string(),
            type_path: path(&["LocalAssoc"]),
        })
    );
    assert_eq!(row.status.status, CallStatusKind::Resolved);
    assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(row.targets.len(), 1);
    assert_eq!(row.targets[0].relation, CallRelationKind::Method);
    assert_eq!(row.targets[0].source_kind, CallSiteKind::Method);
    assert_eq!(row.targets[0].target_kind, CallRelationKind::Method);

    Ok(())
}

#[test]
fn fixture_context_reads_projected_local_and_alias_instance_method_receivers() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let cases = [
        (
            "call_param_instance_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_initialized_local_instance_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_parenthesized_typed_local_instance_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_type_alias_chain_instance_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssocAliasChain"]),
            },
        ),
        (
            "call_imported_type_alias_instance_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["ImportedLocalAssocAlias"]),
            },
        ),
    ];

    for (owner_name, receiver) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = row_by_method_receiver(&context, "instance_value", &receiver);
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.arg_count, Some(0));
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Method,
            CallSiteKind::Method,
            CallRelationKind::Method,
        );
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_associated_function_call() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_local_assoc_make")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "make")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["LocalAssoc", "make"])));
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.status.status, CallStatusKind::Resolved);
    assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(row.targets.len(), 1);
    assert_eq!(row.targets[0].target_id, target);
    assert_eq!(
        row.targets[0].relation,
        CallRelationKind::AssociatedFunction
    );
    assert_eq!(row.targets[0].source_kind, CallSiteKind::Path);
    assert_eq!(row.targets[0].target_kind, CallRelationKind::Method);

    Ok(())
}

#[test]
fn fixture_context_reads_projected_self_and_qualified_associated_function_calls()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "make")?;
    let cases = [
        (
            method_id_by_impl_self_type_name(&db, "LocalAssoc", "call_self_make")?,
            path(&["Self", "make"]),
        ),
        (
            function_id_by_name(&db, "call_qualified_local_assoc_make")?,
            path(&["LocalAssoc", "make"]),
        ),
    ];

    for (owner, expected_path) in cases {
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Path);
        assert_eq!(row.site.path.as_ref(), Some(&expected_path));
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.site.generic_arg_count, Some(0));
        assert_resolved_target(
            row,
            target,
            CallRelationKind::AssociatedFunction,
            CallSiteKind::Path,
            CallRelationKind::Method,
        );
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_imported_associated_function_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "ImportedAssoc", "make")?;
    let cases = [
        (
            "call_imported_type_assoc_make",
            path(&["ImportedAssocAlias", "make"]),
        ),
        (
            "call_glob_imported_type_assoc_make",
            path(&["ImportedAssoc", "make"]),
        ),
        (
            "call_reexported_type_assoc_make",
            path(&["ReexportedAssoc", "make"]),
        ),
    ];

    for (owner_name, expected_path) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Path);
        assert_eq!(row.site.path.as_ref(), Some(&expected_path));
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.status.status, CallStatusKind::Resolved);
        assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(row.targets.len(), 1);
        assert_eq!(row.targets[0].target_id, target);
        assert_eq!(
            row.targets[0].relation,
            CallRelationKind::AssociatedFunction
        );
        assert_eq!(row.targets[0].source_kind, CallSiteKind::Path);
        assert_eq!(row.targets[0].target_kind, CallRelationKind::Method);
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_type_alias_associated_function_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let make_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "make")?;
    let assoc_cases = [
        (
            "call_type_alias_assoc_make",
            path(&["LocalAssocTypeAlias", "make"]),
        ),
        (
            "call_type_alias_chain_assoc_make",
            path(&["LocalAssocAliasChain", "make"]),
        ),
        (
            "call_imported_type_alias_assoc_make",
            path(&["ImportedLocalAssocAlias", "make"]),
        ),
    ];

    for (owner_name, expected_path) in assoc_cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Path);
        assert_eq!(row.site.path.as_ref(), Some(&expected_path));
        assert_eq!(row.site.arg_count, Some(0));
        assert_resolved_target(
            row,
            make_target,
            CallRelationKind::AssociatedFunction,
            CallSiteKind::Path,
            CallRelationKind::Method,
        );
    }

    let owner = function_id_by_name(&db, "call_method_as_associated_function")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "method-as-associated context rows: {context:#?}"
    );

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(
        row.site.path.as_ref(),
        Some(&path(&["LocalAssoc", "instance_value"]))
    );
    assert_eq!(row.site.arg_count, Some(1));
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::AssociatedFunction,
        CallSiteKind::Path,
        CallRelationKind::Method,
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_imported_trait_associated_function_calls() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_trait_name(&db, "LocalAssocFunctionTrait", "trait_make")?;
    let owner = function_id_by_name(&db, "call_trait_associated_function")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "local trait associated function context rows: {context:#?}"
    );

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(
        row.site.path.as_ref(),
        Some(&path(&["LocalAssocFunctionTrait", "trait_make"]))
    );
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_resolved_target(
        row,
        target,
        CallRelationKind::AssociatedFunction,
        CallSiteKind::Path,
        CallRelationKind::Method,
    );

    let target = method_id_by_trait_name(&db, "ImportedAssocFunctionTrait", "imported_trait_make")?;
    let cases = [
        (
            path(&["crate", "trait_assoc_function_scope", "with_direct_import"]),
            "call_direct_imported_trait_associated_function",
            path(&["ImportedAssocFunctionTrait", "imported_trait_make"]),
        ),
        (
            path(&["crate", "trait_assoc_function_scope", "with_alias_import"]),
            "call_alias_imported_trait_associated_function",
            path(&["VisibleAssocFunctionTrait", "imported_trait_make"]),
        ),
        (
            path(&["crate", "trait_assoc_function_scope", "with_glob_import"]),
            "call_glob_imported_trait_associated_function",
            path(&["ImportedAssocFunctionTrait", "imported_trait_make"]),
        ),
        (
            path(&["crate", "trait_assoc_reexport_scope"]),
            "call_reexported_trait_associated_function",
            path(&["ReexportedAssocFunctionTrait", "imported_trait_make"]),
        ),
        (
            path(&["crate", "grouped_trait_assoc_function_scope"]),
            "call_grouped_imported_trait_associated_function",
            path(&["GroupedAssocFunctionTrait", "imported_trait_make"]),
        ),
    ];

    for (module_path, owner_name, expected_path) in cases {
        let refs = module_path.iter().map(String::as_str).collect::<Vec<_>>();
        let owner = function_id_by_name_in_module(&db, &refs, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Path);
        assert_eq!(row.site.path.as_ref(), Some(&expected_path));
        assert_eq!(row.status.status, CallStatusKind::Resolved);
        assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(row.targets.len(), 1);
        assert_eq!(row.targets[0].target_id, target);
        assert_eq!(
            row.targets[0].relation,
            CallRelationKind::AssociatedFunction
        );
        assert_eq!(row.targets[0].source_kind, CallSiteKind::Path);
        assert_eq!(row.targets[0].target_kind, CallRelationKind::Method);
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_function_item_binding_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let local_target = function_id_by_name(&db, "local_target")?;
    let imported_target =
        function_id_by_name_in_module(&db, &["crate", "import_targets"], "imported_target")?;
    let cases = [
        (
            "call_local_function_item_binding",
            path(&["f"]),
            local_target,
        ),
        (
            "call_aliased_function_item_binding",
            path(&["g"]),
            local_target,
        ),
        (
            "call_typed_function_pointer_binding",
            path(&["f"]),
            local_target,
        ),
        (
            "call_typed_function_pointer_alias_binding",
            path(&["g"]),
            local_target,
        ),
        (
            "call_imported_function_item_binding",
            path(&["f"]),
            imported_target,
        ),
    ];

    for (owner_name, expected_path, target) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Path);
        assert_eq!(row.site.path.as_ref(), Some(&expected_path));
        assert_eq!(row.site.arg_count, Some(0));
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallRelationKind::Function,
        );
    }

    let owner = function_id_by_name(&db, "call_shadowed_local_target_binding")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "shadowed binding context rows: {context:#?}"
    );

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["local_target"])));
    assert_eq!(row.status.status, CallStatusKind::Unsupported);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "shadowed closure binding must not fabricate local function edges: {row:#?}"
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_tuple_struct_constructor_call() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_new_type_constructor")?;
    let target = struct_id_by_name(&db, "NewType")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["NewType"])));
    assert_eq!(row.site.arg_count, Some(1));
    assert_eq!(row.status.status, CallStatusKind::Resolved);
    assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(row.targets.len(), 1);
    assert_eq!(row.targets[0].target_id, target);
    assert_eq!(
        row.targets[0].relation,
        CallRelationKind::TupleStructConstructor
    );
    assert_eq!(row.targets[0].source_kind, CallSiteKind::Path);
    assert_eq!(row.targets[0].target_kind, CallRelationKind::Struct);

    Ok(())
}

#[test]
fn fixture_context_reads_projected_dynamic_function_call() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_parenthesized_local_target")?;
    let target = function_id_by_name(&db, "local_target")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.kind, CallSiteKind::Dynamic);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["local_target"])));
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, None);
    assert_eq!(row.status.status, CallStatusKind::Resolved);
    assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(row.targets.len(), 1);
    assert_eq!(row.targets[0].target_id, target);
    assert_eq!(row.targets[0].relation, CallRelationKind::DynamicFunction);
    assert_eq!(row.targets[0].source_kind, CallSiteKind::Dynamic);
    assert_eq!(row.targets[0].target_kind, CallRelationKind::Function);

    Ok(())
}

#[test]
fn fixture_context_reads_projected_parenthesized_binding_dynamic_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let cases = [
        ("call_parenthesized_function_item_binding", &["f"][..]),
        (
            "call_parenthesized_aliased_function_item_binding",
            &["g"][..],
        ),
        (
            "call_parenthesized_typed_function_pointer_alias_binding",
            &["g"][..],
        ),
    ];

    for (owner_name, expected_path) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = row_by_kind_path(&context, CallSiteKind::Dynamic, expected_path);
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.site.generic_arg_count, None);
        assert_eq!(row.site.receiver, None);
        assert_resolved_target(
            row,
            target,
            CallRelationKind::DynamicFunction,
            CallSiteKind::Dynamic,
            CallRelationKind::Function,
        );
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_returned_function_nested_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_returned_function")?;
    let target = function_id_by_name(&db, "make_fn")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "returned function context rows: {context:#?}"
    );

    let row = row_by_path(&context, &["make_fn"]);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallRelationKind::Function,
    );

    let dynamic_rows = context
        .iter()
        .filter(|row| row.site.kind == CallSiteKind::Dynamic)
        .collect::<Vec<_>>();
    assert_eq!(
        dynamic_rows.len(),
        1,
        "expected one outer returned-function dynamic call row: {context:#?}"
    );
    let row = dynamic_rows[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, None);
    assert_eq!(row.status.status, CallStatusKind::Unsupported);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "returned-function dynamic call must remain unsupported without fake targets: {row:#?}"
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_resolved_dynamic_function_shapes() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let cases = [
        (
            "call_function_pointer_cast_path",
            path(&["local_target"]),
            1,
        ),
        ("call_function_pointer_cast_binding", path(&["f"]), 1),
        (
            "call_dereferenced_function_pointer_binding",
            path(&["f"]),
            1,
        ),
        ("call_block_function_item", path(&["local_target"]), 1),
        ("call_if_same_function_item", path(&["local_target"]), 1),
        ("call_match_same_function_item", path(&["local_target"]), 1),
        (
            "call_named_field_function_binding",
            path(&["holder", "callback"]),
            1,
        ),
        (
            "call_aliased_named_field_function_binding",
            path(&["alias", "callback"]),
            1,
        ),
        (
            "call_indexed_named_field_function_binding",
            path(&["holder", "callbacks", "0"]),
            1,
        ),
        (
            "call_indexed_named_field_array_alias_binding",
            path(&["holder", "callbacks", "0"]),
            1,
        ),
        (
            "call_aliased_indexed_named_field_function_binding",
            path(&["alias", "callbacks", "0"]),
            1,
        ),
        (
            "call_indexed_tuple_field_function_binding",
            path(&["holder", "0", "0"]),
            2,
        ),
        (
            "call_indexed_tuple_field_array_alias_binding",
            path(&["holder", "0", "0"]),
            2,
        ),
        (
            "call_aliased_indexed_tuple_field_function_binding",
            path(&["alias", "0", "0"]),
            2,
        ),
        (
            "call_indexed_initialized_function_array",
            path(&["funcs", "0"]),
            1,
        ),
        (
            "call_typed_indexed_initialized_function_array",
            path(&["funcs", "0"]),
            1,
        ),
        (
            "call_aliased_indexed_initialized_function_array",
            path(&["alias", "0"]),
            1,
        ),
    ];

    for (owner_name, expected_path, expected_rows) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let refs = expected_path.iter().map(String::as_str).collect::<Vec<_>>();
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(
            context.len(),
            expected_rows,
            "{owner_name} context rows: {context:#?}"
        );

        let row = row_by_kind_path(&context, CallSiteKind::Dynamic, &refs);
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.site.generic_arg_count, None);
        assert_eq!(row.site.receiver, None);
        assert_resolved_target(
            row,
            target,
            CallRelationKind::DynamicFunction,
            CallSiteKind::Dynamic,
            CallRelationKind::Function,
        );
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_targetless_dynamic_failures() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let cases = [
        (
            "call_if_ambiguous_function_item",
            None,
            CallStatusKind::Ambiguous,
        ),
        (
            "call_match_ambiguous_function_item",
            None,
            CallStatusKind::Ambiguous,
        ),
        (
            "call_match_guarded_function_item",
            None,
            CallStatusKind::Unsupported,
        ),
        ("call_if_closure_branch", None, CallStatusKind::Unsupported),
        ("call_match_closure_arm", None, CallStatusKind::Unsupported),
        (
            "call_parenthesized_function_pointer_param",
            Some(path(&["f"])),
            CallStatusKind::Unsupported,
        ),
        (
            "call_if_function_pointer_param_branch",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_match_function_pointer_param_arm",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_if_nested_branch_expression",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_match_nested_arm_expression",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_closure_binding_cast",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_dereferenced_closure_binding",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_field_function_param",
            Some(path(&["holder", "callback"])),
            CallStatusKind::Unsupported,
        ),
        (
            "call_indexed_function_pointer",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_indexed_field_function_param",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_indexed_tuple_field_function_param",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_move_closure_literal_with_body_call",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_async_closure_literal_with_body_call",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_parenthesized_generic_fn_once_value_binding",
            Some(path(&["generic_f"])),
            CallStatusKind::Unsupported,
        ),
    ];

    for (owner_name, expected_path, expected_status) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Dynamic);
        assert_eq!(row.site.path, expected_path);
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.site.generic_arg_count, None);
        assert_eq!(row.status.status, expected_status);
        assert_eq!(row.status.resolution, None);
        assert!(
            row.targets.is_empty(),
            "{owner_name} dynamic failure must not fabricate targets: {row:#?}"
        );
    }

    let owner = function_id_by_name(&db, "call_parenthesized_boxed_dyn_fn_value_binding")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "boxed dyn Fn context rows: {context:#?}");

    let row = row_by_path(&context, &["Box", "new"]);
    assert_eq!(row.status.status, CallStatusKind::External);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "Box::new must not fabricate local target edges: {row:#?}"
    );

    let row = row_by_kind_path(&context, CallSiteKind::Dynamic, &["boxed_fn"]);
    assert_eq!(row.status.status, CallStatusKind::Unsupported);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "boxed dyn Fn dynamic call must remain unsupported without fake targets: {row:#?}"
    );

    Ok(())
}

#[test]
fn fixture_context_does_not_project_closure_or_async_body_calls_to_outer_owner()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let local_target = function_id_by_name(&db, "local_target")?;
    let forbidden_path = path(&["local_target"]);
    let owners = [
        "closure_body_call_is_not_outer_call_site",
        "async_block_call_is_not_outer_call_site",
        "call_move_closure_literal_with_body_call",
        "call_async_closure_literal_with_body_call",
    ];

    for owner_name in owners {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert!(
            context.iter().all(|row| row.site.kind != CallSiteKind::Path
                || row.site.path.as_ref() != Some(&forbidden_path)),
            "{owner_name} leaked a closure/async body local_target() path row into the outer owner: {context:#?}"
        );
        assert!(
            context
                .iter()
                .flat_map(|row| row.targets.iter())
                .all(|target| target.target_id != local_target),
            "{owner_name} leaked a closure/async body edge to local_target into the outer owner: {context:#?}"
        );
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_callable_value_path_failures_and_vec_external()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let path_failures = [
        ("call_function_pointer_param", &["f"][..]),
        ("call_generic_fn_once_value_binding", &["generic_f"][..]),
    ];

    for (owner_name, expected_path) in path_failures {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = row_by_path(&context, expected_path);
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.site.generic_arg_count, Some(0));
        assert_eq!(row.status.status, CallStatusKind::Unsupported);
        assert_eq!(row.status.resolution, None);
        assert!(
            row.targets.is_empty(),
            "callable value path row must not fabricate local targets: {row:#?}"
        );
    }

    let owner = function_id_by_name(&db, "call_boxed_dyn_fn_value_binding")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "boxed dyn Fn path context rows: {context:#?}"
    );

    let row = row_by_path(&context, &["Box", "new"]);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(1));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_eq!(row.status.status, CallStatusKind::External);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "Box::new setup call must not fabricate local targets: {row:#?}"
    );

    let row = row_by_path(&context, &["boxed_fn"]);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_eq!(row.status.status, CallStatusKind::Unsupported);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "boxed dyn Fn path call must not fabricate local targets: {row:#?}"
    );

    let owner = function_id_by_name(&db, "call_prelude_vec_new")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "Vec::new context rows: {context:#?}");
    let row = row_by_path(&context, &["Vec", "new"]);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_eq!(row.status.status, CallStatusKind::External);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "Vec::new external row must not fabricate local targets: {row:#?}"
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_generic_unsafe_extern_and_chained_calls() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    let owner = function_id_by_name(&db, "call_generic_identity_turbofish")?;
    let target = function_id_by_name(&db, "generic_identity")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "generic function context rows: {context:#?}"
    );
    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["generic_identity"])));
    assert_eq!(row.site.arg_count, Some(1));
    assert_eq!(row.site.generic_arg_count, Some(1));
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallRelationKind::Function,
    );

    let owner = function_id_by_name(&db, "call_method_turbofish")?;
    let target = method_id_by_impl_self_type_name(&db, "GenericMethodTarget", "generic_instance")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "generic method context rows: {context:#?}"
    );
    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(row.site.method.as_deref(), Some("generic_instance"));
    assert_eq!(row.site.arg_count, Some(1));
    assert_eq!(row.site.generic_arg_count, Some(1));
    assert_eq!(
        row.site.receiver,
        Some(CallReceiver::TypedLocalBinding {
            name: "value".to_string(),
            type_path: path(&["GenericMethodTarget"]),
        })
    );
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallRelationKind::Method,
    );

    let owner = function_id_by_name(&db, "call_unsafe_function")?;
    let target = function_id_by_name(&db, "unsafe_target")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "unsafe function context rows: {context:#?}"
    );
    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["unsafe_target"])));
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallRelationKind::Function,
    );

    let owner = function_id_by_name(&db, "call_extern_c_function")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "extern C context rows: {context:#?}");
    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["abs"])));
    assert_eq!(row.site.arg_count, Some(1));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_eq!(row.status.status, CallStatusKind::External);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "extern C call must not fabricate local target edges: {row:#?}"
    );

    let owner = function_id_by_name(&db, "call_chained_returned_function")?;
    let target = function_id_by_name(&db, "make_unary_fn")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "chained call context rows: {context:#?}");

    let row = row_by_path(&context, &["make_unary_fn"]);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallRelationKind::Function,
    );

    let dynamic_rows = context
        .iter()
        .filter(|row| row.site.kind == CallSiteKind::Dynamic)
        .collect::<Vec<_>>();
    assert_eq!(
        dynamic_rows.len(),
        1,
        "expected one outer dynamic call row: {context:#?}"
    );
    let row = dynamic_rows[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(1));
    assert_eq!(row.site.generic_arg_count, None);
    assert_eq!(row.status.status, CallStatusKind::Unsupported);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "returned-function dynamic call must remain unsupported without fake targets: {row:#?}"
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_raw_identifier_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    let owner = function_id_by_name(&db, "call_raw_identifier_function")?;
    let target = function_id_by_exact_name(&db, "r#match")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "raw function context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["r#match"])));
    assert_eq!(row.site.arg_count, Some(0));
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallRelationKind::Function,
    );

    let owner = function_id_by_name(&db, "call_raw_identifier_method")?;
    let target = method_id_by_impl_self_type_exact_name(&db, "RawMethodTarget", "r#type")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "raw method context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(row.site.method.as_deref(), Some("r#type"));
    assert_eq!(
        row.site.receiver,
        Some(CallReceiver::TypedLocalBinding {
            name: "value".to_string(),
            type_path: path(&["RawMethodTarget"]),
        })
    );
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallRelationKind::Method,
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_prelude_drop_shadowing() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    let owner = function_id_by_name(&db, "call_prelude_drop_value")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "prelude drop context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["drop"])));
    assert_eq!(row.site.arg_count, Some(1));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_eq!(row.status.status, CallStatusKind::External);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "prelude drop must not fabricate local target edges: {row:#?}"
    );

    let owner = function_id_by_name_in_module(
        &db,
        &["crate", "prelude_shadow_scope"],
        "call_local_drop_shadow",
    )?;
    let target = function_id_by_name_in_module(&db, &["crate", "prelude_shadow_scope"], "drop")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "local drop shadow context rows: {context:#?}"
    );

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["drop"])));
    assert_eq!(row.site.arg_count, Some(1));
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallRelationKind::Function,
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_external_and_shadowed_method_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    let owner = function_id_by_name(&db, "call_literal_str_to_string")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "literal to_string context rows: {context:#?}"
    );

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(row.site.method.as_deref(), Some("to_string"));
    assert_eq!(row.site.receiver, Some(CallReceiver::Literal));
    assert_eq!(row.status.status, CallStatusKind::External);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "literal to_string must not fabricate local target edges: {row:#?}"
    );

    let owner = function_id_by_name(&db, "call_typed_vec_len_external")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "typed Vec context rows: {context:#?}");

    let new_row = row_by_path(&context, &["Vec", "new"]);
    assert_eq!(new_row.status.status, CallStatusKind::External);
    assert_eq!(new_row.status.resolution, None);
    assert!(
        new_row.targets.is_empty(),
        "Vec::new must not fabricate local target edges: {new_row:#?}"
    );

    let row = row_by_method_receiver(
        &context,
        "len",
        &CallReceiver::TypedLocalBinding {
            name: "value".to_string(),
            type_path: path(&["Vec"]),
        },
    );
    assert_eq!(row.status.status, CallStatusKind::External);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "unshadowed Vec::len must not fabricate local target edges: {row:#?}"
    );

    let owner = function_id_by_name_in_module(
        &db,
        &["crate", "local_prelude_shadow"],
        "call_shadowed_typed_vec_len",
    )?;
    let target = method_id_by_impl_self_type_name(&db, "Vec", "len")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "shadowed Vec::len context rows: {context:#?}"
    );

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(row.site.method.as_deref(), Some("len"));
    assert_eq!(
        row.site.receiver,
        Some(CallReceiver::TypedLocalBinding {
            name: "value".to_string(),
            type_path: path(&["Vec"]),
        })
    );
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallRelationKind::Method,
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_explicit_drop_method_call() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_explicit_drop_method")?;
    let target = method_id_by_impl_self_type_name(&db, "ExplicitDropTarget", "drop")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "explicit drop context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(row.site.method.as_deref(), Some("drop"));
    assert_eq!(
        row.site.receiver,
        Some(CallReceiver::InitializedLocalBinding {
            name: "value".to_string(),
            init_path: path(&["ExplicitDropTarget"]),
        })
    );
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallRelationKind::Method,
    );

    Ok(())
}

#[test]
fn fixture_callers_for_target_reads_real_incoming_callers() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let path_owner = function_id_by_name(&db, "call_crate_local_target")?;
    let dynamic_owner = function_id_by_name(&db, "call_parenthesized_local_target")?;

    let callers = db.callers_for_target(target)?;
    assert!(
        callers.len() >= 2,
        "local_target should have multiple incoming callers: {callers:#?}"
    );
    assert!(
        callers
            .iter()
            .all(|caller| caller.target.target_id == target),
        "incoming caller query returned a mismatched target edge: {callers:#?}"
    );
    assert!(
        callers
            .iter()
            .all(|caller| caller.status.status == CallStatusKind::Resolved
                && caller.status.resolution == Some(CallResolutionKind::LocalExact)),
        "incoming local target callers should preserve resolved statuses: {callers:#?}"
    );

    let path = caller_by_owner_kind_path(
        &callers,
        path_owner,
        CallSiteKind::Path,
        &["crate", "local_target"],
    );
    assert_eq!(path.target.relation, CallRelationKind::Function);
    assert_eq!(path.target.source_kind, CallSiteKind::Path);
    assert_eq!(path.target.target_kind, CallRelationKind::Function);

    let dynamic = caller_by_owner_kind_path(
        &callers,
        dynamic_owner,
        CallSiteKind::Dynamic,
        &["local_target"],
    );
    assert_eq!(dynamic.target.relation, CallRelationKind::DynamicFunction);
    assert_eq!(dynamic.target.source_kind, CallSiteKind::Dynamic);
    assert_eq!(dynamic.target.target_kind, CallRelationKind::Function);

    Ok(())
}

#[test]
fn fixture_callers_for_target_excludes_closure_or_async_body_outer_owners() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let forbidden_owners = [
        function_id_by_name(&db, "closure_body_call_is_not_outer_call_site")?,
        function_id_by_name(&db, "async_block_call_is_not_outer_call_site")?,
        function_id_by_name(&db, "call_move_closure_literal_with_body_call")?,
        function_id_by_name(&db, "call_async_closure_literal_with_body_call")?,
    ];

    let callers = db.callers_for_target(target)?;
    assert!(
        callers
            .iter()
            .all(|caller| !forbidden_owners.contains(&caller.site.owner_id)),
        "target-centered local_target callers leaked closure/async body outer owners: {callers:#?}"
    );

    let incoming = db.expand_call_context(
        CallContextSeed::Target(target),
        CallContextOptions {
            include_outgoing_targets: false,
            ..CallContextOptions::default()
        },
    )?;
    assert!(
        incoming
            .iter()
            .all(|candidate| !forbidden_owners.contains(&candidate.node_id)),
        "target-seeded local_target expansion leaked closure/async body outer owners: {incoming:#?}"
    );

    Ok(())
}

#[test]
fn fixture_expand_call_context_reads_real_outgoing_and_incoming_candidates() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let path_owner = function_id_by_name(&db, "call_crate_local_target")?;
    let dynamic_owner = function_id_by_name(&db, "call_parenthesized_local_target")?;
    let path_context = db.call_context_for_owner(path_owner)?;
    let path_site = row_by_path(&path_context, &["crate", "local_target"])
        .site
        .id;
    let callers = db.callers_for_target(target)?;
    let incoming_path_site = caller_by_owner_kind_path(
        &callers,
        path_owner,
        CallSiteKind::Path,
        &["crate", "local_target"],
    )
    .site
    .id;
    let dynamic_site = caller_by_owner_kind_path(
        &callers,
        dynamic_owner,
        CallSiteKind::Dynamic,
        &["local_target"],
    )
    .site
    .id;
    assert_eq!(
        incoming_path_site, path_site,
        "owner and target helpers should agree on path call-site identity"
    );

    let outgoing = db.expand_call_context(
        CallContextSeed::Owner(path_owner),
        CallContextOptions {
            include_incoming_callers: false,
            ..CallContextOptions::default()
        },
    )?;
    assert_eq!(
        outgoing.len(),
        1,
        "path owner should expose one outgoing call-context candidate: {outgoing:#?}"
    );
    assert_eq!(outgoing[0].node_id, target);
    assert_eq!(outgoing[0].target_id, target);
    assert_eq!(outgoing[0].relation, CallContextRelation::OutgoingTarget);
    assert_eq!(outgoing[0].call_site_id, path_site);
    assert_eq!(outgoing[0].distance, 1);

    let incoming = db.expand_call_context(
        CallContextSeed::Target(target),
        CallContextOptions {
            include_outgoing_targets: false,
            max_candidates: 128,
            ..CallContextOptions::default()
        },
    )?;
    assert!(
        incoming.len() >= 2,
        "local_target should expose multiple incoming caller candidates: {incoming:#?}"
    );
    assert!(
        incoming.iter().all(|candidate| {
            candidate.target_id == target
                && candidate.relation == CallContextRelation::IncomingCaller
                && candidate.distance == 1
        }),
        "incoming expansion should only include caller candidates for the target: {incoming:#?}"
    );
    assert_call_candidate(
        &incoming,
        path_owner,
        CallContextRelation::IncomingCaller,
        path_site,
        target,
        "path caller candidate missing",
    );
    assert_call_candidate(
        &incoming,
        dynamic_owner,
        CallContextRelation::IncomingCaller,
        dynamic_site,
        target,
        "dynamic caller candidate missing",
    );

    Ok(())
}

#[test]
fn fixture_expand_call_context_owner_seed_filters_mixed_status_rows() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_try_result_instance_method")?;
    let try_target = function_id_by_name(&db, "try_local_assoc")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 3, "try-result context rows: {context:#?}");

    let ok_site = row_by_path(&context, &["Ok"]).site.id;
    let try_site = row_by_path(&context, &["try_local_assoc"]).site.id;
    let receiver = CallReceiver::TryPathCallResult {
        path: path(&["try_local_assoc"]),
    };
    let method_site = row_by_method_receiver(&context, "instance_value", &receiver)
        .site
        .id;

    let candidates = db.expand_call_context(
        CallContextSeed::Owner(owner),
        CallContextOptions {
            include_incoming_callers: false,
            ..CallContextOptions::default()
        },
    )?;
    assert_eq!(
        candidates.len(),
        2,
        "owner expansion should promote only resolved rows: {candidates:#?}"
    );
    assert_call_candidate(
        &candidates,
        try_target,
        CallContextRelation::OutgoingTarget,
        try_site,
        try_target,
        "try_local_assoc outgoing target candidate missing",
    );
    assert_call_candidate(
        &candidates,
        method_target,
        CallContextRelation::OutgoingTarget,
        method_site,
        method_target,
        "try-result method outgoing target candidate missing",
    );
    assert!(
        candidates
            .iter()
            .all(|candidate| candidate.call_site_id != ok_site),
        "unsupported Ok path call must not be promoted: {candidates:#?}"
    );

    Ok(())
}

#[test]
fn fixture_expand_call_context_target_seed_preserves_method_family_callers() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let method_owner = function_id_by_name(&db, "call_typed_local_instance_method")?;
    let assoc_owner = function_id_by_name(&db, "call_method_as_associated_function")?;

    let method_context = db.call_context_for_owner(method_owner)?;
    let method_receiver = CallReceiver::TypedLocalBinding {
        name: "value".to_string(),
        type_path: path(&["LocalAssoc"]),
    };
    let method_site = row_by_method_receiver(&method_context, "instance_value", &method_receiver)
        .site
        .id;

    let assoc_context = db.call_context_for_owner(assoc_owner)?;
    let assoc_site = row_by_path(&assoc_context, &["LocalAssoc", "instance_value"])
        .site
        .id;

    let candidates = db.expand_call_context(
        CallContextSeed::Target(target),
        CallContextOptions {
            include_outgoing_targets: false,
            max_candidates: 128,
            ..CallContextOptions::default()
        },
    )?;
    assert!(
        candidates.len() >= 2,
        "method target should expose multiple incoming expansion candidates: {candidates:#?}"
    );
    assert!(
        candidates.iter().all(|candidate| {
            candidate.target_id == target
                && candidate.relation == CallContextRelation::IncomingCaller
                && candidate.distance == 1
        }),
        "incoming method expansion should preserve the seed target and relation: {candidates:#?}"
    );
    assert_call_candidate(
        &candidates,
        method_owner,
        CallContextRelation::IncomingCaller,
        method_site,
        target,
        "method-call incoming candidate missing",
    );
    assert_call_candidate(
        &candidates,
        assoc_owner,
        CallContextRelation::IncomingCaller,
        assoc_site,
        target,
        "associated-function incoming candidate missing",
    );

    Ok(())
}

#[test]
fn fixture_expand_call_context_target_seed_preserves_constructor_callers() -> Result<(), DbError> {
    for case in constructor_cases() {
        let db = setup_call_graph_fixture_db(case.fixture)?;
        let resolved = assert_constructor_context(&db, case)?;
        assert_constructor_callers(&db, case, &resolved)?;

        let candidates = db.expand_call_context(
            CallContextSeed::Target(resolved.target),
            CallContextOptions {
                include_outgoing_targets: false,
                max_candidates: 128,
                ..CallContextOptions::default()
            },
        )?;
        assert_call_candidate(
            &candidates,
            resolved.owner,
            CallContextRelation::IncomingCaller,
            resolved.site,
            resolved.target,
            &format!("{} incoming candidate missing", case.label),
        );
    }

    Ok(())
}

#[test]
fn fixture_callers_for_target_reads_method_and_associated_callers() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let callers = db.callers_for_target(target)?;
    assert!(
        callers.len() >= 2,
        "LocalAssoc::instance_value should have multiple incoming callers: {callers:#?}"
    );
    assert!(
        callers
            .iter()
            .all(|caller| caller.target.target_id == target),
        "method target caller query returned a mismatched target edge: {callers:#?}"
    );
    assert!(
        callers
            .iter()
            .all(|caller| caller.status.status == CallStatusKind::Resolved
                && caller.status.resolution == Some(CallResolutionKind::LocalExact)),
        "incoming method callers should preserve resolved statuses: {callers:#?}"
    );

    let owner = function_id_by_name(&db, "call_typed_local_instance_method")?;
    let caller = caller_by_owner_method_receiver(
        &callers,
        owner,
        "instance_value",
        &CallReceiver::TypedLocalBinding {
            name: "value".to_string(),
            type_path: path(&["LocalAssoc"]),
        },
    );
    assert_eq!(caller.target.relation, CallRelationKind::Method);
    assert_eq!(caller.target.source_kind, CallSiteKind::Method);
    assert_eq!(caller.target.target_kind, CallRelationKind::Method);

    let owner = function_id_by_name(&db, "call_method_as_associated_function")?;
    let caller = caller_by_owner_kind_path(
        &callers,
        owner,
        CallSiteKind::Path,
        &["LocalAssoc", "instance_value"],
    );
    assert_eq!(caller.site.arg_count, Some(1));
    assert_eq!(caller.target.relation, CallRelationKind::AssociatedFunction);
    assert_eq!(caller.target.source_kind, CallSiteKind::Path);
    assert_eq!(caller.target.target_kind, CallRelationKind::Method);

    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "make")?;
    let callers = db.callers_for_target(target)?;
    assert!(
        callers.len() >= 3,
        "LocalAssoc::make should have multiple incoming callers: {callers:#?}"
    );
    assert!(
        callers
            .iter()
            .all(|caller| caller.target.target_id == target),
        "associated-function target caller query returned a mismatched target edge: {callers:#?}"
    );

    let owner = function_id_by_name(&db, "call_local_assoc_make")?;
    let caller =
        caller_by_owner_kind_path(&callers, owner, CallSiteKind::Path, &["LocalAssoc", "make"]);
    assert_eq!(caller.target.relation, CallRelationKind::AssociatedFunction);
    assert_eq!(caller.target.source_kind, CallSiteKind::Path);
    assert_eq!(caller.target.target_kind, CallRelationKind::Method);

    let owner = method_id_by_impl_self_type_name(&db, "LocalAssoc", "call_self_make")?;
    let caller = caller_by_owner_kind_path(&callers, owner, CallSiteKind::Path, &["Self", "make"]);
    assert_eq!(caller.target.relation, CallRelationKind::AssociatedFunction);
    assert_eq!(caller.target.source_kind, CallSiteKind::Path);
    assert_eq!(caller.target.target_kind, CallRelationKind::Method);

    Ok(())
}

#[test]
fn fixture_context_reads_projected_trait_dispatch_method_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "LocalDispatchTrait",
        "TraitDispatchTarget",
        "trait_value",
    )?;
    let cases = [
        (
            "call_param_trait_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_typed_local_trait_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["TraitDispatchTarget"]),
            },
        ),
        (
            "call_initialized_local_trait_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["TraitDispatchTarget"]),
            },
        ),
        (
            "call_parenthesized_initialized_local_trait_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["TraitDispatchTarget"]),
            },
        ),
        (
            "call_concrete_trait_object_binding_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["TraitDispatchTarget"]),
            },
        ),
        (
            "call_aliased_concrete_trait_object_binding_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["TraitDispatchTarget"]),
            },
        ),
        (
            "call_reference_alias_trait_object_binding_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["TraitDispatchTarget"]),
            },
        ),
        (
            "call_reference_chain_trait_object_binding_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["TraitDispatchTarget"]),
            },
        ),
    ];

    for (owner_name, receiver) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Method);
        assert_eq!(row.site.method.as_deref(), Some("trait_value"));
        assert_eq!(row.site.receiver, Some(receiver));
        assert_eq!(row.status.status, CallStatusKind::Resolved);
        assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(row.targets.len(), 1);
        assert_eq!(row.targets[0].target_id, target);
        assert_eq!(row.targets[0].relation, CallRelationKind::Method);
        assert_eq!(row.targets[0].source_kind, CallSiteKind::Method);
        assert_eq!(row.targets[0].target_kind, CallRelationKind::Method);
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_constrained_generic_self_trait_method() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_constrained_generic_self_trait_method")?;
    let target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "ConstrainedGenericSelfTrait",
        "GenericWrapper",
        "constrained_generic_self_value",
    )?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "constrained generic self trait method context rows: {context:#?}"
    );

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(
        row.site.method.as_deref(),
        Some("constrained_generic_self_value")
    );
    assert_eq!(
        row.site.receiver,
        Some(CallReceiver::LocalBinding {
            name: "value".to_string(),
        })
    );
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallRelationKind::Method,
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_generic_bound_and_trait_object_methods() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_trait_name(&db, "GenericBoundTrait", "bound_value")?;
    let cases = [
        (
            "call_inline_generic_bound_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_where_generic_bound_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_impl_trait_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_trait_object_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_local_trait_object_binding_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["GenericBoundTrait"]),
            },
        ),
    ];

    for (owner_name, receiver) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Method);
        assert_eq!(row.site.method.as_deref(), Some("bound_value"));
        assert_eq!(row.site.receiver, Some(receiver));
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Method,
            CallSiteKind::Method,
            CallRelationKind::Method,
        );
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_imported_trait_method_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "ScopedTrait",
        "ScopedTraitTarget",
        "scoped_value",
    )?;
    let cases = [
        (
            &["crate", "trait_scope", "with_direct_import"][..],
            "call_direct_imported_trait_method",
        ),
        (
            &["crate", "trait_scope", "with_alias_import"][..],
            "call_alias_imported_trait_method",
        ),
        (
            &["crate", "trait_scope", "with_glob_import"][..],
            "call_glob_imported_trait_method",
        ),
        (
            &["crate", "trait_reexport_scope"][..],
            "call_reexported_trait_method",
        ),
        (
            &["crate", "grouped_trait_import_scope"][..],
            "call_grouped_imported_trait_method",
        ),
    ];

    for (module_path, owner_name) in cases {
        let owner = function_id_by_name_in_module(&db, module_path, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Method);
        assert_eq!(row.site.method.as_deref(), Some("scoped_value"));
        assert_eq!(
            row.site.receiver,
            Some(CallReceiver::LocalBinding {
                name: "value".to_string(),
            })
        );
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Method,
            CallSiteKind::Method,
            CallRelationKind::Method,
        );
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_blanket_trait_method_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let cases = [
        (
            "call_blanket_trait_method",
            "BlanketDispatchTrait",
            "blanket_value",
        ),
        (
            "call_inline_bound_blanket_trait_method",
            "InlineBoundBlanketTrait",
            "inline_bound_value",
        ),
        (
            "call_where_bound_blanket_trait_method",
            "WhereBoundBlanketTrait",
            "where_bound_value",
        ),
        (
            "call_transitive_bound_blanket_trait_method",
            "TransitiveBoundBlanketTrait",
            "transitive_bound_value",
        ),
    ];

    for (owner_name, trait_name, method_name) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let target = method_id_by_impl_trait_name(&db, trait_name, method_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Method);
        assert_eq!(row.site.method.as_deref(), Some(method_name));
        assert_eq!(
            row.site.receiver,
            Some(CallReceiver::LocalBinding {
                name: "value".to_string(),
            })
        );
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Method,
            CallSiteKind::Method,
            CallRelationKind::Method,
        );
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_borrowed_and_dereferenced_method_receivers()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let cases = [
        (
            "call_borrowed_typed_local_instance_method",
            CallReceiver::BorrowedTypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_dereferenced_local_instance_method",
            CallReceiver::DereferencedInitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_dereferenced_param_instance_method",
            CallReceiver::DereferencedLocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_borrowed_param_instance_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_referenced_local_instance_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_typed_reference_local_instance_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssoc"]),
            },
        ),
    ];

    for (owner_name, receiver) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Method);
        assert_eq!(row.site.method.as_deref(), Some("instance_value"));
        assert_eq!(row.site.receiver, Some(receiver));
        assert_eq!(row.status.status, CallStatusKind::Resolved);
        assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(row.targets.len(), 1);
        assert_eq!(row.targets[0].target_id, target);
        assert_eq!(row.targets[0].relation, CallRelationKind::Method);
        assert_eq!(row.targets[0].source_kind, CallSiteKind::Method);
        assert_eq!(row.targets[0].target_kind, CallRelationKind::Method);
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_method_body_owner_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    let owner = method_id_by_trait_name(&db, "TraitDefaultCall", "default_calls_local")?;
    let target = function_id_by_name(&db, "local_target")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "trait default path context rows: {context:#?}"
    );

    let row = row_by_path(&context, &["local_target"]);
    assert_eq!(row.site.owner_id, owner);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallRelationKind::Function,
    );

    let owner = method_id_by_impl_trait_and_self_type_names(
        &db,
        "TraitImplBodyCallTrait",
        "TraitDispatchTarget",
        "impl_calls_required",
    )?;
    let target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "TraitImplBodyCallTrait",
        "TraitDispatchTarget",
        "required_impl_call",
    )?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "trait impl self-method context rows: {context:#?}"
    );

    let row = row_by_method_receiver(&context, "required_impl_call", &CallReceiver::SelfValue);
    assert_eq!(row.site.owner_id, owner);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallRelationKind::Method,
    );

    let owner = method_id_by_trait_name(&db, "DefaultRequiredCall", "default_calls_required")?;
    let target = method_id_by_trait_name(&db, "DefaultRequiredCall", "required")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "trait default self-method context rows: {context:#?}"
    );

    let row = row_by_method_receiver(&context, "required", &CallReceiver::SelfValue);
    assert_eq!(row.site.owner_id, owner);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallRelationKind::Method,
    );

    let owner = method_id_by_trait_name(&db, "TraitDefaultAssocCall", "default_calls_assoc")?;
    let target = method_id_by_trait_name(&db, "TraitDefaultAssocCall", "required_assoc")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "trait default associated-function context rows: {context:#?}"
    );

    let row = row_by_path(&context, &["Self", "required_assoc"]);
    assert_eq!(row.site.owner_id, owner);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::AssociatedFunction,
        CallSiteKind::Path,
        CallRelationKind::Method,
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_const_and_static_initializer_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_nodes")?;
    let target = function_id_by_name_in_module(&db, &["crate", "const_static"], "five")?;

    let cases = [
        (
            const_id_by_name(&db, "FN_CALL_CONST")?,
            "const initializer context rows",
        ),
        (
            static_id_by_name(&db, "STATIC_FN_CALL")?,
            "static initializer context rows",
        ),
    ];

    for (owner, label) in cases {
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{label}: {context:#?}");

        let row = row_by_path(&context, &["five"]);
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Path);
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.site.generic_arg_count, Some(0));
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallRelationKind::Function,
        );
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_associated_const_initializer_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "assoc_const_value")?;
    let cases = [
        const_id_by_name(&db, "IMPL_ASSOC_VALUE")?,
        const_id_by_name(&db, "TRAIT_ASSOC_VALUE")?,
    ];

    for owner in cases {
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(
            context.len(),
            1,
            "associated const initializer context rows: {context:#?}"
        );

        let row = row_by_path(&context, &["assoc_const_value"]);
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Path);
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.site.generic_arg_count, Some(0));
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallRelationKind::Function,
        );
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_external_path_status_without_targets() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_prelude_string_new")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["String", "new"])));
    assert_eq!(row.status.status, CallStatusKind::External);
    assert_eq!(row.status.resolution, None);
    assert!(row.targets.is_empty(), "external row targets: {row:#?}");

    Ok(())
}

#[test]
fn fixture_context_reads_projected_ambiguous_method_status_without_targets() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_ambiguous_trait_method")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(row.site.method.as_deref(), Some("overlap"));
    assert_eq!(
        row.site.receiver,
        Some(CallReceiver::LocalBinding {
            name: "value".to_string(),
        })
    );
    assert_eq!(row.status.status, CallStatusKind::Ambiguous);
    assert_eq!(row.status.resolution, None);
    assert!(row.targets.is_empty(), "ambiguous row targets: {row:#?}");

    Ok(())
}

#[test]
fn fixture_context_reads_projected_inherent_method_precedence() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_inherent_over_trait_method")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "inherent precedence rows: {context:#?}");

    let receiver = CallReceiver::TypedLocalBinding {
        name: "value".to_string(),
        type_path: path(&["InherentPrecedenceTarget"]),
    };
    let row = row_by_method_receiver(&context, "priority", &receiver);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.status.status, CallStatusKind::Resolved);
    assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(row.targets.len(), 1);
    assert_eq!(row.targets[0].relation, CallRelationKind::Method);
    assert_eq!(row.targets[0].source_kind, CallSiteKind::Method);
    assert_eq!(row.targets[0].target_kind, CallRelationKind::Method);
    assert!(
        method_owner_is_inherent_impl(&db, row.targets[0].target_id, "InherentPrecedenceTarget")?,
        "inherent method call must project a target owned by the inherent impl: {row:#?}"
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_unsupported_method_status_without_targets() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_unimported_trait_method")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(row.site.method.as_deref(), Some("scoped_value"));
    assert_eq!(
        row.site.receiver,
        Some(CallReceiver::LocalBinding {
            name: "value".to_string(),
        })
    );
    assert_eq!(row.status.status, CallStatusKind::Unsupported);
    assert_eq!(row.status.resolution, None);
    assert!(row.targets.is_empty(), "unsupported row targets: {row:#?}");

    Ok(())
}

#[test]
fn fixture_context_preserves_path_result_owner_order_and_targets() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_path_result_instance_method")?;
    let make_target = function_id_by_name(&db, "make_local_assoc")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "context rows: {context:#?}");

    let path_row = &context[0];
    assert_eq!(path_row.site.kind, CallSiteKind::Path);
    assert_eq!(
        path_row.site.path.as_ref(),
        Some(&path(&["make_local_assoc"]))
    );
    assert_eq!(path_row.status.status, CallStatusKind::Resolved);
    assert_eq!(
        path_row.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(path_row.targets.len(), 1);
    assert_eq!(path_row.targets[0].target_id, make_target);
    assert_eq!(path_row.targets[0].relation, CallRelationKind::Function);

    let method_row = &context[1];
    assert_eq!(method_row.site.kind, CallSiteKind::Method);
    assert_eq!(method_row.site.method.as_deref(), Some("instance_value"));
    assert_eq!(
        method_row.site.receiver,
        Some(CallReceiver::PathCallResult {
            path: path(&["make_local_assoc"]),
        })
    );
    assert_eq!(method_row.status.status, CallStatusKind::Resolved);
    assert_eq!(
        method_row.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(method_row.targets.len(), 1);
    assert_eq!(method_row.targets[0].relation, CallRelationKind::Method);

    Ok(())
}

#[test]
fn fixture_low_level_helpers_read_projected_sites_targets_and_statuses() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_try_result_instance_method")?;
    let try_target = function_id_by_name(&db, "try_local_assoc")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;

    let sites = db.call_sites_for_owner(owner)?;
    assert_eq!(sites.len(), 3, "call sites: {sites:#?}");
    assert!(
        sites.windows(2).all(|pair| pair[0].span <= pair[1].span),
        "call_sites_for_owner must preserve source-span order: {sites:#?}"
    );

    let ok_path = path(&["Ok"]);
    let ok_site = sites
        .iter()
        .find(|site| site.kind == CallSiteKind::Path && site.path.as_ref() == Some(&ok_path))
        .expect("Ok path call site");
    let ok_status = db
        .call_resolution_for_site(ok_site.id)?
        .expect("Ok status row");
    assert_eq!(ok_status.site_id, ok_site.id);
    assert_eq!(ok_status.site_kind, CallSiteKind::Path);
    assert_eq!(ok_status.status, CallStatusKind::Unsupported);
    assert_eq!(ok_status.resolution, None);
    assert!(
        db.call_targets_for_site(ok_site.id)?.is_empty(),
        "unsupported Ok path should not have call targets"
    );

    let try_path = path(&["try_local_assoc"]);
    let try_site = sites
        .iter()
        .find(|site| site.kind == CallSiteKind::Path && site.path.as_ref() == Some(&try_path))
        .expect("try_local_assoc path call site");
    let try_status = db
        .call_resolution_for_site(try_site.id)?
        .expect("try_local_assoc status row");
    assert_eq!(try_status.site_id, try_site.id);
    assert_eq!(try_status.site_kind, CallSiteKind::Path);
    assert_eq!(try_status.status, CallStatusKind::Resolved);
    assert_eq!(try_status.resolution, Some(CallResolutionKind::LocalExact));
    let try_targets = db.call_targets_for_site(try_site.id)?;
    assert_eq!(try_targets.len(), 1, "try targets: {try_targets:#?}");
    assert_eq!(try_targets[0].site_id, try_site.id);
    assert_eq!(try_targets[0].target_id, try_target);
    assert_eq!(try_targets[0].relation, CallRelationKind::Function);
    assert_eq!(try_targets[0].source_kind, CallSiteKind::Path);
    assert_eq!(try_targets[0].target_kind, CallRelationKind::Function);

    let receiver = CallReceiver::TryPathCallResult {
        path: path(&["try_local_assoc"]),
    };
    let method_site = sites
        .iter()
        .find(|site| {
            site.kind == CallSiteKind::Method
                && site.method.as_deref() == Some("instance_value")
                && site.receiver.as_ref() == Some(&receiver)
        })
        .expect("try receiver method call site");
    let method_status = db
        .call_resolution_for_site(method_site.id)?
        .expect("method status row");
    assert_eq!(method_status.site_id, method_site.id);
    assert_eq!(method_status.site_kind, CallSiteKind::Method);
    assert_eq!(method_status.status, CallStatusKind::Resolved);
    assert_eq!(
        method_status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    let method_targets = db.call_targets_for_site(method_site.id)?;
    assert_eq!(
        method_targets.len(),
        1,
        "method targets: {method_targets:#?}"
    );
    assert_eq!(method_targets[0].site_id, method_site.id);
    assert_eq!(method_targets[0].target_id, method_target);
    assert_eq!(method_targets[0].relation, CallRelationKind::Method);
    assert_eq!(method_targets[0].source_kind, CallSiteKind::Method);
    assert_eq!(method_targets[0].target_kind, CallRelationKind::Method);

    Ok(())
}

#[test]
fn fixture_context_reads_projected_result_receiver_method_chains() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;

    let owner = function_id_by_name(&db, "call_method_result_instance_method")?;
    let clone_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "clone_assoc")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "method-result context rows: {context:#?}");

    let clone_receiver = CallReceiver::TypedLocalBinding {
        name: "value".to_string(),
        type_path: path(&["LocalAssoc"]),
    };
    let row = row_by_method_receiver(&context, "clone_assoc", &clone_receiver);
    assert_resolved_target(
        row,
        clone_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallRelationKind::Method,
    );

    let result_receiver = CallReceiver::MethodCallResult {
        method_name: "clone_assoc".to_string(),
    };
    let row = row_by_method_receiver(&context, "instance_value", &result_receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallRelationKind::Method,
    );

    let owner = function_id_by_name(&db, "call_await_result_instance_method")?;
    let ready_target = function_id_by_name(&db, "make_ready_local_assoc")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "await-result context rows: {context:#?}");

    let row = row_by_path(&context, &["make_ready_local_assoc"]);
    assert_resolved_target(
        row,
        ready_target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallRelationKind::Function,
    );

    let await_receiver = CallReceiver::AwaitPathCallResult {
        path: path(&["make_ready_local_assoc"]),
    };
    let row = row_by_method_receiver(&context, "instance_value", &await_receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallRelationKind::Method,
    );

    let owner = function_id_by_name(&db, "call_try_result_instance_method")?;
    let try_target = function_id_by_name(&db, "try_local_assoc")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 3, "try-result context rows: {context:#?}");

    let row = row_by_path(&context, &["Ok"]);
    assert_eq!(row.status.status, CallStatusKind::Unsupported);
    assert_eq!(row.status.resolution, None);
    assert!(row.targets.is_empty(), "Ok row targets: {row:#?}");

    let row = row_by_path(&context, &["try_local_assoc"]);
    assert_resolved_target(
        row,
        try_target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallRelationKind::Function,
    );

    let try_receiver = CallReceiver::TryPathCallResult {
        path: path(&["try_local_assoc"]),
    };
    let row = row_by_method_receiver(&context, "instance_value", &try_receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallRelationKind::Method,
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_field_receiver_and_dynamic_field_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;

    let owner = function_id_by_name(&db, "call_tuple_field_instance_method")?;
    let struct_target = struct_id_by_name(&db, "TupleFieldMethodReceiver")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "field-method context rows: {context:#?}");

    let row = row_by_path(&context, &["TupleFieldMethodReceiver"]);
    assert_resolved_target(
        row,
        struct_target,
        CallRelationKind::TupleStructConstructor,
        CallSiteKind::Path,
        CallRelationKind::Struct,
    );

    let receiver = CallReceiver::FieldInitializedLocalBinding {
        name: "value".to_string(),
        init_path: path(&["TupleFieldMethodReceiver"]),
        field_path: path(&["0"]),
    };
    let row = row_by_method_receiver(&context, "instance_value", &receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallRelationKind::Method,
    );

    let owner = function_id_by_name(&db, "call_tuple_field_function")?;
    let struct_target = struct_id_by_name(&db, "TupleFieldFunction")?;
    let function_target = function_id_by_name(&db, "local_target")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "field-dynamic context rows: {context:#?}");

    let row = row_by_path(&context, &["TupleFieldFunction"]);
    assert_resolved_target(
        row,
        struct_target,
        CallRelationKind::TupleStructConstructor,
        CallSiteKind::Path,
        CallRelationKind::Struct,
    );

    let row = row_by_kind_path(&context, CallSiteKind::Dynamic, &["value", "0"]);
    assert_eq!(row.site.receiver, None);
    assert_resolved_target(
        row,
        function_target,
        CallRelationKind::DynamicFunction,
        CallSiteKind::Dynamic,
        CallRelationKind::Function,
    );

    Ok(())
}

#[test]
fn fixture_context_reads_function_pointer_param_cast_path_without_target() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_function_pointer_param_cast")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "function-pointer param cast context rows: {context:#?}"
    );

    let row = row_by_kind_path(&context, CallSiteKind::Dynamic, &["f"]);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.receiver, None);
    assert_eq!(row.status.status, CallStatusKind::Unsupported);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "opaque function-pointer param cast must not fabricate targets: {row:#?}"
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_macro_statuses_without_targets() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let cases = [
        ("call_crate_scoped_macro", "crate::crate_scoped_macro"),
        ("call_vec_macro", "vec"),
        ("call_imported_macro_alias", "imported_macro_alias"),
        ("call_item_macro_inside_body", "call_graph_item_macro"),
    ];

    for (owner_name, macro_name) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Macro);
        assert_eq!(row.site.macro_name.as_deref(), Some(macro_name));
        assert_eq!(row.site.path, None);
        assert_eq!(row.site.receiver, None);
        assert_eq!(row.site.arg_count, None);
        assert_eq!(row.status.status, CallStatusKind::Unsupported);
        assert_eq!(row.status.resolution, None);
        assert!(row.targets.is_empty(), "macro row targets: {row:#?}");
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_enum_variant_constructor_call() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_nodes")?;
    let owner = function_id_by_name_in_module(&db, &["crate", "imports"], "use_imported_items")?;

    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["EnumWithData", "Variant1"]);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(1));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_eq!(row.status.status, CallStatusKind::Resolved);
    assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(row.targets.len(), 1);
    assert_eq!(
        row.targets[0].relation,
        CallRelationKind::EnumVariantConstructor
    );
    assert_eq!(row.targets[0].source_kind, CallSiteKind::Path);
    assert_eq!(row.targets[0].target_kind, CallRelationKind::Variant);

    Ok(())
}

#[test]
fn fixture_projection_stores_real_resolved_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_crate_local_target")?;
    let target = function_id_by_name(&db, "local_target")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");
    let site = context[0].site.id;
    let span = context[0].site.span;

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 3);

    assert_owner_proof_edges(
        &db,
        "resolved call",
        &[OwnerProofEdge {
            owner,
            site,
            span,
            target,
        }],
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_path_resolution_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let local_target = function_id_by_name(&db, "local_target")?;
    let nested_target =
        function_id_by_name_in_module(&db, &["crate", "local_mod"], "nested_target")?;
    let imported_target =
        function_id_by_name_in_module(&db, &["crate", "import_targets"], "imported_target")?;
    let globbed_target =
        function_id_by_name_in_module(&db, &["crate", "import_targets"], "globbed_target")?;
    let cases: [(&[&str], &str, &[&str], Uuid); 11] = [
        (
            &["crate"],
            "call_unqualified_local_target",
            &["local_target"],
            local_target,
        ),
        (
            &["crate", "local_mod"],
            "call_self_nested_target",
            &["self", "nested_target"],
            nested_target,
        ),
        (
            &["crate"],
            "call_crate_module_nested_target",
            &["crate", "local_mod", "nested_target"],
            nested_target,
        ),
        (
            &["crate"],
            "call_self_module_nested_target",
            &["self", "local_mod", "nested_target"],
            nested_target,
        ),
        (
            &["crate", "super_path_scope"],
            "call_super_local_target",
            &["super", "local_target"],
            local_target,
        ),
        (
            &["crate"],
            "call_imported_alias_target",
            &["imported_alias"],
            imported_target,
        ),
        (
            &["crate"],
            "call_glob_imported_target",
            &["globbed_target"],
            globbed_target,
        ),
        (
            &["crate"],
            "call_reexported_target",
            &["reexported_target"],
            imported_target,
        ),
        (
            &["crate"],
            "call_imported_module_target",
            &["targets_alias", "globbed_target"],
            globbed_target,
        ),
        (
            &["crate", "grouped_function_import_scope"],
            "call_grouped_imported_alias_target",
            &["grouped_alias"],
            imported_target,
        ),
        (
            &["crate", "grouped_function_import_scope"],
            "call_grouped_imported_globbed_target",
            &["grouped_globbed_alias"],
            globbed_target,
        ),
    ];
    let mut expected_edges = Vec::new();

    for (module_path, owner_name, expected_path, target) in cases {
        let owner = function_id_by_name_in_module(&db, module_path, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = row_by_path(&context, expected_path);
        let site = row.site.id;
        let span = row.site.span;
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallRelationKind::Function,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 3, "{owner_name} proof fact count");
        expected_edges.push(OwnerProofEdge {
            owner,
            site,
            span,
            target,
        });
    }

    assert_owner_proof_edges(
        &db,
        "path-resolution",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_associated_function_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let local_make = method_id_by_impl_self_type_name(&db, "LocalAssoc", "make")?;
    let imported_make = method_id_by_impl_self_type_name(&db, "ImportedAssoc", "make")?;
    let instance_value = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let local_trait_make = method_id_by_trait_name(&db, "LocalAssocFunctionTrait", "trait_make")?;
    let imported_trait_make =
        method_id_by_trait_name(&db, "ImportedAssocFunctionTrait", "imported_trait_make")?;

    let mut cases = Vec::new();
    cases.push((
        "call_local_assoc_make",
        function_id_by_name(&db, "call_local_assoc_make")?,
        &["LocalAssoc", "make"][..],
        local_make,
    ));
    cases.push((
        "call_self_make",
        method_id_by_impl_self_type_name(&db, "LocalAssoc", "call_self_make")?,
        &["Self", "make"],
        local_make,
    ));
    cases.push((
        "call_qualified_local_assoc_make",
        function_id_by_name(&db, "call_qualified_local_assoc_make")?,
        &["LocalAssoc", "make"],
        local_make,
    ));
    cases.push((
        "call_imported_type_assoc_make",
        function_id_by_name(&db, "call_imported_type_assoc_make")?,
        &["ImportedAssocAlias", "make"],
        imported_make,
    ));
    cases.push((
        "call_glob_imported_type_assoc_make",
        function_id_by_name(&db, "call_glob_imported_type_assoc_make")?,
        &["ImportedAssoc", "make"],
        imported_make,
    ));
    cases.push((
        "call_reexported_type_assoc_make",
        function_id_by_name(&db, "call_reexported_type_assoc_make")?,
        &["ReexportedAssoc", "make"],
        imported_make,
    ));
    cases.push((
        "call_type_alias_assoc_make",
        function_id_by_name(&db, "call_type_alias_assoc_make")?,
        &["LocalAssocTypeAlias", "make"],
        local_make,
    ));
    cases.push((
        "call_type_alias_chain_assoc_make",
        function_id_by_name(&db, "call_type_alias_chain_assoc_make")?,
        &["LocalAssocAliasChain", "make"],
        local_make,
    ));
    cases.push((
        "call_imported_type_alias_assoc_make",
        function_id_by_name(&db, "call_imported_type_alias_assoc_make")?,
        &["ImportedLocalAssocAlias", "make"],
        local_make,
    ));
    cases.push((
        "call_method_as_associated_function",
        function_id_by_name(&db, "call_method_as_associated_function")?,
        &["LocalAssoc", "instance_value"],
        instance_value,
    ));
    cases.push((
        "call_trait_associated_function",
        function_id_by_name(&db, "call_trait_associated_function")?,
        &["LocalAssocFunctionTrait", "trait_make"],
        local_trait_make,
    ));

    for (module_path, owner_name, expected_path) in [
        (
            &["crate", "trait_assoc_function_scope", "with_direct_import"][..],
            "call_direct_imported_trait_associated_function",
            &["ImportedAssocFunctionTrait", "imported_trait_make"][..],
        ),
        (
            &["crate", "trait_assoc_function_scope", "with_alias_import"],
            "call_alias_imported_trait_associated_function",
            &["VisibleAssocFunctionTrait", "imported_trait_make"],
        ),
        (
            &["crate", "trait_assoc_function_scope", "with_glob_import"],
            "call_glob_imported_trait_associated_function",
            &["ImportedAssocFunctionTrait", "imported_trait_make"],
        ),
        (
            &["crate", "trait_assoc_reexport_scope"],
            "call_reexported_trait_associated_function",
            &["ReexportedAssocFunctionTrait", "imported_trait_make"],
        ),
        (
            &["crate", "grouped_trait_assoc_function_scope"],
            "call_grouped_imported_trait_associated_function",
            &["GroupedAssocFunctionTrait", "imported_trait_make"],
        ),
    ] {
        cases.push((
            owner_name,
            function_id_by_name_in_module(&db, module_path, owner_name)?,
            expected_path,
            imported_trait_make,
        ));
    }

    let mut expected_edges = Vec::new();

    for (owner_name, owner, expected_path, target) in cases {
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = row_by_path(&context, expected_path);
        let site = row.site.id;
        let span = row.site.span;
        assert_resolved_target(
            row,
            target,
            CallRelationKind::AssociatedFunction,
            CallSiteKind::Path,
            CallRelationKind::Method,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 3, "{owner_name} proof fact count");
        expected_edges.push(OwnerProofEdge {
            owner,
            site,
            span,
            target,
        });
    }

    assert_owner_proof_edges(
        &db,
        "associated-function",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_local_receiver_method_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let cases = [
        (
            "call_param_instance_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_initialized_local_instance_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_parenthesized_typed_local_instance_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_type_alias_chain_instance_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssocAliasChain"]),
            },
        ),
        (
            "call_imported_type_alias_instance_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["ImportedLocalAssocAlias"]),
            },
        ),
        (
            "call_borrowed_typed_local_instance_method",
            CallReceiver::BorrowedTypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_dereferenced_local_instance_method",
            CallReceiver::DereferencedInitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_dereferenced_param_instance_method",
            CallReceiver::DereferencedLocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_borrowed_param_instance_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_referenced_local_instance_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_typed_reference_local_instance_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssoc"]),
            },
        ),
    ];
    let mut expected_edges = Vec::new();

    for (owner_name, receiver) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = row_by_method_receiver(&context, "instance_value", &receiver);
        let site = row.site.id;
        let span = row.site.span;
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Method,
            CallSiteKind::Method,
            CallRelationKind::Method,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 3, "{owner_name} proof fact count");
        expected_edges.push(OwnerProofEdge {
            owner,
            site,
            span,
            target,
        });
    }

    assert_owner_proof_edges(
        &db,
        "local receiver method",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_trait_family_method_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let generic_target = method_id_by_trait_name(&db, "GenericBoundTrait", "bound_value")?;
    let imported_target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "ScopedTrait",
        "ScopedTraitTarget",
        "scoped_value",
    )?;
    let constrained_target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "ConstrainedGenericSelfTrait",
        "GenericWrapper",
        "constrained_generic_self_value",
    )?;
    let mut cases = Vec::new();

    for owner_name in [
        "call_inline_generic_bound_method",
        "call_where_generic_bound_method",
        "call_impl_trait_method",
        "call_trait_object_method",
    ] {
        cases.push((
            owner_name,
            function_id_by_name(&db, owner_name)?,
            "bound_value",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
            generic_target,
        ));
    }

    cases.push((
        "call_local_trait_object_binding_method",
        function_id_by_name(&db, "call_local_trait_object_binding_method")?,
        "bound_value",
        CallReceiver::TypedLocalBinding {
            name: "value".to_string(),
            type_path: path(&["GenericBoundTrait"]),
        },
        generic_target,
    ));

    for (module_path, owner_name) in [
        (
            &["crate", "trait_scope", "with_direct_import"][..],
            "call_direct_imported_trait_method",
        ),
        (
            &["crate", "trait_scope", "with_alias_import"],
            "call_alias_imported_trait_method",
        ),
        (
            &["crate", "trait_scope", "with_glob_import"],
            "call_glob_imported_trait_method",
        ),
        (
            &["crate", "trait_reexport_scope"],
            "call_reexported_trait_method",
        ),
        (
            &["crate", "grouped_trait_import_scope"],
            "call_grouped_imported_trait_method",
        ),
    ] {
        cases.push((
            owner_name,
            function_id_by_name_in_module(&db, module_path, owner_name)?,
            "scoped_value",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
            imported_target,
        ));
    }

    cases.push((
        "call_constrained_generic_self_trait_method",
        function_id_by_name(&db, "call_constrained_generic_self_trait_method")?,
        "constrained_generic_self_value",
        CallReceiver::LocalBinding {
            name: "value".to_string(),
        },
        constrained_target,
    ));

    for (owner_name, trait_name, method_name) in [
        (
            "call_blanket_trait_method",
            "BlanketDispatchTrait",
            "blanket_value",
        ),
        (
            "call_inline_bound_blanket_trait_method",
            "InlineBoundBlanketTrait",
            "inline_bound_value",
        ),
        (
            "call_where_bound_blanket_trait_method",
            "WhereBoundBlanketTrait",
            "where_bound_value",
        ),
        (
            "call_transitive_bound_blanket_trait_method",
            "TransitiveBoundBlanketTrait",
            "transitive_bound_value",
        ),
    ] {
        cases.push((
            owner_name,
            function_id_by_name(&db, owner_name)?,
            method_name,
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
            method_id_by_impl_trait_name(&db, trait_name, method_name)?,
        ));
    }

    let mut expected_edges = Vec::new();

    for (owner_name, owner, method_name, receiver, target) in cases {
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = row_by_method_receiver(&context, method_name, &receiver);
        let site = row.site.id;
        let span = row.site.span;
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Method,
            CallSiteKind::Method,
            CallRelationKind::Method,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 3, "{owner_name} proof fact count");
        expected_edges.push(OwnerProofEdge {
            owner,
            site,
            span,
            target,
        });
    }

    assert_owner_proof_edges(
        &db,
        "trait family method",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_result_and_field_receiver_method_call_proof_facts()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let clone_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "clone_assoc")?;
    let make_target = function_id_by_name(&db, "make_local_assoc")?;
    let ready_target = function_id_by_name(&db, "make_ready_local_assoc")?;
    let tuple_target = struct_id_by_name(&db, "TupleFieldMethodReceiver")?;
    let mut expected_edges = Vec::new();

    let owner = function_id_by_name(&db, "call_path_result_instance_method")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "path-result context rows: {context:#?}");
    let row = row_by_path(&context, &["make_local_assoc"]);
    assert_resolved_target(
        row,
        make_target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallRelationKind::Function,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: make_target,
    });

    let receiver = CallReceiver::PathCallResult {
        path: path(&["make_local_assoc"]),
    };
    let row = row_by_method_receiver(&context, "instance_value", &receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallRelationKind::Method,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: method_target,
    });
    assert_eq!(
        db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?,
        6
    );

    let owner = function_id_by_name(&db, "call_method_result_instance_method")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "method-result context rows: {context:#?}");
    let receiver = CallReceiver::TypedLocalBinding {
        name: "value".to_string(),
        type_path: path(&["LocalAssoc"]),
    };
    let row = row_by_method_receiver(&context, "clone_assoc", &receiver);
    assert_resolved_target(
        row,
        clone_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallRelationKind::Method,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: clone_target,
    });

    let receiver = CallReceiver::MethodCallResult {
        method_name: "clone_assoc".to_string(),
    };
    let row = row_by_method_receiver(&context, "instance_value", &receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallRelationKind::Method,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: method_target,
    });
    assert_eq!(
        db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?,
        6
    );

    let owner = function_id_by_name(&db, "call_await_result_instance_method")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "await-result context rows: {context:#?}");
    let row = row_by_path(&context, &["make_ready_local_assoc"]);
    assert_resolved_target(
        row,
        ready_target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallRelationKind::Function,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: ready_target,
    });

    let receiver = CallReceiver::AwaitPathCallResult {
        path: path(&["make_ready_local_assoc"]),
    };
    let row = row_by_method_receiver(&context, "instance_value", &receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallRelationKind::Method,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: method_target,
    });
    assert_eq!(
        db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?,
        6
    );

    let owner = function_id_by_name(&db, "call_tuple_field_instance_method")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "field-method context rows: {context:#?}");
    let row = row_by_path(&context, &["TupleFieldMethodReceiver"]);
    assert_resolved_target(
        row,
        tuple_target,
        CallRelationKind::TupleStructConstructor,
        CallSiteKind::Path,
        CallRelationKind::Struct,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: tuple_target,
    });

    let receiver = CallReceiver::FieldInitializedLocalBinding {
        name: "value".to_string(),
        init_path: path(&["TupleFieldMethodReceiver"]),
        field_path: path(&["0"]),
    };
    let row = row_by_method_receiver(&context, "instance_value", &receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallRelationKind::Method,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: method_target,
    });
    assert_eq!(
        db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?,
        6
    );

    assert_owner_proof_edges(
        &db,
        "result/field receiver method",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_proof_query_links_real_resolved_call_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_crate_local_target")?;
    let target = function_id_by_name(&db, "local_target")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");
    let site = context[0].site.id;

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 3);

    let rows = db.proof_graphrag_context(&target.to_string())?;
    assert_eq!(
        rows.len(),
        3,
        "real callee id query should return the edge plus linked call_site and call_resolution rows: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_site"
                && row.call_site_id.as_deref() == Some(site.to_string().as_str())
                && row.caller_def_id.as_deref() == Some(owner.to_string().as_str())
        }),
        "linked real call_site row missing: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_edge"
                && row.call_site_id.as_deref() == Some(site.to_string().as_str())
                && row.callee_def_id.as_deref() == Some(target.to_string().as_str())
        }),
        "matching real call_edge row missing: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(site.to_string().as_str())
                && row.blocker_reason.is_none()
        }),
        "linked real call_resolution row missing: {rows:#?}"
    );

    Ok(())
}

#[test]
fn fixture_proof_symbol_lookup_links_real_resolved_call_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_crate_local_target")?;
    let target = function_id_by_name(&db, "local_target")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");
    let site = context[0].site.id;

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 3);

    let rows = db.proof_symbol_lookup(&target.to_string())?;
    assert_eq!(
        rows.len(),
        3,
        "real symbol lookup should return the edge plus linked call_site and call_resolution rows: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_site"
                && row.call_site_id.as_deref() == Some(site.to_string().as_str())
                && row.caller_def_id.as_deref() == Some(owner.to_string().as_str())
        }),
        "linked real call_site row missing from symbol lookup: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_edge"
                && row.call_site_id.as_deref() == Some(site.to_string().as_str())
                && row.callee_def_id.as_deref() == Some(target.to_string().as_str())
        }),
        "matching real call_edge row missing from symbol lookup: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(site.to_string().as_str())
                && row.blocker_reason.is_none()
        }),
        "linked real call_resolution row missing from symbol lookup: {rows:#?}"
    );

    Ok(())
}

#[test]
fn fixture_proof_symbol_lookup_links_target_centered_local_target_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let path_owner = function_id_by_name(&db, "call_crate_local_target")?;
    let dynamic_owner = function_id_by_name(&db, "call_parenthesized_local_target")?;
    let callers = db.callers_for_target(target)?;
    let path_site = caller_by_owner_kind_path(
        &callers,
        path_owner,
        CallSiteKind::Path,
        &["crate", "local_target"],
    )
    .site
    .id;
    let dynamic_site = caller_by_owner_kind_path(
        &callers,
        dynamic_owner,
        CallSiteKind::Dynamic,
        &["local_target"],
    )
    .site
    .id;

    let count = db.project_call_proof_facts_for_target(target, "bd:fixture-call-graph")?;
    assert_eq!(count, callers.len() * 3);

    let rows = db.proof_symbol_lookup(&target.to_string())?;
    assert_eq!(
        rows.len(),
        count,
        "target-centered symbol lookup should return all linked caller facts: {rows:#?}"
    );

    for (owner, site) in [(path_owner, path_site), (dynamic_owner, dynamic_site)] {
        let owner_id = owner.to_string();
        let target_id = target.to_string();
        let site_id = site.to_string();
        let site_rows = rows
            .iter()
            .filter(|fact| fact.call_site_id.as_deref() == Some(site_id.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            site_rows.len(),
            3,
            "symbol lookup should include linked call_site, call_edge, and call_resolution facts for {site}: {site_rows:#?}"
        );
        assert_eq!(proof_kind_count(&site_rows, "call_site"), 1);
        assert_eq!(proof_kind_count(&site_rows, "call_edge"), 1);
        assert_eq!(proof_kind_count(&site_rows, "call_resolution"), 1);

        let site_fact = proof_fact_for_kind(&site_rows, "call_site");
        assert_eq!(site_fact.caller_def_id.as_deref(), Some(owner_id.as_str()));

        let edge = proof_fact_for_kind(&site_rows, "call_edge");
        assert_eq!(edge.caller_def_id.as_deref(), Some(owner_id.as_str()));
        assert_eq!(edge.callee_def_id.as_deref(), Some(target_id.as_str()));
        assert_eq!(edge.blocker_reason, None);

        let resolution = proof_fact_for_kind(&site_rows, "call_resolution");
        assert_eq!(resolution.blocker_reason, None);
    }

    Ok(())
}

#[test]
fn fixture_projection_marks_real_external_call_without_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_prelude_string_new")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");
    let site = context[0].site.id;

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 2);
    assert!(db.proof_checker_edges()?.is_empty());

    let rows = db.proof_graphrag_context("external_dependency_summary_missing")?;
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(site.to_string().as_str())
                && row.blocker_reason.as_deref() == Some("external_dependency_summary_missing")
        }),
        "external proof rows: {rows:#?}"
    );

    Ok(())
}

#[test]
fn fixture_projected_external_call_blocker_feeds_proof_invariants() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_prelude_string_new")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");
    let site = context[0].site.id;

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 2);

    db.upsert_proof_fact_values(&[serde_json::json!({
        "fact_kind": "effect_seed",
        "schema_version": "ploke-proof-facts.v1",
        "effect_seed_id": "effect:fixture-external-call",
        "call_site_id": site.to_string(),
        "effect_class": "operating_system_process_create",
        "confidence": "fixture-test",
        "blocker_if_unresolved": true,
        "evidence_use": "proof_only"
    })])?;

    let findings = db.proof_invariant_findings()?;
    let finding = findings
        .iter()
        .find(|finding| {
            finding.invariant == "detached_process_successor_handoff"
                && finding.call_site_id.as_deref() == Some(site.to_string().as_str())
        })
        .expect("detached process finding for fixture external call proof");
    assert_eq!(finding.status, ProofInvariantStatus::Blocked);
    assert!(
        finding
            .reason
            .contains("external_dependency_summary_missing"),
        "fixture external call blocker should feed proof invariants: {finding:#?}"
    );

    Ok(())
}

#[test]
fn fixture_projection_marks_real_macro_call_without_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let cases = [
        ("call_crate_scoped_macro", "crate::crate_scoped_macro"),
        ("call_vec_macro", "vec"),
        ("call_imported_macro_alias", "imported_macro_alias"),
        ("call_item_macro_inside_body", "call_graph_item_macro"),
    ];
    let mut expected = Vec::new();

    for (owner_name, macro_name) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = &context[0];
        let site = row.site.id;
        let span = row.site.span;
        assert_eq!(row.site.kind, CallSiteKind::Macro);
        assert_eq!(row.site.macro_name.as_deref(), Some(macro_name));
        assert_eq!(row.status.status, CallStatusKind::Unsupported);
        assert_eq!(row.status.resolution, None);
        assert!(
            row.targets.is_empty(),
            "{owner_name} macro proof setup must be targetless: {row:#?}"
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 2);
        expected.push((site, span));
    }

    assert!(db.proof_checker_edges()?.is_empty());
    let rows = db.proof_graphrag_context("macro_expansion_not_available")?;
    for (site, span) in expected {
        assert!(
            rows.iter().any(|row| {
                row.kind == "call_resolution"
                    && row.call_site_id.as_deref() == Some(site.to_string().as_str())
                    && row.blocker_reason.as_deref() == Some("macro_expansion_not_available")
            }),
            "macro proof rows missing {site}: {rows:#?}"
        );

        let provenance = db
            .proof_source_provenance(&site.to_string())?
            .expect("projected fixture macro call-site source provenance");
        assert!(
            provenance
                .source_file
                .ends_with("fixture_call_graph/src/lib.rs"),
            "source provenance: {provenance:#?}"
        );
        assert_eq!(provenance.start_byte, span.0);
        assert_eq!(provenance.end_byte, span.1);
    }

    Ok(())
}

#[test]
fn fixture_projection_marks_real_ambiguous_call_without_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_ambiguous_trait_method")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");
    let row = &context[0];
    let site = row.site.id;
    let span = row.site.span;
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(row.site.method.as_deref(), Some("overlap"));
    assert_eq!(row.status.status, CallStatusKind::Ambiguous);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "ambiguous proof setup must be targetless: {row:#?}"
    );

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 2);
    assert!(db.proof_checker_edges()?.is_empty());

    let rows = db.proof_graphrag_context("type_resolution_missing")?;
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(site.to_string().as_str())
                && row.blocker_reason.as_deref() == Some("type_resolution_missing")
        }),
        "ambiguous proof rows: {rows:#?}"
    );

    let provenance = db
        .proof_source_provenance(&site.to_string())?
        .expect("projected fixture ambiguous call-site source provenance");
    assert!(
        provenance
            .source_file
            .ends_with("fixture_call_graph/src/lib.rs"),
        "source provenance: {provenance:#?}"
    );
    assert_eq!(provenance.start_byte, span.0);
    assert_eq!(provenance.end_byte, span.1);

    Ok(())
}

#[test]
fn fixture_projection_stores_real_constructor_call_proof_facts() -> Result<(), DbError> {
    for case in constructor_cases() {
        let db = setup_call_graph_fixture_db(case.fixture)?;
        let resolved = assert_constructor_context(&db, case)?;

        let count = db.project_call_proof_facts_for_owner(resolved.owner, case.domain)?;
        assert_eq!(
            count, resolved.proof_count,
            "{} owner-scoped proof count",
            case.label
        );

        let edges = db.proof_checker_edges()?;
        assert_proof_edge(&edges, case, &resolved);
        assert_provenance(&db, case, &resolved)?;
    }

    Ok(())
}

#[test]
fn fixture_projection_stores_real_dynamic_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_aliased_indexed_named_field_function_binding")?;
    let target = function_id_by_name(&db, "local_target")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");
    let row = row_by_kind_path(
        &context,
        CallSiteKind::Dynamic,
        &["alias", "callbacks", "0"],
    );
    let site = row.site.id;
    let span = row.site.span;
    assert_resolved_target(
        row,
        target,
        CallRelationKind::DynamicFunction,
        CallSiteKind::Dynamic,
        CallRelationKind::Function,
    );

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 3);

    assert_owner_proof_edges(
        &db,
        "dynamic",
        &[OwnerProofEdge {
            owner,
            site,
            span,
            target,
        }],
        "fixture_call_graph/src/lib.rs",
        "dynamic_dispatch_unbounded",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_branch_and_match_dynamic_call_proof_facts() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let mut expected_edges = Vec::new();

    for owner_name in [
        "call_if_same_function_item",
        "call_match_same_function_item",
    ] {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = row_by_kind_path(&context, CallSiteKind::Dynamic, &["local_target"]);
        let site = row.site.id;
        let span = row.site.span;
        assert_resolved_target(
            row,
            target,
            CallRelationKind::DynamicFunction,
            CallSiteKind::Dynamic,
            CallRelationKind::Function,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 3);
        expected_edges.push(OwnerProofEdge {
            owner,
            site,
            span,
            target,
        });
    }

    assert_owner_proof_edges(
        &db,
        "branch/match dynamic",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "dynamic_dispatch_unbounded",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_callable_expression_dynamic_call_proof_facts()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let cases: [(&str, &[&str]); 11] = [
        ("call_parenthesized_local_target", &["local_target"]),
        ("call_parenthesized_function_item_binding", &["f"]),
        ("call_parenthesized_aliased_function_item_binding", &["g"]),
        (
            "call_parenthesized_typed_function_pointer_alias_binding",
            &["g"],
        ),
        ("call_function_pointer_cast_path", &["local_target"]),
        ("call_function_pointer_cast_binding", &["f"]),
        ("call_dereferenced_function_pointer_binding", &["f"]),
        ("call_block_function_item", &["local_target"]),
        ("call_indexed_initialized_function_array", &["funcs", "0"]),
        (
            "call_typed_indexed_initialized_function_array",
            &["funcs", "0"],
        ),
        (
            "call_aliased_indexed_initialized_function_array",
            &["alias", "0"],
        ),
    ];
    let mut expected_edges = Vec::new();

    for (owner_name, expected_path) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = row_by_kind_path(&context, CallSiteKind::Dynamic, expected_path);
        let site = row.site.id;
        let span = row.site.span;
        assert_resolved_target(
            row,
            target,
            CallRelationKind::DynamicFunction,
            CallSiteKind::Dynamic,
            CallRelationKind::Function,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 3);
        expected_edges.push(OwnerProofEdge {
            owner,
            site,
            span,
            target,
        });
    }

    assert_owner_proof_edges(
        &db,
        "callable dynamic",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "dynamic_dispatch_unbounded",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_field_dynamic_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let cases: [(&str, &[&str]); 8] = [
        ("call_named_field_function_binding", &["holder", "callback"]),
        (
            "call_aliased_named_field_function_binding",
            &["alias", "callback"],
        ),
        (
            "call_indexed_named_field_function_binding",
            &["holder", "callbacks", "0"],
        ),
        (
            "call_indexed_named_field_array_alias_binding",
            &["holder", "callbacks", "0"],
        ),
        (
            "call_aliased_indexed_named_field_function_binding",
            &["alias", "callbacks", "0"],
        ),
        (
            "call_indexed_tuple_field_function_binding",
            &["holder", "0", "0"],
        ),
        (
            "call_indexed_tuple_field_array_alias_binding",
            &["holder", "0", "0"],
        ),
        (
            "call_aliased_indexed_tuple_field_function_binding",
            &["alias", "0", "0"],
        ),
    ];
    let mut expected_edges = Vec::new();

    for (owner_name, expected_path) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        let expected_count = context
            .iter()
            .map(|row| 2 + row.targets.len())
            .sum::<usize>();
        let row = row_by_kind_path(&context, CallSiteKind::Dynamic, expected_path);
        let site = row.site.id;
        let span = row.site.span;
        assert_resolved_target(
            row,
            target,
            CallRelationKind::DynamicFunction,
            CallSiteKind::Dynamic,
            CallRelationKind::Function,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, expected_count, "{owner_name} proof fact count");
        expected_edges.push(OwnerProofEdge {
            owner,
            site,
            span,
            target,
        });
    }

    assert_owner_proof_edges(
        &db,
        "field dynamic",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "dynamic_dispatch_unbounded",
        ProofEdgeCount::AtLeast,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_marks_real_unsupported_dynamic_call_without_edges() -> Result<(), DbError> {
    for owner_name in [
        "call_closure_binding_cast",
        "call_dereferenced_closure_binding",
    ] {
        let db = setup_call_graph_fixture_db("fixture_call_graph")?;
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = &context[0];
        let site = row.site.id;
        let span = row.site.span;
        assert_eq!(row.site.kind, CallSiteKind::Dynamic);
        assert_eq!(row.status.status, CallStatusKind::Unsupported);
        assert_eq!(row.status.resolution, None);
        assert!(
            row.targets.is_empty(),
            "{owner_name} unsupported dynamic proof setup must be targetless: {row:#?}"
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 2);
        assert!(db.proof_checker_edges()?.is_empty());

        let rows = db.proof_graphrag_context("dynamic_dispatch_unbounded")?;
        assert!(
            rows.iter().any(|row| {
                row.kind == "call_resolution"
                    && row.call_site_id.as_deref() == Some(site.to_string().as_str())
                    && row.blocker_reason.as_deref() == Some("dynamic_dispatch_unbounded")
            }),
            "{owner_name} unsupported dynamic proof rows: {rows:#?}"
        );

        let provenance = db
            .proof_source_provenance(&site.to_string())?
            .expect("projected fixture unsupported dynamic call-site source provenance");
        assert!(
            provenance
                .source_file
                .ends_with("fixture_call_graph/src/lib.rs"),
            "{owner_name} source provenance: {provenance:#?}"
        );
        assert_eq!(provenance.start_byte, span.0);
        assert_eq!(provenance.end_byte, span.1);
    }

    Ok(())
}

#[test]
fn fixture_projection_marks_real_branch_and_match_dynamic_failures_without_edges()
-> Result<(), DbError> {
    let cases = [
        (
            "call_if_ambiguous_function_item",
            CallStatusKind::Ambiguous,
            "type_resolution_missing",
        ),
        (
            "call_match_ambiguous_function_item",
            CallStatusKind::Ambiguous,
            "type_resolution_missing",
        ),
        (
            "call_match_guarded_function_item",
            CallStatusKind::Unsupported,
            "dynamic_dispatch_unbounded",
        ),
        (
            "call_if_closure_branch",
            CallStatusKind::Unsupported,
            "dynamic_dispatch_unbounded",
        ),
        (
            "call_match_closure_arm",
            CallStatusKind::Unsupported,
            "dynamic_dispatch_unbounded",
        ),
        (
            "call_if_function_pointer_param_branch",
            CallStatusKind::Unsupported,
            "dynamic_dispatch_unbounded",
        ),
        (
            "call_match_function_pointer_param_arm",
            CallStatusKind::Unsupported,
            "dynamic_dispatch_unbounded",
        ),
        (
            "call_if_nested_branch_expression",
            CallStatusKind::Unsupported,
            "dynamic_dispatch_unbounded",
        ),
        (
            "call_match_nested_arm_expression",
            CallStatusKind::Unsupported,
            "dynamic_dispatch_unbounded",
        ),
    ];

    for (owner_name, expected_status, blocker_reason) in cases {
        let db = setup_call_graph_fixture_db("fixture_call_graph")?;
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = &context[0];
        let site = row.site.id;
        let span = row.site.span;
        assert_eq!(row.site.kind, CallSiteKind::Dynamic);
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.status.status, expected_status);
        assert_eq!(row.status.resolution, None);
        assert!(
            row.targets.is_empty(),
            "{owner_name} dynamic failure proof setup must be targetless: {row:#?}"
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 2);
        assert!(
            db.proof_checker_edges()?.is_empty(),
            "{owner_name} dynamic failure must not fabricate proof edges"
        );

        let rows = db.proof_graphrag_context(blocker_reason)?;
        assert!(
            rows.iter().any(|row| {
                row.kind == "call_resolution"
                    && row.call_site_id.as_deref() == Some(site.to_string().as_str())
                    && row.blocker_reason.as_deref() == Some(blocker_reason)
            }),
            "{owner_name} dynamic failure proof rows: {rows:#?}"
        );

        let provenance = db
            .proof_source_provenance(&site.to_string())?
            .expect("projected fixture branch/match dynamic call-site source provenance");
        assert!(
            provenance
                .source_file
                .ends_with("fixture_call_graph/src/lib.rs"),
            "{owner_name} source provenance: {provenance:#?}"
        );
        assert_eq!(provenance.start_byte, span.0);
        assert_eq!(provenance.end_byte, span.1);
    }

    Ok(())
}

#[test]
fn fixture_projection_stores_real_returned_function_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_returned_function")?;
    let target = function_id_by_name(&db, "make_fn")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "returned function proof context rows: {context:#?}"
    );

    let path_row = row_by_path(&context, &["make_fn"]);
    let path_site = path_row.site.id;
    let path_span = path_row.site.span;
    assert_resolved_target(
        path_row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallRelationKind::Function,
    );

    let dynamic_rows = context
        .iter()
        .filter(|row| row.site.kind == CallSiteKind::Dynamic)
        .collect::<Vec<_>>();
    assert_eq!(
        dynamic_rows.len(),
        1,
        "expected one outer returned-function dynamic row: {context:#?}"
    );
    let dynamic_row = dynamic_rows[0];
    let dynamic_site = dynamic_row.site.id;
    let dynamic_span = dynamic_row.site.span;
    assert_eq!(dynamic_row.status.status, CallStatusKind::Unsupported);
    assert_eq!(dynamic_row.status.resolution, None);
    assert!(
        dynamic_row.targets.is_empty(),
        "returned-function dynamic proof setup must be targetless: {dynamic_row:#?}"
    );

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 5);

    let edges = db.proof_checker_edges()?;
    assert_eq!(edges.len(), 1, "returned-function proof edges: {edges:#?}");
    assert_eq!(edges[0].call_site_id, path_site.to_string());
    assert_eq!(edges[0].caller_def_id, owner.to_string());
    assert_eq!(
        edges[0].callee_def_id.as_deref(),
        Some(target.to_string().as_str())
    );
    assert_eq!(edges[0].resolution_state, "resolved");
    assert!(edges[0].blocker_reason.is_none());

    let blocked = db.proof_graphrag_context("dynamic_dispatch_unbounded")?;
    assert!(
        blocked.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(dynamic_site.to_string().as_str())
                && row.blocker_reason.as_deref() == Some("dynamic_dispatch_unbounded")
        }),
        "returned-function dynamic blocker rows: {blocked:#?}"
    );

    for (site, span) in [(path_site, path_span), (dynamic_site, dynamic_span)] {
        let provenance = db
            .proof_source_provenance(&site.to_string())?
            .expect("projected returned-function call-site source provenance");
        assert!(
            provenance
                .source_file
                .ends_with("fixture_call_graph/src/lib.rs"),
            "source provenance: {provenance:#?}"
        );
        assert_eq!(provenance.start_byte, span.0);
        assert_eq!(provenance.end_byte, span.1);
    }

    Ok(())
}

#[test]
fn fixture_projection_marks_real_callable_path_and_vec_external_rows_without_edges()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let mut type_missing = Vec::new();
    let mut external_missing = Vec::new();

    for (owner_name, expected_path) in [
        ("call_function_pointer_param", &["f"][..]),
        ("call_generic_fn_once_value_binding", &["generic_f"][..]),
    ] {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(
            context.len(),
            1,
            "{owner_name} proof context rows: {context:#?}"
        );

        let row = row_by_path(&context, expected_path);
        assert_eq!(row.status.status, CallStatusKind::Unsupported);
        assert_eq!(row.status.resolution, None);
        assert!(
            row.targets.is_empty(),
            "{owner_name} callable path proof setup must be targetless: {row:#?}"
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 2);
        type_missing.push((row.site.id, row.site.span));
    }

    let owner = function_id_by_name(&db, "call_boxed_dyn_fn_value_binding")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "boxed dyn Fn proof context rows: {context:#?}"
    );
    let box_new = row_by_path(&context, &["Box", "new"]);
    assert_eq!(box_new.status.status, CallStatusKind::External);
    assert!(
        box_new.targets.is_empty(),
        "Box::new proof setup must be targetless: {box_new:#?}"
    );
    let boxed_fn = row_by_path(&context, &["boxed_fn"]);
    assert_eq!(boxed_fn.status.status, CallStatusKind::Unsupported);
    assert!(
        boxed_fn.targets.is_empty(),
        "boxed dyn Fn proof setup must be targetless: {boxed_fn:#?}"
    );

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 4);
    external_missing.push((box_new.site.id, box_new.site.span));
    type_missing.push((boxed_fn.site.id, boxed_fn.site.span));

    let owner = function_id_by_name(&db, "call_prelude_vec_new")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "Vec::new proof context rows: {context:#?}"
    );
    let vec_new = row_by_path(&context, &["Vec", "new"]);
    assert_eq!(vec_new.status.status, CallStatusKind::External);
    assert!(
        vec_new.targets.is_empty(),
        "Vec::new proof setup must be targetless: {vec_new:#?}"
    );

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 2);
    external_missing.push((vec_new.site.id, vec_new.site.span));

    assert!(db.proof_checker_edges()?.is_empty());

    let rows = db.proof_graphrag_context("type_resolution_missing")?;
    for (site, _) in &type_missing {
        assert!(
            rows.iter().any(|row| {
                row.kind == "call_resolution"
                    && row.call_site_id.as_deref() == Some(site.to_string().as_str())
                    && row.blocker_reason.as_deref() == Some("type_resolution_missing")
            }),
            "callable path proof rows missing {site}: {rows:#?}"
        );
    }

    let rows = db.proof_graphrag_context("external_dependency_summary_missing")?;
    for (site, _) in &external_missing {
        assert!(
            rows.iter().any(|row| {
                row.kind == "call_resolution"
                    && row.call_site_id.as_deref() == Some(site.to_string().as_str())
                    && row.blocker_reason.as_deref() == Some("external_dependency_summary_missing")
            }),
            "external callable proof rows missing {site}: {rows:#?}"
        );
    }

    for (site, span) in type_missing.into_iter().chain(external_missing) {
        let provenance = db
            .proof_source_provenance(&site.to_string())?
            .expect("projected callable path source provenance");
        assert!(
            provenance
                .source_file
                .ends_with("fixture_call_graph/src/lib.rs"),
            "source provenance: {provenance:#?}"
        );
        assert_eq!(provenance.start_byte, span.0);
        assert_eq!(provenance.end_byte, span.1);
    }

    Ok(())
}

#[test]
fn fixture_projection_marks_real_parenthesized_callable_dynamic_rows_without_edges()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let mut dynamic_missing = Vec::new();
    let mut external_missing = Vec::new();

    let owner = function_id_by_name(&db, "call_parenthesized_generic_fn_once_value_binding")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "parenthesized generic FnOnce proof context rows: {context:#?}"
    );
    let generic_f = row_by_kind_path(&context, CallSiteKind::Dynamic, &["generic_f"]);
    assert_eq!(generic_f.status.status, CallStatusKind::Unsupported);
    assert_eq!(generic_f.status.resolution, None);
    assert!(
        generic_f.targets.is_empty(),
        "parenthesized generic FnOnce dynamic proof setup must be targetless: {generic_f:#?}"
    );

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 2);
    dynamic_missing.push((generic_f.site.id, generic_f.site.span));

    let owner = function_id_by_name(&db, "call_parenthesized_boxed_dyn_fn_value_binding")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "parenthesized boxed dyn Fn proof context rows: {context:#?}"
    );
    let box_new = row_by_path(&context, &["Box", "new"]);
    assert_eq!(box_new.status.status, CallStatusKind::External);
    assert_eq!(box_new.status.resolution, None);
    assert!(
        box_new.targets.is_empty(),
        "parenthesized boxed dyn Fn Box::new proof setup must be targetless: {box_new:#?}"
    );
    let boxed_fn = row_by_kind_path(&context, CallSiteKind::Dynamic, &["boxed_fn"]);
    assert_eq!(boxed_fn.status.status, CallStatusKind::Unsupported);
    assert_eq!(boxed_fn.status.resolution, None);
    assert!(
        boxed_fn.targets.is_empty(),
        "parenthesized boxed dyn Fn dynamic proof setup must be targetless: {boxed_fn:#?}"
    );

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 4);
    external_missing.push((box_new.site.id, box_new.site.span));
    dynamic_missing.push((boxed_fn.site.id, boxed_fn.site.span));

    assert!(db.proof_checker_edges()?.is_empty());

    let rows = db.proof_graphrag_context("dynamic_dispatch_unbounded")?;
    for (site, _) in &dynamic_missing {
        assert!(
            rows.iter().any(|row| {
                row.kind == "call_resolution"
                    && row.call_site_id.as_deref() == Some(site.to_string().as_str())
                    && row.blocker_reason.as_deref() == Some("dynamic_dispatch_unbounded")
            }),
            "parenthesized callable dynamic proof rows missing {site}: {rows:#?}"
        );
    }

    let rows = db.proof_graphrag_context("external_dependency_summary_missing")?;
    for (site, _) in &external_missing {
        assert!(
            rows.iter().any(|row| {
                row.kind == "call_resolution"
                    && row.call_site_id.as_deref() == Some(site.to_string().as_str())
                    && row.blocker_reason.as_deref() == Some("external_dependency_summary_missing")
            }),
            "parenthesized callable external proof rows missing {site}: {rows:#?}"
        );
    }

    for (site, span) in dynamic_missing.into_iter().chain(external_missing) {
        let provenance = db
            .proof_source_provenance(&site.to_string())?
            .expect("projected parenthesized callable source provenance");
        assert!(
            provenance
                .source_file
                .ends_with("fixture_call_graph/src/lib.rs"),
            "source provenance: {provenance:#?}"
        );
        assert_eq!(provenance.start_byte, span.0);
        assert_eq!(provenance.end_byte, span.1);
    }

    Ok(())
}

#[test]
fn fixture_projection_stores_real_const_and_static_initializer_call_proof_facts()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_nodes")?;
    let target = function_id_by_name_in_module(&db, &["crate", "const_static"], "five")?;
    let cases = [
        (
            const_id_by_name(&db, "FN_CALL_CONST")?,
            "const initializer proof context rows",
        ),
        (
            static_id_by_name(&db, "STATIC_FN_CALL")?,
            "static initializer proof context rows",
        ),
    ];
    let mut expected = Vec::new();

    for (owner, label) in cases {
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{label}: {context:#?}");

        let row = row_by_path(&context, &["five"]);
        let site = row.site.id;
        let span = row.site.span;
        assert_eq!(row.site.owner_id, owner);
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallRelationKind::Function,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-nodes")?;
        assert_eq!(count, 3);
        expected.push((owner, site, span));
    }

    let edges = db.proof_checker_edges()?;
    assert_eq!(
        edges.len(),
        expected.len(),
        "initializer proof edges: {edges:#?}"
    );
    for (owner, site, span) in expected {
        assert!(
            edges.iter().any(|edge| {
                edge.call_site_id == site.to_string()
                    && edge.caller_def_id == owner.to_string()
                    && edge.callee_def_id.as_deref() == Some(target.to_string().as_str())
                    && edge.resolution_state == "resolved"
                    && edge.blocker_reason.is_none()
            }),
            "initializer proof edge missing for {site}: {edges:#?}"
        );

        let provenance = db
            .proof_source_provenance(&site.to_string())?
            .expect("projected const/static initializer call-site source provenance");
        assert!(
            provenance
                .source_file
                .ends_with("fixture_nodes/src/const_static.rs"),
            "source provenance: {provenance:#?}"
        );
        assert_eq!(provenance.start_byte, span.0);
        assert_eq!(provenance.end_byte, span.1);
    }

    assert!(
        db.proof_graphrag_context("type_resolution_missing")?
            .is_empty(),
        "resolved const/static initializer proofs should not produce blockers"
    );

    Ok(())
}

#[test]
fn fixture_projection_stores_real_associated_const_initializer_call_proof_facts()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "assoc_const_value")?;
    let cases = [
        const_id_by_name(&db, "IMPL_ASSOC_VALUE")?,
        const_id_by_name(&db, "TRAIT_ASSOC_VALUE")?,
    ];
    let mut expected = Vec::new();

    for owner in cases {
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(
            context.len(),
            1,
            "associated const initializer proof context rows: {context:#?}"
        );

        let row = row_by_path(&context, &["assoc_const_value"]);
        let site = row.site.id;
        let span = row.site.span;
        assert_eq!(row.site.owner_id, owner);
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallRelationKind::Function,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 3);
        expected.push((owner, site, span));
    }

    let edges = db.proof_checker_edges()?;
    assert_eq!(
        edges.len(),
        expected.len(),
        "associated const initializer proof edges: {edges:#?}"
    );
    for (owner, site, span) in expected {
        assert!(
            edges.iter().any(|edge| {
                edge.call_site_id == site.to_string()
                    && edge.caller_def_id == owner.to_string()
                    && edge.callee_def_id.as_deref() == Some(target.to_string().as_str())
                    && edge.resolution_state == "resolved"
                    && edge.blocker_reason.is_none()
            }),
            "associated const initializer proof edge missing for {site}: {edges:#?}"
        );

        let provenance = db
            .proof_source_provenance(&site.to_string())?
            .expect("projected associated const initializer call-site source provenance");
        assert!(
            provenance
                .source_file
                .ends_with("fixture_call_graph/src/lib.rs"),
            "source provenance: {provenance:#?}"
        );
        assert_eq!(provenance.start_byte, span.0);
        assert_eq!(provenance.end_byte, span.1);
    }

    assert!(
        db.proof_graphrag_context("type_resolution_missing")?
            .is_empty(),
        "resolved associated const initializer proofs should not produce blockers"
    );

    Ok(())
}

#[test]
fn fixture_projection_stores_real_multi_row_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_try_result_instance_method")?;
    let try_target = function_id_by_name(&db, "try_local_assoc")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 3, "context rows: {context:#?}");
    let ok_site = row_by_path(&context, &["Ok"]).site.id;
    let try_site = row_by_path(&context, &["try_local_assoc"]).site.id;
    let receiver = CallReceiver::TryPathCallResult {
        path: path(&["try_local_assoc"]),
    };
    let method_site = row_by_method_receiver(&context, "instance_value", &receiver)
        .site
        .id;

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 8);

    let mut edges = db.proof_checker_edges()?;
    edges.sort_by(|left, right| left.call_site_id.cmp(&right.call_site_id));
    assert_eq!(edges.len(), 2, "proof checker edges: {edges:#?}");
    assert!(
        edges
            .iter()
            .all(|edge| edge.caller_def_id == owner.to_string())
    );
    assert!(edges.iter().all(|edge| edge.resolution_state == "resolved"));
    assert!(edges.iter().all(|edge| edge.blocker_reason.is_none()));
    assert!(
        edges.iter().any(|edge| {
            edge.call_site_id == try_site.to_string()
                && edge.callee_def_id.as_deref() == Some(try_target.to_string().as_str())
        }),
        "try path proof edges: {edges:#?}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge.call_site_id == method_site.to_string()
                && edge.callee_def_id.as_deref() == Some(method_target.to_string().as_str())
        }),
        "method proof edges: {edges:#?}"
    );

    let blocked = db.proof_graphrag_context("type_resolution_missing")?;
    assert!(
        blocked.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(ok_site.to_string().as_str())
                && row.blocker_reason.as_deref() == Some("type_resolution_missing")
        }),
        "blocked proof rows: {blocked:#?}"
    );

    for site in [ok_site, try_site, method_site] {
        let provenance = db
            .proof_source_provenance(&site.to_string())?
            .expect("projected fixture call-site source provenance");
        assert!(
            provenance
                .source_file
                .ends_with("fixture_call_graph/src/lib.rs"),
            "source provenance: {provenance:#?}"
        );
    }

    Ok(())
}

#[test]
fn fixture_projection_links_mixed_owner_proof_rows_to_call_context() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_try_result_instance_method")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 3, "context rows: {context:#?}");

    let expected_fact_count = context
        .iter()
        .map(|row| 2 + row.targets.len())
        .sum::<usize>();
    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(
        count, expected_fact_count,
        "proof projection should emit one call_site and one call_resolution per site plus resolved edges"
    );

    let proof_rows = db.proof_graphrag_context("")?;
    let resolved_context = context
        .iter()
        .filter(|row| row.status.status == CallStatusKind::Resolved)
        .count();
    let checker_edges = db.proof_checker_edges()?;
    assert_eq!(
        checker_edges.len(),
        resolved_context,
        "proof checker edges should match resolved call-context rows: {checker_edges:#?}"
    );

    for row in &context {
        let site = row.site.id.to_string();
        let site_rows = proof_rows
            .iter()
            .filter(|fact| fact.call_site_id.as_deref() == Some(site.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            proof_kind_count(&site_rows, "call_site"),
            1,
            "projected proof rows should include one call_site fact for {site}: {site_rows:#?}"
        );
        assert_eq!(
            proof_kind_count(&site_rows, "call_resolution"),
            1,
            "projected proof rows should include one call_resolution fact for {site}: {site_rows:#?}"
        );

        let edge_rows = site_rows
            .iter()
            .filter(|fact| fact.kind == "call_edge")
            .collect::<Vec<_>>();
        match row.status.status {
            CallStatusKind::Resolved => {
                assert_eq!(
                    edge_rows.len(),
                    1,
                    "resolved call site {site} should project one call_edge fact: {site_rows:#?}"
                );
                let owner_id = owner.to_string();
                let target_id = row.targets[0].target_id.to_string();
                assert_eq!(
                    edge_rows[0].caller_def_id.as_deref(),
                    Some(owner_id.as_str())
                );
                assert_eq!(
                    edge_rows[0].callee_def_id.as_deref(),
                    Some(target_id.as_str())
                );
                assert_eq!(edge_rows[0].blocker_reason, None);
            }
            CallStatusKind::Unresolved
            | CallStatusKind::Ambiguous
            | CallStatusKind::External
            | CallStatusKind::Unsupported => {
                assert!(
                    edge_rows.is_empty(),
                    "non-resolved call site {site} must not project call_edge facts: {site_rows:#?}"
                );
                let resolution = proof_fact_for_kind(&site_rows, "call_resolution");
                assert!(
                    resolution.blocker_reason.is_some(),
                    "non-resolved call site {site} should project a blocker reason: {site_rows:#?}"
                );
            }
        }
    }

    Ok(())
}

#[test]
fn fixture_projection_links_target_centered_proof_rows_to_callers() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let callers = db.callers_for_target(target)?;
    assert!(
        callers.len() >= 2,
        "local_target should expose multiple incoming callers: {callers:#?}"
    );
    assert!(
        callers.iter().all(|caller| {
            caller.target.target_id == target
                && caller.status.status == CallStatusKind::Resolved
                && caller.status.resolution == Some(CallResolutionKind::LocalExact)
        }),
        "target-centered proof linkage setup should use resolved callers for the seed target: {callers:#?}"
    );

    let count = db.project_call_proof_facts_for_target(target, "bd:fixture-call-graph")?;
    assert_eq!(
        count,
        callers.len() * 3,
        "target-centered proof projection should emit call_site, call_edge, and call_resolution facts per incoming caller"
    );

    let proof_rows = db.proof_graphrag_context("")?;
    assert_eq!(
        proof_rows.len(),
        count,
        "target-centered projection should not store unrelated proof rows: {proof_rows:#?}"
    );
    let checker_edges = db.proof_checker_edges()?;
    assert_eq!(
        checker_edges.len(),
        callers.len(),
        "proof checker edges should match target-centered caller rows: {checker_edges:#?}"
    );

    for caller in &callers {
        let site = caller.site.id.to_string();
        let site_rows = proof_rows
            .iter()
            .filter(|fact| fact.call_site_id.as_deref() == Some(site.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            proof_kind_count(&site_rows, "call_site"),
            1,
            "target-centered proof rows should include one call_site fact for {site}: {site_rows:#?}"
        );
        assert_eq!(
            proof_kind_count(&site_rows, "call_resolution"),
            1,
            "target-centered proof rows should include one call_resolution fact for {site}: {site_rows:#?}"
        );
        assert_eq!(
            proof_kind_count(&site_rows, "call_edge"),
            1,
            "target-centered proof rows should include one call_edge fact for {site}: {site_rows:#?}"
        );

        let edge = proof_fact_for_kind(&site_rows, "call_edge");
        let owner_id = caller.site.owner_id.to_string();
        let target_id = target.to_string();
        assert_eq!(edge.caller_def_id.as_deref(), Some(owner_id.as_str()));
        assert_eq!(edge.callee_def_id.as_deref(), Some(target_id.as_str()));
        assert_eq!(edge.blocker_reason, None);

        let resolution = proof_fact_for_kind(&site_rows, "call_resolution");
        assert_eq!(resolution.blocker_reason, None);

        let provenance = db
            .proof_source_provenance(&site)?
            .expect("projected target-centered caller source provenance");
        assert_eq!(provenance.call_site_id, site);
        assert!(
            provenance
                .source_file
                .ends_with("fixture_call_graph/src/lib.rs"),
            "source provenance: {provenance:#?}"
        );
        assert_eq!(provenance.start_byte, caller.site.span.0);
        assert_eq!(provenance.end_byte, caller.site.span.1);
    }

    Ok(())
}

#[test]
fn fixture_projection_stores_real_target_centered_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "try_local_assoc")?;
    let owner = function_id_by_name(&db, "call_try_result_instance_method")?;
    let context = db.call_context_for_owner(owner)?;
    let site = row_by_path(&context, &["try_local_assoc"]).site.id;

    let count = db.project_call_proof_facts_for_target(target, "bd:fixture-call-graph")?;
    assert_eq!(count, 3);

    let edges = db.proof_checker_edges()?;
    assert_eq!(edges.len(), 1, "target-centered proof edges: {edges:#?}");
    assert_eq!(edges[0].call_site_id, site.to_string());
    assert_eq!(edges[0].caller_def_id, owner.to_string());
    assert_eq!(
        edges[0].callee_def_id.as_deref(),
        Some(target.to_string().as_str())
    );
    assert_eq!(edges[0].resolution_state, "resolved");
    assert!(edges[0].blocker_reason.is_none());

    assert!(
        db.proof_graphrag_context("type_resolution_missing")?
            .is_empty(),
        "target-centered projection should not include unrelated unsupported calls from the owner"
    );

    let provenance = db
        .proof_source_provenance(&site.to_string())?
        .expect("projected target-centered call-site source provenance");
    assert!(
        provenance
            .source_file
            .ends_with("fixture_call_graph/src/lib.rs"),
        "source provenance: {provenance:#?}"
    );

    Ok(())
}

#[test]
fn fixture_projection_stores_real_target_centered_dynamic_call_proof_facts() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let owner = function_id_by_name(&db, "call_aliased_indexed_named_field_function_binding")?;
    let context = db.call_context_for_owner(owner)?;
    let site = row_by_kind_path(
        &context,
        CallSiteKind::Dynamic,
        &["alias", "callbacks", "0"],
    )
    .site
    .id;

    let callers = db.callers_for_target(target)?;
    assert!(
        callers.len() >= 2,
        "local_target should have multiple incoming callers: {callers:#?}"
    );
    assert!(
        callers
            .iter()
            .all(|caller| caller.target.target_id == target),
        "target-centered dynamic proof setup returned mismatched target rows: {callers:#?}"
    );
    assert!(
        callers
            .iter()
            .all(|caller| caller.status.status == CallStatusKind::Resolved
                && caller.status.resolution == Some(CallResolutionKind::LocalExact)),
        "target-centered dynamic proof setup should only include resolved local callers: {callers:#?}"
    );
    let dynamic = caller_by_owner_kind_path(
        &callers,
        owner,
        CallSiteKind::Dynamic,
        &["alias", "callbacks", "0"],
    );
    assert_eq!(dynamic.target.relation, CallRelationKind::DynamicFunction);

    let count = db.project_call_proof_facts_for_target(target, "bd:fixture-call-graph")?;
    assert_eq!(count, callers.len() * 3);

    let edges = db.proof_checker_edges()?;
    assert_eq!(
        edges.len(),
        callers.len(),
        "target-centered dynamic proof edges: {edges:#?}"
    );
    assert!(
        edges.iter().all(
            |edge| edge.callee_def_id.as_deref() == Some(target.to_string().as_str())
                && edge.resolution_state == "resolved"
                && edge.blocker_reason.is_none()
        ),
        "target-centered dynamic proof edges should all resolve to the seed target: {edges:#?}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge.call_site_id == site.to_string() && edge.caller_def_id == owner.to_string()
        }),
        "dynamic incoming proof edge missing: {edges:#?}"
    );

    assert!(
        db.proof_graphrag_context("dynamic_dispatch_unbounded")?
            .is_empty(),
        "target-centered dynamic projection should not include unrelated unsupported calls"
    );

    let provenance = db
        .proof_source_provenance(&site.to_string())?
        .expect("projected target-centered dynamic call-site source provenance");
    assert!(
        provenance
            .source_file
            .ends_with("fixture_call_graph/src/lib.rs"),
        "source provenance: {provenance:#?}"
    );

    Ok(())
}

#[test]
fn fixture_projection_stores_real_target_centered_constructor_call_proof_facts()
-> Result<(), DbError> {
    for case in constructor_cases() {
        let db = setup_call_graph_fixture_db(case.fixture)?;
        let resolved = assert_constructor_context(&db, case)?;
        let callers = assert_constructor_callers(&db, case, &resolved)?;

        let count = db.project_call_proof_facts_for_target(resolved.target, case.domain)?;
        assert_eq!(
            count,
            callers.len() * 3,
            "{} target-centered proof count",
            case.label
        );

        let edges = db.proof_checker_edges()?;
        assert_eq!(
            edges.len(),
            callers.len(),
            "{} target-centered proof edges: {edges:#?}",
            case.label
        );
        assert!(
            edges.iter().all(|edge| edge.callee_def_id.as_deref()
                == Some(resolved.target_str.as_str())
                && edge.resolution_state == "resolved"
                && edge.blocker_reason.is_none()),
            "{} target-centered proof edges should all resolve to the seed target: {edges:#?}",
            case.label
        );
        assert_proof_edge(&edges, case, &resolved);
        assert_provenance(&db, case, &resolved)?;
    }

    Ok(())
}

#[test]
fn fixture_projection_excludes_closure_async_outer_owners_from_target_proof() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let forbidden_owners = [
        function_id_by_name(&db, "closure_body_call_is_not_outer_call_site")?,
        function_id_by_name(&db, "async_block_call_is_not_outer_call_site")?,
        function_id_by_name(&db, "call_move_closure_literal_with_body_call")?,
        function_id_by_name(&db, "call_async_closure_literal_with_body_call")?,
    ]
    .map(|owner| owner.to_string());

    let count = db.project_call_proof_facts_for_target(target, "bd:fixture-call-graph")?;
    assert!(
        count > 0,
        "target-centered proof should project real callers"
    );

    let edges = db.proof_checker_edges()?;
    assert!(
        !edges.is_empty(),
        "target-centered local_target proof should include resolved proof edges"
    );
    assert!(
        edges
            .iter()
            .all(|edge| !forbidden_owners.contains(&edge.caller_def_id)),
        "target-centered local_target proof leaked closure/async outer owners into proof edges: {edges:#?}"
    );

    let rows = db.proof_graphrag_context("")?;
    assert!(
        rows.iter().all(|row| match row.caller_def_id.as_ref() {
            Some(caller) => !forbidden_owners.contains(caller),
            None => true,
        }),
        "target-centered local_target proof leaked closure/async outer owners into proof facts: {rows:#?}"
    );

    Ok(())
}

#[test]
fn fixture_projection_stores_real_target_centered_method_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let method_owner = function_id_by_name(&db, "call_typed_local_instance_method")?;
    let assoc_owner = function_id_by_name(&db, "call_method_as_associated_function")?;

    let method_context = db.call_context_for_owner(method_owner)?;
    let method_receiver = CallReceiver::TypedLocalBinding {
        name: "value".to_string(),
        type_path: path(&["LocalAssoc"]),
    };
    let method_site = row_by_method_receiver(&method_context, "instance_value", &method_receiver)
        .site
        .id;

    let assoc_context = db.call_context_for_owner(assoc_owner)?;
    let assoc_site = row_by_path(&assoc_context, &["LocalAssoc", "instance_value"])
        .site
        .id;

    let callers = db.callers_for_target(target)?;
    assert_resolved_target_callers(&callers, target, 2, "target-centered method")?;
    assert_target_proof_projection(
        &db,
        "target-centered method",
        "bd:fixture-call-graph",
        target,
        &callers,
        &[
            TargetProofSite {
                owner: method_owner,
                site: method_site,
            },
            TargetProofSite {
                owner: assoc_owner,
                site: assoc_site,
            },
        ],
        "fixture_call_graph/src/lib.rs",
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_target_centered_associated_function_call_proof_facts()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "make")?;
    let local_owner = function_id_by_name(&db, "call_local_assoc_make")?;
    let self_owner = method_id_by_impl_self_type_name(&db, "LocalAssoc", "call_self_make")?;
    let qualified_owner = function_id_by_name(&db, "call_qualified_local_assoc_make")?;

    let local_context = db.call_context_for_owner(local_owner)?;
    let local_site = row_by_path(&local_context, &["LocalAssoc", "make"]).site.id;

    let self_context = db.call_context_for_owner(self_owner)?;
    let self_site = row_by_path(&self_context, &["Self", "make"]).site.id;

    let qualified_context = db.call_context_for_owner(qualified_owner)?;
    let qualified_site = row_by_path(&qualified_context, &["LocalAssoc", "make"])
        .site
        .id;

    let callers = db.callers_for_target(target)?;
    assert_resolved_target_callers(&callers, target, 3, "target-centered associated-function")?;

    for (owner, expected_path) in [
        (local_owner, &["LocalAssoc", "make"][..]),
        (self_owner, &["Self", "make"][..]),
        (qualified_owner, &["LocalAssoc", "make"][..]),
    ] {
        let caller = caller_by_owner_kind_path(&callers, owner, CallSiteKind::Path, expected_path);
        assert_eq!(caller.target.relation, CallRelationKind::AssociatedFunction);
        assert_eq!(caller.target.source_kind, CallSiteKind::Path);
        assert_eq!(caller.target.target_kind, CallRelationKind::Method);
    }

    assert_target_proof_projection(
        &db,
        "target-centered associated-function",
        "bd:fixture-call-graph",
        target,
        &callers,
        &[
            TargetProofSite {
                owner: local_owner,
                site: local_site,
            },
            TargetProofSite {
                owner: self_owner,
                site: self_site,
            },
            TargetProofSite {
                owner: qualified_owner,
                site: qualified_site,
            },
        ],
        "src/lib.rs",
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_target_centered_imported_trait_assoc_function_call_proof_facts()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_trait_name(&db, "ImportedAssocFunctionTrait", "imported_trait_make")?;
    let cases = [
        (
            &["crate", "trait_assoc_function_scope", "with_direct_import"][..],
            "call_direct_imported_trait_associated_function",
            &["ImportedAssocFunctionTrait", "imported_trait_make"][..],
        ),
        (
            &["crate", "trait_assoc_function_scope", "with_alias_import"],
            "call_alias_imported_trait_associated_function",
            &["VisibleAssocFunctionTrait", "imported_trait_make"],
        ),
        (
            &["crate", "trait_assoc_function_scope", "with_glob_import"],
            "call_glob_imported_trait_associated_function",
            &["ImportedAssocFunctionTrait", "imported_trait_make"],
        ),
        (
            &["crate", "trait_assoc_reexport_scope"],
            "call_reexported_trait_associated_function",
            &["ReexportedAssocFunctionTrait", "imported_trait_make"],
        ),
        (
            &["crate", "grouped_trait_assoc_function_scope"],
            "call_grouped_imported_trait_associated_function",
            &["GroupedAssocFunctionTrait", "imported_trait_make"],
        ),
    ];

    let mut expected = Vec::new();
    for (module_path, owner_name, expected_path) in cases {
        let owner = function_id_by_name_in_module(&db, module_path, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        let site = row_by_path(&context, expected_path).site.id;
        expected.push((owner, site, expected_path));
    }

    let callers = db.callers_for_target(target)?;
    assert_resolved_target_callers(
        &callers,
        target,
        expected.len(),
        "target-centered imported trait associated-function",
    )?;

    for (owner, _, expected_path) in &expected {
        let caller = caller_by_owner_kind_path(&callers, *owner, CallSiteKind::Path, expected_path);
        assert_eq!(caller.target.relation, CallRelationKind::AssociatedFunction);
        assert_eq!(caller.target.source_kind, CallSiteKind::Path);
        assert_eq!(caller.target.target_kind, CallRelationKind::Method);
    }

    let expected_sites = expected
        .iter()
        .map(|(owner, site, _)| TargetProofSite {
            owner: *owner,
            site: *site,
        })
        .collect::<Vec<_>>();
    assert_target_proof_projection(
        &db,
        "target-centered imported trait associated-function",
        "bd:fixture-call-graph",
        target,
        &callers,
        &expected_sites,
        "src/lib.rs",
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_trait_dispatch_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "LocalDispatchTrait",
        "TraitDispatchTarget",
        "trait_value",
    )?;
    let cases = [
        "call_initialized_local_trait_method",
        "call_concrete_trait_object_binding_method",
        "call_reference_chain_trait_object_binding_method",
    ];
    let mut sites = Vec::new();

    for owner_name in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let receiver = CallReceiver::InitializedLocalBinding {
            name: "value".to_string(),
            init_path: path(&["TraitDispatchTarget"]),
        };
        let row = row_by_method_receiver(&context, "trait_value", &receiver);
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Method,
            CallSiteKind::Method,
            CallRelationKind::Method,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 3);
        sites.push((owner, row.site.id, row.site.span));
    }

    let edges = db.proof_checker_edges()?;
    assert_eq!(
        edges.len(),
        sites.len(),
        "trait dispatch proof edges: {edges:#?}"
    );
    assert!(
        edges.iter().all(
            |edge| edge.callee_def_id.as_deref() == Some(target.to_string().as_str())
                && edge.resolution_state == "resolved"
                && edge.blocker_reason.is_none()
        ),
        "trait dispatch proof edges should all resolve to the impl method: {edges:#?}"
    );
    for (owner, site, _) in &sites {
        assert!(
            edges.iter().any(|edge| {
                edge.call_site_id == site.to_string() && edge.caller_def_id == owner.to_string()
            }),
            "trait dispatch proof edge missing for {site}: {edges:#?}"
        );
    }

    assert!(
        db.proof_graphrag_context("type_resolution_missing")?
            .is_empty(),
        "resolved trait dispatch proofs should not produce blockers"
    );

    for (_, site, span) in sites {
        let provenance = db
            .proof_source_provenance(&site.to_string())?
            .expect("projected trait dispatch call-site source provenance");
        assert!(
            provenance
                .source_file
                .ends_with("fixture_call_graph/src/lib.rs"),
            "source provenance: {provenance:#?}"
        );
        assert_eq!(provenance.start_byte, span.0);
        assert_eq!(provenance.end_byte, span.1);
    }

    Ok(())
}

#[test]
fn fixture_projection_stores_real_target_centered_trait_dispatch_call_proof_facts()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "LocalDispatchTrait",
        "TraitDispatchTarget",
        "trait_value",
    )?;
    let initialized_owner = function_id_by_name(&db, "call_initialized_local_trait_method")?;
    let chained_owner =
        function_id_by_name(&db, "call_reference_chain_trait_object_binding_method")?;
    let receiver = CallReceiver::InitializedLocalBinding {
        name: "value".to_string(),
        init_path: path(&["TraitDispatchTarget"]),
    };

    let initialized_context = db.call_context_for_owner(initialized_owner)?;
    let initialized_site = row_by_method_receiver(&initialized_context, "trait_value", &receiver)
        .site
        .id;

    let chained_context = db.call_context_for_owner(chained_owner)?;
    let chained_site = row_by_method_receiver(&chained_context, "trait_value", &receiver)
        .site
        .id;

    let callers = db.callers_for_target(target)?;
    assert_resolved_target_callers(&callers, target, 3, "target-centered trait dispatch")?;

    let initialized =
        caller_by_owner_method_receiver(&callers, initialized_owner, "trait_value", &receiver);
    assert_eq!(initialized.target.relation, CallRelationKind::Method);
    let chained =
        caller_by_owner_method_receiver(&callers, chained_owner, "trait_value", &receiver);
    assert_eq!(chained.target.relation, CallRelationKind::Method);

    assert_target_proof_projection(
        &db,
        "target-centered trait dispatch",
        "bd:fixture-call-graph",
        target,
        &callers,
        &[
            TargetProofSite {
                owner: initialized_owner,
                site: initialized_site,
            },
            TargetProofSite {
                owner: chained_owner,
                site: chained_site,
            },
        ],
        "fixture_call_graph/src/lib.rs",
    )?;

    Ok(())
}

#[derive(Clone, Copy)]
struct OwnerProofEdge {
    owner: Uuid,
    site: Uuid,
    span: (u32, u32),
    target: Uuid,
}

#[derive(Clone, Copy)]
enum ProofEdgeCount {
    Exact,
    AtLeast,
}

#[derive(Clone, Copy)]
struct TargetProofSite {
    owner: Uuid,
    site: Uuid,
}

fn assert_owner_proof_edges(
    db: &Database,
    label: &str,
    expected: &[OwnerProofEdge],
    source_suffix: &str,
    blocker_reason: &str,
    count: ProofEdgeCount,
) -> Result<(), DbError> {
    let edges = db.proof_checker_edges()?;
    match count {
        ProofEdgeCount::Exact => assert_eq!(
            edges.len(),
            expected.len(),
            "{label} proof checker edges: {edges:#?}"
        ),
        ProofEdgeCount::AtLeast => assert!(
            edges.len() >= expected.len(),
            "{label} proof checker edges should include at least the expected edges: {edges:#?}"
        ),
    }

    for expected in expected {
        let owner = expected.owner.to_string();
        let site = expected.site.to_string();
        let target = expected.target.to_string();
        assert!(
            edges.iter().any(|edge| {
                edge.call_site_id == site
                    && edge.caller_def_id == owner
                    && edge.callee_def_id.as_deref() == Some(target.as_str())
                    && edge.resolution_state == "resolved"
                    && edge.blocker_reason.is_none()
            }),
            "{label} proof edge missing for {site}: {edges:#?}"
        );

        let provenance = db
            .proof_source_provenance(&site)?
            .unwrap_or_else(|| panic!("projected fixture {label} call-site source provenance"));
        assert!(
            provenance.source_file.ends_with(source_suffix),
            "source provenance for {site}: {provenance:#?}"
        );
        assert_eq!(provenance.start_byte, expected.span.0);
        assert_eq!(provenance.end_byte, expected.span.1);
    }

    assert!(
        db.proof_graphrag_context(blocker_reason)?.is_empty(),
        "resolved {label} proofs should not produce {blocker_reason} blockers"
    );

    Ok(())
}

fn assert_resolved_target_callers(
    callers: &[CallCallerRow],
    target: Uuid,
    min_count: usize,
    label: &str,
) -> Result<(), DbError> {
    assert!(
        callers.len() >= min_count,
        "{label} should have at least {min_count} incoming callers: {callers:#?}"
    );
    assert!(
        callers
            .iter()
            .all(|caller| caller.target.target_id == target),
        "{label} setup returned mismatched target rows: {callers:#?}"
    );
    assert!(
        callers
            .iter()
            .all(|caller| caller.status.status == CallStatusKind::Resolved
                && caller.status.resolution == Some(CallResolutionKind::LocalExact)),
        "{label} setup should only include resolved local callers: {callers:#?}"
    );
    Ok(())
}

fn assert_target_proof_projection(
    db: &Database,
    label: &str,
    domain: &str,
    target: Uuid,
    callers: &[CallCallerRow],
    expected: &[TargetProofSite],
    source_suffix: &str,
) -> Result<(), DbError> {
    let count = db.project_call_proof_facts_for_target(target, domain)?;
    assert_eq!(
        count,
        callers.len() * 3,
        "{label} target-centered proof count"
    );

    let target_str = target.to_string();
    let edges = db.proof_checker_edges()?;
    assert_eq!(
        edges.len(),
        callers.len(),
        "{label} target-centered proof edges: {edges:#?}"
    );
    assert!(
        edges.iter().all(
            |edge| edge.callee_def_id.as_deref() == Some(target_str.as_str())
                && edge.resolution_state == "resolved"
                && edge.blocker_reason.is_none()
        ),
        "{label} target-centered proof edges should all resolve to the seed target: {edges:#?}"
    );

    for site in expected {
        let owner = site.owner.to_string();
        let call_site = site.site.to_string();
        assert!(
            edges
                .iter()
                .any(|edge| { edge.call_site_id == call_site && edge.caller_def_id == owner }),
            "{label} incoming proof edge missing for {call_site}: {edges:#?}"
        );
    }

    assert!(
        db.proof_graphrag_context("type_resolution_missing")?
            .is_empty(),
        "{label} projection should not include unrelated blockers"
    );

    for site in expected {
        let provenance = db
            .proof_source_provenance(&site.site.to_string())?
            .unwrap_or_else(|| panic!("projected {label} source provenance"));
        assert!(
            provenance.source_file.ends_with(source_suffix),
            "source provenance: {provenance:#?}"
        );
        assert!(provenance.start_byte < provenance.end_byte);
    }

    Ok(())
}

#[derive(Clone, Copy)]
struct ConstructorCase {
    label: &'static str,
    fixture: &'static str,
    domain: &'static str,
    owner_module: &'static [&'static str],
    owner: &'static str,
    path: &'static [&'static str],
    target: ConstructorTarget,
    relation: CallRelationKind,
    endpoint: CallRelationKind,
    source_suffix: &'static str,
}

#[derive(Clone, Copy)]
enum ConstructorTarget {
    Struct {
        name: &'static str,
    },
    Variant {
        enum_name: &'static str,
        variant_name: &'static str,
    },
}

struct ResolvedConstructor {
    owner: Uuid,
    target: Uuid,
    site: Uuid,
    span: (u32, u32),
    proof_count: usize,
    owner_str: String,
    target_str: String,
    site_str: String,
}

const CONSTRUCTOR_CASES: &[ConstructorCase] = &[
    ConstructorCase {
        label: "tuple-struct constructor",
        fixture: "fixture_call_graph",
        domain: "bd:fixture-call-graph",
        owner_module: &["crate"],
        owner: "call_new_type_constructor",
        path: &["NewType"],
        target: ConstructorTarget::Struct { name: "NewType" },
        relation: CallRelationKind::TupleStructConstructor,
        endpoint: CallRelationKind::Struct,
        source_suffix: "fixture_call_graph/src/lib.rs",
    },
    ConstructorCase {
        label: "enum-variant constructor",
        fixture: "fixture_nodes",
        domain: "bd:fixture-nodes",
        owner_module: &["crate", "imports"],
        owner: "use_imported_items",
        path: &["EnumWithData", "Variant1"],
        target: ConstructorTarget::Variant {
            enum_name: "EnumWithData",
            variant_name: "Variant1",
        },
        relation: CallRelationKind::EnumVariantConstructor,
        endpoint: CallRelationKind::Variant,
        source_suffix: "fixture_nodes/src/imports.rs",
    },
];

fn constructor_cases() -> &'static [ConstructorCase] {
    CONSTRUCTOR_CASES
}

impl ConstructorTarget {
    fn id(self, db: &Database) -> Result<Uuid, DbError> {
        match self {
            Self::Struct { name } => struct_id_by_name(db, name),
            Self::Variant {
                enum_name,
                variant_name,
            } => variant_id_by_enum_and_variant_names(db, enum_name, variant_name),
        }
    }
}

fn assert_constructor_context(
    db: &Database,
    case: &ConstructorCase,
) -> Result<ResolvedConstructor, DbError> {
    let owner = function_id_by_name_in_module(db, case.owner_module, case.owner)?;
    let target = case.target.id(db)?;
    let context = db.call_context_for_owner(owner)?;
    let proof_count = context
        .iter()
        .map(|row| 2 + row.targets.len())
        .sum::<usize>();
    let row = row_by_path(&context, case.path);
    assert_resolved_target(
        row,
        target,
        case.relation,
        CallSiteKind::Path,
        case.endpoint,
    );

    Ok(ResolvedConstructor {
        owner,
        target,
        site: row.site.id,
        span: row.site.span,
        proof_count,
        owner_str: owner.to_string(),
        target_str: target.to_string(),
        site_str: row.site.id.to_string(),
    })
}

fn assert_constructor_callers(
    db: &Database,
    case: &ConstructorCase,
    resolved: &ResolvedConstructor,
) -> Result<Vec<CallCallerRow>, DbError> {
    let callers = db.callers_for_target(resolved.target)?;
    let caller = caller_by_owner_kind_path(&callers, resolved.owner, CallSiteKind::Path, case.path);
    assert_eq!(caller.site.id, resolved.site);
    assert_eq!(
        caller.target.relation, case.relation,
        "{} caller relation",
        case.label
    );
    assert_eq!(caller.target.source_kind, CallSiteKind::Path);
    assert_eq!(
        caller.target.target_kind, case.endpoint,
        "{} caller endpoint kind",
        case.label
    );
    Ok(callers)
}

fn assert_proof_edge(
    edges: &[ProofCheckerEdgeRow],
    case: &ConstructorCase,
    resolved: &ResolvedConstructor,
) {
    assert!(
        edges.iter().any(|edge| {
            edge.call_site_id == resolved.site_str
                && edge.caller_def_id == resolved.owner_str
                && edge.callee_def_id.as_deref() == Some(resolved.target_str.as_str())
                && edge.resolution_state == "resolved"
                && edge.blocker_reason.is_none()
        }),
        "{} proof edge missing: {edges:#?}",
        case.label
    );
}

fn assert_provenance(
    db: &Database,
    case: &ConstructorCase,
    resolved: &ResolvedConstructor,
) -> Result<(), DbError> {
    let provenance = db
        .proof_source_provenance(&resolved.site_str)?
        .unwrap_or_else(|| panic!("projected {} source provenance", case.label));
    assert!(
        provenance.source_file.ends_with(case.source_suffix),
        "source provenance: {provenance:#?}"
    );
    assert_eq!(provenance.start_byte, resolved.span.0);
    assert_eq!(provenance.end_byte, resolved.span.1);
    Ok(())
}

fn setup_call_graph_fixture_db(fixture: &'static str) -> Result<Database, DbError> {
    let db = Db::new(MemStorage::default()).expect("in-memory cozo db");
    db.initialize().expect("initialize cozo db");
    create_schema_all(&db).map_err(|err| DbError::QueryExecution(err.to_string()))?;

    let mut merged = syn_parser::parser::ParsedCodeGraph::merge_new(
        ploke_test_utils::test_run_phases_and_collect(fixture),
    )
    .map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let tree = merged
        .build_tree_and_prune()
        .map_err(|err| DbError::QueryExecution(err.to_string()))?;
    transform_parsed_graph(&db, merged, &tree)
        .map_err(|err| DbError::QueryExecution(err.to_string()))?;

    Ok(Database::new(db))
}

fn function_id_by_name(db: &Database, name: &str) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[id] :=
                *function {{ id, name: "{name}" @ 'NOW' }}"#
        ),
        0,
    )
}

fn function_id_by_exact_name(db: &Database, name: &str) -> Result<Uuid, DbError> {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));
    exactly_one_uuid_params(
        db,
        r#"?[id] :=
            *function { id, name: $name @ 'NOW' }"#,
        params,
        0,
    )
}

fn method_id_by_impl_self_type_name(
    db: &Database,
    self_type_name: &str,
    method_name: &str,
) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[method_id] :=
                *method {{ id: method_id, name: "{method_name}", owner_id: impl_id @ 'NOW' }},
                *impl {{ id: impl_id @ 'NOW' }},
                *type_use {{
                    owner_id: impl_id,
                    root_type_id: self_type_id,
                    role: "ImplSelf" @ 'NOW'
                }},
                *type_relation {{
                    source_id: self_type_id,
                    target_id: self_target_id,
                    relation_kind: "Ordinary" @ 'NOW'
                }},
                *struct {{ id: self_target_id, name: "{self_type_name}" @ 'NOW' }}"#
        ),
        0,
    )
}

fn method_owner_is_inherent_impl(
    db: &Database,
    method_id: Uuid,
    self_type_name: &str,
) -> Result<bool, DbError> {
    let mut params = BTreeMap::new();
    params.insert(
        "method_id".to_string(),
        DataValue::Uuid(UuidWrapper(method_id)),
    );
    params.insert(
        "self_type_name".to_string(),
        DataValue::from(self_type_name),
    );

    let self_rows = db.raw_query_params(
        r#"?[impl_id] :=
            *method { id: $method_id, owner_id: impl_id @ 'NOW' },
            *impl { id: impl_id @ 'NOW' },
            *type_use {
                owner_id: impl_id,
                root_type_id: self_type_id,
                role: "ImplSelf" @ 'NOW'
            },
            *type_relation {
                source_id: self_type_id,
                target_id: self_target_id,
                relation_kind: "Ordinary" @ 'NOW'
            },
            *struct { id: self_target_id, name: $self_type_name @ 'NOW' }"#,
        params.clone(),
    )?;
    let trait_rows = db.raw_query_params(
        r#"?[trait_type_id] :=
            *method { id: $method_id, owner_id: impl_id @ 'NOW' },
            *type_use {
                owner_id: impl_id,
                root_type_id: trait_type_id,
                role: "ImplTrait" @ 'NOW'
            }"#,
        params,
    )?;

    Ok(self_rows.rows.len() == 1 && trait_rows.rows.is_empty())
}

fn method_id_by_impl_self_type_exact_name(
    db: &Database,
    self_type_name: &str,
    method_name: &str,
) -> Result<Uuid, DbError> {
    let mut params = BTreeMap::new();
    params.insert(
        "self_type_name".to_string(),
        DataValue::from(self_type_name),
    );
    params.insert("method_name".to_string(), DataValue::from(method_name));
    exactly_one_uuid_params(
        db,
        r#"?[method_id] :=
            *method { id: method_id, name: $method_name, owner_id: impl_id @ 'NOW' },
            *impl { id: impl_id @ 'NOW' },
            *type_use {
                owner_id: impl_id,
                root_type_id: self_type_id,
                role: "ImplSelf" @ 'NOW'
            },
            *type_relation {
                source_id: self_type_id,
                target_id: self_target_id,
                relation_kind: "Ordinary" @ 'NOW'
            },
            *struct { id: self_target_id, name: $self_type_name @ 'NOW' }"#,
        params,
        0,
    )
}

fn method_id_by_impl_trait_and_self_type_names(
    db: &Database,
    trait_name: &str,
    self_type_name: &str,
    method_name: &str,
) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[method_id] :=
                *method {{ id: method_id, name: "{method_name}", owner_id: impl_id @ 'NOW' }},
                *impl {{ id: impl_id @ 'NOW' }},
                *type_use {{
                    owner_id: impl_id,
                    root_type_id: self_type_id,
                    role: "ImplSelf" @ 'NOW'
                }},
                *type_relation {{
                    source_id: self_type_id,
                    target_id: self_target_id,
                    relation_kind: "Ordinary" @ 'NOW'
                }},
                *struct {{ id: self_target_id, name: "{self_type_name}" @ 'NOW' }},
                *type_use {{
                    owner_id: impl_id,
                    root_type_id: trait_type_id,
                    role: "ImplTrait" @ 'NOW'
                }},
                *type_relation {{
                    source_id: trait_type_id,
                    target_id: trait_target_id,
                    relation_kind: "Trait" @ 'NOW'
                }},
                *trait {{ id: trait_target_id, name: "{trait_name}" @ 'NOW' }}"#
        ),
        0,
    )
}

fn method_id_by_impl_trait_name(
    db: &Database,
    trait_name: &str,
    method_name: &str,
) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[method_id] :=
                *method {{ id: method_id, name: "{method_name}", owner_id: impl_id @ 'NOW' }},
                *impl {{ id: impl_id @ 'NOW' }},
                *type_use {{
                    owner_id: impl_id,
                    root_type_id: trait_type_id,
                    role: "ImplTrait" @ 'NOW'
                }},
                *type_relation {{
                    source_id: trait_type_id,
                    target_id: trait_target_id,
                    relation_kind: "Trait" @ 'NOW'
                }},
                *trait {{ id: trait_target_id, name: "{trait_name}" @ 'NOW' }}"#
        ),
        0,
    )
}

fn method_id_by_trait_name(
    db: &Database,
    trait_name: &str,
    method_name: &str,
) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[method_id] :=
                *method {{ id: method_id, name: "{method_name}", owner_id: trait_id @ 'NOW' }},
                *trait {{ id: trait_id, name: "{trait_name}" @ 'NOW' }}"#
        ),
        0,
    )
}

fn function_id_by_name_in_module(
    db: &Database,
    module_path: &[&str],
    name: &str,
) -> Result<Uuid, DbError> {
    let module_path = cozo_path_literal(module_path);
    exactly_one_uuid(
        db,
        &format!(
            r#"?[id] :=
                *function {{ id, name: "{name}", module_id @ 'NOW' }},
                *module {{ id: module_id, path: {module_path} @ 'NOW' }}"#
        ),
        0,
    )
}

fn struct_id_by_name(db: &Database, name: &str) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[id] :=
                *struct {{ id, name: "{name}" @ 'NOW' }}"#
        ),
        0,
    )
}

fn variant_id_by_enum_and_variant_names(
    db: &Database,
    enum_name: &str,
    variant_name: &str,
) -> Result<Uuid, DbError> {
    let mut params = BTreeMap::new();
    params.insert("enum_name".to_string(), DataValue::from(enum_name));
    params.insert("variant_name".to_string(), DataValue::from(variant_name));
    exactly_one_uuid_params(
        db,
        r#"?[id] :=
            *enum { id: enum_id, name: $enum_name @ 'NOW' },
            *variant { id, name: $variant_name, owner_id: enum_id @ 'NOW' }"#,
        params,
        0,
    )
}

fn const_id_by_name(db: &Database, name: &str) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[id] :=
                *const {{ id, name: "{name}" @ 'NOW' }}"#
        ),
        0,
    )
}

fn static_id_by_name(db: &Database, name: &str) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[id] :=
                *static {{ id, name: "{name}" @ 'NOW' }}"#
        ),
        0,
    )
}

fn exactly_one_uuid(db: &Database, script: &str, column: usize) -> Result<Uuid, DbError> {
    let rows = db.raw_query(script)?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one row for query:\n{script}\nrows: {:#?}",
        rows.rows
    );
    to_uuid(&rows.rows[0][column])
}

fn exactly_one_uuid_params(
    db: &Database,
    script: &str,
    params: BTreeMap<String, DataValue>,
    column: usize,
) -> Result<Uuid, DbError> {
    let rows = db.raw_query_params(script, params)?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one row for query:\n{script}\nrows: {:#?}",
        rows.rows
    );
    to_uuid(&rows.rows[0][column])
}

fn data_str<'a>(value: &'a DataValue, label: &str) -> &'a str {
    value
        .get_str()
        .unwrap_or_else(|| panic!("{label} should be a string, got {value:?}"))
}

fn optional_data_str<'a>(value: &'a DataValue, label: &str) -> Option<&'a str> {
    match value {
        DataValue::Null => None,
        other => Some(data_str(other, label)),
    }
}

fn body_edges_for_site(db: &Database, site_id: Uuid) -> Result<QueryResult, DbError> {
    let mut params = BTreeMap::new();
    params.insert("site_id".to_string(), DataValue::Uuid(UuidWrapper(site_id)));
    db.raw_query_params(
        r#"?[source_id, target_id, source_kind, target_kind] :=
            site_id = $site_id,
            *call_site_edge {
                source_id,
                target_id,
                relation_kind: "BodyContainsCall",
                source_kind,
                target_kind @ 'NOW'
            },
            target_id = site_id"#,
        params,
    )
}

fn statuses_for_site(db: &Database, site_id: Uuid) -> Result<QueryResult, DbError> {
    let mut params = BTreeMap::new();
    params.insert("site_id".to_string(), DataValue::Uuid(UuidWrapper(site_id)));
    db.raw_query_params(
        r#"?[source_kind, status_kind, resolution_kind] :=
            site_id = $site_id,
            *call_resolution_status {
                source_id,
                source_kind,
                status_kind,
                resolution_kind @ 'NOW'
            },
            source_id = site_id"#,
        params,
    )
}

fn relations_for_site(db: &Database, site_id: Uuid) -> Result<QueryResult, DbError> {
    let mut params = BTreeMap::new();
    params.insert("site_id".to_string(), DataValue::Uuid(UuidWrapper(site_id)));
    db.raw_query_params(
        r#"?[target_id, relation_kind, source_kind, target_kind] :=
            site_id = $site_id,
            *call_relation {
                source_id,
                target_id,
                relation_kind,
                source_kind,
                target_kind @ 'NOW'
            },
            source_id = site_id"#,
        params,
    )
}

fn call_site_owner_and_kind(db: &Database, site_id: Uuid) -> Result<(Uuid, String), DbError> {
    let mut params = BTreeMap::new();
    params.insert("site_id".to_string(), DataValue::Uuid(UuidWrapper(site_id)));
    let rows = db.raw_query_params(
        r#"?[owner_id, call_kind] :=
            site_id = $site_id,
            *call_site { id, owner_id, call_kind @ 'NOW' },
            id = site_id"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one call_site row for {site_id}; rows: {:#?}",
        rows.rows
    );
    Ok((
        to_uuid(&rows.rows[0][0])?,
        data_str(&rows.rows[0][1], "call_site.call_kind").to_string(),
    ))
}

fn call_site_kind_for_site(db: &Database, site_id: Uuid) -> Result<String, DbError> {
    let mut params = BTreeMap::new();
    params.insert("site_id".to_string(), DataValue::Uuid(UuidWrapper(site_id)));
    let rows = db.raw_query_params(
        r#"?[call_kind] :=
            site_id = $site_id,
            *call_site { id, call_kind @ 'NOW' },
            id = site_id"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "call_relation source should point to exactly one call_site {site_id}; rows: {:#?}",
        rows.rows
    );
    Ok(data_str(&rows.rows[0][0], "call_site.call_kind").to_string())
}

fn owner_kind_for_call_body_owner(db: &Database, owner: Uuid) -> Result<String, DbError> {
    let mut params = BTreeMap::new();
    params.insert("owner".to_string(), DataValue::Uuid(UuidWrapper(owner)));
    let rows = db.raw_query_params(
        r#"?[kind] :=
            owner = $owner,
            (
                *function { id: owner @ 'NOW' },
                kind = "Function"
            ) or (
                *method { id: owner @ 'NOW' },
                kind = "Method"
            ) or (
                *const { id: owner @ 'NOW' },
                kind = "Const"
            ) or (
                *static { id: owner @ 'NOW' },
                kind = "Static"
            )"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "call-site owner should be exactly one call body owner {owner}; rows: {:#?}",
        rows.rows
    );
    Ok(data_str(&rows.rows[0][0], "call body owner kind").to_string())
}

fn is_valid_call_relation_family(relation: &str, source: &str, target: &str) -> bool {
    matches!(
        (relation, source, target),
        ("Function", "Path", "Function")
            | ("DynamicFunction", "Dynamic", "Function")
            | ("Method", "Method", "Method")
            | ("AssociatedFunction", "Path", "Method")
            | ("TupleStructConstructor", "Path", "Struct")
            | ("EnumVariantConstructor", "Path", "Variant")
    )
}

fn call_target_exists(db: &Database, target: Uuid, kind: &str) -> Result<bool, DbError> {
    let relation = match kind {
        "Function" => "function",
        "Method" => "method",
        "Struct" => "struct",
        "Variant" => "variant",
        other => panic!("unexpected call relation target kind {other}"),
    };
    let rows = db.raw_query(&format!(
        r#"?[id] :=
            id = to_uuid("{target}"),
            *{relation} {{ id @ 'NOW' }}"#
    ))?;
    Ok(rows.rows.len() == 1)
}

fn assert_valid_status_shape(site_id: Uuid, status: &str, resolution: Option<&str>) {
    let valid = match status {
        "Resolved" => resolution == Some("LocalExact"),
        "Unresolved" | "Ambiguous" | "External" | "Unsupported" => resolution.is_none(),
        other => panic!("unexpected call status kind {other} for {site_id}"),
    };
    assert!(
        valid,
        "call_resolution_status resolution_kind {resolution:?} is invalid for {status} call site {site_id}"
    );
}

fn proof_kind_count(rows: &[&ProofGraphContextRow], kind: &str) -> usize {
    rows.iter().filter(|row| row.kind == kind).count()
}

fn proof_fact_for_kind<'a>(
    rows: &'a [&ProofGraphContextRow],
    kind: &str,
) -> &'a ProofGraphContextRow {
    let matches = rows
        .iter()
        .copied()
        .filter(|row| row.kind == kind)
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one {kind} proof fact; rows: {rows:#?}"
    );
    matches[0]
}

fn row_by_path<'a>(context: &'a [CallContextRow], expected: &[&str]) -> &'a CallContextRow {
    let expected = path(expected);
    let matches = context
        .iter()
        .filter(|row| {
            row.site.kind == CallSiteKind::Path && row.site.path.as_ref() == Some(&expected)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one path row {expected:?}; context rows: {context:#?}"
    );
    matches[0]
}

fn row_by_method_receiver<'a>(
    context: &'a [CallContextRow],
    method: &str,
    receiver: &CallReceiver,
) -> &'a CallContextRow {
    let matches = context
        .iter()
        .filter(|row| {
            row.site.kind == CallSiteKind::Method
                && row.site.method.as_deref() == Some(method)
                && row.site.receiver.as_ref() == Some(receiver)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one method row {method} with receiver {receiver:?}; context rows: {context:#?}"
    );
    matches[0]
}

fn row_by_kind_path<'a>(
    context: &'a [CallContextRow],
    kind: CallSiteKind,
    expected: &[&str],
) -> &'a CallContextRow {
    let expected = path(expected);
    let matches = context
        .iter()
        .filter(|row| row.site.kind == kind && row.site.path.as_ref() == Some(&expected))
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one {kind:?} row {expected:?}; context rows: {context:#?}"
    );
    matches[0]
}

fn caller_by_owner_kind_path<'a>(
    callers: &'a [CallCallerRow],
    owner: Uuid,
    kind: CallSiteKind,
    expected: &[&str],
) -> &'a CallCallerRow {
    let expected = path(expected);
    let matches = callers
        .iter()
        .filter(|row| {
            row.site.owner_id == owner
                && row.site.kind == kind
                && row.site.path.as_ref() == Some(&expected)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one incoming {kind:?} caller row {expected:?}; caller rows: {callers:#?}"
    );
    matches[0]
}

fn caller_by_owner_method_receiver<'a>(
    callers: &'a [CallCallerRow],
    owner: Uuid,
    method: &str,
    receiver: &CallReceiver,
) -> &'a CallCallerRow {
    let matches = callers
        .iter()
        .filter(|row| {
            row.site.owner_id == owner
                && row.site.kind == CallSiteKind::Method
                && row.site.method.as_deref() == Some(method)
                && row.site.receiver.as_ref() == Some(receiver)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one incoming method caller {method} with receiver {receiver:?}; caller rows: {callers:#?}"
    );
    matches[0]
}

fn assert_resolved_target(
    row: &CallContextRow,
    target: Uuid,
    relation: CallRelationKind,
    source: CallSiteKind,
    target_kind: CallRelationKind,
) {
    assert_eq!(row.status.status, CallStatusKind::Resolved);
    assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(row.targets.len(), 1);
    assert_eq!(row.targets[0].target_id, target);
    assert_eq!(row.targets[0].relation, relation);
    assert_eq!(row.targets[0].source_kind, source);
    assert_eq!(row.targets[0].target_kind, target_kind);
}

fn assert_call_candidate(
    candidates: &[CallContextCandidate],
    node_id: Uuid,
    relation: CallContextRelation,
    call_site_id: Uuid,
    target_id: Uuid,
    message: &str,
) {
    assert!(
        candidates.iter().any(|candidate| {
            candidate.node_id == node_id
                && candidate.relation == relation
                && candidate.call_site_id == call_site_id
                && candidate.target_id == target_id
                && candidate.distance == 1
        }),
        "{message}; candidates: {candidates:#?}"
    );
}

fn path(segments: &[&str]) -> Vec<String> {
    segments
        .iter()
        .map(|segment| (*segment).to_string())
        .collect()
}

fn cozo_path_literal(segments: &[&str]) -> String {
    let joined = segments
        .iter()
        .map(|segment| format!(r#""{segment}""#))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{joined}]")
}
