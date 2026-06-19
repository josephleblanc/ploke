use ploke_records::proof_facts::{
    AuthorityTerm, BuildDomainFact, BuildDomainId, CallResolutionFact, CallSiteId, EffectClass,
    EffectSeedFact, EffectSeedId, EvidenceUse, ExpansionBoundaryFact, ExpansionBoundaryId,
    ExpansionBoundaryKind, ExpansionState, HandoffEvidence, HandoffRole, ObligationStatus,
    PROOF_FACT_SCHEMA_VERSION, ProcessLifetime, ProofBlockerReason, ProofFactRecord,
    ResolutionState, SourceSpanRecord, TargetKind,
};

fn span() -> SourceSpanRecord {
    SourceSpanRecord {
        file: "crates/ploke-eval/src/lib.rs".to_string(),
        start_byte: 10,
        end_byte: 42,
        line_start: Some(2),
        line_end: Some(4),
    }
}

#[test]
fn build_domain_fact_serializes_as_versioned_snake_case_record() {
    let fact = ProofFactRecord::BuildDomain(BuildDomainFact {
        schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
        build_domain_id: BuildDomainId("bd:ploke-eval:lib".to_string()),
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
    });

    let json = serde_json::to_value(&fact).expect("serialize proof fact");

    assert_eq!(json["fact_kind"], "build_domain");
    assert_eq!(json["target_kind"], "library");
    assert_eq!(json["build_domain_id"], "bd:ploke-eval:lib");
    assert_eq!(json["schema_version"], PROOF_FACT_SCHEMA_VERSION);

    let round_trip: ProofFactRecord = serde_json::from_value(json).expect("round-trip proof fact");
    assert_eq!(round_trip, fact);
}

#[test]
fn expansion_boundary_carries_specific_proof_blocker_not_generic_unresolved() {
    let boundary = ExpansionBoundaryFact {
        schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
        boundary_id: ExpansionBoundaryId("boundary:macro:1".to_string()),
        build_domain_id: BuildDomainId("bd:ploke-eval:lib".to_string()),
        boundary_kind: ExpansionBoundaryKind::ProcMacroDerive,
        source_span: span(),
        expansion_state: ExpansionState::Blocked,
        blocking_reason: Some(ProofBlockerReason::ProcMacroSummaryMissing),
        macro_def_id: None,
        proc_macro_crate_id: Some("serde_derive".to_string()),
        build_script_package_id: None,
    };

    assert!(boundary.is_proof_blocking());

    let json = serde_json::to_value(ProofFactRecord::ExpansionBoundary(boundary))
        .expect("serialize expansion boundary");

    assert_eq!(json["fact_kind"], "expansion_boundary");
    assert_eq!(json["boundary_kind"], "proc_macro_derive");
    assert_eq!(json["expansion_state"], "blocked");
    assert_eq!(json["blocking_reason"], "proc_macro_summary_missing");
}

#[test]
fn unresolved_process_effect_seed_is_explicitly_proof_blocking() {
    let resolution = CallResolutionFact {
        schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
        call_site_id: CallSiteId("call:1".to_string()),
        resolution_state: ResolutionState::Unresolved,
        resolved_def_id: None,
        candidate_def_ids: Vec::new(),
        external_summary_id: None,
        blocking_reason: Some(ProofBlockerReason::TypeResolutionMissing),
    };

    assert!(resolution.is_proof_blocking());

    let effect = EffectSeedFact {
        schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
        effect_seed_id: EffectSeedId("effect:1".to_string()),
        call_site_id: CallSiteId("call:1".to_string()),
        effect_class: EffectClass::OperatingSystemProcessCreate,
        confidence: "seed".to_string(),
        blocker_if_unresolved: true,
        evidence_use: EvidenceUse::ProofOnly,
    };

    assert!(effect.blocks_proof_if_unresolved(&resolution));

    let json =
        serde_json::to_value(ProofFactRecord::EffectSeed(effect)).expect("serialize effect seed");

    assert_eq!(json["fact_kind"], "effect_seed");
    assert_eq!(json["effect_class"], "operating_system_process_create");
    assert_eq!(json["blocker_if_unresolved"], true);
}

#[test]
fn resolution_vocabulary_distinguishes_proof_from_navigation_consumers() {
    assert!(ResolutionState::Resolved.can_satisfy_proof(EvidenceUse::ProofOnly));
    assert!(ResolutionState::Resolved.can_satisfy_proof(EvidenceUse::ProofAndNavigation));
    assert!(!ResolutionState::Resolved.can_satisfy_proof(EvidenceUse::NavigationOnly));

    for state in [
        ResolutionState::CandidateSet,
        ResolutionState::Ambiguous,
        ResolutionState::Unresolved,
        ResolutionState::ExternallySummarized,
        ResolutionState::Blocked,
    ] {
        assert!(
            !state.can_satisfy_proof(EvidenceUse::ProofOnly),
            "{state:?} must not satisfy a proof obligation without stronger evidence"
        );
    }
}

#[test]
fn proof_obligation_status_is_fail_closed() {
    assert!(ObligationStatus::Admitted.satisfies_proof());
    assert!(!ObligationStatus::Rejected.satisfies_proof());
    assert!(!ObligationStatus::Blocked.satisfies_proof());
    assert!(ObligationStatus::Rejected.is_terminal_failure());
    assert!(ObligationStatus::Blocked.blocks_until_evidence());
}

#[test]
fn lifetime_vocabulary_requires_full_handoff_evidence_for_detached_processes() {
    assert!(!ProcessLifetime::RuntimeBounded.blocks_detached_process_proof(None));
    assert!(ProcessLifetime::Detached.blocks_detached_process_proof(None));

    let successor_only = HandoffEvidence {
        successor: ObligationStatus::Admitted,
        predecessor_retired: ObligationStatus::Blocked,
        exact_one_successor: ObligationStatus::Blocked,
    };
    assert!(ProcessLifetime::Detached.blocks_detached_process_proof(Some(successor_only)));

    let complete = HandoffEvidence::complete_successor_handoff();
    assert!(!ProcessLifetime::Detached.blocks_detached_process_proof(Some(complete)));
    assert!(ProcessLifetime::Unknown.blocks_detached_process_proof(Some(complete)));

    assert!(HandoffRole::Successor.requires_detached_exception());
    assert!(HandoffRole::PredecessorRetired.is_authority_retirement_evidence());
}

#[test]
fn authority_vocabulary_labels_boundaries_without_granting_authority() {
    assert!(AuthorityTerm::ParentLineage.is_permissioned_authority());
    assert!(AuthorityTerm::CrownRuling.is_permissioned_authority());
    assert!(AuthorityTerm::AuthorityTokenConstructor.is_authority_boundary());
    assert!(AuthorityTerm::ImmutableSurfaceDigestAdmission.is_authority_boundary());
    assert!(!AuthorityTerm::Successor.is_permissioned_authority());
    assert!(!AuthorityTerm::PredecessorRetired.is_permissioned_authority());

    for term in [
        AuthorityTerm::ParentLineage,
        AuthorityTerm::CrownRuling,
        AuthorityTerm::AuthorityTokenConstructor,
        AuthorityTerm::Successor,
        AuthorityTerm::PredecessorRetired,
        AuthorityTerm::ImmutableSurfaceDigestAdmission,
    ] {
        assert!(
            !term.record_deserialization_grants_authority(),
            "{term:?} is proof vocabulary only; deserialization must stay inert"
        );
    }
}

#[test]
fn effect_seed_records_proof_only_versus_navigation_only_evidence() {
    let effect = EffectSeedFact {
        schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
        effect_seed_id: EffectSeedId("effect:navigation:1".to_string()),
        call_site_id: CallSiteId("call:navigation:1".to_string()),
        effect_class: EffectClass::ExternalSummaryBoundary,
        confidence: "navigation-hint".to_string(),
        blocker_if_unresolved: true,
        evidence_use: EvidenceUse::NavigationOnly,
    };

    assert!(!effect.evidence_use.can_satisfy_proof());
    assert!(effect.evidence_use.visible_to_navigation());

    let unresolved = CallResolutionFact {
        schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
        call_site_id: CallSiteId("call:navigation:1".to_string()),
        resolution_state: ResolutionState::Unresolved,
        resolved_def_id: None,
        candidate_def_ids: Vec::new(),
        external_summary_id: None,
        blocking_reason: Some(ProofBlockerReason::TypeResolutionMissing),
    };
    assert!(unresolved.is_proof_blocking());
    assert!(
        !effect.blocks_proof_if_unresolved(&unresolved),
        "navigation-only evidence can guide browsing but must not participate in proof blocking"
    );

    let json =
        serde_json::to_value(ProofFactRecord::EffectSeed(effect)).expect("serialize effect seed");

    assert_eq!(json["fact_kind"], "effect_seed");
    assert_eq!(json["evidence_use"], "navigation_only");
}
