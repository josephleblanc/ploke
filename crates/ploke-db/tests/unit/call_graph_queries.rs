use cozo::{DataValue, Db, MemStorage};
use ploke_db::{
    CallContextOptions, CallContextRelation, CallContextSeed, CallReceiver, CallRelationKind,
    CallResolutionKind, CallSiteKind, CallStatusKind, CallTargetKind, Database, DbError,
    ProofGraphStore, ProofInvariantStatus,
};
use uuid::Uuid;

use super::call_graph_common::*;

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
fn call_graph_queries_exclude_invalid_endpoint_family_rows() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x2a1);
    let site = Uuid::from_u128(0x2a2);
    let valid_target = Uuid::from_u128(0x2a3);
    let wrong_source = Uuid::from_u128(0x2a4);
    let wrong_target = Uuid::from_u128(0x2a5);
    let wrong_relation = Uuid::from_u128(0x2a6);

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
    insert_relation(&db, site, valid_target, "Function", "Path", "Function")?;
    insert_relation(&db, site, wrong_source, "Method", "Method", "Method")?;
    insert_relation(&db, site, wrong_target, "Function", "Path", "Method")?;
    insert_relation(
        &db,
        site,
        wrong_relation,
        "TupleStructConstructor",
        "Path",
        "Method",
    )?;
    insert_status(&db, site, "Path", "Resolved", Some("LocalExact"))?;

    let targets = db.call_targets_for_site(site)?;
    assert_eq!(targets.len(), 1, "target rows: {targets:#?}");
    assert_eq!(targets[0].target_id, valid_target);
    assert_eq!(targets[0].relation, CallRelationKind::Function);
    assert_eq!(targets[0].source_kind, CallSiteKind::Path);
    assert_eq!(targets[0].target_kind, CallTargetKind::Function);

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "owner context: {context:#?}");
    assert_eq!(
        context[0].targets.len(),
        1,
        "owner context should exclude invalid relation families: {context:#?}"
    );
    assert_eq!(context[0].targets[0].target_id, valid_target);

    let callers = db.callers_for_target(valid_target)?;
    assert_eq!(callers.len(), 1, "valid callers: {callers:#?}");
    assert_eq!(callers[0].site.owner_id, owner);
    assert_eq!(callers[0].target.target_id, valid_target);

    for invalid in [wrong_source, wrong_target, wrong_relation] {
        let callers = db.callers_for_target(invalid)?;
        assert!(
            callers.is_empty(),
            "invalid target {invalid} should not surface target-centered callers: {callers:#?}"
        );

        let incoming = db.expand_call_context(
            CallContextSeed::Target(invalid),
            CallContextOptions {
                include_outgoing_targets: false,
                ..CallContextOptions::default()
            },
        )?;
        assert!(
            incoming.is_empty(),
            "invalid target {invalid} should not be promoted for call-context expansion: {incoming:#?}"
        );
    }

    let outgoing = db.expand_call_context(
        CallContextSeed::Owner(owner),
        CallContextOptions {
            include_incoming_callers: false,
            ..CallContextOptions::default()
        },
    )?;
    assert_eq!(outgoing.len(), 1, "outgoing candidates: {outgoing:#?}");
    assert_eq!(outgoing[0].node_id, valid_target);
    assert_eq!(outgoing[0].target_id, valid_target);

    Ok(())
}

#[test]
fn call_graph_queries_exclude_missing_endpoint_target_rows() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x2b1);
    let site = Uuid::from_u128(0x2b2);
    let valid_target = Uuid::from_u128(0x2b3);
    let missing_target = Uuid::from_u128(0x2b4);

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
    insert_relation(&db, site, valid_target, "Function", "Path", "Function")?;
    insert_relation_raw(&db, site, missing_target, "Function", "Path", "Function")?;
    insert_status(&db, site, "Path", "Resolved", Some("LocalExact"))?;

    let targets = db.call_targets_for_site(site)?;
    assert_eq!(
        targets.len(),
        1,
        "call targets should exclude relation rows whose declared endpoint is missing: {targets:#?}"
    );
    assert_eq!(targets[0].target_id, valid_target);

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "owner context: {context:#?}");
    assert_eq!(
        context[0].targets.len(),
        1,
        "owner context should not count missing endpoint rows as resolved targets: {context:#?}"
    );
    assert_eq!(context[0].targets[0].target_id, valid_target);

    let callers = db.callers_for_target(valid_target)?;
    assert_eq!(callers.len(), 1, "valid callers: {callers:#?}");
    assert_eq!(callers[0].target.target_id, valid_target);

    let dangling_callers = db.callers_for_target(missing_target)?;
    assert!(
        dangling_callers.is_empty(),
        "target-centered helper must not surface missing endpoint callers: {dangling_callers:#?}"
    );

    let outgoing = db.expand_call_context(
        CallContextSeed::Owner(owner),
        CallContextOptions {
            include_incoming_callers: false,
            ..CallContextOptions::default()
        },
    )?;
    assert_eq!(outgoing.len(), 1, "outgoing candidates: {outgoing:#?}");
    assert_eq!(outgoing[0].node_id, valid_target);

    let incoming = db.expand_call_context(
        CallContextSeed::Target(missing_target),
        CallContextOptions {
            include_outgoing_targets: false,
            ..CallContextOptions::default()
        },
    )?;
    assert!(
        incoming.is_empty(),
        "call-context expansion must not promote missing endpoint callers: {incoming:#?}"
    );

    Ok(())
}

#[test]
fn call_graph_queries_reject_status_source_kind_mismatch() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x2b1);
    let site = Uuid::from_u128(0x2b2);
    let target = Uuid::from_u128(0x2b3);

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
    insert_status(&db, site, "Method", "Resolved", Some("LocalExact"))?;

    let status_error = db
        .call_resolution_for_site(site)
        .expect_err("status source-kind mismatch must be rejected");
    assert!(
        status_error
            .to_string()
            .contains("call_resolution_status source_kind"),
        "unexpected status error: {status_error}"
    );

    let context_error = db
        .call_context_for_owner(owner)
        .expect_err("owner context must reject mismatched status rows");
    assert!(
        context_error
            .to_string()
            .contains("call_resolution_status source_kind"),
        "unexpected context error: {context_error}"
    );

    Ok(())
}

#[test]
fn call_graph_queries_reject_status_resolution_kind_mismatch() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x2f1);
    let resolved_site = Uuid::from_u128(0x2f2);
    let resolved_target = Uuid::from_u128(0x2f3);
    let external_site = Uuid::from_u128(0x2f4);
    let external_target = Uuid::from_u128(0x2f5);

    insert_call_site(
        &db,
        SiteSeed {
            id: resolved_site,
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
    insert_edge(&db, owner, resolved_site, "Path")?;
    insert_relation(
        &db,
        resolved_site,
        resolved_target,
        "Function",
        "Path",
        "Function",
    )?;
    insert_status(&db, resolved_site, "Path", "Resolved", None)?;

    insert_call_site(
        &db,
        SiteSeed {
            id: external_site,
            owner,
            kind: "Path",
            span: (30, 40),
            path: Some(vec!["std", "target"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, external_site, "Path")?;
    insert_relation(
        &db,
        external_site,
        external_target,
        "Function",
        "Path",
        "Function",
    )?;
    insert_status(&db, external_site, "Path", "External", Some("LocalExact"))?;

    for site in [resolved_site, external_site] {
        let status_error = db
            .call_resolution_for_site(site)
            .expect_err("status/resolution mismatch must be rejected");
        assert!(
            status_error
                .to_string()
                .contains("call_resolution_status resolution_kind"),
            "unexpected status error for {site}: {status_error}"
        );
    }

    let context_error = db
        .call_context_for_owner(owner)
        .expect_err("owner context must reject status/resolution mismatch");
    assert!(
        context_error
            .to_string()
            .contains("call_resolution_status resolution_kind"),
        "unexpected owner context error: {context_error}"
    );

    let caller_error = db
        .callers_for_target(resolved_target)
        .expect_err("target-centered callers must reject status/resolution mismatch");
    assert!(
        caller_error
            .to_string()
            .contains("call_resolution_status resolution_kind"),
        "unexpected target-centered caller error: {caller_error}"
    );

    let incoming_error = db
        .expand_call_context(
            CallContextSeed::Target(resolved_target),
            CallContextOptions {
                include_outgoing_targets: false,
                ..CallContextOptions::default()
            },
        )
        .expect_err("call-context expansion must reject status/resolution mismatch");
    assert!(
        incoming_error
            .to_string()
            .contains("call_resolution_status resolution_kind"),
        "unexpected expansion error: {incoming_error}"
    );

    Ok(())
}

#[test]
fn call_graph_queries_reject_resolved_target_cardinality_mismatch() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let missing_owner = Uuid::from_u128(0x301);
    let missing_site = Uuid::from_u128(0x302);
    let multi_owner = Uuid::from_u128(0x303);
    let multi_site = Uuid::from_u128(0x304);
    let first_target = Uuid::from_u128(0x305);
    let second_target = Uuid::from_u128(0x306);

    insert_call_site(
        &db,
        SiteSeed {
            id: missing_site,
            owner: missing_owner,
            kind: "Path",
            span: (10, 20),
            path: Some(vec!["crate", "missing_target"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, missing_owner, missing_site, "Path")?;
    insert_status(&db, missing_site, "Path", "Resolved", Some("LocalExact"))?;

    insert_call_site(
        &db,
        SiteSeed {
            id: multi_site,
            owner: multi_owner,
            kind: "Path",
            span: (30, 40),
            path: Some(vec!["crate", "multi_target"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, multi_owner, multi_site, "Path")?;
    insert_relation(
        &db,
        multi_site,
        first_target,
        "Function",
        "Path",
        "Function",
    )?;
    insert_relation(
        &db,
        multi_site,
        second_target,
        "Function",
        "Path",
        "Function",
    )?;
    insert_status(&db, multi_site, "Path", "Resolved", Some("LocalExact"))?;

    for owner in [missing_owner, multi_owner] {
        let context_error = db
            .call_context_for_owner(owner)
            .expect_err("resolved call target cardinality mismatch must be rejected");
        assert!(
            context_error.to_string().contains("resolved call site"),
            "unexpected owner context error for {owner}: {context_error}"
        );

        let expansion_error = db
            .expand_call_context(
                CallContextSeed::Owner(owner),
                CallContextOptions {
                    include_incoming_callers: false,
                    ..CallContextOptions::default()
                },
            )
            .expect_err("owner-seeded expansion must reject resolved target cardinality mismatch");
        assert!(
            expansion_error.to_string().contains("resolved call site"),
            "unexpected expansion error for {owner}: {expansion_error}"
        );
    }

    Ok(())
}

#[test]
fn callers_for_target_rejects_resolved_target_cardinality_mismatch() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x321);
    let site = Uuid::from_u128(0x322);
    let first_target = Uuid::from_u128(0x323);
    let second_target = Uuid::from_u128(0x324);

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
    insert_relation(&db, site, first_target, "Function", "Path", "Function")?;
    insert_relation(&db, site, second_target, "Function", "Path", "Function")?;
    insert_status(&db, site, "Path", "Resolved", Some("LocalExact"))?;

    for target in [first_target, second_target] {
        let caller_error = db
            .callers_for_target(target)
            .expect_err("target-centered callers must reject resolved multi-target sites");
        assert!(
            caller_error.to_string().contains("resolved call site"),
            "unexpected target-centered caller error for {target}: {caller_error}"
        );

        let incoming_error = db
            .expand_call_context(
                CallContextSeed::Target(target),
                CallContextOptions {
                    include_outgoing_targets: false,
                    ..CallContextOptions::default()
                },
            )
            .expect_err("target-centered expansion must reject resolved multi-target sites");
        assert!(
            incoming_error.to_string().contains("resolved call site"),
            "unexpected expansion error for {target}: {incoming_error}"
        );
    }

    Ok(())
}

#[test]
fn call_graph_queries_exclude_body_contains_call_site_kind_mismatch() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x2c1);
    let site = Uuid::from_u128(0x2c2);
    let target = Uuid::from_u128(0x2c3);

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
    insert_edge(&db, owner, site, "Method")?;
    insert_relation(&db, site, target, "Function", "Path", "Function")?;
    insert_status(&db, site, "Path", "Resolved", Some("LocalExact"))?;

    let targets = db.call_targets_for_site(site)?;
    assert_eq!(
        targets.len(),
        1,
        "site-local target lookup should still expose valid direct target rows: {targets:#?}"
    );

    let sites = db.call_sites_for_owner(owner)?;
    assert!(
        sites.is_empty(),
        "owner query must exclude BodyContainsCall rows whose target kind disagrees with call_site.call_kind: {sites:#?}"
    );

    let context = db.call_context_for_owner(owner)?;
    assert!(
        context.is_empty(),
        "owner context must not assemble mismatched BodyContainsCall rows: {context:#?}"
    );

    let callers = db.callers_for_target(target)?;
    assert!(
        callers.is_empty(),
        "target-centered callers must exclude mismatched BodyContainsCall rows: {callers:#?}"
    );

    let incoming = db.expand_call_context(
        CallContextSeed::Target(target),
        CallContextOptions {
            include_outgoing_targets: false,
            ..CallContextOptions::default()
        },
    )?;
    assert!(
        incoming.is_empty(),
        "call-context expansion must not promote mismatched BodyContainsCall rows: {incoming:#?}"
    );

    Ok(())
}

#[test]
fn call_graph_queries_exclude_body_contains_owner_kind_mismatch() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x2d1);
    let site = Uuid::from_u128(0x2d2);
    let target = Uuid::from_u128(0x2d3);

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
    insert_edge_with_source_kind(&db, owner, site, "Method", "Path")?;
    insert_relation(&db, site, target, "Function", "Path", "Function")?;
    insert_status(&db, site, "Path", "Resolved", Some("LocalExact"))?;

    let targets = db.call_targets_for_site(site)?;
    assert_eq!(
        targets.len(),
        1,
        "site-local target lookup should still expose valid direct target rows: {targets:#?}"
    );

    let sites = db.call_sites_for_owner(owner)?;
    assert!(
        sites.is_empty(),
        "owner query must exclude BodyContainsCall rows whose source kind disagrees with the stored owner kind: {sites:#?}"
    );

    let context = db.call_context_for_owner(owner)?;
    assert!(
        context.is_empty(),
        "owner context must not assemble owner-kind mismatched BodyContainsCall rows: {context:#?}"
    );

    let callers = db.callers_for_target(target)?;
    assert!(
        callers.is_empty(),
        "target-centered callers must exclude owner-kind mismatched BodyContainsCall rows: {callers:#?}"
    );

    let incoming = db.expand_call_context(
        CallContextSeed::Target(target),
        CallContextOptions {
            include_outgoing_targets: false,
            ..CallContextOptions::default()
        },
    )?;
    assert!(
        incoming.is_empty(),
        "call-context expansion must not promote owner-kind mismatched BodyContainsCall rows: {incoming:#?}"
    );

    Ok(())
}

#[test]
fn call_graph_queries_reject_malformed_call_site_shape() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x2e1);
    let site = Uuid::from_u128(0x2e2);
    let target = Uuid::from_u128(0x2e3);

    insert_call_site(
        &db,
        SiteSeed {
            id: site,
            owner,
            kind: "Path",
            span: (10, 20),
            path: Some(vec!["crate", "target"]),
            method: Some("leaked_method"),
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, site, "Path")?;
    insert_relation(&db, site, target, "Function", "Path", "Function")?;
    insert_status(&db, site, "Path", "Resolved", Some("LocalExact"))?;

    let owner_error = db
        .call_sites_for_owner(owner)
        .expect_err("malformed call-site shape must be rejected");
    assert!(
        owner_error.to_string().contains("malformed Path call_site"),
        "unexpected owner query error: {owner_error}"
    );

    let context_error = db
        .call_context_for_owner(owner)
        .expect_err("owner context must reject malformed call-site shape");
    assert!(
        context_error
            .to_string()
            .contains("malformed Path call_site"),
        "unexpected owner context error: {context_error}"
    );

    let caller_error = db
        .callers_for_target(target)
        .expect_err("target-centered callers must reject malformed call-site shape");
    assert!(
        caller_error
            .to_string()
            .contains("malformed Path call_site"),
        "unexpected target-centered caller error: {caller_error}"
    );

    let incoming_error = db
        .expand_call_context(
            CallContextSeed::Target(target),
            CallContextOptions {
                include_outgoing_targets: false,
                ..CallContextOptions::default()
            },
        )
        .expect_err("call-context expansion must reject malformed call-site shape");
    assert!(
        incoming_error
            .to_string()
            .contains("malformed Path call_site"),
        "unexpected expansion error: {incoming_error}"
    );

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
fn callers_for_target_rejects_missing_status() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x311);
    let site = Uuid::from_u128(0x312);
    let target = Uuid::from_u128(0x313);

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

    let caller_error = db
        .callers_for_target(target)
        .expect_err("target-centered callers must reject missing call status");
    assert!(
        caller_error
            .to_string()
            .contains("missing call_resolution_status"),
        "unexpected target-centered caller error: {caller_error}"
    );

    let incoming_error = db
        .expand_call_context(
            CallContextSeed::Target(target),
            CallContextOptions {
                include_outgoing_targets: false,
                ..CallContextOptions::default()
            },
        )
        .expect_err("target-centered expansion must reject missing call status");
    assert!(
        incoming_error
            .to_string()
            .contains("missing call_resolution_status"),
        "unexpected expansion error: {incoming_error}"
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
fn owner_and_target_projection_share_stable_fact_identity() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x171);
    let module = Uuid::from_u128(0x172);
    let site = Uuid::from_u128(0x173);
    let target = Uuid::from_u128(0x174);

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

    assert_eq!(
        db.project_call_proof_facts_for_target(target, "bd:test")?,
        3
    );
    assert_eq!(
        db.proof_graphrag_context("")?.len(),
        3,
        "target-centered projection should store one call_site, one call_edge, and one call_resolution fact"
    );

    assert_eq!(db.project_call_proof_facts_for_owner(owner, "bd:test")?, 3);
    let rows = db.proof_graphrag_context("")?;
    assert_eq!(
        rows.len(),
        3,
        "owner projection of the same call site must upsert stable proof fact ids instead of duplicating rows: {rows:#?}"
    );

    let edges = db.proof_checker_edges()?;
    assert_eq!(edges.len(), 1, "proof checker edges: {edges:#?}");
    assert_eq!(edges[0].call_site_id, site.to_string());
    assert_eq!(
        edges[0].callee_def_id.as_deref(),
        Some(target.to_string().as_str())
    );

    let provenance = db
        .proof_source_provenance(&site.to_string())?
        .expect("stable projected call-site source provenance");
    assert_eq!(provenance.source_file, "src/lib.rs");
    assert_eq!(provenance.start_byte, 10);
    assert_eq!(provenance.end_byte, 24);

    Ok(())
}

#[test]
fn proof_projection_requires_non_empty_build_domain_id() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x1b1);
    let module = Uuid::from_u128(0x1b2);
    let site = Uuid::from_u128(0x1b3);
    let target = Uuid::from_u128(0x1b4);

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

    for error in [
        db.call_proof_facts_for_owner(owner, "")
            .expect_err("owner-scoped proof fact generation requires a build domain"),
        db.call_proof_facts_for_target(target, "")
            .expect_err("target-centered proof fact generation requires a build domain"),
        db.project_call_proof_facts_for_owner(owner, "")
            .expect_err("owner-scoped proof projection requires a build domain"),
        db.project_call_proof_facts_for_target(target, "")
            .expect_err("target-centered proof projection requires a build domain"),
    ] {
        assert!(
            error
                .to_string()
                .contains("requires non-empty build_domain_id"),
            "unexpected error: {error}"
        );
    }
    assert!(
        db.proof_graphrag_context("")?.is_empty(),
        "empty build-domain projection attempts must not store partial proof facts"
    );

    Ok(())
}

#[test]
fn proof_projection_requires_source_provenance_before_storage() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x1c1);
    let site = Uuid::from_u128(0x1c2);
    let target = Uuid::from_u128(0x1c3);

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

    let owner_error = db
        .project_call_proof_facts_for_owner(owner, "bd:test")
        .expect_err("owner-scoped projection requires source provenance");
    assert!(
        owner_error.to_string().contains("missing source file"),
        "unexpected owner projection error: {owner_error}"
    );

    let target_error = db
        .project_call_proof_facts_for_target(target, "bd:test")
        .expect_err("target-centered projection requires caller source provenance");
    assert!(
        target_error.to_string().contains("missing source file"),
        "unexpected target projection error: {target_error}"
    );

    assert!(
        db.proof_graphrag_context("")?.is_empty(),
        "missing source provenance must not leave partial proof facts"
    );

    Ok(())
}

#[test]
fn target_centered_proof_projection_prevalidates_caller_sources_before_storage()
-> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let good_owner = Uuid::from_u128(0x1d1);
    let good_module = Uuid::from_u128(0x1d2);
    let good_site = Uuid::from_u128(0x1d3);
    let bad_owner = Uuid::from_u128(0x1d4);
    let bad_site = Uuid::from_u128(0x1d5);
    let target = Uuid::from_u128(0x1d6);

    insert_owner_source(&db, good_owner, good_module, "src/good.rs")?;
    insert_call_site(
        &db,
        SiteSeed {
            id: good_site,
            owner: good_owner,
            kind: "Path",
            span: (10, 24),
            path: Some(vec!["crate", "target"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, good_owner, good_site, "Path")?;
    insert_relation(&db, good_site, target, "Function", "Path", "Function")?;
    insert_status(&db, good_site, "Path", "Resolved", Some("LocalExact"))?;

    insert_call_site(
        &db,
        SiteSeed {
            id: bad_site,
            owner: bad_owner,
            kind: "Path",
            span: (30, 44),
            path: Some(vec!["crate", "target"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, bad_owner, bad_site, "Path")?;
    insert_relation(&db, bad_site, target, "Function", "Path", "Function")?;
    insert_status(&db, bad_site, "Path", "Resolved", Some("LocalExact"))?;

    let callers = db.callers_for_target(target)?;
    assert_eq!(callers.len(), 2, "target callers: {callers:#?}");

    let facts_error = db
        .call_proof_facts_for_target(target, "bd:test")
        .expect_err("target-centered fact generation requires every caller source");
    assert!(
        facts_error.to_string().contains("missing source file"),
        "unexpected target-centered fact generation error: {facts_error}"
    );

    let projection_error = db
        .project_call_proof_facts_for_target(target, "bd:test")
        .expect_err("target-centered projection requires every caller source");
    assert!(
        projection_error.to_string().contains("missing source file"),
        "unexpected target-centered projection error: {projection_error}"
    );

    assert!(
        db.proof_graphrag_context("")?.is_empty(),
        "target-centered source prevalidation must not store partial proof facts"
    );

    Ok(())
}

#[test]
fn proof_graphrag_context_links_generated_call_facts_by_call_site() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x181);
    let module = Uuid::from_u128(0x182);
    let site = Uuid::from_u128(0x183);
    let target = Uuid::from_u128(0x184);

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

    assert_eq!(db.project_call_proof_facts_for_owner(owner, "bd:test")?, 3);

    let rows = db.proof_graphrag_context(&target.to_string())?;
    assert_eq!(
        rows.len(),
        3,
        "callee id query should return the matching edge plus linked call_site and call_resolution rows: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_site"
                && row.call_site_id.as_deref() == Some(site.to_string().as_str())
                && row.caller_def_id.as_deref() == Some(owner.to_string().as_str())
        }),
        "linked call_site row missing: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_edge"
                && row.call_site_id.as_deref() == Some(site.to_string().as_str())
                && row.callee_def_id.as_deref() == Some(target.to_string().as_str())
        }),
        "matching call_edge row missing: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(site.to_string().as_str())
                && row.detail.is_none()
                && row.blocker_reason.is_none()
        }),
        "linked resolved call_resolution row missing: {rows:#?}"
    );

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
fn call_proof_facts_for_owner_preserves_mixed_resolution_shape() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(141);
    let module = Uuid::from_u128(142);
    let resolved = Uuid::from_u128(143);
    let target = Uuid::from_u128(144);
    let external = Uuid::from_u128(145);
    let dynamic = Uuid::from_u128(146);

    insert_owner_source(&db, owner, module, "src/lib.rs")?;
    insert_call_site(
        &db,
        SiteSeed {
            id: resolved,
            owner,
            kind: "Path",
            span: (10, 20),
            path: Some(vec!["crate", "helper"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, resolved, "Path")?;
    insert_relation(&db, resolved, target, "Function", "Path", "Function")?;
    insert_status(&db, resolved, "Path", "Resolved", Some("LocalExact"))?;

    insert_call_site(
        &db,
        SiteSeed {
            id: external,
            owner,
            kind: "Path",
            span: (30, 40),
            path: Some(vec!["String", "new"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
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

    let facts = db.call_proof_facts_for_owner(owner, "bd:test")?;
    assert_eq!(facts.len(), 7, "proof facts: {facts:#?}");
    assert_eq!(fact_count(&facts, "call_site"), 3);
    assert_eq!(fact_count(&facts, "call_edge"), 1);
    assert_eq!(fact_count(&facts, "call_resolution"), 3);

    let edge = fact_for_call_site(&facts, "call_edge", resolved);
    assert_eq!(
        edge.get("callee_def_id")
            .and_then(serde_json::Value::as_str),
        Some(target.to_string().as_str())
    );
    assert_eq!(
        edge.get("resolution_state")
            .and_then(serde_json::Value::as_str),
        Some("resolved")
    );

    let resolved_status = fact_for_call_site(&facts, "call_resolution", resolved);
    assert_eq!(
        resolved_status
            .get("resolved_def_id")
            .and_then(serde_json::Value::as_str),
        Some(target.to_string().as_str())
    );
    assert!(
        resolved_status.get("blocking_reason").is_none(),
        "resolved status should not include a blocker: {resolved_status:#?}"
    );

    let external_status = fact_for_call_site(&facts, "call_resolution", external);
    assert_eq!(
        external_status
            .get("resolution_state")
            .and_then(serde_json::Value::as_str),
        Some("blocked")
    );
    assert_eq!(
        external_status
            .get("blocking_reason")
            .and_then(serde_json::Value::as_str),
        Some("external_dependency_summary_missing")
    );

    let dynamic_status = fact_for_call_site(&facts, "call_resolution", dynamic);
    assert_eq!(
        dynamic_status
            .get("resolution_state")
            .and_then(serde_json::Value::as_str),
        Some("blocked")
    );
    assert_eq!(
        dynamic_status
            .get("blocking_reason")
            .and_then(serde_json::Value::as_str),
        Some("dynamic_dispatch_unbounded")
    );

    Ok(())
}

#[test]
fn generated_call_resolution_blockers_feed_proof_invariants() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(151);
    let module = Uuid::from_u128(152);
    let external = Uuid::from_u128(153);

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

    let count = db.project_call_proof_facts_for_owner(owner, "bd:test")?;
    assert_eq!(count, 2);

    db.upsert_proof_fact_values(&[serde_json::json!({
        "fact_kind": "effect_seed",
        "schema_version": "ploke-proof-facts.v1",
        "effect_seed_id": "effect:external-command-new",
        "call_site_id": external.to_string(),
        "effect_class": "operating_system_process_create",
        "confidence": "synthetic-test",
        "blocker_if_unresolved": true,
        "evidence_use": "proof_only"
    })])?;

    let findings = db.proof_invariant_findings()?;
    let finding = findings
        .iter()
        .find(|finding| {
            finding.invariant == "detached_process_successor_handoff"
                && finding.call_site_id.as_deref() == Some(external.to_string().as_str())
        })
        .expect("detached process finding for generated external call proof");
    assert_eq!(finding.status, ProofInvariantStatus::Blocked);
    assert!(
        finding
            .reason
            .contains("external_dependency_summary_missing"),
        "finding should be blocked by generated external call resolution: {finding:#?}"
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

#[test]
fn proof_projection_rejects_ambiguous_local_targets() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x151);
    let module = Uuid::from_u128(0x152);
    let site = Uuid::from_u128(0x153);
    let target = Uuid::from_u128(0x154);

    insert_owner_source(&db, owner, module, "src/lib.rs")?;
    insert_call_site(
        &db,
        SiteSeed {
            id: site,
            owner,
            kind: "Method",
            span: (100, 130),
            path: None,
            method: Some("overlap"),
            macro_name: None,
            receiver: Some(("LocalBinding", vec!["value"])),
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, site, "Method")?;
    insert_relation(&db, site, target, "Method", "Method", "Method")?;
    insert_status(&db, site, "Method", "Ambiguous", None)?;

    let error = db
        .project_call_proof_facts_for_owner(owner, "bd:test")
        .expect_err("ambiguous call site with local targets must reject");
    assert!(
        error.to_string().contains("non-resolved call site"),
        "unexpected error: {error}"
    );
    assert!(db.proof_graphrag_context("")?.is_empty());

    Ok(())
}

#[test]
fn target_centered_proof_projection_rejects_non_resolved_local_targets() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let resolved_owner = Uuid::from_u128(61);
    let resolved_module = Uuid::from_u128(62);
    let unresolved_owner = Uuid::from_u128(63);
    let unresolved_module = Uuid::from_u128(64);
    let target = Uuid::from_u128(65);
    let resolved_site = Uuid::from_u128(66);
    let unresolved_site = Uuid::from_u128(67);

    insert_owner_source(&db, resolved_owner, resolved_module, "src/resolved.rs")?;
    insert_call_site(
        &db,
        SiteSeed {
            id: resolved_site,
            owner: resolved_owner,
            kind: "Path",
            span: (10, 30),
            path: Some(vec!["crate", "target"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, resolved_owner, resolved_site, "Path")?;
    insert_relation(&db, resolved_site, target, "Function", "Path", "Function")?;
    insert_status(&db, resolved_site, "Path", "Resolved", Some("LocalExact"))?;

    insert_owner_source(
        &db,
        unresolved_owner,
        unresolved_module,
        "src/unresolved.rs",
    )?;
    insert_call_site(
        &db,
        SiteSeed {
            id: unresolved_site,
            owner: unresolved_owner,
            kind: "Path",
            span: (40, 60),
            path: Some(vec!["crate", "target"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, unresolved_owner, unresolved_site, "Path")?;
    insert_relation(&db, unresolved_site, target, "Function", "Path", "Function")?;
    insert_status(&db, unresolved_site, "Path", "Unresolved", None)?;

    let callers = db.callers_for_target(target)?;
    assert_eq!(callers.len(), 2, "target callers: {callers:#?}");

    let error = db
        .project_call_proof_facts_for_target(target, "bd:test")
        .expect_err("target-centered projection must reject inconsistent incoming rows");
    assert!(
        error.to_string().contains("non-resolved call site"),
        "unexpected error: {error}"
    );
    assert!(
        db.proof_graphrag_context("")?.is_empty(),
        "target-centered projection must not store partial proof facts after rejection"
    );

    Ok(())
}

#[test]
fn target_centered_proof_projection_rejects_ambiguous_local_targets() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x161);
    let module = Uuid::from_u128(0x162);
    let target = Uuid::from_u128(0x163);
    let site = Uuid::from_u128(0x164);

    insert_owner_source(&db, owner, module, "src/ambiguous.rs")?;
    insert_call_site(
        &db,
        SiteSeed {
            id: site,
            owner,
            kind: "Method",
            span: (140, 170),
            path: None,
            method: Some("overlap"),
            macro_name: None,
            receiver: Some(("LocalBinding", vec!["value"])),
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, site, "Method")?;
    insert_relation(&db, site, target, "Method", "Method", "Method")?;
    insert_status(&db, site, "Method", "Ambiguous", None)?;

    let callers = db.callers_for_target(target)?;
    assert_eq!(callers.len(), 1, "target callers: {callers:#?}");

    let error = db
        .project_call_proof_facts_for_target(target, "bd:test")
        .expect_err("target-centered projection must reject ambiguous incoming rows");
    assert!(
        error.to_string().contains("non-resolved call site"),
        "unexpected error: {error}"
    );
    assert!(
        db.proof_graphrag_context("")?.is_empty(),
        "target-centered projection must not store partial proof facts after rejection"
    );

    Ok(())
}
