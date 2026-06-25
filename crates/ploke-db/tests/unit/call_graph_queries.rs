use cozo::{DataValue, Db, MemStorage};
use ploke_db::{
    CallContextOptions, CallContextRelation, CallContextSeed, CallReceiver, CallRelationKind,
    CallResolutionKind, CallSiteKind, CallStatusKind, CallTargetKind, Database, DbError,
    ProofGraphStore, ProofInvariantStatus,
};
use uuid::Uuid;

use super::call_graph_common::*;

mod invariants;
mod proof_projection;

#[test]
fn has_call_graph_relations_reports_schema_presence() -> Result<(), DbError> {
    let raw = Db::new(MemStorage::default()).expect("in-memory cozo db");
    raw.initialize().expect("initialize cozo db");
    let empty = Database::new(raw);
    assert!(
        !empty.has_call_graph_relations()?,
        "empty database should not report call-graph relations"
    );

    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    assert!(
        db.has_call_graph_relations()?,
        "full schema should include all call-graph relations"
    );

    Ok(())
}

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
    assert_eq!(dyn_row.targets[0].target_kind, CallTargetKind::Function);

    Ok(())
}

#[test]
fn callers_for_target_returns_sites_statuses_and_matching_edges() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x101);
    let target = Uuid::from_u128(0x102);
    let other_target = Uuid::from_u128(0x103);
    let path_site = Uuid::from_u128(0x104);
    let method_site = Uuid::from_u128(0x105);
    let excluded_site = Uuid::from_u128(0x106);

    insert_call_site(
        &db,
        SiteSeed {
            id: path_site,
            owner,
            kind: "Path",
            span: (10, 20),
            path: Some(vec!["crate", "target"]),
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
            id: method_site,
            owner,
            kind: "Method",
            span: (30, 45),
            path: None,
            method: Some("target_method"),
            macro_name: None,
            receiver: Some(("TypedLocalBinding", vec!["value", "TargetType"])),
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
            id: excluded_site,
            owner,
            kind: "Path",
            span: (50, 60),
            path: Some(vec!["crate", "other"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, excluded_site, "Path")?;
    insert_relation(
        &db,
        excluded_site,
        other_target,
        "Function",
        "Path",
        "Function",
    )?;
    insert_status(&db, excluded_site, "Path", "Resolved", Some("LocalExact"))?;

    let callers = db.callers_for_target(target)?;
    assert_eq!(callers.len(), 2, "callers: {callers:#?}");
    assert!(
        callers
            .iter()
            .all(|caller| caller.target.target_id == target),
        "target-centered query returned a mismatched edge: {callers:#?}"
    );
    assert!(
        callers
            .iter()
            .all(|caller| caller.status.status == CallStatusKind::Resolved
                && caller.status.resolution == Some(CallResolutionKind::LocalExact)),
        "callers should preserve status rows: {callers:#?}"
    );

    let path = callers
        .iter()
        .find(|caller| caller.site.id == path_site)
        .expect("path caller row");
    assert_eq!(path.site.kind, CallSiteKind::Path);
    assert_eq!(
        path.site.path.as_deref(),
        Some(["crate".to_string(), "target".to_string()].as_slice())
    );
    assert_eq!(path.target.relation, CallRelationKind::Function);
    assert_eq!(path.target.source_kind, CallSiteKind::Path);
    assert_eq!(path.target.target_kind, CallTargetKind::Function);

    let method = callers
        .iter()
        .find(|caller| caller.site.id == method_site)
        .expect("method caller row");
    assert_eq!(method.site.kind, CallSiteKind::Method);
    assert_eq!(method.site.method.as_deref(), Some("target_method"));
    assert_eq!(
        method.site.receiver,
        Some(CallReceiver::TypedLocalBinding {
            name: "value".to_string(),
            type_path: vec!["TargetType".to_string()]
        })
    );
    assert_eq!(method.target.relation, CallRelationKind::Method);
    assert_eq!(method.target.source_kind, CallSiteKind::Method);
    assert_eq!(method.target.target_kind, CallTargetKind::Method);

    Ok(())
}

#[test]
fn expand_call_context_returns_outgoing_targets_and_incoming_callers() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x191);
    let other_owner = Uuid::from_u128(0x192);
    let target = Uuid::from_u128(0x193);
    let other_target = Uuid::from_u128(0x194);
    let site = Uuid::from_u128(0x195);
    let other_site = Uuid::from_u128(0x196);
    let assoc_site = Uuid::from_u128(0x197);

    insert_call_site(
        &db,
        SiteSeed {
            id: site,
            owner,
            kind: "Path",
            span: (10, 20),
            path: Some(vec!["crate", "target"]),
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

    insert_call_site(
        &db,
        SiteSeed {
            id: assoc_site,
            owner,
            kind: "Path",
            span: (22, 29),
            path: Some(vec!["LocalAssoc", "make"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, assoc_site, "Path")?;
    insert_relation(
        &db,
        assoc_site,
        other_target,
        "AssociatedFunction",
        "Path",
        "Method",
    )?;
    insert_status(&db, assoc_site, "Path", "Resolved", Some("LocalExact"))?;

    insert_call_site(
        &db,
        SiteSeed {
            id: other_site,
            owner: other_owner,
            kind: "Method",
            span: (30, 45),
            path: None,
            method: Some("target_method"),
            macro_name: None,
            receiver: Some(("TypedLocalBinding", vec!["value", "TargetType"])),
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, other_owner, other_site, "Method")?;
    insert_relation(&db, other_site, target, "Method", "Method", "Method")?;
    insert_status(&db, other_site, "Method", "Resolved", Some("LocalExact"))?;

    let outgoing = db.expand_call_context(
        CallContextSeed::Owner(owner),
        CallContextOptions {
            include_incoming_callers: false,
            ..CallContextOptions::default()
        },
    )?;
    assert_eq!(outgoing.len(), 2, "outgoing candidates: {outgoing:#?}");
    assert!(
        outgoing.iter().any(|candidate| {
            candidate.node_id == target
                && candidate.target_id == target
                && candidate.call_site_id == site
                && candidate.relation == CallContextRelation::OutgoingTarget
                && candidate.distance == 1
        }),
        "target outgoing candidate missing: {outgoing:#?}"
    );
    assert!(
        outgoing.iter().any(|candidate| {
            candidate.node_id == other_target
                && candidate.target_id == other_target
                && candidate.call_site_id == assoc_site
                && candidate.relation == CallContextRelation::OutgoingTarget
                && candidate.distance == 1
        }),
        "second outgoing candidate missing: {outgoing:#?}"
    );

    let incoming = db.expand_call_context(
        CallContextSeed::Target(target),
        CallContextOptions {
            include_outgoing_targets: false,
            ..CallContextOptions::default()
        },
    )?;
    assert_eq!(incoming.len(), 2, "incoming candidates: {incoming:#?}");
    assert!(
        incoming.iter().any(|candidate| {
            candidate.node_id == owner
                && candidate.target_id == target
                && candidate.call_site_id == site
                && candidate.relation == CallContextRelation::IncomingCaller
                && candidate.distance == 1
        }),
        "path caller candidate missing: {incoming:#?}"
    );
    assert!(
        incoming.iter().any(|candidate| {
            candidate.node_id == other_owner
                && candidate.target_id == target
                && candidate.call_site_id == other_site
                && candidate.relation == CallContextRelation::IncomingCaller
                && candidate.distance == 1
        }),
        "method caller candidate missing: {incoming:#?}"
    );

    let truncated = db.expand_call_context(
        CallContextSeed::Target(target),
        CallContextOptions {
            include_outgoing_targets: false,
            max_candidates: 1,
            ..CallContextOptions::default()
        },
    )?;
    assert_eq!(
        truncated.len(),
        1,
        "max_candidates should cap incoming expansion: {truncated:#?}"
    );

    let disabled = db.expand_call_context(
        CallContextSeed::Target(target),
        CallContextOptions {
            include_incoming_callers: false,
            ..CallContextOptions::default()
        },
    )?;
    assert!(
        disabled.is_empty(),
        "disabled incoming expansion should return no candidates: {disabled:#?}"
    );

    Ok(())
}

#[test]
fn expand_call_context_does_not_promote_non_resolved_target_edges() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let resolved_owner = Uuid::from_u128(0x1a1);
    let unresolved_owner = Uuid::from_u128(0x1a2);
    let ambiguous_owner = Uuid::from_u128(0x1a3);
    let target = Uuid::from_u128(0x1a4);
    let resolved_site = Uuid::from_u128(0x1a5);
    let unresolved_site = Uuid::from_u128(0x1a6);
    let ambiguous_site = Uuid::from_u128(0x1a7);

    for (owner, site, status, resolution) in [
        (
            resolved_owner,
            resolved_site,
            "Resolved",
            Some("LocalExact"),
        ),
        (unresolved_owner, unresolved_site, "Unresolved", None),
        (ambiguous_owner, ambiguous_site, "Ambiguous", None),
    ] {
        insert_call_site(
            &db,
            SiteSeed {
                id: site,
                owner,
                kind: "Path",
                span: (10, 20),
                path: Some(vec!["crate", "target"]),
                method: None,
                macro_name: None,
                receiver: None,
                arg_count: Some(0),
                generic_arg_count: Some(0),
            },
        )?;
        insert_edge(&db, owner, site, "Path")?;
        insert_relation(&db, site, target, "Function", "Path", "Function")?;
        insert_status(&db, site, "Path", status, resolution)?;
    }

    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        3,
        "low-level target-centered helper should expose all persisted target rows: {callers:#?}"
    );

    let incoming = db.expand_call_context(
        CallContextSeed::Target(target),
        CallContextOptions {
            include_outgoing_targets: false,
            ..CallContextOptions::default()
        },
    )?;
    assert_eq!(
        incoming.len(),
        1,
        "call-context expansion should only promote resolved target rows: {incoming:#?}"
    );
    assert_eq!(incoming[0].node_id, resolved_owner);
    assert_eq!(incoming[0].call_site_id, resolved_site);
    assert_eq!(incoming[0].relation, CallContextRelation::IncomingCaller);

    for owner in [unresolved_owner, ambiguous_owner] {
        let outgoing = db.expand_call_context(
            CallContextSeed::Owner(owner),
            CallContextOptions {
                include_incoming_callers: false,
                ..CallContextOptions::default()
            },
        )?;
        assert!(
            outgoing.is_empty(),
            "non-resolved owner {owner} should not promote target edges: {outgoing:#?}"
        );
    }

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
fn context_for_owner_decodes_remaining_method_receivers() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x231);
    let cases = vec![
        ("SelfValue", DataValue::Null, CallReceiver::SelfValue),
        (
            "BorrowedLocalBinding",
            list(&["borrowed"]),
            CallReceiver::BorrowedLocalBinding {
                name: "borrowed".to_string(),
            },
        ),
        (
            "DereferencedLocalBinding",
            list(&["deref"]),
            CallReceiver::DereferencedLocalBinding {
                name: "deref".to_string(),
            },
        ),
        (
            "FieldLocalBinding",
            list(&["fielded", "inner"]),
            CallReceiver::FieldLocalBinding {
                name: "fielded".to_string(),
                field_path: vec!["inner".to_string()],
            },
        ),
        (
            "FieldTypedLocalBinding",
            list(&["fielded", "Holder", "", "inner"]),
            CallReceiver::FieldTypedLocalBinding {
                name: "fielded".to_string(),
                type_path: vec!["Holder".to_string()],
                field_path: vec!["inner".to_string()],
            },
        ),
        (
            "PathCallResult",
            list(&["make_local_assoc"]),
            CallReceiver::PathCallResult {
                path: vec!["make_local_assoc".to_string()],
            },
        ),
        (
            "MethodCallResult",
            list(&["clone_assoc"]),
            CallReceiver::MethodCallResult {
                method_name: "clone_assoc".to_string(),
            },
        ),
        ("AwaitResult", DataValue::Null, CallReceiver::AwaitResult),
        ("TryResult", DataValue::Null, CallReceiver::TryResult),
        ("Literal", DataValue::Null, CallReceiver::Literal),
    ];

    for (idx, (kind, path, _expected)) in cases.iter().enumerate() {
        let site = Uuid::from_u128(0x240 + idx as u128);
        let target = Uuid::from_u128(0x260 + idx as u128);
        insert_call_site_raw_receiver(
            &db,
            SiteSeed {
                id: site,
                owner,
                kind: "Method",
                span: (200 + idx as i64 * 10, 209 + idx as i64 * 10),
                path: None,
                method: Some("instance_value"),
                macro_name: None,
                receiver: None,
                arg_count: Some(0),
                generic_arg_count: Some(0),
            },
            DataValue::from(*kind),
            path.clone(),
        )?;
        insert_edge(&db, owner, site, "Method")?;
        insert_relation(&db, site, target, "Method", "Method", "Method")?;
        insert_status(&db, site, "Method", "Resolved", Some("LocalExact"))?;
    }

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), cases.len(), "context rows: {context:#?}");
    for (idx, (_kind, _path, expected)) in cases.iter().enumerate() {
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
    assert_eq!(context[0].targets[0].target_kind, CallTargetKind::Method);

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
    assert_eq!(context[0].targets[0].target_kind, CallTargetKind::Struct);
    assert_eq!(
        context[1].targets[0].relation,
        CallRelationKind::EnumVariantConstructor
    );
    assert_eq!(context[1].targets[0].target_kind, CallTargetKind::Variant);

    Ok(())
}
