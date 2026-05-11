use ploke_db::{DbError, TypeContainmentKind};

use super::common::{setup_typed_fixture_db, type_alias_row_by_name};

/// Elementary structural contract: generic arguments of a named type should be
/// queryable through one normalized containment surface, preserving argument
/// order.
///
/// Grounding: `uuid_phase2_partial_graphs/nodes/type_alias.rs` already asserts
/// that `fixture_nodes::type_alias::Mapping` has target type
/// `std::collections::HashMap<K, V>` with two related type nodes in order.
/// This test describes the Cozo-facing shape we want for that same fact.
#[test]
fn named_generic_arguments_preserve_order() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_nodes")?;
    let (_alias_id, root_type_id) = type_alias_row_by_name(&db, "Mapping")?;

    let edges = db.direct_type_contains(root_type_id)?;
    let argument_positions: Vec<_> = edges
        .iter()
        .filter(|edge| edge.kind == TypeContainmentKind::Argument)
        .map(|edge| edge.position)
        .collect();

    assert_eq!(
        argument_positions,
        vec![Some(0), Some(1)],
        "expected ordered HashMap<K, V> argument containment edges; edges: {edges:#?}"
    );
    Ok(())
}

/// Structural contract for tuple element containment.
///
/// `Point = (i32, i32)` should expose two element edges in order. These edges
/// support graphRAG distance between exact tuple users and users of the element
/// types.
#[test]
fn tuple_elements_preserve_order() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_nodes")?;
    let (_alias_id, root_type_id) = type_alias_row_by_name(&db, "Point")?;

    let edges = db.direct_type_contains(root_type_id)?;
    let element_positions: Vec<_> = edges
        .iter()
        .filter(|edge| edge.kind == TypeContainmentKind::Element)
        .map(|edge| edge.position)
        .collect();

    assert_eq!(
        element_positions,
        vec![Some(0), Some(1)],
        "expected ordered tuple element containment edges; edges: {edges:#?}"
    );
    Ok(())
}

/// Structural contract for pointer-like wrappers.
///
/// `StrSlice = &str` should have a single `Referenced` containment edge. The
/// exact target type is fixture-dependent, but the shape and cardinality are
/// the DB contract here.
#[test]
fn reference_type_has_single_referenced_child() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_nodes")?;
    let (_alias_id, root_type_id) = type_alias_row_by_name(&db, "StrSlice")?;

    let edges = db.direct_type_contains(root_type_id)?;
    let referenced_edges: Vec<_> = edges
        .iter()
        .filter(|edge| edge.kind == TypeContainmentKind::Referenced)
        .collect();

    assert_eq!(
        referenced_edges.len(),
        1,
        "expected one referenced child for &str alias; edges: {edges:#?}"
    );
    assert_eq!(referenced_edges[0].position, None);
    Ok(())
}

/// Structural contract for function pointer types.
///
/// `MathOperation = fn(i32, i32) -> i32` should expose parameter child edges
/// and a return child edge. This is important for graphRAG because function
/// pointer users should connect both through the callable shape and through
/// its argument/return types.
#[test]
fn function_pointer_type_contains_params_and_return() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_nodes")?;
    let (_alias_id, root_type_id) = type_alias_row_by_name(&db, "MathOperation")?;

    let edges = db.direct_type_contains(root_type_id)?;
    let param_positions: Vec<_> = edges
        .iter()
        .filter(|edge| edge.kind == TypeContainmentKind::FunctionParam)
        .map(|edge| edge.position)
        .collect();
    let return_count = edges
        .iter()
        .filter(|edge| edge.kind == TypeContainmentKind::FunctionReturn)
        .count();

    assert_eq!(
        param_positions,
        vec![Some(0), Some(1)],
        "expected ordered fn pointer parameter containment edges; edges: {edges:#?}"
    );
    assert_eq!(
        return_count, 1,
        "expected one fn pointer return containment edge; edges: {edges:#?}"
    );
    Ok(())
}

/// Structural contract for trait object bounds.
///
/// `DynDrawable = dyn std::fmt::Debug` should expose a trait-bound child. This
/// is the local DB shape that later composes with trait-position resolution.
#[test]
fn trait_object_contains_trait_bound_child() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_nodes")?;
    let (_alias_id, root_type_id) = type_alias_row_by_name(&db, "DynDrawable")?;

    let edges = db.direct_type_contains(root_type_id)?;
    let bound_edges: Vec<_> = edges
        .iter()
        .filter(|edge| edge.kind == TypeContainmentKind::TraitBound)
        .collect();

    assert_eq!(
        bound_edges.len(),
        1,
        "expected one trait bound child for dyn trait alias; edges: {edges:#?}"
    );
    assert_eq!(bound_edges[0].position, Some(0));
    Ok(())
}

/// Structural contract for qualified associated type projections.
///
/// `<Const<N> as IntoArrayLength>::ArrayLength` should expose its qualified
/// self type and explicit trait qualifier through typed containment kinds. This
/// prevents transform and DB decoding from drifting into string-only agreement.
#[test]
fn qualified_projection_contains_self_and_trait_qualifier() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let (_alias_id, root_type_id) = type_alias_row_by_name(&db, "ProjectedArrayLength")?;

    let edges = db.direct_type_contains(root_type_id)?;
    let qualified_self_count = edges
        .iter()
        .filter(|edge| edge.kind == TypeContainmentKind::QualifiedSelf)
        .count();
    let qualified_trait_count = edges
        .iter()
        .filter(|edge| edge.kind == TypeContainmentKind::QualifiedTrait)
        .count();

    assert_eq!(
        qualified_self_count, 1,
        "expected one qualified self child for projection alias; edges: {edges:#?}"
    );
    assert_eq!(
        qualified_trait_count, 1,
        "expected one qualified trait child for projection alias; edges: {edges:#?}"
    );
    Ok(())
}
