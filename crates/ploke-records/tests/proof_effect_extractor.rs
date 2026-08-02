use ploke_records::proof_effects::{ProofExtractionConfig, extract_proof_facts_from_source};
use ploke_records::proof_facts::{
    BuildDomainId, BuildEvidenceSnapshot, BuildRustcInvocationCapture, EffectClass, EvidenceUse,
    ExpansionState, ObligationStatus, ProofBlockerReason, ProofFactRecord, ProofFactSet,
    TargetKind,
};

fn build_records(build_domain_id: &BuildDomainId) -> Vec<ProofFactRecord> {
    BuildEvidenceSnapshot {
        build_domain_id: build_domain_id.clone(),
        cargo_metadata_json: br#"{"packages":[{"name":"fixture"}]}"#.to_vec(),
        cargo_lock: b"version = 4\n".to_vec(),
        package_id: "path+file:///fixture#fixture@0.1.0".to_string(),
        target_kind: TargetKind::Library,
        target_name: "fixture".to_string(),
        target_root: "src/lib.rs".to_string(),
        target_triple: "x86_64-unknown-linux-gnu".to_string(),
        host_triple: "x86_64-unknown-linux-gnu".to_string(),
        profile: "check".to_string(),
        selected_features: vec!["default".to_string()],
        active_cfg_atoms: vec!["target_os=linux".to_string()],
        rustc_version: "rustc 1.96.0".to_string(),
        rustc_commit_hash: None,
        extractor_version: "proof-effect-test".to_string(),
        proof_policy_version: "detached-process-crown-v0".to_string(),
        immutable_surface_digest: None,
    }
    .proof_records(
        BuildRustcInvocationCapture {
            invocation_id: "rustc:fixture".to_string(),
            rustc_program: "rustc".to_string(),
            working_directory: "/fixture".to_string(),
            args: vec!["--crate-name".to_string(), "fixture".to_string()],
            environment: Vec::new(),
            response_file_contents: None,
            status: ObligationStatus::Admitted,
            blocking_reason: None,
        },
        Vec::new(),
    )
}

#[test]
fn extractor_blocks_command_spawn_until_lifetime_evidence_exists() {
    let build_domain_id = BuildDomainId("bd:fixture".to_string());
    let mut records = build_records(&build_domain_id);
    records.extend(
        extract_proof_facts_from_source(
            r#"
        pub fn launch() {
            let _child = std::process::Command::new("sleep").arg("1").spawn();
        }
        "#,
            ProofExtractionConfig::for_source(build_domain_id, "src/lib.rs"),
        )
        .expect("extract proof facts"),
    );

    let imported = ProofFactSet::from_records(records).expect("import proof facts");
    let report = imported.validate_for_proof();

    assert_eq!(report.status, ObligationStatus::Blocked);
    assert!(
        report
            .blockers
            .iter()
            .any(|blocker| blocker.reason == ProofBlockerReason::ProcessLifetimeEvidenceMissing)
    );
}

#[test]
fn extractor_separates_proof_critical_unknown_from_navigation_only_unknown() {
    let build_domain_id = BuildDomainId("bd:fixture".to_string());
    let config = ProofExtractionConfig::for_source(build_domain_id.clone(), "src/lib.rs")
        .with_proof_critical_unknowns(["external_process_escape"]);
    let facts = extract_proof_facts_from_source(
        r#"
        fn external_process_escape() {}
        pub fn caller() {
            helper_for_navigation();
            external_process_escape();
        }
        "#,
        config,
    )
    .expect("extract proof facts");

    assert!(facts.iter().any(|record| matches!(record, ProofFactRecord::CallSite(site) if site.evidence_use == EvidenceUse::NavigationOnly)));
    assert!(facts.iter().any(|record| matches!(record, ProofFactRecord::CallSite(site) if site.evidence_use.can_satisfy_proof())));

    let mut records = build_records(&build_domain_id);
    records.extend(facts);
    let report = ProofFactSet::from_records(records)
        .expect("import proof facts")
        .validate_for_proof();

    assert_eq!(report.status, ObligationStatus::Blocked);
    assert!(
        report
            .blockers
            .iter()
            .any(|blocker| blocker.reason == ProofBlockerReason::ExternalDependencySummaryMissing)
    );
}

#[test]
fn extractor_keeps_navigation_only_unknowns_out_of_proof_blockers() {
    let build_domain_id = BuildDomainId("bd:fixture".to_string());
    let mut records = build_records(&build_domain_id);
    records.extend(
        extract_proof_facts_from_source(
            r#"
            pub fn caller() {
                helper_for_navigation();
            }
            "#,
            ProofExtractionConfig::for_source(build_domain_id, "src/lib.rs"),
        )
        .expect("extract proof facts"),
    );

    let report = ProofFactSet::from_records(records)
        .expect("import proof facts")
        .validate_for_proof();

    assert_eq!(report.status, ObligationStatus::Admitted);
}

#[test]
fn extractor_records_async_durable_write_and_macro_boundaries() {
    let build_domain_id = BuildDomainId("bd:fixture".to_string());
    let facts = extract_proof_facts_from_source(
        r#"
        macro_rules! spawn_child { () => { std::process::Command::new("sleep").spawn() }; }
        pub fn caller() {
            tokio::spawn(async {});
            std::fs::write("evidence.txt", "ok");
            spawn_child!();
        }
        "#,
        ProofExtractionConfig::for_source(build_domain_id, "src/lib.rs"),
    )
    .expect("extract proof facts");

    assert!(facts.iter().any(|record| matches!(record, ProofFactRecord::EffectSeed(seed) if seed.effect_class == EffectClass::AsyncTaskSpawn)));
    assert!(facts.iter().any(|record| matches!(record, ProofFactRecord::EffectSeed(seed) if seed.effect_class == EffectClass::DurableEvidenceWrite)));
    assert!(facts.iter().any(|record| matches!(record, ProofFactRecord::ExpansionBoundary(boundary) if boundary.expansion_state == ExpansionState::Blocked && boundary.blocking_reason == Some(ProofBlockerReason::MacroExpansionNotAvailable))));
}

#[test]
fn extractor_blocks_variable_command_status_and_output() {
    let build_domain_id = BuildDomainId("bd:fixture".to_string());
    let mut records = build_records(&build_domain_id);
    records.extend(
        extract_proof_facts_from_source(
            r#"
            pub fn caller() {
                let mut cmd = std::process::Command::new("sleep");
                cmd.status();
                cmd.output();
            }
            "#,
            ProofExtractionConfig::for_source(build_domain_id, "src/lib.rs"),
        )
        .expect("extract proof facts"),
    );

    let report = ProofFactSet::from_records(records)
        .expect("import proof facts")
        .validate_for_proof();

    assert_eq!(report.status, ObligationStatus::Blocked);
    assert!(
        report
            .blockers
            .iter()
            .any(|blocker| blocker.reason == ProofBlockerReason::ProcessLifetimeEvidenceMissing)
    );
}

#[test]
fn extractor_classifies_tokio_shell_and_cleanup_effects() {
    let build_domain_id = BuildDomainId("bd:fixture".to_string());
    let facts = extract_proof_facts_from_source(
        r#"
        pub async fn caller() {
            let mut child = tokio::process::Command::new("sh").arg("-c").arg("sleep 1").spawn();
            child.wait().await;
            child.kill().await;
        }
        "#,
        ProofExtractionConfig::for_source(build_domain_id, "src/lib.rs"),
    )
    .expect("extract proof facts");

    assert!(facts.iter().any(|record| matches!(record, ProofFactRecord::EffectSeed(seed) if seed.effect_class == EffectClass::OperatingSystemProcessConfigure && seed.confidence == "tokio-command-configure")));
    assert!(facts.iter().any(|record| matches!(record, ProofFactRecord::EffectSeed(seed) if seed.effect_class == EffectClass::OperatingSystemProcessCreate && seed.confidence == "shell-command-spawn")));
    assert!(facts.iter().any(|record| matches!(record, ProofFactRecord::EffectSeed(seed) if seed.effect_class == EffectClass::OperatingSystemProcessWait)));
    assert!(facts.iter().any(|record| matches!(record, ProofFactRecord::EffectSeed(seed) if seed.effect_class == EffectClass::OperatingSystemProcessKill)));
}
