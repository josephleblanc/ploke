use ploke_records::proof_facts::{
    AuthorityFact, AuthorityFactId, AuthorityTerm, BlockerFactId, BuildDomainFact, BuildDomainId,
    CallEdgeFact, CallEdgeId, CallResolutionFact, CallSiteFact, CallSiteId, DefinitionId,
    EffectClass, EffectSeedFact, EffectSeedId, EvidenceUse, ExpandedItemFact, ExpandedItemId,
    ExpansionBoundaryFact, ExpansionBoundaryId, ExpansionBoundaryKind, ExpansionState,
    ObligationStatus, PROOF_FACT_SCHEMA_VERSION, ProofBlockerFact, ProofBlockerReason,
    ProofFactRecord, ProofFactSet, ResolutionState, SourceSpanRecord, TargetKind,
};

fn span(file: &str) -> SourceSpanRecord {
    SourceSpanRecord {
        file: file.to_string(),
        start_byte: 10,
        end_byte: 20,
        line_start: Some(1),
        line_end: Some(1),
    }
}

fn build_domain(id: &str) -> BuildDomainFact {
    BuildDomainFact {
        schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
        build_domain_id: BuildDomainId(id.to_string()),
        cargo_metadata_hash: "sha256:metadata".to_string(),
        cargo_lock_hash: "sha256:lock".to_string(),
        package_id: "path+file:///ploke#ploke-eval@0.1.0".to_string(),
        target_kind: TargetKind::Library,
        target_name: "ploke-eval".to_string(),
        target_root: "crates/ploke-eval/src/lib.rs".to_string(),
        target_triple: "x86_64-unknown-linux-gnu".to_string(),
        host_triple: "x86_64-unknown-linux-gnu".to_string(),
        profile: "dev".to_string(),
        features_hash: "sha256:features".to_string(),
        active_cfg_hash: "sha256:cfg".to_string(),
        rustc_version: "rustc 1.96.0".to_string(),
        rustc_commit_hash: Some("ac68faa20".to_string()),
        extractor_version: "ploke-rustc-extractor 0.1.0".to_string(),
        proof_policy_version: "detached-process-proof.v1".to_string(),
        immutable_surface_digest: Some("sha256:surface".to_string()),
    }
}

fn call_site(id: &str, build_domain_id: &str) -> CallSiteFact {
    CallSiteFact {
        schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
        call_site_id: CallSiteId(id.to_string()),
        build_domain_id: BuildDomainId(build_domain_id.to_string()),
        caller_def_id: DefinitionId("def:caller".to_string()),
        source_span: span("crates/ploke-eval/src/lib.rs"),
        evidence_use: EvidenceUse::ProofOnly,
    }
}

fn expansion_boundary(id: &str, build_domain_id: &str) -> ExpansionBoundaryFact {
    ExpansionBoundaryFact {
        schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
        boundary_id: ExpansionBoundaryId(id.to_string()),
        build_domain_id: BuildDomainId(build_domain_id.to_string()),
        boundary_kind: ExpansionBoundaryKind::MacroRulesInvocation,
        source_span: span("crates/ploke-eval/src/lib.rs"),
        expansion_state: ExpansionState::Expanded,
        blocking_reason: None,
        macro_def_id: Some(DefinitionId("def:macro".to_string())),
        proc_macro_crate_id: None,
        build_script_package_id: None,
    }
}

#[test]
fn validator_blocks_unresolved_process_effect_with_typed_reason() {
    let records = vec![
        ProofFactRecord::BuildDomain(build_domain("bd:main")),
        ProofFactRecord::CallSite(call_site("call:spawn", "bd:main")),
        ProofFactRecord::CallResolution(CallResolutionFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            call_site_id: CallSiteId("call:spawn".to_string()),
            resolution_state: ResolutionState::Unresolved,
            resolved_def_id: None,
            candidate_def_ids: Vec::new(),
            external_summary_id: None,
            blocking_reason: Some(ProofBlockerReason::TypeResolutionMissing),
        }),
        ProofFactRecord::EffectSeed(EffectSeedFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            effect_seed_id: EffectSeedId("effect:spawn".to_string()),
            call_site_id: CallSiteId("call:spawn".to_string()),
            effect_class: EffectClass::OperatingSystemProcessCreate,
            confidence: "seed".to_string(),
            blocker_if_unresolved: true,
            evidence_use: EvidenceUse::ProofOnly,
        }),
    ];

    let imported = ProofFactSet::from_records(records).expect("import proof facts");
    let report = imported.validate_for_proof();

    assert_eq!(report.status, ObligationStatus::Blocked);
    assert!(report.blockers.iter().any(|blocker| {
        blocker.reason == ProofBlockerReason::TypeResolutionMissing
            && blocker
                .call_site_id
                .as_ref()
                .is_some_and(|id| id.as_str() == "call:spawn")
    }));
}

#[test]
fn validator_blocks_mismatched_call_site_identity() {
    let records = vec![
        ProofFactRecord::BuildDomain(build_domain("bd:main")),
        ProofFactRecord::CallSite(call_site("call:mismatch", "bd:missing")),
    ];

    let imported = ProofFactSet::from_records(records).expect("import proof facts");
    let report = imported.validate_for_proof();

    assert_eq!(report.status, ObligationStatus::Blocked);
    assert!(
        report
            .blockers
            .iter()
            .any(|blocker| blocker.reason == ProofBlockerReason::CanonicalIdentityMismatch)
    );
}

#[test]
fn validator_blocks_call_edge_with_missing_call_site() {
    let records = vec![
        ProofFactRecord::BuildDomain(build_domain("bd:main")),
        ProofFactRecord::CallEdge(CallEdgeFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            call_edge_id: CallEdgeId("edge:1".to_string()),
            call_site_id: CallSiteId("call:missing".to_string()),
            caller_def_id: DefinitionId("def:caller".to_string()),
            callee_def_id: Some(DefinitionId("def:callee".to_string())),
            resolution_state: ResolutionState::Resolved,
            evidence_use: EvidenceUse::ProofOnly,
        }),
    ];

    let imported = ProofFactSet::from_records(records).expect("import proof facts");
    let report = imported.validate_for_proof();

    assert_eq!(report.status, ObligationStatus::Blocked);
    assert!(report.blockers.iter().any(|blocker| {
        blocker.reason == ProofBlockerReason::CanonicalIdentityMismatch
            && blocker
                .call_site_id
                .as_ref()
                .is_some_and(|id| id.as_str() == "call:missing")
    }));
}

#[test]
fn jsonl_import_preserves_inert_authority_records() {
    let authority = ProofFactRecord::Authority(AuthorityFact {
        schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
        authority_fact_id: AuthorityFactId("authority:1".to_string()),
        build_domain_id: BuildDomainId("bd:main".to_string()),
        source_span: span("crates/ploke-eval/src/cli/prototype1_state/inner.rs"),
        authority_term: AuthorityTerm::CrownRuling,
        status: ObligationStatus::Admitted,
        evidence_use: EvidenceUse::ProofOnly,
    });

    let jsonl = serde_json::to_string(&authority).expect("serialize authority fact");
    let imported = ProofFactSet::from_jsonl(&jsonl).expect("import jsonl proof facts");
    let imported_authority = imported
        .authority_facts()
        .next()
        .expect("authority fact imported");

    assert_eq!(imported.records().len(), 1);
    assert_eq!(
        imported_authority.authority_term,
        AuthorityTerm::CrownRuling
    );
    assert!(!imported_authority.record_deserialization_grants_authority());
}

#[test]
fn expanded_item_records_keep_expansion_provenance_but_do_not_satisfy_missing_build_domain() {
    let records = vec![ProofFactRecord::ExpandedItem(ExpandedItemFact {
        schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
        expanded_item_id: ExpandedItemId("expanded:item:1".to_string()),
        boundary_id: ExpansionBoundaryId("boundary:macro:missing".to_string()),
        build_domain_id: BuildDomainId("bd:missing".to_string()),
        definition_id: DefinitionId("def:expanded".to_string()),
        source_span: span("crates/ploke-eval/src/lib.rs"),
        evidence_use: EvidenceUse::ProofOnly,
    })];

    let imported = ProofFactSet::from_records(records).expect("import proof facts");
    let report = imported.validate_for_proof();

    assert_eq!(report.status, ObligationStatus::Blocked);
    assert!(
        report
            .blockers
            .iter()
            .any(|blocker| blocker.reason == ProofBlockerReason::CanonicalIdentityMismatch)
    );
}

#[test]
fn validator_blocks_duplicate_call_site_id_conflicts() {
    let mut other = call_site("call:dup", "bd:main");
    other.caller_def_id = DefinitionId("def:other".to_string());
    let records = vec![
        ProofFactRecord::BuildDomain(build_domain("bd:main")),
        ProofFactRecord::CallSite(call_site("call:dup", "bd:main")),
        ProofFactRecord::CallSite(other),
    ];

    let imported = ProofFactSet::from_records(records).expect("import proof facts");
    let report = imported.validate_for_proof();

    assert_eq!(report.status, ObligationStatus::Blocked);
    assert!(
        report
            .blockers
            .iter()
            .any(|blocker| blocker.reason == ProofBlockerReason::CanonicalIdentityMismatch)
    );
}

#[test]
fn validator_rejects_authority_rejection() {
    let records = vec![
        ProofFactRecord::BuildDomain(build_domain("bd:main")),
        ProofFactRecord::Authority(AuthorityFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            authority_fact_id: AuthorityFactId("authority:rejected".to_string()),
            build_domain_id: BuildDomainId("bd:main".to_string()),
            source_span: span("crates/ploke-eval/src/cli/prototype1_state/inner.rs"),
            authority_term: AuthorityTerm::CrownRuling,
            status: ObligationStatus::Rejected,
            evidence_use: EvidenceUse::ProofOnly,
        }),
    ];

    let imported = ProofFactSet::from_records(records).expect("import proof facts");
    let report = imported.validate_for_proof();

    assert_eq!(report.status, ObligationStatus::Rejected);
    assert!(
        report
            .blockers
            .iter()
            .any(|blocker| blocker.reason == ProofBlockerReason::AuthorityEvidenceMissing)
    );
}

#[test]
fn validator_blocks_standalone_blocking_call_resolution() {
    let records = vec![
        ProofFactRecord::BuildDomain(build_domain("bd:main")),
        ProofFactRecord::CallSite(call_site("call:standalone", "bd:main")),
        ProofFactRecord::CallResolution(CallResolutionFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            call_site_id: CallSiteId("call:standalone".to_string()),
            resolution_state: ResolutionState::CandidateSet,
            resolved_def_id: None,
            candidate_def_ids: Vec::new(),
            external_summary_id: None,
            blocking_reason: Some(ProofBlockerReason::DynamicDispatchUnbounded),
        }),
    ];

    let imported = ProofFactSet::from_records(records).expect("import proof facts");
    let report = imported.validate_for_proof();

    assert_eq!(report.status, ObligationStatus::Blocked);
    assert!(
        report
            .blockers
            .iter()
            .any(|blocker| blocker.reason == ProofBlockerReason::DynamicDispatchUnbounded)
    );
}

#[test]
fn validator_blocks_cross_build_domain_expansion_boundary() {
    let records = vec![
        ProofFactRecord::BuildDomain(build_domain("bd:main")),
        ProofFactRecord::BuildDomain(build_domain("bd:other")),
        ProofFactRecord::ExpansionBoundary(expansion_boundary("boundary:macro:1", "bd:other")),
        ProofFactRecord::ExpandedItem(ExpandedItemFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            expanded_item_id: ExpandedItemId("expanded:item:cross".to_string()),
            boundary_id: ExpansionBoundaryId("boundary:macro:1".to_string()),
            build_domain_id: BuildDomainId("bd:main".to_string()),
            definition_id: DefinitionId("def:expanded".to_string()),
            source_span: span("crates/ploke-eval/src/lib.rs"),
            evidence_use: EvidenceUse::ProofOnly,
        }),
    ];

    let imported = ProofFactSet::from_records(records).expect("import proof facts");
    let report = imported.validate_for_proof();

    assert_eq!(report.status, ObligationStatus::Blocked);
    assert!(
        report
            .blockers
            .iter()
            .any(|blocker| blocker.reason == ProofBlockerReason::CanonicalIdentityMismatch)
    );
}

#[test]
fn validator_blocks_call_edge_payload_mismatches() {
    let mut site = call_site("call:edge-mismatch", "bd:main");
    site.caller_def_id = DefinitionId("def:caller-a".to_string());
    let records = vec![
        ProofFactRecord::BuildDomain(build_domain("bd:main")),
        ProofFactRecord::CallSite(site),
        ProofFactRecord::CallEdge(CallEdgeFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            call_edge_id: CallEdgeId("edge:mismatch".to_string()),
            call_site_id: CallSiteId("call:edge-mismatch".to_string()),
            caller_def_id: DefinitionId("def:caller-b".to_string()),
            callee_def_id: None,
            resolution_state: ResolutionState::Resolved,
            evidence_use: EvidenceUse::ProofOnly,
        }),
    ];

    let imported = ProofFactSet::from_records(records).expect("import proof facts");
    let report = imported.validate_for_proof();

    assert_eq!(report.status, ObligationStatus::Blocked);
    assert!(
        report
            .blockers
            .iter()
            .any(|blocker| blocker.reason == ProofBlockerReason::CanonicalIdentityMismatch)
    );
    assert!(
        report
            .blockers
            .iter()
            .any(|blocker| blocker.reason == ProofBlockerReason::TypeResolutionMissing)
    );
}

#[test]
fn validator_emits_distinct_blocker_ids_for_distinct_call_sites() {
    let records = vec![
        ProofFactRecord::CallEdge(CallEdgeFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            call_edge_id: CallEdgeId("edge:missing-a".to_string()),
            call_site_id: CallSiteId("call:missing-a".to_string()),
            caller_def_id: DefinitionId("def:caller".to_string()),
            callee_def_id: Some(DefinitionId("def:callee".to_string())),
            resolution_state: ResolutionState::Resolved,
            evidence_use: EvidenceUse::ProofOnly,
        }),
        ProofFactRecord::CallEdge(CallEdgeFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            call_edge_id: CallEdgeId("edge:missing-b".to_string()),
            call_site_id: CallSiteId("call:missing-b".to_string()),
            caller_def_id: DefinitionId("def:caller".to_string()),
            callee_def_id: Some(DefinitionId("def:callee".to_string())),
            resolution_state: ResolutionState::Resolved,
            evidence_use: EvidenceUse::ProofOnly,
        }),
    ];

    let imported = ProofFactSet::from_records(records).expect("import proof facts");
    let report = imported.validate_for_proof();
    let ids = report
        .blockers
        .iter()
        .map(|blocker| blocker.blocker_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(report.status, ObligationStatus::Blocked);
    assert!(report.blockers.len() >= 2);
    assert_eq!(ids.len(), report.blockers.len());
}

#[test]
fn validator_blocks_external_summary_resolution_without_summary_id() {
    let records = vec![
        ProofFactRecord::BuildDomain(build_domain("bd:main")),
        ProofFactRecord::CallSite(call_site("call:external", "bd:main")),
        ProofFactRecord::CallResolution(CallResolutionFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            call_site_id: CallSiteId("call:external".to_string()),
            resolution_state: ResolutionState::ExternallySummarized,
            resolved_def_id: None,
            candidate_def_ids: Vec::new(),
            external_summary_id: None,
            blocking_reason: None,
        }),
    ];

    let imported = ProofFactSet::from_records(records).expect("import proof facts");
    let report = imported.validate_for_proof();

    assert_eq!(report.status, ObligationStatus::Blocked);
    assert!(
        report
            .blockers
            .iter()
            .any(|blocker| blocker.reason == ProofBlockerReason::ExternalDependencySummaryMissing)
    );
}

#[test]
fn validator_blocks_duplicate_effect_seed_id_conflicts() {
    let mut other = EffectSeedFact {
        schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
        effect_seed_id: EffectSeedId("effect:dup".to_string()),
        call_site_id: CallSiteId("call:effect".to_string()),
        effect_class: EffectClass::OperatingSystemProcessCreate,
        confidence: "seed".to_string(),
        blocker_if_unresolved: false,
        evidence_use: EvidenceUse::ProofOnly,
    };
    other.effect_class = EffectClass::DurableEvidenceWrite;

    let records = vec![
        ProofFactRecord::BuildDomain(build_domain("bd:main")),
        ProofFactRecord::CallSite(call_site("call:effect", "bd:main")),
        ProofFactRecord::EffectSeed(EffectSeedFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            effect_seed_id: EffectSeedId("effect:dup".to_string()),
            call_site_id: CallSiteId("call:effect".to_string()),
            effect_class: EffectClass::OperatingSystemProcessCreate,
            confidence: "seed".to_string(),
            blocker_if_unresolved: false,
            evidence_use: EvidenceUse::ProofOnly,
        }),
        ProofFactRecord::EffectSeed(other),
    ];

    let imported = ProofFactSet::from_records(records).expect("import proof facts");
    let report = imported.validate_for_proof();

    assert_eq!(report.status, ObligationStatus::Blocked);
    assert!(
        report
            .blockers
            .iter()
            .any(|blocker| blocker.reason == ProofBlockerReason::CanonicalIdentityMismatch)
    );
}

#[test]
fn validator_emits_distinct_blocker_ids_for_same_build_domain_expanded_items() {
    let records = vec![
        ProofFactRecord::BuildDomain(build_domain("bd:main")),
        ProofFactRecord::ExpandedItem(ExpandedItemFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            expanded_item_id: ExpandedItemId("expanded:item:a".to_string()),
            boundary_id: ExpansionBoundaryId("boundary:missing:a".to_string()),
            build_domain_id: BuildDomainId("bd:main".to_string()),
            definition_id: DefinitionId("def:expanded:a".to_string()),
            source_span: span("crates/ploke-eval/src/lib.rs"),
            evidence_use: EvidenceUse::ProofOnly,
        }),
        ProofFactRecord::ExpandedItem(ExpandedItemFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            expanded_item_id: ExpandedItemId("expanded:item:b".to_string()),
            boundary_id: ExpansionBoundaryId("boundary:missing:b".to_string()),
            build_domain_id: BuildDomainId("bd:main".to_string()),
            definition_id: DefinitionId("def:expanded:b".to_string()),
            source_span: span("crates/ploke-eval/src/lib.rs"),
            evidence_use: EvidenceUse::ProofOnly,
        }),
    ];

    let imported = ProofFactSet::from_records(records).expect("import proof facts");
    let report = imported.validate_for_proof();
    let ids = report
        .blockers
        .iter()
        .map(|blocker| blocker.blocker_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(report.status, ObligationStatus::Blocked);
    assert_eq!(report.blockers.len(), 2);
    assert_eq!(ids.len(), report.blockers.len());
}

#[test]
fn validator_avoids_blocker_id_suffix_collision_with_imported_blocker_ids() {
    let base = "blocker:CanonicalIdentityMismatch:bd:bd:main:call:<none>";
    let records = vec![
        ProofFactRecord::BuildDomain(build_domain("bd:main")),
        ProofFactRecord::ExpandedItem(ExpandedItemFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            expanded_item_id: ExpandedItemId("expanded:item:a".to_string()),
            boundary_id: ExpansionBoundaryId("boundary:missing:a".to_string()),
            build_domain_id: BuildDomainId("bd:main".to_string()),
            definition_id: DefinitionId("def:expanded:a".to_string()),
            source_span: span("crates/ploke-eval/src/lib.rs"),
            evidence_use: EvidenceUse::ProofOnly,
        }),
        ProofFactRecord::ExpandedItem(ExpandedItemFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            expanded_item_id: ExpandedItemId("expanded:item:b".to_string()),
            boundary_id: ExpansionBoundaryId("boundary:missing:b".to_string()),
            build_domain_id: BuildDomainId("bd:main".to_string()),
            definition_id: DefinitionId("def:expanded:b".to_string()),
            source_span: span("crates/ploke-eval/src/lib.rs"),
            evidence_use: EvidenceUse::ProofOnly,
        }),
        ProofFactRecord::ProofBlocker(ProofBlockerFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            blocker_id: BlockerFactId(format!("{base}:duplicate:1")),
            reason: ProofBlockerReason::CanonicalIdentityMismatch,
            status: ObligationStatus::Blocked,
            build_domain_id: Some(BuildDomainId("bd:main".to_string())),
            call_site_id: None,
            detail: "imported blocker id must not collide with generated suffix".to_string(),
        }),
    ];

    let imported = ProofFactSet::from_records(records).expect("import proof facts");
    let report = imported.validate_for_proof();
    let ids = report
        .blockers
        .iter()
        .map(|blocker| blocker.blocker_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(report.status, ObligationStatus::Blocked);
    assert_eq!(report.blockers.len(), 3);
    assert_eq!(ids.len(), report.blockers.len());
}

#[test]
fn validator_does_not_let_navigation_only_call_site_satisfy_proof_edge() {
    let mut site = call_site("call:navigation-only", "bd:main");
    site.evidence_use = EvidenceUse::NavigationOnly;
    let records = vec![
        ProofFactRecord::BuildDomain(build_domain("bd:main")),
        ProofFactRecord::CallSite(site),
        ProofFactRecord::CallEdge(CallEdgeFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            call_edge_id: CallEdgeId("edge:proof".to_string()),
            call_site_id: CallSiteId("call:navigation-only".to_string()),
            caller_def_id: DefinitionId("def:caller".to_string()),
            callee_def_id: Some(DefinitionId("def:callee".to_string())),
            resolution_state: ResolutionState::Resolved,
            evidence_use: EvidenceUse::ProofOnly,
        }),
    ];

    let imported = ProofFactSet::from_records(records).expect("import proof facts");
    let report = imported.validate_for_proof();

    assert_eq!(report.status, ObligationStatus::Blocked);
    assert!(
        report
            .blockers
            .iter()
            .any(|blocker| blocker.reason == ProofBlockerReason::CanonicalIdentityMismatch)
    );
}

#[test]
fn validator_does_not_let_navigation_only_call_site_satisfy_resolution_or_effect() {
    let mut site = call_site("call:navigation-only", "bd:main");
    site.evidence_use = EvidenceUse::NavigationOnly;
    let records = vec![
        ProofFactRecord::BuildDomain(build_domain("bd:main")),
        ProofFactRecord::CallSite(site),
        ProofFactRecord::CallResolution(CallResolutionFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            call_site_id: CallSiteId("call:navigation-only".to_string()),
            resolution_state: ResolutionState::Resolved,
            resolved_def_id: Some(DefinitionId("def:callee".to_string())),
            candidate_def_ids: Vec::new(),
            external_summary_id: None,
            blocking_reason: None,
        }),
        ProofFactRecord::EffectSeed(EffectSeedFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            effect_seed_id: EffectSeedId("effect:proof".to_string()),
            call_site_id: CallSiteId("call:navigation-only".to_string()),
            effect_class: EffectClass::OperatingSystemProcessCreate,
            confidence: "seed".to_string(),
            blocker_if_unresolved: false,
            evidence_use: EvidenceUse::ProofOnly,
        }),
    ];

    let imported = ProofFactSet::from_records(records).expect("import proof facts");
    let report = imported.validate_for_proof();

    assert_eq!(report.status, ObligationStatus::Blocked);
    assert!(
        report
            .blockers
            .iter()
            .filter(|blocker| blocker.reason == ProofBlockerReason::CanonicalIdentityMismatch)
            .count()
            >= 2
    );
}

#[test]
fn validator_ignores_navigation_only_expanded_item_missing_boundary_for_proof() {
    let records = vec![ProofFactRecord::ExpandedItem(ExpandedItemFact {
        schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
        expanded_item_id: ExpandedItemId("expanded:navigation:item".to_string()),
        boundary_id: ExpansionBoundaryId("boundary:navigation:missing".to_string()),
        build_domain_id: BuildDomainId("bd:navigation:missing".to_string()),
        definition_id: DefinitionId("def:navigation".to_string()),
        source_span: span("crates/ploke-eval/src/lib.rs"),
        evidence_use: EvidenceUse::NavigationOnly,
    })];

    let imported = ProofFactSet::from_records(records).expect("import proof facts");
    let report = imported.validate_for_proof();

    assert_eq!(report.status, ObligationStatus::Admitted);
    assert!(report.blockers.is_empty());
}

#[test]
fn validator_blocks_resolved_process_create_without_lifetime_or_handoff_evidence() {
    let records = vec![
        ProofFactRecord::BuildDomain(build_domain("bd:main")),
        ProofFactRecord::CallSite(call_site("call:process-create", "bd:main")),
        ProofFactRecord::CallResolution(CallResolutionFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            call_site_id: CallSiteId("call:process-create".to_string()),
            resolution_state: ResolutionState::Resolved,
            resolved_def_id: Some(DefinitionId("def:std-process-command-spawn".to_string())),
            candidate_def_ids: Vec::new(),
            external_summary_id: None,
            blocking_reason: None,
        }),
        ProofFactRecord::EffectSeed(EffectSeedFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            effect_seed_id: EffectSeedId("effect:process-create".to_string()),
            call_site_id: CallSiteId("call:process-create".to_string()),
            effect_class: EffectClass::OperatingSystemProcessCreate,
            confidence: "seed".to_string(),
            blocker_if_unresolved: true,
            evidence_use: EvidenceUse::ProofOnly,
        }),
    ];

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
