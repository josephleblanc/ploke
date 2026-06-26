use super::super::super::super::*;
use super::super::helpers::*;

#[cfg(feature = "call_graph")]
#[tokio::test]
async fn call_context_collection_attaches_outgoing_call_payloads() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::init_with_schema()?);
    let owner = Uuid::from_u128(0x101);
    let site = Uuid::from_u128(0x102);
    let target = Uuid::from_u128(0x103);
    let assoc_site = Uuid::from_u128(0x104);
    let assoc_target = Uuid::from_u128(0x105);
    let tuple_site = Uuid::from_u128(0x106);
    let tuple_target = Uuid::from_u128(0x107);
    let variant_site = Uuid::from_u128(0x108);
    let variant_target = Uuid::from_u128(0x109);
    let method_site = Uuid::from_u128(0x10a);
    let method_target = Uuid::from_u128(0x10b);
    let init_method_site = Uuid::from_u128(0x10c);
    let init_method_target = Uuid::from_u128(0x10d);
    let try_method_site = Uuid::from_u128(0x10e);
    let try_method_target = Uuid::from_u128(0x10f);
    let dynamic_site = Uuid::from_u128(0x110);
    let dynamic_target = Uuid::from_u128(0x111);

    insert_call_site(
        &db,
        CallSeed {
            id: site,
            owner,
            kind: "Path",
            span: (12, 25),
            path: Some(vec!["crate", "helper"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_call_edge(&db, owner, site, "Path")?;
    insert_call_target(&db, site, target, "Function", "Path", "Function")?;
    insert_call_status(&db, site, "Path", "Resolved", Some("LocalExact"))?;
    insert_call_site(
        &db,
        CallSeed {
            id: assoc_site,
            owner,
            kind: "Path",
            span: (40, 58),
            path: Some(vec!["LocalAssoc", "make"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_call_edge(&db, owner, assoc_site, "Path")?;
    insert_call_target(
        &db,
        assoc_site,
        assoc_target,
        "AssociatedFunction",
        "Path",
        "Method",
    )?;
    insert_call_status(&db, assoc_site, "Path", "Resolved", Some("LocalExact"))?;
    insert_call_site(
        &db,
        CallSeed {
            id: tuple_site,
            owner,
            kind: "Path",
            span: (60, 77),
            path: Some(vec!["TupleStruct"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(2),
            generic_arg_count: Some(0),
        },
    )?;
    insert_call_edge(&db, owner, tuple_site, "Path")?;
    insert_call_target(
        &db,
        tuple_site,
        tuple_target,
        "TupleStructConstructor",
        "Path",
        "Struct",
    )?;
    insert_call_status(&db, tuple_site, "Path", "Resolved", Some("LocalExact"))?;
    insert_call_site(
        &db,
        CallSeed {
            id: variant_site,
            owner,
            kind: "Path",
            span: (80, 105),
            path: Some(vec!["EnumWithData", "Variant1"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(1),
            generic_arg_count: Some(0),
        },
    )?;
    insert_call_edge(&db, owner, variant_site, "Path")?;
    insert_call_target(
        &db,
        variant_site,
        variant_target,
        "EnumVariantConstructor",
        "Path",
        "Variant",
    )?;
    insert_call_status(&db, variant_site, "Path", "Resolved", Some("LocalExact"))?;
    insert_call_site(
        &db,
        CallSeed {
            id: method_site,
            owner,
            kind: "Method",
            span: (110, 132),
            path: None,
            method: Some("instance_value"),
            macro_name: None,
            receiver: Some(("LocalBinding", vec!["value"])),
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_call_edge(&db, owner, method_site, "Method")?;
    insert_call_target(
        &db,
        method_site,
        method_target,
        "Method",
        "Method",
        "Method",
    )?;
    insert_call_status(&db, method_site, "Method", "Resolved", Some("LocalExact"))?;
    insert_call_site(
        &db,
        CallSeed {
            id: init_method_site,
            owner,
            kind: "Method",
            span: (134, 156),
            path: None,
            method: Some("instance_value"),
            macro_name: None,
            receiver: Some(("InitializedLocalBinding", vec!["value", "LocalAssoc"])),
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_call_edge(&db, owner, init_method_site, "Method")?;
    insert_call_target(
        &db,
        init_method_site,
        init_method_target,
        "Method",
        "Method",
        "Method",
    )?;
    insert_call_status(
        &db,
        init_method_site,
        "Method",
        "Resolved",
        Some("LocalExact"),
    )?;
    insert_call_site(
        &db,
        CallSeed {
            id: try_method_site,
            owner,
            kind: "Method",
            span: (158, 184),
            path: None,
            method: Some("instance_value"),
            macro_name: None,
            receiver: Some(("TryPathCallResult", vec!["try_local_assoc"])),
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_call_edge(&db, owner, try_method_site, "Method")?;
    insert_call_target(
        &db,
        try_method_site,
        try_method_target,
        "Method",
        "Method",
        "Method",
    )?;
    insert_call_status(
        &db,
        try_method_site,
        "Method",
        "Resolved",
        Some("LocalExact"),
    )?;
    insert_call_site(
        &db,
        CallSeed {
            id: dynamic_site,
            owner,
            kind: "Dynamic",
            span: (186, 202),
            path: None,
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: None,
        },
    )?;
    insert_call_edge(&db, owner, dynamic_site, "Dynamic")?;
    insert_call_target(
        &db,
        dynamic_site,
        dynamic_target,
        "DynamicFunction",
        "Dynamic",
        "Function",
    )?;
    insert_call_status(&db, dynamic_site, "Dynamic", "Resolved", Some("LocalExact"))?;

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh call_graph schema should enable call context collection"
    );

    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let owner_context = call_context
        .get(&owner)
        .expect("owner should receive outgoing call context");
    assert_eq!(owner_context.len(), 8);
    let call = &owner_context[0];
    assert_eq!(call.site_id, site);
    assert_eq!(call.kind, CallSiteKind::Path);
    assert_eq!(
        call.callee,
        CallCalleeInfo::Path {
            path: vec!["crate".to_string(), "helper".to_string()]
        }
    );
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, target);
    assert_eq!(call.targets[0].relation, CallTargetKind::Function);
    let assoc_call = &owner_context[1];
    assert_eq!(assoc_call.site_id, assoc_site);
    assert_eq!(assoc_call.targets.len(), 1);
    assert_eq!(assoc_call.targets[0].target_id, assoc_target);
    assert_eq!(
        assoc_call.targets[0].relation,
        CallTargetKind::AssociatedFunction
    );
    let tuple_call = &owner_context[2];
    assert_eq!(tuple_call.site_id, tuple_site);
    assert_eq!(tuple_call.targets.len(), 1);
    assert_eq!(tuple_call.targets[0].target_id, tuple_target);
    assert_eq!(
        tuple_call.targets[0].relation,
        CallTargetKind::TupleStructConstructor
    );
    let variant_call = &owner_context[3];
    assert_eq!(variant_call.site_id, variant_site);
    assert_eq!(variant_call.targets.len(), 1);
    assert_eq!(variant_call.targets[0].target_id, variant_target);
    assert_eq!(
        variant_call.targets[0].relation,
        CallTargetKind::EnumVariantConstructor
    );
    let method_call = &owner_context[4];
    assert_eq!(method_call.site_id, method_site);
    assert_eq!(
        method_call.callee,
        CallCalleeInfo::Method {
            name: "instance_value".to_string(),
            receiver: Some(CallReceiverInfo::LocalBinding {
                name: "value".to_string()
            })
        }
    );
    assert_eq!(method_call.targets.len(), 1);
    assert_eq!(method_call.targets[0].target_id, method_target);
    assert_eq!(method_call.targets[0].relation, CallTargetKind::Method);
    let init_method_call = &owner_context[5];
    assert_eq!(init_method_call.site_id, init_method_site);
    assert_eq!(
        init_method_call.callee,
        CallCalleeInfo::Method {
            name: "instance_value".to_string(),
            receiver: Some(CallReceiverInfo::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: vec!["LocalAssoc".to_string()]
            })
        }
    );
    assert_eq!(init_method_call.targets.len(), 1);
    assert_eq!(init_method_call.targets[0].target_id, init_method_target);
    assert_eq!(init_method_call.targets[0].relation, CallTargetKind::Method);
    let try_method_call = &owner_context[6];
    assert_eq!(try_method_call.site_id, try_method_site);
    assert_eq!(
        try_method_call.callee,
        CallCalleeInfo::Method {
            name: "instance_value".to_string(),
            receiver: Some(CallReceiverInfo::TryPathCallResult {
                path: vec!["try_local_assoc".to_string()]
            })
        }
    );
    assert_eq!(try_method_call.targets.len(), 1);
    assert_eq!(try_method_call.targets[0].target_id, try_method_target);
    assert_eq!(try_method_call.targets[0].relation, CallTargetKind::Method);
    let dynamic_call = &owner_context[7];
    assert_eq!(dynamic_call.site_id, dynamic_site);
    assert_eq!(dynamic_call.kind, CallSiteKind::Dynamic);
    assert_eq!(dynamic_call.callee, CallCalleeInfo::Dynamic);
    assert_eq!(dynamic_call.status, CallStatusKind::Resolved);
    assert_eq!(
        dynamic_call.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(dynamic_call.targets.len(), 1);
    assert_eq!(dynamic_call.targets[0].target_id, dynamic_target);
    assert_eq!(
        dynamic_call.targets[0].relation,
        CallTargetKind::DynamicFunction
    );

    Ok(())
}
