use std::collections::BTreeMap;

use cozo::{DataValue, UuidWrapper};
use ploke_db::{
    CallReceiver, CallRelationKind, CallResolutionKind, CallSiteKind, CallStatusKind, Database,
    DbError, ProofGraphStore,
};
use uuid::Uuid;

#[test]
fn context_for_owner_returns_sites_statuses_and_targets() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(1);
    let path_site = Uuid::from_u128(2);
    let target = Uuid::from_u128(3);
    let dyn_site = Uuid::from_u128(4);
    let dyn_target = Uuid::from_u128(5);

    insert_call_site(
        &db,
        SiteSeed {
            id: path_site,
            owner,
            kind: "Path",
            span: (10, 24),
            path: Some(vec!["crate", "helper"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, path_site, "Path")?;
    insert_relation(&db, path_site, target, "Function", "Path", "Function")?;
    insert_status(&db, path_site, "Path", "Resolved", Some("LocalExact"))?;

    insert_call_site(
        &db,
        SiteSeed {
            id: dyn_site,
            owner,
            kind: "Dynamic",
            span: (30, 41),
            path: None,
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(1),
            generic_arg_count: None,
        },
    )?;
    insert_edge(&db, owner, dyn_site, "Dynamic")?;
    insert_relation(
        &db,
        dyn_site,
        dyn_target,
        "DynamicFunction",
        "Dynamic",
        "Function",
    )?;
    insert_status(&db, dyn_site, "Dynamic", "Resolved", Some("LocalExact"))?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "context rows: {context:#?}");

    let path_row = &context[0];
    assert_eq!(path_row.site.id, path_site);
    assert_eq!(path_row.site.kind, CallSiteKind::Path);
    assert_eq!(
        path_row.site.path.as_deref(),
        Some(["crate".to_string(), "helper".to_string()].as_slice())
    );
    assert_eq!(path_row.status.status, CallStatusKind::Resolved);
    assert_eq!(
        path_row.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(path_row.targets.len(), 1);
    assert_eq!(path_row.targets[0].target_id, target);
    assert_eq!(path_row.targets[0].relation, CallRelationKind::Function);

    let dyn_row = &context[1];
    assert_eq!(dyn_row.site.id, dyn_site);
    assert_eq!(dyn_row.site.kind, CallSiteKind::Dynamic);
    assert_eq!(dyn_row.status.status, CallStatusKind::Resolved);
    assert_eq!(
        dyn_row.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(dyn_row.targets.len(), 1);
    assert_eq!(dyn_row.targets[0].target_id, dyn_target);
    assert_eq!(
        dyn_row.targets[0].relation,
        CallRelationKind::DynamicFunction
    );
    assert_eq!(dyn_row.targets[0].target_kind, CallRelationKind::Function);

    Ok(())
}

#[test]
fn context_for_owner_decodes_method_receiver() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(11);
    let method_site = Uuid::from_u128(12);
    let target = Uuid::from_u128(13);
    let binding_site = Uuid::from_u128(14);
    let binding_target = Uuid::from_u128(15);
    let typed_site = Uuid::from_u128(16);
    let typed_target = Uuid::from_u128(17);
    let init_site = Uuid::from_u128(18);
    let init_target = Uuid::from_u128(19);

    insert_call_site(
        &db,
        SiteSeed {
            id: method_site,
            owner,
            kind: "Method",
            span: (3, 20),
            path: None,
            method: Some("len"),
            macro_name: None,
            receiver: Some(("SelfField", vec!["secret"])),
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, method_site, "Method")?;
    insert_relation(&db, method_site, target, "Method", "Method", "Method")?;
    insert_status(&db, method_site, "Method", "Resolved", Some("LocalExact"))?;
    insert_call_site(
        &db,
        SiteSeed {
            id: binding_site,
            owner,
            kind: "Method",
            span: (21, 39),
            path: None,
            method: Some("instance_value"),
            macro_name: None,
            receiver: Some(("LocalBinding", vec!["value"])),
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, binding_site, "Method")?;
    insert_relation(
        &db,
        binding_site,
        binding_target,
        "Method",
        "Method",
        "Method",
    )?;
    insert_status(&db, binding_site, "Method", "Resolved", Some("LocalExact"))?;
    insert_call_site(
        &db,
        SiteSeed {
            id: typed_site,
            owner,
            kind: "Method",
            span: (40, 62),
            path: None,
            method: Some("instance_value"),
            macro_name: None,
            receiver: Some(("TypedLocalBinding", vec!["typed", "LocalAssoc"])),
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, typed_site, "Method")?;
    insert_relation(&db, typed_site, typed_target, "Method", "Method", "Method")?;
    insert_status(&db, typed_site, "Method", "Resolved", Some("LocalExact"))?;
    insert_call_site(
        &db,
        SiteSeed {
            id: init_site,
            owner,
            kind: "Method",
            span: (63, 85),
            path: None,
            method: Some("instance_value"),
            macro_name: None,
            receiver: Some(("InitializedLocalBinding", vec!["init", "LocalAssoc"])),
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, init_site, "Method")?;
    insert_relation(&db, init_site, init_target, "Method", "Method", "Method")?;
    insert_status(&db, init_site, "Method", "Resolved", Some("LocalExact"))?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 4, "context rows: {context:#?}");
    assert_eq!(context[0].site.method.as_deref(), Some("len"));
    assert_eq!(
        context[0].site.receiver,
        Some(CallReceiver::SelfField {
            path: vec!["secret".to_string()]
        })
    );
    assert_eq!(context[0].targets[0].relation, CallRelationKind::Method);
    assert_eq!(context[1].site.method.as_deref(), Some("instance_value"));
    assert_eq!(
        context[1].site.receiver,
        Some(CallReceiver::LocalBinding {
            name: "value".to_string()
        })
    );
    assert_eq!(context[1].targets[0].relation, CallRelationKind::Method);
    assert_eq!(context[2].site.method.as_deref(), Some("instance_value"));
    assert_eq!(
        context[2].site.receiver,
        Some(CallReceiver::TypedLocalBinding {
            name: "typed".to_string(),
            type_path: vec!["LocalAssoc".to_string()]
        })
    );
    assert_eq!(context[2].targets[0].relation, CallRelationKind::Method);
    assert_eq!(context[3].site.method.as_deref(), Some("instance_value"));
    assert_eq!(
        context[3].site.receiver,
        Some(CallReceiver::InitializedLocalBinding {
            name: "init".to_string(),
            init_path: vec!["LocalAssoc".to_string()]
        })
    );
    assert_eq!(context[3].targets[0].relation, CallRelationKind::Method);

    Ok(())
}

#[test]
fn context_for_owner_decodes_extended_method_receivers() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x201);
    let cases = vec![
        (
            "BorrowedTypedLocalBinding",
            vec!["borrowed", "LocalAssoc"],
            CallReceiver::BorrowedTypedLocalBinding {
                name: "borrowed".to_string(),
                type_path: vec!["LocalAssoc".to_string()],
            },
        ),
        (
            "DereferencedInitializedLocalBinding",
            vec!["deref", "LocalAssoc"],
            CallReceiver::DereferencedInitializedLocalBinding {
                name: "deref".to_string(),
                init_path: vec!["LocalAssoc".to_string()],
            },
        ),
        (
            "FieldInitializedLocalBinding",
            vec!["fielded", "TupleFieldMethodReceiver", "", "0"],
            CallReceiver::FieldInitializedLocalBinding {
                name: "fielded".to_string(),
                init_path: vec!["TupleFieldMethodReceiver".to_string()],
                field_path: vec!["0".to_string()],
            },
        ),
        (
            "AwaitPathCallResult",
            vec!["make_ready_local_assoc"],
            CallReceiver::AwaitPathCallResult {
                path: vec!["make_ready_local_assoc".to_string()],
            },
        ),
        (
            "TryPathCallResult",
            vec!["try_local_assoc"],
            CallReceiver::TryPathCallResult {
                path: vec!["try_local_assoc".to_string()],
            },
        ),
    ];

    for (idx, (kind, receiver_path, _expected)) in cases.iter().enumerate() {
        let site = Uuid::from_u128(0x210 + idx as u128);
        let target = Uuid::from_u128(0x220 + idx as u128);
        insert_call_site(
            &db,
            SiteSeed {
                id: site,
                owner,
                kind: "Method",
                span: (100 + idx as i64 * 10, 109 + idx as i64 * 10),
                path: None,
                method: Some("instance_value"),
                macro_name: None,
                receiver: Some((*kind, receiver_path.clone())),
                arg_count: Some(0),
                generic_arg_count: Some(0),
            },
        )?;
        insert_edge(&db, owner, site, "Method")?;
        insert_relation(&db, site, target, "Method", "Method", "Method")?;
        insert_status(&db, site, "Method", "Resolved", Some("LocalExact"))?;
    }

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), cases.len(), "context rows: {context:#?}");
    for (idx, (_kind, _receiver_path, expected)) in cases.iter().enumerate() {
        assert_eq!(context[idx].site.method.as_deref(), Some("instance_value"));
        assert_eq!(context[idx].site.receiver.as_ref(), Some(expected));
        assert_eq!(context[idx].targets[0].relation, CallRelationKind::Method);
    }

    Ok(())
}

#[test]
fn context_for_owner_decodes_associated_function_relation() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(14);
    let site = Uuid::from_u128(15);
    let target = Uuid::from_u128(16);

    insert_call_site(
        &db,
        SiteSeed {
            id: site,
            owner,
            kind: "Path",
            span: (21, 39),
            path: Some(vec!["LocalAssoc", "make"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, site, "Path")?;
    insert_relation(&db, site, target, "AssociatedFunction", "Path", "Method")?;
    insert_status(&db, site, "Path", "Resolved", Some("LocalExact"))?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");
    assert_eq!(
        context[0].targets[0].relation,
        CallRelationKind::AssociatedFunction
    );
    assert_eq!(context[0].targets[0].target_kind, CallRelationKind::Method);

    Ok(())
}

#[test]
fn context_for_owner_decodes_constructor_relations() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(17);
    let tuple_site = Uuid::from_u128(18);
    let tuple_target = Uuid::from_u128(19);
    let variant_site = Uuid::from_u128(20);
    let variant_target = Uuid::from_u128(21);

    insert_call_site(
        &db,
        SiteSeed {
            id: tuple_site,
            owner,
            kind: "Path",
            span: (50, 67),
            path: Some(vec!["TupleStruct"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(2),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, tuple_site, "Path")?;
    insert_relation(
        &db,
        tuple_site,
        tuple_target,
        "TupleStructConstructor",
        "Path",
        "Struct",
    )?;
    insert_status(&db, tuple_site, "Path", "Resolved", Some("LocalExact"))?;

    insert_call_site(
        &db,
        SiteSeed {
            id: variant_site,
            owner,
            kind: "Path",
            span: (70, 95),
            path: Some(vec!["EnumWithData", "Variant1"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(1),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, variant_site, "Path")?;
    insert_relation(
        &db,
        variant_site,
        variant_target,
        "EnumVariantConstructor",
        "Path",
        "Variant",
    )?;
    insert_status(&db, variant_site, "Path", "Resolved", Some("LocalExact"))?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "context rows: {context:#?}");
    assert_eq!(
        context[0].targets[0].relation,
        CallRelationKind::TupleStructConstructor
    );
    assert_eq!(context[0].targets[0].target_kind, CallRelationKind::Struct);
    assert_eq!(
        context[1].targets[0].relation,
        CallRelationKind::EnumVariantConstructor
    );
    assert_eq!(context[1].targets[0].target_kind, CallRelationKind::Variant);

    Ok(())
}

#[test]
fn context_for_owner_rejects_missing_status() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(21);
    let site = Uuid::from_u128(22);

    insert_call_site(
        &db,
        SiteSeed {
            id: site,
            owner,
            kind: "Macro",
            span: (7, 18),
            path: None,
            method: None,
            macro_name: Some("println"),
            receiver: None,
            arg_count: None,
            generic_arg_count: None,
        },
    )?;
    insert_edge(&db, owner, site, "Macro")?;

    let error = db
        .call_context_for_owner(owner)
        .expect_err("missing call status should fail");
    assert!(
        error.to_string().contains("missing call_resolution_status"),
        "unexpected error: {error}"
    );

    Ok(())
}

#[test]
fn proof_projection_stores_resolved_call_facts() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(31);
    let module = Uuid::from_u128(32);
    let site = Uuid::from_u128(33);
    let target = Uuid::from_u128(34);

    insert_owner_source(&db, owner, module, "src/lib.rs")?;
    insert_call_site(
        &db,
        SiteSeed {
            id: site,
            owner,
            kind: "Path",
            span: (10, 24),
            path: Some(vec!["crate", "helper"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, site, "Path")?;
    insert_relation(&db, site, target, "Function", "Path", "Function")?;
    insert_status(&db, site, "Path", "Resolved", Some("LocalExact"))?;

    let count = db.project_call_proof_facts_for_owner(owner, "bd:test")?;
    assert_eq!(count, 3);

    let edges = db.proof_checker_edges()?;
    assert_eq!(edges.len(), 1, "proof checker edges: {edges:#?}");
    assert_eq!(edges[0].call_site_id, site.to_string());
    assert_eq!(edges[0].caller_def_id, owner.to_string());
    assert_eq!(
        edges[0].callee_def_id.as_deref(),
        Some(target.to_string().as_str())
    );
    assert_eq!(edges[0].resolution_state, "resolved");
    assert!(edges[0].blocker_reason.is_none());

    let provenance = db
        .proof_source_provenance(&site.to_string())?
        .expect("projected call site source provenance");
    assert_eq!(provenance.source_file, "src/lib.rs");
    assert_eq!(provenance.start_byte, 10);
    assert_eq!(provenance.end_byte, 24);

    Ok(())
}

#[test]
fn proof_projection_marks_unresolved_call_statuses() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(41);
    let module = Uuid::from_u128(42);
    let external = Uuid::from_u128(43);
    let dynamic = Uuid::from_u128(44);

    insert_owner_source(&db, owner, module, "src/lib.rs")?;
    insert_call_site(
        &db,
        SiteSeed {
            id: external,
            owner,
            kind: "Path",
            span: (30, 40),
            path: Some(vec!["std", "process", "Command", "new"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(1),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, external, "Path")?;
    insert_status(&db, external, "Path", "External", None)?;

    insert_call_site(
        &db,
        SiteSeed {
            id: dynamic,
            owner,
            kind: "Dynamic",
            span: (50, 60),
            path: None,
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: None,
        },
    )?;
    insert_edge(&db, owner, dynamic, "Dynamic")?;
    insert_status(&db, dynamic, "Dynamic", "Unsupported", None)?;

    let count = db.project_call_proof_facts_for_owner(owner, "bd:test")?;
    assert_eq!(count, 4);
    assert!(db.proof_checker_edges()?.is_empty());

    let external_rows = db.proof_graphrag_context("external_dependency_summary_missing")?;
    assert!(
        external_rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(external.to_string().as_str())
                && row.blocker_reason.as_deref() == Some("external_dependency_summary_missing")
        }),
        "external proof rows: {external_rows:#?}"
    );

    let dynamic_rows = db.proof_graphrag_context("dynamic_dispatch_unbounded")?;
    assert!(
        dynamic_rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(dynamic.to_string().as_str())
                && row.blocker_reason.as_deref() == Some("dynamic_dispatch_unbounded")
        }),
        "dynamic proof rows: {dynamic_rows:#?}"
    );

    Ok(())
}

#[test]
fn proof_projection_rejects_non_resolved_local_targets() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(51);
    let module = Uuid::from_u128(52);
    let site = Uuid::from_u128(53);
    let target = Uuid::from_u128(54);

    insert_owner_source(&db, owner, module, "src/lib.rs")?;
    insert_call_site(
        &db,
        SiteSeed {
            id: site,
            owner,
            kind: "Path",
            span: (70, 90),
            path: Some(vec!["unknown"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, site, "Path")?;
    insert_relation(&db, site, target, "Function", "Path", "Function")?;
    insert_status(&db, site, "Path", "Unresolved", None)?;

    let error = db
        .project_call_proof_facts_for_owner(owner, "bd:test")
        .expect_err("unresolved call site with local targets must reject");
    assert!(
        error.to_string().contains("non-resolved call site"),
        "unexpected error: {error}"
    );
    assert!(db.proof_graphrag_context("")?.is_empty());

    Ok(())
}

struct SiteSeed<'a> {
    id: Uuid,
    owner: Uuid,
    kind: &'a str,
    span: (i64, i64),
    path: Option<Vec<&'a str>>,
    method: Option<&'a str>,
    macro_name: Option<&'a str>,
    receiver: Option<(&'a str, Vec<&'a str>)>,
    arg_count: Option<i64>,
    generic_arg_count: Option<i64>,
}

fn insert_call_site(db: &Database, seed: SiteSeed<'_>) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("id".to_string(), uuid(seed.id));
    params.insert("owner_id".to_string(), uuid(seed.owner));
    params.insert("call_kind".to_string(), DataValue::from(seed.kind));
    params.insert("span".to_string(), span(seed.span));
    params.insert("cfgs".to_string(), list(&[]));
    params.insert("path".to_string(), option_list(seed.path));
    params.insert("method_name".to_string(), option_str(seed.method));
    params.insert("macro_name".to_string(), option_str(seed.macro_name));
    let (kind, path) = seed
        .receiver
        .map(|(kind, path)| (DataValue::from(kind), list(&path)))
        .unwrap_or((DataValue::Null, DataValue::Null));
    params.insert("receiver_kind".to_string(), kind);
    params.insert("receiver_path".to_string(), path);
    params.insert("arg_count".to_string(), option_int(seed.arg_count));
    params.insert(
        "generic_arg_count".to_string(),
        option_int(seed.generic_arg_count),
    );

    db.raw_query_mut_params(
        r#"?[id, at, owner_id, call_kind, span, cfgs, path, method_name, macro_name, receiver_kind, receiver_path, arg_count, generic_arg_count] :=
            id = $id,
            owner_id = $owner_id,
            call_kind = $call_kind,
            span = $span,
            cfgs = $cfgs,
            path = $path,
            method_name = $method_name,
            macro_name = $macro_name,
            receiver_kind = $receiver_kind,
            receiver_path = $receiver_path,
            arg_count = $arg_count,
            generic_arg_count = $generic_arg_count,
            at = 'ASSERT'
        :put call_site { id, at => owner_id, call_kind, span, cfgs, path, method_name, macro_name, receiver_kind, receiver_path, arg_count, generic_arg_count }"#,
        params,
    )?;
    Ok(())
}

fn insert_edge(db: &Database, owner: Uuid, site: Uuid, kind: &str) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("owner_id".to_string(), uuid(owner));
    params.insert("site_id".to_string(), uuid(site));
    params.insert("target_kind".to_string(), DataValue::from(kind));

    db.raw_query_mut_params(
        r#"?[source_id, target_id, at, relation_kind, source_kind, target_kind] :=
            source_id = $owner_id,
            target_id = $site_id,
            relation_kind = "BodyContainsCall",
            source_kind = "Function",
            target_kind = $target_kind,
            at = 'ASSERT'
        :put call_site_edge { source_id, target_id, at => relation_kind, source_kind, target_kind }"#,
        params,
    )?;
    Ok(())
}

fn insert_relation(
    db: &Database,
    site: Uuid,
    target: Uuid,
    relation: &str,
    source_kind: &str,
    target_kind: &str,
) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("site_id".to_string(), uuid(site));
    params.insert("target_id".to_string(), uuid(target));
    params.insert("relation_kind".to_string(), DataValue::from(relation));
    params.insert("source_kind".to_string(), DataValue::from(source_kind));
    params.insert("target_kind".to_string(), DataValue::from(target_kind));

    db.raw_query_mut_params(
        r#"?[source_id, target_id, at, relation_kind, source_kind, target_kind] :=
            source_id = $site_id,
            target_id = $target_id,
            relation_kind = $relation_kind,
            source_kind = $source_kind,
            target_kind = $target_kind,
            at = 'ASSERT'
        :put call_relation { source_id, target_id, at => relation_kind, source_kind, target_kind }"#,
        params,
    )?;
    Ok(())
}

fn insert_status(
    db: &Database,
    site: Uuid,
    kind: &str,
    status: &str,
    resolution: Option<&str>,
) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("site_id".to_string(), uuid(site));
    params.insert("source_kind".to_string(), DataValue::from(kind));
    params.insert("status_kind".to_string(), DataValue::from(status));
    params.insert("resolution_kind".to_string(), option_str(resolution));

    db.raw_query_mut_params(
        r#"?[source_id, at, source_kind, status_kind, resolution_kind] :=
            source_id = $site_id,
            source_kind = $source_kind,
            status_kind = $status_kind,
            resolution_kind = $resolution_kind,
            at = 'ASSERT'
        :put call_resolution_status { source_id, at => source_kind, status_kind, resolution_kind }"#,
        params,
    )?;
    Ok(())
}

fn insert_owner_source(
    db: &Database,
    owner: Uuid,
    module: Uuid,
    file: &str,
) -> Result<(), DbError> {
    let namespace = Uuid::from_u128(99);
    insert_module(db, module)?;
    insert_function(db, owner, module)?;
    insert_contains(db, module, owner)?;

    let mut params = BTreeMap::new();
    params.insert("owner_id".to_string(), uuid(module));
    params.insert("file_path".to_string(), DataValue::from(file));
    params.insert("file_docs".to_string(), DataValue::Null);
    params.insert("items".to_string(), DataValue::List(vec![uuid(owner)]));
    params.insert("namespace".to_string(), uuid(namespace));

    db.raw_query_mut_params(
        r#"?[owner_id, at, file_path, file_docs, items, namespace] :=
            owner_id = $owner_id,
            file_path = $file_path,
            file_docs = $file_docs,
            items = $items,
            namespace = $namespace,
            at = 'ASSERT'
        :put file_mod { owner_id, at => file_path, file_docs, items, namespace }"#,
        params,
    )?;
    Ok(())
}

fn insert_module(db: &Database, module: Uuid) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("id".to_string(), uuid(module));
    params.insert("name".to_string(), DataValue::from("crate"));
    params.insert("path".to_string(), list(&["crate"]));
    params.insert("vis_kind".to_string(), DataValue::from("Public"));
    params.insert("vis_path".to_string(), DataValue::Null);
    params.insert("docstring".to_string(), DataValue::Null);
    params.insert("span".to_string(), span((0, 100)));
    params.insert("tracking_hash".to_string(), uuid(Uuid::from_u128(98)));
    params.insert("module_kind".to_string(), DataValue::from("FileBased"));
    params.insert("cfgs".to_string(), list(&[]));

    db.raw_query_mut_params(
        r#"?[id, at, name, path, vis_kind, vis_path, docstring, span, tracking_hash, module_kind, cfgs] :=
            id = $id,
            name = $name,
            path = $path,
            vis_kind = $vis_kind,
            vis_path = $vis_path,
            docstring = $docstring,
            span = $span,
            tracking_hash = $tracking_hash,
            module_kind = $module_kind,
            cfgs = $cfgs,
            at = 'ASSERT'
        :put module { id, at => name, path, vis_kind, vis_path, docstring, span, tracking_hash, module_kind, cfgs }"#,
        params,
    )?;
    Ok(())
}

fn insert_function(db: &Database, owner: Uuid, module: Uuid) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("id".to_string(), uuid(owner));
    params.insert("name".to_string(), DataValue::from("caller"));
    params.insert("docstring".to_string(), DataValue::Null);
    params.insert("vis_kind".to_string(), DataValue::from("Public"));
    params.insert("vis_path".to_string(), DataValue::Null);
    params.insert("span".to_string(), span((0, 100)));
    params.insert("tracking_hash".to_string(), uuid(Uuid::from_u128(97)));
    params.insert("cfgs".to_string(), list(&[]));
    params.insert("return_type_id".to_string(), DataValue::Null);
    params.insert("body".to_string(), DataValue::Null);
    params.insert("module_id".to_string(), uuid(module));

    db.raw_query_mut_params(
        r#"?[id, at, name, docstring, vis_kind, vis_path, span, tracking_hash, cfgs, return_type_id, body, module_id] :=
            id = $id,
            name = $name,
            docstring = $docstring,
            vis_kind = $vis_kind,
            vis_path = $vis_path,
            span = $span,
            tracking_hash = $tracking_hash,
            cfgs = $cfgs,
            return_type_id = $return_type_id,
            body = $body,
            module_id = $module_id,
            at = 'ASSERT'
        :put function { id, at => name, docstring, vis_kind, vis_path, span, tracking_hash, cfgs, return_type_id, body, module_id }"#,
        params,
    )?;
    Ok(())
}

fn insert_contains(db: &Database, module: Uuid, owner: Uuid) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("module_id".to_string(), uuid(module));
    params.insert("owner_id".to_string(), uuid(owner));

    db.raw_query_mut_params(
        r#"?[source_id, target_id, at, relation_kind, source_kind, target_kind] :=
            source_id = $module_id,
            target_id = $owner_id,
            relation_kind = "Contains",
            source_kind = "Module",
            target_kind = "Function",
            at = 'ASSERT'
        :put syntax_edge { source_id, target_id, at => relation_kind, source_kind, target_kind }"#,
        params,
    )?;
    Ok(())
}

fn uuid(value: Uuid) -> DataValue {
    DataValue::Uuid(UuidWrapper(value))
}

fn span((start, end): (i64, i64)) -> DataValue {
    DataValue::List(vec![DataValue::from(start), DataValue::from(end)])
}

fn list(items: &[&str]) -> DataValue {
    DataValue::List(items.iter().map(|item| DataValue::from(*item)).collect())
}

fn option_list(items: Option<Vec<&str>>) -> DataValue {
    items.map(|items| list(&items)).unwrap_or(DataValue::Null)
}

fn option_str(value: Option<&str>) -> DataValue {
    value.map(DataValue::from).unwrap_or(DataValue::Null)
}

fn option_int(value: Option<i64>) -> DataValue {
    value.map(DataValue::from).unwrap_or(DataValue::Null)
}
