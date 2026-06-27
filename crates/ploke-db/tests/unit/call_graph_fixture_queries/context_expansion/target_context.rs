use super::*;

#[derive(Clone, Copy)]
struct TargetContextCase {
    label: &'static str,
    target: Uuid,
    min_rows: usize,
}

impl TargetContextCase {
    fn new(label: &'static str, target: Uuid, min_rows: usize) -> Self {
        Self {
            label,
            target,
            min_rows,
        }
    }
}

#[test]
fn fixture_call_context_for_target_preserves_full_rows_by_target_family() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let cases = [
        TargetContextCase::new("function", function_id_by_name(&db, "try_local_assoc")?, 1),
        TargetContextCase::new(
            "method",
            method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?,
            4,
        ),
        TargetContextCase::new(
            "associated function",
            method_id_by_impl_self_type_name(&db, "LocalAssoc", "make")?,
            3,
        ),
    ];

    for case in cases {
        assert_target_context_matches_callers(&db, case)?;
    }

    for constructor in constructor_cases() {
        let db = setup_call_graph_fixture_db(constructor.fixture)?;
        let resolved = assert_constructor_context(&db, constructor)?;
        assert_target_context_matches_callers(
            &db,
            TargetContextCase::new(constructor.label, resolved.target, 1),
        )?;
    }

    Ok(())
}

fn assert_target_context_matches_callers(
    db: &Database,
    case: TargetContextCase,
) -> Result<(), DbError> {
    let callers = db.callers_for_target(case.target)?;
    let context = db.call_context_for_target(case.target)?;
    let sites = db.call_sites_for_target(case.target)?;
    assert!(
        context.len() >= case.min_rows,
        "{} should expose at least {} target-centered context rows: {context:#?}",
        case.label,
        case.min_rows
    );
    assert_eq!(
        context.len(),
        callers.len(),
        "{} target context should preserve the same caller cardinality as callers_for_target",
        case.label
    );
    assert_eq!(
        sites.len(),
        callers.len(),
        "{} target sites should preserve the same caller cardinality as callers_for_target",
        case.label
    );

    for caller in &callers {
        assert!(
            sites.iter().any(|site| site == &caller.site),
            "{} target sites should include caller site {}: {sites:#?}",
            case.label,
            caller.site.id
        );
        let row = context
            .iter()
            .find(|row| row.site.id == caller.site.id)
            .unwrap_or_else(|| {
                panic!(
                    "{} target context missing caller site {}: {context:#?}",
                    case.label, caller.site.id
                )
            });
        assert_eq!(row.site, caller.site, "{} caller site row", case.label);
        assert_eq!(
            row.status, caller.status,
            "{} caller status row",
            case.label
        );
        assert!(
            row.targets.iter().any(|target| target == &caller.target),
            "{} full target context should include the matched caller target {:?}: {row:#?}",
            case.label,
            caller.target
        );

        if row.status.status == CallStatusKind::Resolved {
            assert_eq!(
                row.targets.len(),
                1,
                "{} resolved target context rows should carry exactly one local target",
                case.label
            );
            assert_eq!(
                row.targets[0].target_id, case.target,
                "{} resolved target row should point back to the seed target",
                case.label
            );
        }
    }

    Ok(())
}
