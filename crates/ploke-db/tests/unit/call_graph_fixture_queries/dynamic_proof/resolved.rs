use super::*;

#[test]
fn fixture_projection_stores_real_dynamic_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let expected_edges = resolved_dynamic_proof_edges(
        &db,
        target,
        &[ResolvedDynamicContextCase {
            owner: "call_aliased_indexed_named_field_function_binding",
            path: &["alias", "callbacks", "0"],
            expected_rows: 1,
        }],
    );
    let expected_edges = expected_edges?;

    assert_owner_proof_edges(
        &db,
        "dynamic",
        &expected_edges,
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
    let expected_edges = resolved_dynamic_proof_edges(
        &db,
        target,
        &[
            ResolvedDynamicContextCase {
                owner: "call_if_same_function_item",
                path: &["local_target"],
                expected_rows: 1,
            },
            ResolvedDynamicContextCase {
                owner: "call_match_same_function_item",
                path: &["local_target"],
                expected_rows: 1,
            },
        ],
    )?;

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
    let cases = [
        ResolvedDynamicContextCase {
            owner: "call_parenthesized_local_target",
            path: &["local_target"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_parenthesized_function_item_binding",
            path: &["f"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_parenthesized_aliased_function_item_binding",
            path: &["g"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_parenthesized_typed_function_pointer_alias_binding",
            path: &["g"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_function_pointer_cast_path",
            path: &["local_target"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_function_pointer_cast_binding",
            path: &["f"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_dereferenced_function_pointer_binding",
            path: &["f"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_block_function_item",
            path: &["local_target"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_indexed_initialized_function_array",
            path: &["funcs", "0"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_typed_indexed_initialized_function_array",
            path: &["funcs", "0"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_aliased_indexed_initialized_function_array",
            path: &["alias", "0"],
            expected_rows: 1,
        },
    ];
    let expected_edges = resolved_dynamic_proof_edges(&db, target, &cases)?;

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
    let cases = [
        ResolvedDynamicContextCase {
            owner: "call_named_field_function_binding",
            path: &["holder", "callback"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_aliased_named_field_function_binding",
            path: &["alias", "callback"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_indexed_named_field_function_binding",
            path: &["holder", "callbacks", "0"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_indexed_named_field_array_alias_binding",
            path: &["holder", "callbacks", "0"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_aliased_indexed_named_field_function_binding",
            path: &["alias", "callbacks", "0"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_indexed_tuple_field_function_binding",
            path: &["holder", "0", "0"],
            expected_rows: 2,
        },
        ResolvedDynamicContextCase {
            owner: "call_indexed_tuple_field_array_alias_binding",
            path: &["holder", "0", "0"],
            expected_rows: 2,
        },
        ResolvedDynamicContextCase {
            owner: "call_aliased_indexed_tuple_field_function_binding",
            path: &["alias", "0", "0"],
            expected_rows: 2,
        },
    ];
    let expected_edges = resolved_dynamic_proof_edges(&db, target, &cases)?;

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
