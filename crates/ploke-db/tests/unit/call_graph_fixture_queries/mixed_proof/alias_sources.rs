use super::*;
use ploke_db::LocalBindingRelationKind;

#[test]
fn fixture_projection_stores_alias_source_function_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;

    let owner = function_id_by_name(&db, "call_single_aliased_function_pointer_param")?;

    // Source oracle:
    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1685-1691
    // `call_single_aliased_function_pointer_param(f) { let g = f; g() }`
    // is private, and its only local caller supplies `local_target`. The
    // resolved `g()` edge proves both the parameter binding `f` and its local
    // alias `g` are exact function-item sources in this owner.
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_kind_path(&context, CallSiteKind::Path, &["g"]);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let bindings = db.local_bindings_for_owner(owner)?;
    let parameter = bindings
        .iter()
        .find(|binding| binding.kind == "ParameterBinding" && binding.name == "f")
        .expect("parameter binding f should be persisted");
    assert_eq!(parameter.source_kind, "Parameter");

    let alias = bindings
        .iter()
        .find(|binding| binding.kind == "LetBinding" && binding.name == "g")
        .expect("alias binding g should be persisted");
    assert_eq!(alias.source_kind, "ValueAlias");
    assert_eq!(alias.source_path.as_ref(), Some(&path(&["f"])));

    let edges = db.local_binding_edges_for_owner(owner)?;
    assert!(
        edges.iter().any(
            |edge| edge.relation == LocalBindingRelationKind::BindingAliasesBinding
                && edge.source_id == alias.id
                && edge.source_kind == "LocalBinding"
                && edge.target_id == parameter.id
                && edge.target_kind == "LocalBinding"
        ),
        "alias binding should point to the parameter binding: {edges:#?}"
    );
    assert!(
        edges.iter().any(
            |edge| edge.relation == LocalBindingRelationKind::BindingSourceFunction
                && edge.source_id == parameter.id
                && edge.source_kind == "LocalBinding"
                && edge.target_id == target
                && edge.target_kind == "Function"
        ),
        "parameter binding should carry the exact source-function proof: {edges:#?}"
    );
    assert!(
        edges.iter().any(
            |edge| edge.relation == LocalBindingRelationKind::BindingSourceFunction
                && edge.source_id == alias.id
                && edge.source_kind == "LocalBinding"
                && edge.target_id == target
                && edge.target_kind == "Function"
        ),
        "alias binding should carry the exact source-function proof: {edges:#?}"
    );

    Ok(())
}
