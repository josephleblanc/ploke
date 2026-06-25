use super::*;

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
            CallTargetKind::Method,
        );
    }

    Ok(())
}
