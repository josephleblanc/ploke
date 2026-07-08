//! DB assertions for the shared real-target call-shape matrix.

use ploke_test_utils::{
    CallExpected, CallOwnerSelector, CallShapeCase, CallSiteSelector, CallTargetSelector,
    call_shape_cases,
};
use uuid::Uuid;

use super::super::*;
use super::common::*;

#[test]
fn shared_call_shape_matrix_cases_match_registered_backups() -> Result<(), DbError> {
    for case in call_shape_cases() {
        eprintln!("shared call-shape case: {}", case.name);
        let db = setup_call_graph_db(case.fixture.fixture())?;
        assert_case(&db, case)?;
    }

    Ok(())
}

fn assert_case(db: &Database, case: &CallShapeCase) -> Result<(), DbError> {
    let owner = resolve_owner(db, case.owner)?;
    let context = db.call_context_for_owner(owner)?;
    let row = select_site(&context, case)?;

    match case.expected {
        CallExpected::Resolved {
            target,
            relation,
            target_kind,
            edge_count,
        } => {
            let target = resolve_target(db, target)?;
            assert_resolved_target(row, target, relation, row.site.kind, target_kind);
            assert_eq!(
                relations_for_site(db, row.site.id)?.rows.len(),
                edge_count,
                "{} should preserve exactly {edge_count} raw edge(s); source: {}",
                case.name,
                case.source
            );
            assert_one_edge_traversal(
                db,
                TraversalExpectation {
                    label: case.name,
                    owner,
                    target,
                    site_id: row.site.id,
                    expected_edge_count: edge_count,
                },
            )?;
        }
        CallExpected::Targetless { status } => {
            assert_targetless_status(row, status);
            assert!(
                relations_for_site(db, row.site.id)?.rows.is_empty(),
                "{} should remain targetless with zero raw edges; source: {}",
                case.name,
                case.source
            );
            assert_no_traversal_candidates_for_site(db, owner, row.site.id, case.name)?;
        }
    }

    Ok(())
}

fn resolve_owner(db: &Database, owner: CallOwnerSelector) -> Result<Uuid, DbError> {
    match owner {
        CallOwnerSelector::FunctionInModule { module_path, name } => {
            function_id_by_name_in_module(db, module_path, name)
        }
        CallOwnerSelector::MethodByBody { name, body } => {
            method_id_by_name_and_body_substring(db, name, body)
        }
        CallOwnerSelector::MethodByBodyFile {
            name,
            body,
            file_suffix,
        } => method_id_by_name_body_and_file_suffix(db, name, body, file_suffix),
    }
}

fn resolve_target(db: &Database, target: CallTargetSelector) -> Result<Uuid, DbError> {
    match target {
        CallTargetSelector::FunctionInModule { module_path, name } => {
            function_id_by_name_in_module(db, module_path, name)
        }
        CallTargetSelector::Variant {
            enum_name,
            variant_name,
        } => variant_id_by_enum_and_variant_names(db, enum_name, variant_name),
    }
}

fn select_site<'a>(
    context: &'a [ploke_db::CallContextRow],
    case: &CallShapeCase,
) -> Result<&'a ploke_db::CallContextRow, DbError> {
    let row = match case.site {
        CallSiteSelector::Path {
            segments,
            arg_count,
        } => {
            let row = row_by_path(context, segments);
            assert_eq!(
                row.site.arg_count, arg_count,
                "{} should preserve path argument count; source: {}",
                case.name, case.source
            );
            row
        }
        CallSiteSelector::Dynamic { arg_count } => {
            let matches = context
                .iter()
                .filter(|row| row.site.kind == CallSiteKind::Dynamic)
                .collect::<Vec<_>>();
            assert_eq!(
                matches.len(),
                1,
                "{} should expose exactly one dynamic row; source: {}; context: {context:#?}",
                case.name,
                case.source
            );
            let row = matches[0];
            assert_eq!(
                row.site.arg_count, arg_count,
                "{} should preserve dynamic argument count; source: {}",
                case.name, case.source
            );
            row
        }
    };

    Ok(row)
}
