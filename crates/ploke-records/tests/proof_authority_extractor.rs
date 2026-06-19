use ploke_records::proof_authority::{
    AuthorityAdmissionSite, AuthorityExtractionConfig, extract_authority_facts_from_source,
};
use ploke_records::proof_facts::{
    AuthorityTerm, BuildDomainId, ObligationStatus, ProofBlockerReason, ProofFactRecord,
};

fn authority_terms(records: &[ProofFactRecord]) -> Vec<(AuthorityTerm, ObligationStatus)> {
    records
        .iter()
        .filter_map(|record| match record {
            ProofFactRecord::Authority(fact) => Some((fact.authority_term, fact.status)),
            _ => None,
        })
        .collect()
}

fn trusted_site(source: &str, canonical_call: &str, needle: &str) -> AuthorityAdmissionSite {
    let line_start = source
        .lines()
        .position(|line| line.contains(needle))
        .map(|line| line as u32 + 1)
        .expect("fixture contains trusted authority call");
    AuthorityAdmissionSite::from_canonical_call_site(canonical_call, line_start)
}

#[test]
fn authority_extractor_admits_explicit_typestate_boundaries() {
    let source = r#"
        struct Ruling;
        struct Crown<T>(T);
        impl<T> Crown<T> {
            fn admit_authority() {}
        }
        struct ParentLineage;
        impl ParentLineage { fn admit_parent() {} }
        struct AuthorityToken;
        impl AuthorityToken { fn construct() {} }
        struct Handoff;
        impl Handoff { fn admit_successor_parent() {} }
        struct Predecessor;
        impl Predecessor { fn retire_authority() {} }
        struct ImmutableSurfaceDigest;
        impl ImmutableSurfaceDigest { fn admit() {} }

        fn handoff() {
            ParentLineage::admit_parent();
            Crown::<Ruling>::admit_authority();
            AuthorityToken::construct();
            Handoff::admit_successor_parent();
            Predecessor::retire_authority();
            ImmutableSurfaceDigest::admit();
        }
    "#;

    let records = extract_authority_facts_from_source(
        source,
        AuthorityExtractionConfig::for_source(
            BuildDomainId("bd:authority".to_string()),
            "src/auth.rs",
        )
        .with_admitted_sites([
            trusted_site(
                source,
                "ParentLineage::admit_parent",
                "ParentLineage::admit_parent",
            ),
            trusted_site(
                source,
                "Crown::<Ruling>::admit_authority",
                "Crown::<Ruling>::admit_authority",
            ),
            trusted_site(
                source,
                "AuthorityToken::construct",
                "AuthorityToken::construct",
            ),
            trusted_site(
                source,
                "Handoff::admit_successor_parent",
                "Handoff::admit_successor_parent",
            ),
            trusted_site(
                source,
                "Predecessor::retire_authority",
                "Predecessor::retire_authority",
            ),
            trusted_site(
                source,
                "ImmutableSurfaceDigest::admit",
                "ImmutableSurfaceDigest::admit",
            ),
        ]),
    )
    .expect("parse source");
    let terms = authority_terms(&records);

    for expected in [
        AuthorityTerm::ParentLineage,
        AuthorityTerm::CrownRuling,
        AuthorityTerm::AuthorityTokenConstructor,
        AuthorityTerm::Successor,
        AuthorityTerm::PredecessorRetired,
        AuthorityTerm::ImmutableSurfaceDigestAdmission,
    ] {
        assert!(
            terms.contains(&(expected, ObligationStatus::Admitted)),
            "missing admitted authority term {expected:?} in {terms:?}"
        );
    }
}

#[test]
fn authority_extractor_blocks_structural_similarity_and_invalid_handoff() {
    let source = r#"
        struct Ruling;
        struct Crown<T>(T);
        impl<T> Crown<T> { fn new() -> Self { todo!() } }
        struct Handoff;
        impl Handoff { fn unchecked_successor_parent() {} }

        fn fake() {
            let _fake = Crown::<Ruling>::new();
            Handoff::unchecked_successor_parent();
        }
    "#;

    let records = extract_authority_facts_from_source(
        source,
        AuthorityExtractionConfig::for_source(
            BuildDomainId("bd:authority".to_string()),
            "src/fake.rs",
        ),
    )
    .expect("parse source");

    assert!(records.iter().all(|record| match record {
        ProofFactRecord::Authority(fact) => fact.status != ObligationStatus::Admitted,
        _ => true,
    }));
    assert!(records.iter().any(|record| matches!(
        record,
        ProofFactRecord::ProofBlocker(blocker)
            if blocker.reason == ProofBlockerReason::AuthorityEvidenceMissing
    )));
}

#[test]
fn authority_extractor_does_not_promote_passive_dto_deserialization() {
    let source = r#"
        fn load_passive_record(input: &str) {
            let _record: ploke_records::proof_facts::AuthorityFact = serde_json::from_str(input).unwrap();
        }
    "#;

    let records = extract_authority_facts_from_source(
        source,
        AuthorityExtractionConfig::for_source(
            BuildDomainId("bd:authority".to_string()),
            "src/dto.rs",
        ),
    )
    .expect("parse source");

    assert!(records.iter().all(|record| match record {
        ProofFactRecord::Authority(fact) => fact.status != ObligationStatus::Admitted,
        _ => true,
    }));
}

#[test]
fn authority_extractor_rejects_spoofed_prefix_suffix_and_parent_helpers() {
    let source = r#"
        struct FakeCrownRuling;
        impl FakeCrownRuling { fn admit_authority() {} }
        struct AuthorityTokenLike;
        impl AuthorityTokenLike { fn construct() {} }
        struct Handoff;
        impl Handoff { fn admit_successor_parent_unchecked() {} }
        struct ParentLineageAlias;
        impl ParentLineageAlias { fn admit_parent() {} }

        fn parent() {}
        fn fake() {
            FakeCrownRuling::admit_authority();
            AuthorityTokenLike::construct();
            Handoff::admit_successor_parent_unchecked();
            ParentLineageAlias::admit_parent();
            parent();
        }
    "#;

    let records = extract_authority_facts_from_source(
        source,
        AuthorityExtractionConfig::for_source(
            BuildDomainId("bd:authority".to_string()),
            "src/spoof.rs",
        ),
    )
    .expect("parse source");

    assert!(records.iter().all(|record| match record {
        ProofFactRecord::Authority(fact) => fact.status != ObligationStatus::Admitted,
        _ => true,
    }));
    assert!(records.iter().any(|record| matches!(
        record,
        ProofFactRecord::ProofBlocker(blocker)
            if blocker.reason == ProofBlockerReason::AuthorityEvidenceMissing
    )));
    let authority_ids = records
        .iter()
        .filter_map(|record| match record {
            ProofFactRecord::Authority(fact) => Some(fact.authority_fact_id.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        !authority_ids.iter().any(|id| id.ends_with(":parent")),
        "ordinary parent() helper must not become authority evidence: {authority_ids:?}"
    );
    assert!(
        !authority_ids
            .iter()
            .any(|id| id.contains("parentlineagealias")),
        "alias names must not spoof the ParentLineage boundary: {authority_ids:?}"
    );
}

#[test]
fn authority_extractor_blocks_qualified_ufcs_and_method_forms_without_trusted_site() {
    let source = r#"
        fn fake(
            crown: Crown<Ruling>,
            c: Crown<Ruling>,
            lineage: ParentLineage,
            l: ParentLineage,
            token: AuthorityToken,
            t: AuthorityToken,
            handoff: Handoff,
            h: Handoff,
            predecessor: Predecessor,
            p: Predecessor,
            digest: ImmutableSurfaceDigest,
            d: ImmutableSurfaceDigest,
        ) {
            crate::ParentLineage::admit_parent();
            some_module::Crown::<Ruling>::admit_authority();
            <crate::ParentLineage>::admit_parent();
            <crate::Crown<Ruling>>::admit_authority();
            <crate::AuthorityToken>::construct();
            <crate::Handoff>::admit_successor_parent();
            <crate::Predecessor>::retire_authority();
            <crate::ImmutableSurfaceDigest>::admit();
            <crate::assets::Crown<Ruling> as Authority>::admit_authority();
            <crate::has_authority::ImmutableSurfaceDigest as Authority>::admit();
            <ParentLineage as Authority>::admit_parent();
            <Crown<Ruling> as Authority>::admit_authority();
            <AuthorityToken as Authority>::construct();
            <Handoff as Authority>::admit_successor_parent();
            <Predecessor as Authority>::retire_authority();
            <ImmutableSurfaceDigest as Authority>::admit();
            <Crown<Ruling>>::admit_authority();
            crown.admit_authority();
            c.admit_authority();
            token.construct();
            predecessor.retire_authority();
            <ParentLineage>::admit_parent();
            <AuthorityToken>::construct();
            <AuthorityToken>::mint();
            <AuthorityToken>::admit();
            <Handoff>::admit_successor_parent();
            <Predecessor>::retire_authority();
            <ImmutableSurfaceDigest>::admit();
            <ImmutableSurfaceDigest>::verify();
            <ImmutableSurfaceDigest>::compare();
            lineage.admit_parent();
            l.admit_parent();
            token.mint();
            token.admit();
            t.construct();
            t.mint();
            t.admit();
            handoff.admit_successor_parent();
            h.admit_successor_parent();
            predecessor.lock_authority();
            p.retire_authority();
            p.lock_authority();
            digest.admit();
            digest.verify();
            digest.compare();
            d.admit();
            d.verify();
            d.compare();
            let x: Crown<Ruling> = todo!();
            let y: ParentLineage = todo!();
            let z: AuthorityToken = todo!();
            let q: Handoff = todo!();
            let r: Predecessor = todo!();
            let s: ImmutableSurfaceDigest = todo!();
            x.admit_authority();
            y.admit_parent();
            z.construct();
            q.admit_successor_parent();
            r.retire_authority();
            s.admit();
        }

        impl AuthoritySurface {
            fn method(&self, c: Crown<Ruling>, d: ImmutableSurfaceDigest) {
                c.admit_authority();
                d.admit();
            }
        }

        trait AuthorityTrait {
            fn trait_method(c: Crown<Ruling>, d: ImmutableSurfaceDigest) {
                c.admit_authority();
                d.admit();
            }
        }

        fn closure_site() {
            let closure = |c: Crown<Ruling>, d: ImmutableSurfaceDigest| {
                c.admit_authority();
                d.admit();
            };
            let _ = closure;
        }
    "#;

    let records = extract_authority_facts_from_source(
        source,
        AuthorityExtractionConfig::for_source(
            BuildDomainId("bd:authority".to_string()),
            "src/qualified.rs",
        ),
    )
    .expect("parse source");

    assert!(records.iter().all(|record| match record {
        ProofFactRecord::Authority(fact) => fact.status != ObligationStatus::Admitted,
        _ => true,
    }));
    let blocker_count = records
        .iter()
        .filter(|record| matches!(record, ProofFactRecord::ProofBlocker(_)))
        .count();
    assert!(
        blocker_count >= 60,
        "qualified, UFCS, and method authority forms must block instead of disappearing; got {records:?}"
    );
}
