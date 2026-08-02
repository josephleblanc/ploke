use ploke_records::proof_facts::{
    BuildDomainId, BuildEvidenceSnapshot, BuildRustcInvocationCapture, ExpansionBoundaryCapture,
    ExpansionBoundaryId, ExpansionBoundaryKind, ExpansionState, ObligationStatus,
    PROOF_FACT_SCHEMA_VERSION, ProofBlockerReason, ProofFactRecord, ProofFactSet, SourceSpanRecord,
    TargetKind,
};

fn span(file: &str) -> SourceSpanRecord {
    SourceSpanRecord {
        file: file.to_string(),
        start_byte: 10,
        end_byte: 20,
        line_start: Some(2),
        line_end: Some(2),
    }
}

fn snapshot() -> BuildEvidenceSnapshot {
    BuildEvidenceSnapshot {
        build_domain_id: BuildDomainId("bd:ploke-eval:lib".to_string()),
        cargo_metadata_json: br#"{"packages":[{"name":"ploke-eval"}]}"#.to_vec(),
        cargo_lock: b"version = 4\n[[package]]\nname = \"ploke-eval\"\n".to_vec(),
        package_id: "path+file:///workspace/ploke#ploke-eval@0.1.0".to_string(),
        target_kind: TargetKind::Library,
        target_name: "ploke-eval".to_string(),
        target_root: "crates/ploke-eval/src/lib.rs".to_string(),
        target_triple: "x86_64-unknown-linux-gnu".to_string(),
        host_triple: "x86_64-unknown-linux-gnu".to_string(),
        profile: "check".to_string(),
        selected_features: vec!["default".to_string(), "protocol".to_string()],
        active_cfg_atoms: vec![
            "debug_assertions".to_string(),
            "target_os=linux".to_string(),
        ],
        rustc_version: "rustc 1.96.0".to_string(),
        rustc_commit_hash: None,
        extractor_version: "ploke-proof-capture.test".to_string(),
        proof_policy_version: "detached-process-crown-v0".to_string(),
        immutable_surface_digest: Some("sha256:surface".to_string()),
    }
}

#[test]
fn build_evidence_snapshot_emits_admitted_records_consumed_by_validator() {
    let snapshot = snapshot();
    let records = snapshot.proof_records(
        BuildRustcInvocationCapture {
            invocation_id: "rustc:ploke-eval:lib".into(),
            rustc_program: "rustc".to_string(),
            working_directory: "/workspace/ploke".to_string(),
            args: vec!["--crate-name".to_string(), "ploke_eval".to_string()],
            environment: vec![
                ("CARGO_PKG_NAME".to_string(), "ploke-eval".to_string()),
                ("RUSTC_BOOTSTRAP".to_string(), "".to_string()),
            ],
            response_file_contents: None,
            status: ObligationStatus::Admitted,
            blocking_reason: None,
        },
        vec![ExpansionBoundaryCapture {
            boundary_id: ExpansionBoundaryId("boundary:macro:1".to_string()),
            boundary_kind: ExpansionBoundaryKind::MacroRulesInvocation,
            source_span: span("crates/ploke-eval/src/lib.rs"),
            expansion_state: ExpansionState::Expanded,
            blocking_reason: None,
            macro_def_id: None,
            proc_macro_crate_id: None,
            build_script_package_id: None,
        }],
    );

    let imported = ProofFactSet::from_records(records).expect("import build evidence records");
    let report = imported.validate_for_proof();

    assert_eq!(report.status, ObligationStatus::Admitted);
    assert!(report.blockers.is_empty());
}

#[test]
fn build_evidence_snapshot_blocks_missing_cfg_domain_instead_of_generic_unresolved() {
    let snapshot = snapshot();
    let mut records = snapshot.proof_records(
        BuildRustcInvocationCapture {
            invocation_id: "rustc:ploke-eval:lib".into(),
            rustc_program: "rustc".to_string(),
            working_directory: "/workspace/ploke".to_string(),
            args: vec!["--crate-name".to_string(), "ploke_eval".to_string()],
            environment: Vec::new(),
            response_file_contents: None,
            status: ObligationStatus::Admitted,
            blocking_reason: None,
        },
        Vec::new(),
    );
    records.retain(|record| !matches!(record, ProofFactRecord::CfgDomain(_)));

    let imported = ProofFactSet::from_records(records).expect("import build evidence records");
    let report = imported.validate_for_proof();

    assert_eq!(report.status, ObligationStatus::Blocked);
    assert!(
        report
            .blockers
            .iter()
            .any(|blocker| blocker.reason == ProofBlockerReason::CfgDomainNotMaterialized)
    );
}

#[test]
fn build_script_boundary_missing_summary_is_typed_proof_blocker() {
    let snapshot = snapshot();
    let records = snapshot.proof_records(
        BuildRustcInvocationCapture {
            invocation_id: "rustc:ploke-eval:lib".into(),
            rustc_program: "rustc".to_string(),
            working_directory: "/workspace/ploke".to_string(),
            args: vec!["--crate-name".to_string(), "ploke_eval".to_string()],
            environment: Vec::new(),
            response_file_contents: None,
            status: ObligationStatus::Admitted,
            blocking_reason: None,
        },
        vec![ExpansionBoundaryCapture {
            boundary_id: ExpansionBoundaryId("boundary:build-script:dep".to_string()),
            boundary_kind: ExpansionBoundaryKind::BuildScript,
            source_span: span("Cargo.toml"),
            expansion_state: ExpansionState::Blocked,
            blocking_reason: Some(ProofBlockerReason::BuildScriptSummaryMissing),
            macro_def_id: None,
            proc_macro_crate_id: None,
            build_script_package_id: Some("dep build script".to_string()),
        }],
    );

    let imported = ProofFactSet::from_records(records).expect("import build evidence records");
    let report = imported.validate_for_proof();

    assert_eq!(report.status, ObligationStatus::Blocked);
    assert!(
        report
            .blockers
            .iter()
            .any(|blocker| blocker.reason == ProofBlockerReason::BuildScriptSummaryMissing)
    );
}

#[test]
fn rustc_invocation_hashes_arguments_without_storing_raw_expanded_source() {
    let snapshot = snapshot();
    let records = snapshot.proof_records(
        BuildRustcInvocationCapture {
            invocation_id: "rustc:ploke-eval:lib".into(),
            rustc_program: "rustc".to_string(),
            working_directory: "/workspace/ploke".to_string(),
            args: vec!["--crate-name".to_string(), "ploke_eval".to_string()],
            environment: Vec::new(),
            response_file_contents: Some(b"--cfg\nfeature=\"protocol\"".to_vec()),
            status: ObligationStatus::Admitted,
            blocking_reason: None,
        },
        Vec::new(),
    );

    let invocation = records
        .iter()
        .find_map(|record| match record {
            ProofFactRecord::RustcInvocation(fact) => Some(fact),
            _ => None,
        })
        .expect("rustc invocation fact");

    assert!(invocation.argument_vector_hash.starts_with("sha256:"));
    assert!(invocation.environment_hash.starts_with("sha256:"));
    assert!(
        invocation
            .response_file_hash
            .as_deref()
            .is_some_and(|hash| hash.starts_with("sha256:"))
    );
    assert_eq!(invocation.schema_version, PROOF_FACT_SCHEMA_VERSION);
}
