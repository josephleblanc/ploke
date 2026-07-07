use super::*;

#[test]
fn fixture_projection_stores_real_dynamic_call_proof_facts() -> Result<(), DbError> {
    let cases = [ResolvedDynamicContextCase {
        owner: "call_aliased_indexed_named_field_function_binding",
        path: &["alias", "callbacks", "0"],
        expected_rows: 1,
    }];

    assert_fixture_resolved_dynamic_proof_batches(&[ResolvedDynamicProofBatch {
        label: "dynamic",
        cases: &cases,
        count: ProofEdgeCount::Exact,
    }])
}

#[test]
fn fixture_projection_stores_real_branch_and_match_dynamic_call_proof_facts() -> Result<(), DbError>
{
    let cases = [
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
        ResolvedDynamicContextCase {
            owner: "call_match_guarded_function_item",
            path: &["local_target"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_single_if_function_pointer_param_branch",
            path: &["f"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_single_match_function_pointer_param_arm",
            path: &["f"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_if_nested_branch_expression",
            path: &["local_target"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_match_nested_arm_expression",
            path: &["local_target"],
            expected_rows: 1,
        },
    ];

    assert_fixture_resolved_dynamic_proof_batches(&[ResolvedDynamicProofBatch {
        label: "branch/match dynamic",
        cases: &cases,
        count: ProofEdgeCount::Exact,
    }])
}

#[test]
fn fixture_projection_stores_real_callable_expression_dynamic_call_proof_facts()
-> Result<(), DbError> {
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
            owner: "call_single_parenthesized_function_pointer_param",
            path: &["f"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_single_parenthesized_aliased_function_pointer_param",
            path: &["g"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_single_function_pointer_param_cast",
            path: &["f"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_parenthesized_block_initialized_function_item_binding",
            path: &["f"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_parenthesized_match_initialized_function_item_binding",
            path: &["f"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_parenthesized_boxed_dyn_fn_value_binding",
            path: &["boxed_fn"],
            expected_rows: 2,
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

    assert_fixture_resolved_dynamic_proof_batches(&[ResolvedDynamicProofBatch {
        label: "callable dynamic",
        cases: &cases,
        count: ProofEdgeCount::AtLeast,
    }])
}

#[test]
fn fixture_projection_stores_dereferenced_closure_binding_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_dereferenced_closure_binding")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "dereferenced closure binding context rows: {context:#?}"
    );

    let row = row_by_kind_path(&context, CallSiteKind::Dynamic, &["closure"]);
    assert_eq!(
        row.targets.len(),
        1,
        "dereferenced closure binding should resolve to one closure owner: {row:#?}"
    );
    let closure = row.targets[0].target_id;
    assert_resolved_target(
        row,
        closure,
        CallRelationKind::DynamicClosure,
        CallSiteKind::Dynamic,
        CallTargetKind::Closure,
    );
    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 3, "dereferenced closure binding proof fact count");

    assert_owner_proof_edges(
        &db,
        "dereferenced closure binding dynamic",
        &[OwnerProofEdge {
            owner,
            site: row.site.id,
            span: row.site.span,
            target: closure,
        }],
        "fixture_call_graph/src/lib.rs",
        "dynamic_dispatch_unbounded",
        ProofEdgeCount::Exact,
    )
}

#[test]
fn fixture_projection_stores_real_field_dynamic_call_proof_facts() -> Result<(), DbError> {
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
        ResolvedDynamicContextCase {
            owner: "call_single_named_field_function_param",
            path: &["holder", "callback"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_single_indexed_function_pointer_param",
            path: &["funcs", "0"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_single_indexed_field_function_param",
            path: &["holder", "callbacks", "0"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_single_indexed_tuple_field_function_param",
            path: &["holder", "0", "0"],
            expected_rows: 1,
        },
    ];

    assert_fixture_resolved_dynamic_proof_batches(&[ResolvedDynamicProofBatch {
        label: "field dynamic",
        cases: &cases,
        count: ProofEdgeCount::AtLeast,
    }])
}
