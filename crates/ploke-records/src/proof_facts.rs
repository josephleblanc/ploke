//! Passive proof-fact DTOs for proof-grade call/effect graph extraction.
//!
//! These records are the stable Ploke-facing shape emitted by parser, Cargo,
//! rustc, or external-summary extractors. They intentionally do not validate a
//! proof, admit History, grant Crown authority, or execute build/proc-macro
//! code. Proof checking and authority transitions remain outside this module.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PROOF_FACT_SCHEMA_VERSION: &str = "ploke-proof-facts.v1";

macro_rules! string_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

string_id! {
    /// Identity for one admitted Cargo/rustc proof boundary.
    BuildDomainId
}

string_id! {
    /// Identity for one materialized cfg domain within a build domain.
    CfgDomainId
}

string_id! {
    /// Identity for one captured rustc invocation within a build domain.
    RustcInvocationId
}

string_id! {
    /// Identity for one macro, build-script, proc-macro, include, or summary boundary.
    ExpansionBoundaryId
}

string_id! {
    /// Identity for one call expression or desugared executable call site.
    CallSiteId
}

string_id! {
    /// Backend-neutral identity for one resolved or candidate definition.
    DefinitionId
}

string_id! {
    /// Identity for one external summary artifact.
    ExternalSummaryId
}

string_id! {
    /// Identity for one semantic effect seed attached to a call site.
    EffectSeedId
}

string_id! {
    /// Identity for one normalized call edge.
    CallEdgeId
}

string_id! {
    /// Identity for one expanded item linked to an expansion boundary.
    ExpandedItemId
}

string_id! {
    /// Identity for one authority proof fact.
    AuthorityFactId
}

string_id! {
    /// Identity for one active validation blocker.
    BlockerFactId
}

/// Source coordinate retained for proof facts and diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpanRecord {
    pub file: String,
    pub start_byte: u32,
    pub end_byte: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_start: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_end: Option<u32>,
}

/// Cargo target kind for a build domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    Library,
    Binary,
    Test,
    Example,
    Benchmark,
    BuildScript,
    ProcMacro,
}

/// One proof boundary over a Cargo-resolved package target and toolchain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildDomainFact {
    pub schema_version: String,
    pub build_domain_id: BuildDomainId,
    pub cargo_metadata_hash: String,
    pub cargo_lock_hash: String,
    pub package_id: String,
    pub target_kind: TargetKind,
    pub target_name: String,
    pub target_root: String,
    pub target_triple: String,
    pub host_triple: String,
    pub profile: String,
    pub features_hash: String,
    pub active_cfg_hash: String,
    pub rustc_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rustc_commit_hash: Option<String>,
    pub extractor_version: String,
    pub proof_policy_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub immutable_surface_digest: Option<String>,
}

/// Materialized conditional-compilation domain for a build domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CfgDomainFact {
    pub schema_version: String,
    pub cfg_domain_id: CfgDomainId,
    pub build_domain_id: BuildDomainId,
    pub active_cfg_hash: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cfg_atoms: Vec<String>,
    pub status: ObligationStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocking_reason: Option<ProofBlockerReason>,
}

/// Captured rustc invocation evidence for one build domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RustcInvocationFact {
    pub schema_version: String,
    pub invocation_id: RustcInvocationId,
    pub build_domain_id: BuildDomainId,
    pub rustc_program: String,
    pub rustc_version: String,
    pub working_directory: String,
    pub argument_vector_hash: String,
    pub environment_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_file_hash: Option<String>,
    pub status: ObligationStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocking_reason: Option<ProofBlockerReason>,
    #[serde(default)]
    pub evidence_use: EvidenceUse,
}

/// Captured inputs for one Cargo/rustc build proof boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildEvidenceSnapshot {
    pub build_domain_id: BuildDomainId,
    pub cargo_metadata_json: Vec<u8>,
    pub cargo_lock: Vec<u8>,
    pub package_id: String,
    pub target_kind: TargetKind,
    pub target_name: String,
    pub target_root: String,
    pub target_triple: String,
    pub host_triple: String,
    pub profile: String,
    pub selected_features: Vec<String>,
    pub active_cfg_atoms: Vec<String>,
    pub rustc_version: String,
    pub rustc_commit_hash: Option<String>,
    pub extractor_version: String,
    pub proof_policy_version: String,
    pub immutable_surface_digest: Option<String>,
}

/// Captured rustc invocation input. Raw arguments are reduced to stable hashes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildRustcInvocationCapture {
    pub invocation_id: String,
    pub rustc_program: String,
    pub working_directory: String,
    pub args: Vec<String>,
    pub environment: Vec<(String, String)>,
    pub response_file_contents: Option<Vec<u8>>,
    pub status: ObligationStatus,
    pub blocking_reason: Option<ProofBlockerReason>,
}

/// Captured macro/build/proc-macro/include boundary input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpansionBoundaryCapture {
    pub boundary_id: ExpansionBoundaryId,
    pub boundary_kind: ExpansionBoundaryKind,
    pub source_span: SourceSpanRecord,
    pub expansion_state: ExpansionState,
    pub blocking_reason: Option<ProofBlockerReason>,
    pub macro_def_id: Option<DefinitionId>,
    pub proc_macro_crate_id: Option<String>,
    pub build_script_package_id: Option<String>,
}

impl BuildEvidenceSnapshot {
    /// Convert captured build evidence into passive proof records consumed by validation.
    pub fn proof_records(
        &self,
        rustc_invocation: BuildRustcInvocationCapture,
        expansion_boundaries: Vec<ExpansionBoundaryCapture>,
    ) -> Vec<ProofFactRecord> {
        let mut records = vec![
            ProofFactRecord::BuildDomain(self.build_domain_fact()),
            ProofFactRecord::CfgDomain(self.cfg_domain_fact()),
            ProofFactRecord::RustcInvocation(self.rustc_invocation_fact(rustc_invocation)),
        ];
        records.extend(expansion_boundaries.into_iter().map(|boundary| {
            ProofFactRecord::ExpansionBoundary(self.expansion_boundary_fact(boundary))
        }));
        records
    }

    pub fn build_domain_fact(&self) -> BuildDomainFact {
        BuildDomainFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            build_domain_id: self.build_domain_id.clone(),
            cargo_metadata_hash: hash_bytes(&self.cargo_metadata_json),
            cargo_lock_hash: hash_bytes(&self.cargo_lock),
            package_id: self.package_id.clone(),
            target_kind: self.target_kind,
            target_name: self.target_name.clone(),
            target_root: self.target_root.clone(),
            target_triple: self.target_triple.clone(),
            host_triple: self.host_triple.clone(),
            profile: self.profile.clone(),
            features_hash: hash_unordered_strings(&self.selected_features),
            active_cfg_hash: hash_unordered_strings(&self.active_cfg_atoms),
            rustc_version: self.rustc_version.clone(),
            rustc_commit_hash: self.rustc_commit_hash.clone(),
            extractor_version: self.extractor_version.clone(),
            proof_policy_version: self.proof_policy_version.clone(),
            immutable_surface_digest: self.immutable_surface_digest.clone(),
        }
    }

    pub fn cfg_domain_fact(&self) -> CfgDomainFact {
        let mut cfg_atoms = self.active_cfg_atoms.clone();
        cfg_atoms.sort();
        CfgDomainFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            cfg_domain_id: CfgDomainId(format!("cfg:{}", self.build_domain_id.as_str())),
            build_domain_id: self.build_domain_id.clone(),
            active_cfg_hash: hash_unordered_strings(&self.active_cfg_atoms),
            cfg_atoms,
            status: ObligationStatus::Admitted,
            blocking_reason: None,
        }
    }

    pub fn rustc_invocation_fact(
        &self,
        capture: BuildRustcInvocationCapture,
    ) -> RustcInvocationFact {
        RustcInvocationFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            invocation_id: RustcInvocationId(capture.invocation_id),
            build_domain_id: self.build_domain_id.clone(),
            rustc_program: capture.rustc_program,
            rustc_version: self.rustc_version.clone(),
            working_directory: capture.working_directory,
            argument_vector_hash: hash_string_sequence(&capture.args),
            environment_hash: hash_environment(&capture.environment),
            response_file_hash: capture.response_file_contents.as_deref().map(hash_bytes),
            status: capture.status,
            blocking_reason: capture.blocking_reason,
            evidence_use: EvidenceUse::ProofOnly,
        }
    }

    pub fn expansion_boundary_fact(
        &self,
        capture: ExpansionBoundaryCapture,
    ) -> ExpansionBoundaryFact {
        ExpansionBoundaryFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            boundary_id: capture.boundary_id,
            build_domain_id: self.build_domain_id.clone(),
            boundary_kind: capture.boundary_kind,
            source_span: capture.source_span,
            expansion_state: capture.expansion_state,
            blocking_reason: capture.blocking_reason,
            macro_def_id: capture.macro_def_id,
            proc_macro_crate_id: capture.proc_macro_crate_id,
            build_script_package_id: capture.build_script_package_id,
        }
    }
}

/// Boundary category for expansion/provenance facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpansionBoundaryKind {
    MacroRulesInvocation,
    ProcMacroDerive,
    ProcMacroAttribute,
    ProcMacroFunction,
    BuildScript,
    Include,
    ExternalSummary,
}

/// Expansion state after applying the current extractor and proof policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpansionState {
    Expanded,
    Unresolved,
    ExternallySummarized,
    Blocked,
}

/// Stable proof-blocker categories. These replace generic "unknown edge" states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofBlockerReason {
    MacroExpansionNotAvailable,
    ProcMacroSummaryMissing,
    BuildScriptSummaryMissing,
    ExternalDependencySummaryMissing,
    CfgDomainNotMaterialized,
    TypeResolutionMissing,
    DynamicDispatchUnbounded,
    ExternalCommandSummaryMissing,
    BuildScriptExecutionBlocked,
    ProcMacroExecutionBlocked,
    CanonicalIdentityMismatch,
    SchemaVersionMismatch,
    AuthorityEvidenceMissing,
    ProcessLifetimeEvidenceMissing,
    RustcInvocationEvidenceMissing,
}

/// Whether an evidence row may participate in proof, navigation, or both.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceUse {
    /// Evidence consumed only by proof checkers and proof reports.
    #[default]
    ProofOnly,
    /// Evidence used for symbol lookup, browsing, or GraphRAG hints only.
    NavigationOnly,
    /// Evidence may be shown to navigation consumers and checked by proof consumers.
    ProofAndNavigation,
}

impl EvidenceUse {
    /// True when this row may be used to satisfy proof obligations.
    pub fn can_satisfy_proof(self) -> bool {
        matches!(self, Self::ProofOnly | Self::ProofAndNavigation)
    }

    /// True when this row may be displayed by navigation or GraphRAG consumers.
    pub fn visible_to_navigation(self) -> bool {
        matches!(self, Self::NavigationOnly | Self::ProofAndNavigation)
    }
}

/// Fail-closed status for a normalized proof obligation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObligationStatus {
    /// Required evidence has been admitted by the checker boundary.
    Admitted,
    /// Evidence contradicts the obligation.
    Rejected,
    /// Evidence is missing, ambiguous, unresolved, or outside the proof domain.
    Blocked,
}

impl ObligationStatus {
    /// True only when the obligation can satisfy a proof consumer.
    pub fn satisfies_proof(self) -> bool {
        matches!(self, Self::Admitted)
    }

    /// True when the checker found contradictory evidence rather than missing evidence.
    pub fn is_terminal_failure(self) -> bool {
        matches!(self, Self::Rejected)
    }

    /// True when more typed evidence is required before a proof can pass.
    pub fn blocks_until_evidence(self) -> bool {
        matches!(self, Self::Blocked)
    }
}

/// Handoff role relevant to the detached-process exception.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandoffRole {
    /// The one admitted detached successor runtime for a lineage transition.
    Successor,
    /// Evidence that the predecessor authority has retired or locked.
    PredecessorRetired,
}

impl HandoffRole {
    /// True for the role that may require the detached-process exception.
    pub fn requires_detached_exception(self) -> bool {
        matches!(self, Self::Successor)
    }

    /// True for the role proving predecessor authority is no longer ruling.
    pub fn is_authority_retirement_evidence(self) -> bool {
        matches!(self, Self::PredecessorRetired)
    }
}

/// Minimal fail-closed evidence needed before a detached successor may be exempted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandoffEvidence {
    /// Successor-side admission evidence.
    pub successor: ObligationStatus,
    /// Predecessor authority has retired or locked before successor authority.
    pub predecessor_retired: ObligationStatus,
    /// Exactly one detached successor is admitted for this lineage transition.
    pub exact_one_successor: ObligationStatus,
}

impl HandoffEvidence {
    /// Convenience value for tests and checker fixtures with all handoff gates satisfied.
    pub fn complete_successor_handoff() -> Self {
        Self {
            successor: ObligationStatus::Admitted,
            predecessor_retired: ObligationStatus::Admitted,
            exact_one_successor: ObligationStatus::Admitted,
        }
    }

    /// True only when all detached-successor handoff gates satisfy proof.
    pub fn satisfies_detached_successor(self) -> bool {
        self.successor.satisfies_proof()
            && self.predecessor_retired.satisfies_proof()
            && self.exact_one_successor.satisfies_proof()
    }
}

/// Conservative lifetime classification for process effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessLifetime {
    /// The creating runtime owns, waits, kills, reaps, or otherwise bounds the process.
    RuntimeBounded,
    /// The process may outlive the creating runtime.
    Detached,
    /// The extractor cannot prove whether the process is runtime-bounded.
    Unknown,
}

impl ProcessLifetime {
    /// True when this lifetime classification blocks the detached-process proof.
    pub fn blocks_detached_process_proof(self, handoff: Option<HandoffEvidence>) -> bool {
        match self {
            Self::RuntimeBounded => false,
            Self::Detached => !handoff.is_some_and(HandoffEvidence::satisfies_detached_successor),
            Self::Unknown => true,
        }
    }
}

/// Authority vocabulary used by proof facts without granting authority itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityTerm {
    /// Permissioned lineage authority currently allowed to rule.
    CrownRuling,
    /// Code boundary that can construct or move authority tokens.
    AuthorityTokenConstructor,
    /// Successor-side role in a handoff.
    Successor,
    /// Predecessor has retired or locked authority before successor admission.
    PredecessorRetired,
}

impl AuthorityTerm {
    /// True for the vocabulary term representing permissioned active authority.
    pub fn is_permissioned_authority(self) -> bool {
        matches!(self, Self::CrownRuling)
    }

    /// True for terms that identify authority-construction proof boundaries.
    pub fn is_authority_boundary(self) -> bool {
        matches!(self, Self::AuthorityTokenConstructor)
    }

    /// Passive record vocabulary is inert and can never grant authority.
    pub fn record_deserialization_grants_authority(self) -> bool {
        let _ = self;
        false
    }
}

/// Macro/build/proc-macro provenance boundary for a build domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpansionBoundaryFact {
    pub schema_version: String,
    pub boundary_id: ExpansionBoundaryId,
    pub build_domain_id: BuildDomainId,
    pub boundary_kind: ExpansionBoundaryKind,
    pub source_span: SourceSpanRecord,
    pub expansion_state: ExpansionState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocking_reason: Option<ProofBlockerReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub macro_def_id: Option<DefinitionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proc_macro_crate_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_script_package_id: Option<String>,
}

impl ExpansionBoundaryFact {
    pub fn is_proof_blocking(&self) -> bool {
        matches!(
            self.expansion_state,
            ExpansionState::Blocked | ExpansionState::Unresolved
        ) || self.blocking_reason.is_some()
    }
}

/// Callee resolution status for one executable call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionState {
    Resolved,
    CandidateSet,
    Ambiguous,
    Unresolved,
    ExternallySummarized,
    Blocked,
}

/// Resolution fact for one call site.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallResolutionFact {
    pub schema_version: String,
    pub call_site_id: CallSiteId,
    pub resolution_state: ResolutionState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_def_id: Option<DefinitionId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidate_def_ids: Vec<DefinitionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_summary_id: Option<ExternalSummaryId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocking_reason: Option<ProofBlockerReason>,
}

impl CallResolutionFact {
    pub fn is_proof_blocking(&self) -> bool {
        matches!(
            self.resolution_state,
            ResolutionState::CandidateSet
                | ResolutionState::Ambiguous
                | ResolutionState::Unresolved
                | ResolutionState::ExternallySummarized
                | ResolutionState::Blocked
        ) || self.blocking_reason.is_some()
    }
}

impl ResolutionState {
    /// True only for resolved call evidence that is allowed to satisfy proof.
    pub fn can_satisfy_proof(self, evidence_use: EvidenceUse) -> bool {
        matches!(self, Self::Resolved) && evidence_use.can_satisfy_proof()
    }
}

/// Semantic effect classes seeded from call resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectClass {
    OperatingSystemProcessCreate,
    OperatingSystemProcessReplace,
    OperatingSystemProcessConfigure,
    OperatingSystemProcessWait,
    OperatingSystemProcessKill,
    OperatingSystemProcessReap,
    AsyncTaskSpawn,
    AsyncTaskJoin,
    AsyncTaskAbort,
    AuthorityMint,
    AuthorityRetire,
    AuthorityLock,
    AuthorityUnlock,
    HistoryOpenBlock,
    HistorySealBlock,
    HistoryAppendBlock,
    SurfaceMeasure,
    SurfaceDigestCompare,
    DurableEvidenceWrite,
    DurableEvidenceRead,
    ExternalSummaryBoundary,
}

/// Initial effect classification for a call site.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectSeedFact {
    pub schema_version: String,
    pub effect_seed_id: EffectSeedId,
    pub call_site_id: CallSiteId,
    pub effect_class: EffectClass,
    pub confidence: String,
    pub blocker_if_unresolved: bool,
    #[serde(default)]
    pub evidence_use: EvidenceUse,
}

impl EffectSeedFact {
    pub fn blocks_proof_if_unresolved(&self, resolution: &CallResolutionFact) -> bool {
        self.call_site_id == resolution.call_site_id
            && self.evidence_use.can_satisfy_proof()
            && self.blocker_if_unresolved
            && resolution.is_proof_blocking()
    }

    pub fn requires_process_lifetime_evidence(&self) -> bool {
        matches!(
            self.effect_class,
            EffectClass::OperatingSystemProcessCreate | EffectClass::OperatingSystemProcessReplace
        )
    }
}

/// Normalized executable call site with build-domain provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallSiteFact {
    pub schema_version: String,
    pub call_site_id: CallSiteId,
    pub build_domain_id: BuildDomainId,
    pub caller_def_id: DefinitionId,
    pub source_span: SourceSpanRecord,
    #[serde(default)]
    pub evidence_use: EvidenceUse,
}

/// Normalized call edge from one call site to one resolved or candidate callee.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallEdgeFact {
    pub schema_version: String,
    pub call_edge_id: CallEdgeId,
    pub call_site_id: CallSiteId,
    pub caller_def_id: DefinitionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub callee_def_id: Option<DefinitionId>,
    pub resolution_state: ResolutionState,
    #[serde(default)]
    pub evidence_use: EvidenceUse,
}

/// Expanded item linked to the boundary that produced or summarized it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpandedItemFact {
    pub schema_version: String,
    pub expanded_item_id: ExpandedItemId,
    pub boundary_id: ExpansionBoundaryId,
    pub build_domain_id: BuildDomainId,
    pub definition_id: DefinitionId,
    pub source_span: SourceSpanRecord,
    #[serde(default)]
    pub evidence_use: EvidenceUse,
}

/// Authority-related fact. This is a passive label and cannot grant authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityFact {
    pub schema_version: String,
    pub authority_fact_id: AuthorityFactId,
    pub build_domain_id: BuildDomainId,
    pub source_span: SourceSpanRecord,
    pub authority_term: AuthorityTerm,
    pub status: ObligationStatus,
    #[serde(default)]
    pub evidence_use: EvidenceUse,
}

impl AuthorityFact {
    /// Passive record vocabulary is inert and can never grant authority.
    pub fn record_deserialization_grants_authority(&self) -> bool {
        false
    }
}

/// Active validation blocker produced from normalized proof facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofBlockerFact {
    pub schema_version: String,
    pub blocker_id: BlockerFactId,
    pub reason: ProofBlockerReason,
    pub status: ObligationStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_domain_id: Option<BuildDomainId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_site_id: Option<CallSiteId>,
    pub detail: String,
}

/// Active validation report over a proof fact set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofValidationReport {
    pub status: ObligationStatus,
    pub blockers: Vec<ProofBlockerFact>,
}

/// JSONL-friendly wrapper for stable proof facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "fact_kind", rename_all = "snake_case")]
pub enum ProofFactRecord {
    BuildDomain(BuildDomainFact),
    CfgDomain(CfgDomainFact),
    RustcInvocation(RustcInvocationFact),
    ExpansionBoundary(ExpansionBoundaryFact),
    ExpandedItem(ExpandedItemFact),
    CallSite(CallSiteFact),
    CallEdge(CallEdgeFact),
    CallResolution(CallResolutionFact),
    EffectSeed(EffectSeedFact),
    Authority(AuthorityFact),
    ProofBlocker(ProofBlockerFact),
}

impl ProofFactRecord {
    fn schema_version(&self) -> &str {
        match self {
            Self::BuildDomain(fact) => &fact.schema_version,
            Self::CfgDomain(fact) => &fact.schema_version,
            Self::RustcInvocation(fact) => &fact.schema_version,
            Self::ExpansionBoundary(fact) => &fact.schema_version,
            Self::ExpandedItem(fact) => &fact.schema_version,
            Self::CallSite(fact) => &fact.schema_version,
            Self::CallEdge(fact) => &fact.schema_version,
            Self::CallResolution(fact) => &fact.schema_version,
            Self::EffectSeed(fact) => &fact.schema_version,
            Self::Authority(fact) => &fact.schema_version,
            Self::ProofBlocker(fact) => &fact.schema_version,
        }
    }
}

/// Error produced while importing proof facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofFactImportError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ProofFactImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "proof fact import failed on line {}: {}",
            self.line, self.message
        )
    }
}

impl std::error::Error for ProofFactImportError {}

/// Imported proof facts plus an active validation/checker entrypoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofFactSet {
    records: Vec<ProofFactRecord>,
}

impl ProofFactSet {
    /// Import proof facts from already deserialized passive records.
    pub fn from_records(records: Vec<ProofFactRecord>) -> Result<Self, ProofFactImportError> {
        Ok(Self { records })
    }

    /// Import newline-delimited proof facts.
    pub fn from_jsonl(input: &str) -> Result<Self, ProofFactImportError> {
        let mut records = Vec::new();
        for (index, line) in input.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let record = serde_json::from_str::<ProofFactRecord>(line).map_err(|source| {
                ProofFactImportError {
                    line: index + 1,
                    message: source.to_string(),
                }
            })?;
            records.push(record);
        }
        Self::from_records(records)
    }

    /// Passive records retained by the import path.
    pub fn records(&self) -> &[ProofFactRecord] {
        &self.records
    }

    /// Iterate authority facts without granting authority.
    pub fn authority_facts(&self) -> impl Iterator<Item = &AuthorityFact> {
        self.records.iter().filter_map(|record| match record {
            ProofFactRecord::Authority(fact) => Some(fact),
            _ => None,
        })
    }

    /// Active validation path used by checker scaffolding.
    pub fn validate_for_proof(&self) -> ProofValidationReport {
        let mut blockers = Vec::new();
        let mut build_domains: HashMap<BuildDomainId, &BuildDomainFact> = HashMap::new();
        let mut cfg_domains: HashMap<CfgDomainId, &CfgDomainFact> = HashMap::new();
        let mut rustc_invocations: HashMap<RustcInvocationId, &RustcInvocationFact> =
            HashMap::new();
        let mut boundaries: HashMap<ExpansionBoundaryId, &ExpansionBoundaryFact> = HashMap::new();
        let mut expanded_items: HashMap<ExpandedItemId, &ExpandedItemFact> = HashMap::new();
        let mut call_sites: HashMap<CallSiteId, &CallSiteFact> = HashMap::new();
        let mut proof_call_sites: HashMap<CallSiteId, &CallSiteFact> = HashMap::new();
        let mut call_edges: HashMap<CallEdgeId, &CallEdgeFact> = HashMap::new();
        let mut resolutions: HashMap<CallSiteId, &CallResolutionFact> = HashMap::new();
        let mut effect_seeds: HashMap<EffectSeedId, &EffectSeedFact> = HashMap::new();
        let mut authority_facts: HashMap<AuthorityFactId, &AuthorityFact> = HashMap::new();

        for record in &self.records {
            match record {
                ProofFactRecord::BuildDomain(fact) => {
                    if let Some(existing) = build_domains.insert(fact.build_domain_id.clone(), fact)
                    {
                        if existing != fact {
                            blockers.push(blocker(
                                ProofBlockerReason::CanonicalIdentityMismatch,
                                ObligationStatus::Blocked,
                                Some(fact.build_domain_id.clone()),
                                None,
                                "duplicate build domain id has conflicting payload".to_string(),
                            ));
                        }
                    }
                }
                ProofFactRecord::CfgDomain(fact) => {
                    if let Some(existing) = cfg_domains.insert(fact.cfg_domain_id.clone(), fact) {
                        if existing != fact {
                            blockers.push(blocker(
                                ProofBlockerReason::CanonicalIdentityMismatch,
                                ObligationStatus::Blocked,
                                Some(fact.build_domain_id.clone()),
                                None,
                                "duplicate cfg domain id has conflicting payload".to_string(),
                            ));
                        }
                    }
                }
                ProofFactRecord::RustcInvocation(fact) => {
                    if let Some(existing) =
                        rustc_invocations.insert(fact.invocation_id.clone(), fact)
                    {
                        if existing != fact {
                            blockers.push(blocker(
                                ProofBlockerReason::CanonicalIdentityMismatch,
                                ObligationStatus::Blocked,
                                Some(fact.build_domain_id.clone()),
                                None,
                                "duplicate rustc invocation id has conflicting payload".to_string(),
                            ));
                        }
                    }
                }
                ProofFactRecord::ExpansionBoundary(fact) => {
                    if let Some(existing) = boundaries.insert(fact.boundary_id.clone(), fact) {
                        if existing != fact {
                            blockers.push(blocker(
                                ProofBlockerReason::CanonicalIdentityMismatch,
                                ObligationStatus::Blocked,
                                Some(fact.build_domain_id.clone()),
                                None,
                                "duplicate expansion boundary id has conflicting payload"
                                    .to_string(),
                            ));
                        }
                    }
                }
                ProofFactRecord::ExpandedItem(fact) => {
                    if let Some(existing) =
                        expanded_items.insert(fact.expanded_item_id.clone(), fact)
                    {
                        if existing != fact {
                            blockers.push(blocker(
                                ProofBlockerReason::CanonicalIdentityMismatch,
                                ObligationStatus::Blocked,
                                Some(fact.build_domain_id.clone()),
                                None,
                                "duplicate expanded item id has conflicting payload".to_string(),
                            ));
                        }
                    }
                }
                ProofFactRecord::CallSite(fact) => {
                    if let Some(existing) = call_sites.insert(fact.call_site_id.clone(), fact) {
                        if existing != fact {
                            blockers.push(blocker(
                                ProofBlockerReason::CanonicalIdentityMismatch,
                                ObligationStatus::Blocked,
                                Some(fact.build_domain_id.clone()),
                                Some(fact.call_site_id.clone()),
                                "duplicate call site id has conflicting payload".to_string(),
                            ));
                        }
                    }
                    if fact.evidence_use.can_satisfy_proof() {
                        proof_call_sites.insert(fact.call_site_id.clone(), fact);
                    }
                }
                ProofFactRecord::CallEdge(fact) => {
                    if let Some(existing) = call_edges.insert(fact.call_edge_id.clone(), fact) {
                        if existing != fact {
                            blockers.push(blocker(
                                ProofBlockerReason::CanonicalIdentityMismatch,
                                ObligationStatus::Blocked,
                                None,
                                Some(fact.call_site_id.clone()),
                                "duplicate call edge id has conflicting payload".to_string(),
                            ));
                        }
                    }
                }
                ProofFactRecord::CallResolution(fact) => {
                    if let Some(existing) = resolutions.insert(fact.call_site_id.clone(), fact) {
                        if existing != fact {
                            blockers.push(blocker(
                                ProofBlockerReason::CanonicalIdentityMismatch,
                                ObligationStatus::Blocked,
                                None,
                                Some(fact.call_site_id.clone()),
                                "duplicate call resolution id has conflicting payload".to_string(),
                            ));
                        }
                    }
                }
                ProofFactRecord::EffectSeed(fact) => {
                    if let Some(existing) = effect_seeds.insert(fact.effect_seed_id.clone(), fact) {
                        if existing != fact {
                            blockers.push(blocker(
                                ProofBlockerReason::CanonicalIdentityMismatch,
                                ObligationStatus::Blocked,
                                None,
                                Some(fact.call_site_id.clone()),
                                "duplicate effect seed id has conflicting payload".to_string(),
                            ));
                        }
                    }
                }
                ProofFactRecord::Authority(fact) => {
                    if let Some(existing) =
                        authority_facts.insert(fact.authority_fact_id.clone(), fact)
                    {
                        if existing != fact {
                            blockers.push(blocker(
                                ProofBlockerReason::CanonicalIdentityMismatch,
                                ObligationStatus::Blocked,
                                Some(fact.build_domain_id.clone()),
                                None,
                                "duplicate authority fact id has conflicting payload".to_string(),
                            ));
                        }
                    }
                }
                ProofFactRecord::ProofBlocker(_) => {}
            }
        }

        for build_domain in build_domains.values() {
            let admitted_cfg = cfg_domains.values().any(|cfg| {
                cfg.build_domain_id == build_domain.build_domain_id
                    && cfg.status.satisfies_proof()
                    && cfg.active_cfg_hash == build_domain.active_cfg_hash
            });
            if !admitted_cfg {
                blockers.push(blocker(
                    ProofBlockerReason::CfgDomainNotMaterialized,
                    ObligationStatus::Blocked,
                    Some(build_domain.build_domain_id.clone()),
                    None,
                    "build domain has no admitted matching cfg-domain evidence".to_string(),
                ));
            }

            let admitted_rustc = rustc_invocations.values().any(|invocation| {
                invocation.evidence_use.can_satisfy_proof()
                    && invocation.build_domain_id == build_domain.build_domain_id
                    && invocation.status.satisfies_proof()
                    && invocation.rustc_version == build_domain.rustc_version
                    && !invocation.argument_vector_hash.is_empty()
                    && !invocation.environment_hash.is_empty()
            });
            if !admitted_rustc {
                blockers.push(blocker(
                    ProofBlockerReason::RustcInvocationEvidenceMissing,
                    ObligationStatus::Blocked,
                    Some(build_domain.build_domain_id.clone()),
                    None,
                    "build domain has no admitted rustc invocation evidence".to_string(),
                ));
            }
        }

        for record in &self.records {
            if record.schema_version() != PROOF_FACT_SCHEMA_VERSION {
                blockers.push(blocker(
                    ProofBlockerReason::SchemaVersionMismatch,
                    ObligationStatus::Blocked,
                    None,
                    None,
                    format!("unsupported proof fact schema {}", record.schema_version()),
                ));
            }

            match record {
                ProofFactRecord::BuildDomain(_) => {}
                ProofFactRecord::CfgDomain(fact) => {
                    match build_domains.get(&fact.build_domain_id) {
                        Some(build_domain)
                            if build_domain.active_cfg_hash != fact.active_cfg_hash =>
                        {
                            blockers.push(blocker(
                                ProofBlockerReason::CfgDomainNotMaterialized,
                                ObligationStatus::Blocked,
                                Some(fact.build_domain_id.clone()),
                                None,
                                "cfg domain hash does not match build domain active cfg hash"
                                    .to_string(),
                            ));
                        }
                        None => blockers.push(blocker(
                            ProofBlockerReason::CanonicalIdentityMismatch,
                            ObligationStatus::Blocked,
                            Some(fact.build_domain_id.clone()),
                            None,
                            "cfg domain references missing build domain".to_string(),
                        )),
                        _ => {}
                    }
                    if hash_unordered_strings(&fact.cfg_atoms) != fact.active_cfg_hash {
                        blockers.push(blocker(
                            ProofBlockerReason::CfgDomainNotMaterialized,
                            ObligationStatus::Blocked,
                            Some(fact.build_domain_id.clone()),
                            None,
                            "cfg domain atoms do not match claimed active cfg hash".to_string(),
                        ));
                    }
                    if !fact.status.satisfies_proof() {
                        blockers.push(blocker(
                            fact.blocking_reason
                                .unwrap_or(ProofBlockerReason::CfgDomainNotMaterialized),
                            fact.status,
                            Some(fact.build_domain_id.clone()),
                            None,
                            "cfg domain is not admitted".to_string(),
                        ));
                    }
                }
                ProofFactRecord::RustcInvocation(fact) => {
                    if fact.evidence_use.can_satisfy_proof() {
                        match build_domains.get(&fact.build_domain_id) {
                            Some(build_domain)
                                if build_domain.rustc_version != fact.rustc_version =>
                            {
                                blockers.push(blocker(
                                    ProofBlockerReason::CanonicalIdentityMismatch,
                                    ObligationStatus::Blocked,
                                    Some(fact.build_domain_id.clone()),
                                    None,
                                    "rustc invocation version does not match build domain"
                                        .to_string(),
                                ));
                            }
                            None => blockers.push(blocker(
                                ProofBlockerReason::CanonicalIdentityMismatch,
                                ObligationStatus::Blocked,
                                Some(fact.build_domain_id.clone()),
                                None,
                                "rustc invocation references missing build domain".to_string(),
                            )),
                            _ => {}
                        }
                        if !fact.status.satisfies_proof() {
                            blockers.push(blocker(
                                fact.blocking_reason
                                    .unwrap_or(ProofBlockerReason::RustcInvocationEvidenceMissing),
                                fact.status,
                                Some(fact.build_domain_id.clone()),
                                None,
                                "rustc invocation evidence is not admitted".to_string(),
                            ));
                        }
                        if fact.argument_vector_hash.is_empty() || fact.environment_hash.is_empty()
                        {
                            blockers.push(blocker(
                                ProofBlockerReason::RustcInvocationEvidenceMissing,
                                ObligationStatus::Blocked,
                                Some(fact.build_domain_id.clone()),
                                None,
                                "rustc invocation is missing argument or environment hash"
                                    .to_string(),
                            ));
                        }
                    }
                }
                ProofFactRecord::ExpansionBoundary(fact) => {
                    if !build_domains.contains_key(&fact.build_domain_id) {
                        blockers.push(blocker(
                            ProofBlockerReason::CanonicalIdentityMismatch,
                            ObligationStatus::Blocked,
                            Some(fact.build_domain_id.clone()),
                            None,
                            "expansion boundary references missing build domain".to_string(),
                        ));
                    }
                    if fact.is_proof_blocking() {
                        blockers.push(blocker(
                            fact.blocking_reason
                                .unwrap_or(ProofBlockerReason::MacroExpansionNotAvailable),
                            ObligationStatus::Blocked,
                            Some(fact.build_domain_id.clone()),
                            None,
                            "expansion boundary blocks proof".to_string(),
                        ));
                    }
                }
                ProofFactRecord::ExpandedItem(fact) => {
                    if fact.evidence_use.can_satisfy_proof() {
                        match boundaries.get(&fact.boundary_id) {
                            Some(boundary) if boundary.build_domain_id != fact.build_domain_id => {
                                blockers.push(blocker(
                                    ProofBlockerReason::CanonicalIdentityMismatch,
                                    ObligationStatus::Blocked,
                                    Some(fact.build_domain_id.clone()),
                                    None,
                                    "expanded item and expansion boundary use different build domains"
                                        .to_string(),
                                ));
                            }
                            Some(_) if !build_domains.contains_key(&fact.build_domain_id) => {
                                blockers.push(blocker(
                                    ProofBlockerReason::CanonicalIdentityMismatch,
                                    ObligationStatus::Blocked,
                                    Some(fact.build_domain_id.clone()),
                                    None,
                                    "expanded item references missing build domain".to_string(),
                                ));
                            }
                            None => blockers.push(blocker(
                                ProofBlockerReason::CanonicalIdentityMismatch,
                                ObligationStatus::Blocked,
                                Some(fact.build_domain_id.clone()),
                                None,
                                "expanded item references missing expansion boundary".to_string(),
                            )),
                            _ => {}
                        }
                    }
                }
                ProofFactRecord::CallSite(fact) => {
                    if fact.evidence_use.can_satisfy_proof()
                        && !build_domains.contains_key(&fact.build_domain_id)
                    {
                        blockers.push(blocker(
                            ProofBlockerReason::CanonicalIdentityMismatch,
                            ObligationStatus::Blocked,
                            Some(fact.build_domain_id.clone()),
                            Some(fact.call_site_id.clone()),
                            "call site references missing build domain".to_string(),
                        ));
                    }
                }
                ProofFactRecord::CallEdge(fact) => {
                    if fact.evidence_use.can_satisfy_proof() {
                        match proof_call_sites.get(&fact.call_site_id) {
                            Some(call_site) if call_site.caller_def_id != fact.caller_def_id => {
                                blockers.push(blocker(
                                    ProofBlockerReason::CanonicalIdentityMismatch,
                                    ObligationStatus::Blocked,
                                    Some(call_site.build_domain_id.clone()),
                                    Some(fact.call_site_id.clone()),
                                    "call edge caller does not match proof call site caller"
                                        .to_string(),
                                ));
                            }
                            None => blockers.push(blocker(
                                ProofBlockerReason::CanonicalIdentityMismatch,
                                ObligationStatus::Blocked,
                                None,
                                Some(fact.call_site_id.clone()),
                                "proof call edge references missing or navigation-only call site"
                                    .to_string(),
                            )),
                            _ => {}
                        }
                        if !fact.resolution_state.can_satisfy_proof(fact.evidence_use)
                            || fact.callee_def_id.is_none()
                        {
                            blockers.push(blocker(
                                ProofBlockerReason::TypeResolutionMissing,
                                ObligationStatus::Blocked,
                                None,
                                Some(fact.call_site_id.clone()),
                                "proof call edge is not resolved to a callee definition"
                                    .to_string(),
                            ));
                        }
                        if let Some(resolution) = resolutions.get(&fact.call_site_id) {
                            if let (Some(edge_callee), Some(resolved_callee)) =
                                (&fact.callee_def_id, &resolution.resolved_def_id)
                            {
                                if edge_callee != resolved_callee {
                                    blockers.push(blocker(
                                        ProofBlockerReason::CanonicalIdentityMismatch,
                                        ObligationStatus::Blocked,
                                        None,
                                        Some(fact.call_site_id.clone()),
                                        "call edge callee does not match call resolution callee"
                                            .to_string(),
                                    ));
                                }
                            }
                        }
                    }
                }
                ProofFactRecord::CallResolution(fact) => {
                    if !proof_call_sites.contains_key(&fact.call_site_id) {
                        blockers.push(blocker(
                            ProofBlockerReason::CanonicalIdentityMismatch,
                            ObligationStatus::Blocked,
                            None,
                            Some(fact.call_site_id.clone()),
                            "call resolution references missing or navigation-only proof call site"
                                .to_string(),
                        ));
                    }
                    let resolution_blocker = match fact.resolution_state {
                        ResolutionState::ExternallySummarized => Some(
                            fact.blocking_reason
                                .unwrap_or(ProofBlockerReason::ExternalDependencySummaryMissing),
                        ),
                        ResolutionState::Resolved if fact.resolved_def_id.is_none() => {
                            Some(ProofBlockerReason::TypeResolutionMissing)
                        }
                        _ if fact.is_proof_blocking() => Some(
                            fact.blocking_reason
                                .unwrap_or(ProofBlockerReason::TypeResolutionMissing),
                        ),
                        _ => None,
                    };
                    if let Some(reason) = resolution_blocker {
                        blockers.push(blocker(
                            reason,
                            ObligationStatus::Blocked,
                            None,
                            Some(fact.call_site_id.clone()),
                            "call resolution cannot satisfy proof".to_string(),
                        ));
                    }
                }
                ProofFactRecord::EffectSeed(fact) => {
                    if fact.evidence_use.can_satisfy_proof()
                        && !proof_call_sites.contains_key(&fact.call_site_id)
                    {
                        blockers.push(blocker(
                            ProofBlockerReason::CanonicalIdentityMismatch,
                            ObligationStatus::Blocked,
                            None,
                            Some(fact.call_site_id.clone()),
                            "effect seed references missing or navigation-only proof call site"
                                .to_string(),
                        ));
                    }
                    if fact.evidence_use.can_satisfy_proof() && fact.blocker_if_unresolved {
                        match resolutions.get(&fact.call_site_id) {
                            Some(resolution) if fact.blocks_proof_if_unresolved(resolution) => {
                                blockers.push(blocker(
                                    if resolution.resolution_state
                                        == ResolutionState::ExternallySummarized
                                    {
                                        resolution.blocking_reason.unwrap_or(
                                            ProofBlockerReason::ExternalDependencySummaryMissing,
                                        )
                                    } else {
                                        resolution
                                            .blocking_reason
                                            .unwrap_or(ProofBlockerReason::TypeResolutionMissing)
                                    },
                                    ObligationStatus::Blocked,
                                    None,
                                    Some(fact.call_site_id.clone()),
                                    "proof effect is attached to unresolved call".to_string(),
                                ));
                            }
                            None => blockers.push(blocker(
                                ProofBlockerReason::TypeResolutionMissing,
                                ObligationStatus::Blocked,
                                None,
                                Some(fact.call_site_id.clone()),
                                "proof effect has no call resolution".to_string(),
                            )),
                            _ => {}
                        }
                    }
                    if fact.evidence_use.can_satisfy_proof()
                        && fact.requires_process_lifetime_evidence()
                    {
                        blockers.push(blocker(
                            ProofBlockerReason::ProcessLifetimeEvidenceMissing,
                            ObligationStatus::Blocked,
                            None,
                            Some(fact.call_site_id.clone()),
                            "process effect has no runtime-bounded lifetime or admitted successor handoff evidence"
                                .to_string(),
                        ));
                    }
                }
                ProofFactRecord::Authority(fact) => {
                    if fact.evidence_use.can_satisfy_proof()
                        && !build_domains.contains_key(&fact.build_domain_id)
                    {
                        blockers.push(blocker(
                            ProofBlockerReason::CanonicalIdentityMismatch,
                            ObligationStatus::Blocked,
                            Some(fact.build_domain_id.clone()),
                            None,
                            "authority fact references missing build domain".to_string(),
                        ));
                    }
                    if fact.evidence_use.can_satisfy_proof() && !fact.status.satisfies_proof() {
                        blockers.push(blocker(
                            ProofBlockerReason::AuthorityEvidenceMissing,
                            fact.status,
                            Some(fact.build_domain_id.clone()),
                            None,
                            "authority fact is not admitted".to_string(),
                        ));
                    }
                }
                ProofFactRecord::ProofBlocker(fact) => blockers.push(fact.clone()),
            }
        }

        deduplicate_blocker_ids(&mut blockers);

        let status = if blockers
            .iter()
            .any(|blocker| blocker.status.is_terminal_failure())
        {
            ObligationStatus::Rejected
        } else if blockers.is_empty() {
            ObligationStatus::Admitted
        } else {
            ObligationStatus::Blocked
        };

        ProofValidationReport { status, blockers }
    }
}

fn blocker(
    reason: ProofBlockerReason,
    status: ObligationStatus,
    build_domain_id: Option<BuildDomainId>,
    call_site_id: Option<CallSiteId>,
    detail: String,
) -> ProofBlockerFact {
    let build_domain_component = build_domain_id
        .as_ref()
        .map_or_else(|| "bd:<none>".to_string(), |id| format!("bd:{id}"));
    let call_site_component = call_site_id
        .as_ref()
        .map_or_else(|| "call:<none>".to_string(), |id| format!("call:{id}"));

    ProofBlockerFact {
        schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
        blocker_id: BlockerFactId(format!(
            "blocker:{reason:?}:{build_domain_component}:{call_site_component}"
        )),
        reason,
        status,
        build_domain_id,
        call_site_id,
        detail,
    }
}

fn deduplicate_blocker_ids(blockers: &mut [ProofBlockerFact]) {
    let mut reserved = HashMap::<String, ()>::new();
    for blocker in blockers {
        let base = blocker.blocker_id.0.clone();
        let mut candidate = base.clone();
        let mut suffix = 1usize;
        while reserved.contains_key(&candidate) {
            candidate = format!("{base}:duplicate:{suffix}");
            suffix += 1;
        }
        blocker.blocker_id = BlockerFactId(candidate.clone());
        reserved.insert(candidate, ());
    }
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

fn hash_string_sequence(values: &[String]) -> String {
    let mut hasher = Sha256::new();
    for value in values {
        let bytes = value.as_bytes();
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn hash_unordered_strings(values: &[String]) -> String {
    let mut sorted = values.to_vec();
    sorted.sort();
    hash_string_sequence(&sorted)
}

fn hash_environment(values: &[(String, String)]) -> String {
    let mut sorted = values.to_vec();
    sorted.sort();
    let mut hasher = Sha256::new();
    for (key, value) in sorted {
        let key = key.as_bytes();
        let value = value.as_bytes();
        hasher.update((key.len() as u64).to_le_bytes());
        hasher.update(key);
        hasher.update((value.len() as u64).to_le_bytes());
        hasher.update(value);
    }
    format!("sha256:{:x}", hasher.finalize())
}
