use ploke_selection_score::papers::deepcode;

#[test]
fn blueprint_fixture_reports_required_sections_and_file_oracle() {
    let report = deepcode::blueprint_fixture_report();

    assert_eq!(report.required_coverage, 1.0);
    assert_eq!(report.files, vec!["src/lib.rs", "tests/greet.rs"]);
    assert_eq!(report.file_precision, 1.0);
    assert_eq!(report.file_recall, 1.0);
    assert!(report.validation_present);
    assert!(report.fallback_rejected);
    assert_eq!(report.authority_violations, 0);
}

#[test]
fn codemem_fixture_reports_ledger_progress_and_interface_consistency() {
    let report = deepcode::codemem_fixture_report();

    assert_eq!(
        report.implemented_files,
        vec!["src/lib.rs", "src/parser.rs"]
    );
    assert_eq!(report.unimplemented_files, vec!["tests/parser.rs"]);
    assert_eq!(report.summary_coverage, 1.0);
    assert_eq!(report.file_delta, 0);
    assert!(report.unimplemented_exact);
    assert_eq!(report.interface_errors, 0);
    assert_eq!(report.stale_claims, 0);
    assert!(!report.next_step_persisted);
}

#[test]
fn coderag_fixture_reports_hand_labeled_tuple_metrics_only() {
    let report = deepcode::coderag_fixture_report();

    assert_eq!(report.tuple_precision, 1.0);
    assert_eq!(report.tuple_recall, 1.0);
    assert_eq!(report.forbidden_count, 0);
    assert_eq!(report.unknown_type_count, 0);
    assert!(!report.confidence_authority_violation);
}

#[test]
fn boundary_fixture_reports_stagnation_and_admission_boundaries() {
    let report = deepcode::boundary_fixture_report();

    assert!(report.loop_warning);
    assert_eq!(report.unique_completed, 1);
    assert_eq!(report.total_planned, 2);
    assert_eq!(report.verification_failures, 1);
    assert_eq!(report.proposals_without_admission, 1);
    assert_eq!(report.admission_violations, 0);
    assert_eq!(report.selector_violations, 0);
    assert!(report.report_only);
}
