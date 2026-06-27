use super::super::super::super::super::*;
use super::super::super::helpers::*;

struct Case {
    label: &'static str,
    fixture: &'static str,
    owner_relation: &'static str,
    owner: &'static str,
    target_module: &'static [&'static str],
    target: &'static str,
    path: &'static [&'static str],
}

struct ResolvedCase {
    owner: Uuid,
    target: Uuid,
}

#[tokio::test]
async fn call_context_collection_reads_real_initializer_owner_rows() -> Result<(), Error> {
    init_tracing_once();
    let cases = initializer_cases();

    for case in cases {
        let db = Arc::new(Database::new(setup_db_full_multi_embedding(case.fixture)?));
        let resolved = resolve_case(&db, &case)?;
        let rag = init_test_rag_mock(Arc::clone(&db));
        assert!(
            !rag.call_context_degraded(),
            "fresh {} call_graph schema should enable {} call context",
            case.fixture,
            case.label
        );

        let call_context = rag.collect_call_context(&[(resolved.owner, 1.0)])?;
        let owner_context = call_context
            .get(&resolved.owner)
            .unwrap_or_else(|| panic!("{} owner should receive outgoing call context", case.label));
        assert_eq!(
            owner_context.len(),
            1,
            "{} owner context: {owner_context:#?}",
            case.label
        );
        let call = &owner_context[0];
        assert_eq!(call.kind, CallSiteKind::Path);
        assert_eq!(
            call.callee,
            CallCalleeInfo::Path {
                path: path(case.path),
            }
        );
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, resolved.target);
        assert_eq!(call.targets[0].relation, CallTargetKind::Function);
    }

    Ok(())
}

fn initializer_cases() -> [Case; 4] {
    [
        Case {
            label: "const initializer",
            fixture: "fixture_nodes",
            owner_relation: "const",
            owner: "FN_CALL_CONST",
            target_module: &["crate", "const_static"],
            target: "five",
            path: &["five"],
        },
        Case {
            label: "static initializer",
            fixture: "fixture_nodes",
            owner_relation: "static",
            owner: "STATIC_FN_CALL",
            target_module: &["crate", "const_static"],
            target: "five",
            path: &["five"],
        },
        Case {
            label: "impl associated const initializer",
            fixture: "fixture_call_graph",
            owner_relation: "const",
            owner: "IMPL_ASSOC_VALUE",
            target_module: &["crate"],
            target: "assoc_const_value",
            path: &["assoc_const_value"],
        },
        Case {
            label: "trait associated const initializer",
            fixture: "fixture_call_graph",
            owner_relation: "const",
            owner: "TRAIT_ASSOC_VALUE",
            target_module: &["crate"],
            target: "assoc_const_value",
            path: &["assoc_const_value"],
        },
    ]
}

fn resolve_case(db: &Database, case: &Case) -> Result<ResolvedCase, Error> {
    Ok(ResolvedCase {
        owner: one_uuid(db, &item_by_name_query(case.owner_relation, case.owner))?,
        target: one_uuid(
            db,
            &function_in_module_query(case.target_module, case.target),
        )?,
    })
}

fn item_by_name_query(relation: &str, name: &str) -> String {
    format!(r#"?[id] := *{relation} {{ id, name: "{name}" @ 'NOW' }}"#)
}

fn path(segments: &[&str]) -> Vec<String> {
    segments
        .iter()
        .map(|segment| (*segment).to_string())
        .collect()
}
