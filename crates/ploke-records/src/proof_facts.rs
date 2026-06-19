//! Passive proof-fact DTOs for proof-grade call/effect graph extraction.
//!
//! These records are the stable Ploke-facing shape emitted by parser, Cargo,
//! rustc, or external-summary extractors. They intentionally do not validate a
//! proof, admit History, grant Crown authority, or execute build/proc-macro
//! code. Proof checking and authority transitions remain outside this module.

use serde::{Deserialize, Serialize};

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
}

/// JSONL-friendly wrapper for stable proof facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "fact_kind", rename_all = "snake_case")]
pub enum ProofFactRecord {
    BuildDomain(BuildDomainFact),
    ExpansionBoundary(ExpansionBoundaryFact),
    CallResolution(CallResolutionFact),
    EffectSeed(EffectSeedFact),
}
