use super::*;
use ploke_db::{CallPathOptions, LocalBindingRelationKind};

#[test]
fn fixture_projection_stores_forwarded_field_argument_parameter_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;

    struct Step {
        caller: &'static str,
        callee: &'static str,
    }

    struct Case {
        label: &'static str,
        source: &'static str,
        leaf: &'static str,
        steps: &'static [Step],
        depth: u32,
    }

    let cases = [
        Case {
            label: "one-hop forwarded named-field holder",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:1982-1993",
            leaf: "call_forwarded_named_field_leaf",
            steps: &[
                Step {
                    caller: "call_forwarded_named_field_param_with_local_target",
                    callee: "call_forwarded_named_field_wrapper",
                },
                Step {
                    caller: "call_forwarded_named_field_wrapper",
                    callee: "call_forwarded_named_field_leaf",
                },
            ],
            depth: 3,
        },
        Case {
            label: "two-hop forwarded named-field holder",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:2052-2066",
            leaf: "call_two_hop_forwarded_named_field_leaf",
            steps: &[
                Step {
                    caller: "call_two_hop_forwarded_named_field_param_with_local_target",
                    callee: "call_two_hop_forwarded_named_field_wrapper",
                },
                Step {
                    caller: "call_two_hop_forwarded_named_field_wrapper",
                    callee: "call_two_hop_forwarded_named_field_middle",
                },
                Step {
                    caller: "call_two_hop_forwarded_named_field_middle",
                    callee: "call_two_hop_forwarded_named_field_leaf",
                },
            ],
            depth: 4,
        },
    ];

    for case in cases {
        // Source oracle: a private caller constructs
        // `CallbackHolder { callback: local_target }`, then each private helper
        // forwards its `holder` parameter to the next helper. The durable proof
        // chain is the per-callee `ArgumentSuppliesParameter` edge for each
        // forwarding callsite, not a new traversal relation.
        let leaf = function_id_by_name(&db, case.leaf)?;
        let leaf_context = db.call_context_for_owner(leaf)?;
        let dynamic = row_by_kind_path(
            &leaf_context,
            CallSiteKind::Dynamic,
            &["holder", "callback"],
        );
        assert_resolved_target(
            dynamic,
            target,
            CallRelationKind::DynamicFunction,
            CallSiteKind::Dynamic,
            CallTargetKind::Function,
        );

        for step in case.steps {
            assert_holder_argument_edge(&db, step.caller, step.callee, case.label, case.source)?;
        }

        let root = function_id_by_name(&db, case.steps[0].caller)?;
        let paths = db.call_paths_between(
            root,
            target,
            CallPathOptions {
                max_depth: case.depth,
                max_paths: 8,
            },
        )?;
        assert!(
            paths.iter().any(|path| path.start_id == root
                && path.end_id == target
                && path.depth == case.depth),
            "{} should preserve the existing traversal while exposing argument proof edges: {paths:#?}",
            case.label
        );
    }

    Ok(())
}

fn assert_holder_argument_edge(
    db: &ploke_db::Database,
    caller_name: &str,
    callee_name: &str,
    label: &str,
    source: &str,
) -> Result<(), DbError> {
    let caller = function_id_by_name(db, caller_name)?;
    let callee = function_id_by_name(db, callee_name)?;
    let caller_context = db.call_context_for_owner(caller)?;
    let call = row_by_path(&caller_context, &[callee_name]);
    assert_resolved_target(
        call,
        callee,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let bindings = db.local_bindings_for_owner(callee)?;
    let holder = bindings
        .iter()
        .find(|binding| binding.kind == "ParameterBinding" && binding.name == "holder")
        .unwrap_or_else(|| {
            panic!(
                "{label} should persist callee holder parameter binding for {callee_name}: {source}; bindings: {bindings:#?}"
            )
        });
    assert_eq!(holder.source_kind, "Parameter");

    let edges = db.local_binding_edges_for_owner(callee)?;
    assert!(
        edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::ArgumentSuppliesParameter
            && edge.source_id == call.site.id
            && edge.source_kind == "Path"
            && edge.target_id == holder.id
            && edge.target_kind == "LocalBinding"),
        "{label} missing forwarding argument edge {caller_name} -> {callee_name}: {source}; edges: {edges:#?}"
    );

    Ok(())
}
