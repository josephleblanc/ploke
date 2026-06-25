use super::*;

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
