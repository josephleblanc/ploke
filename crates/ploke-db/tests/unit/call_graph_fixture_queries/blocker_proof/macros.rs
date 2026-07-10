use super::*;

struct MacroBlockerCase {
    module_path: &'static [&'static str],
    owner_name: &'static str,
    macro_name: &'static str,
}

#[test]
fn fixture_projection_marks_real_macro_call_without_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let cases = [
        MacroBlockerCase {
            module_path: &["crate"],
            owner_name: "call_crate_scoped_macro",
            macro_name: "crate::crate_scoped_macro",
        },
        MacroBlockerCase {
            module_path: &["crate"],
            owner_name: "call_vec_macro",
            macro_name: "vec",
        },
        MacroBlockerCase {
            module_path: &["crate"],
            owner_name: "call_imported_macro_alias",
            macro_name: "imported_macro_alias",
        },
        MacroBlockerCase {
            module_path: &["crate"],
            owner_name: "call_item_macro_inside_body",
            macro_name: "call_graph_item_macro",
        },
        MacroBlockerCase {
            module_path: &["crate", "call_graph_tests"],
            owner_name: "assert_eq_macro_call",
            macro_name: "assert_eq",
        },
    ];
    let mut expected = Vec::new();

    for case in cases {
        let owner = function_id_by_name_in_module(&db, case.module_path, case.owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(
            context.len(),
            1,
            "{} context rows: {context:#?}",
            case.owner_name
        );
        let row = assert_targetless_macro_row(
            &context,
            owner,
            TargetlessMacroCase::macro_call(case.macro_name, case.owner_name),
        );
        let site = row.site.id;
        let span = row.site.span;

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 2);
        expected.push(BlockerProofSite {
            site,
            span,
            blocker_reason: "macro_expansion_not_available",
        });
    }

    assert_targetless_blocker_proofs(&db, "macro", &expected, "fixture_call_graph/src/lib.rs")?;

    Ok(())
}
