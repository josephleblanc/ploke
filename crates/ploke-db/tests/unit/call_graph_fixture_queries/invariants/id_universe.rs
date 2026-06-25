use super::*;

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
