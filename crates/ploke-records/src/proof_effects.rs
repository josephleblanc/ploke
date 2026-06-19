//! Proof-oriented source extractor for call/effect seed facts.
//!
//! This module is intentionally conservative. It uses `syn` syntax traversal as
//! a capture scaffold, but emits Ploke's stable proof-fact records rather than
//! treating syn or rustc internal node shapes as canonical proof objects.

use std::collections::BTreeSet;

use crate::proof_facts::{
    BuildDomainId, CallEdgeFact, CallEdgeId, CallResolutionFact, CallSiteFact, CallSiteId,
    DefinitionId, EffectClass, EffectSeedFact, EffectSeedId, EvidenceUse, ExpansionBoundaryFact,
    ExpansionBoundaryId, ExpansionBoundaryKind, ExpansionState, PROOF_FACT_SCHEMA_VERSION,
    ProofBlockerReason, ProofFactRecord, ResolutionState, SourceSpanRecord,
};
use quote::ToTokens;
use syn1::spanned::Spanned;
use syn1::visit::{self, Visit};
use syn1::{Expr, ExprCall, ExprMacro, ExprMethodCall, ItemFn, ItemMacro, Macro};

/// Configuration for extracting proof facts from one source file.
#[derive(Debug, Clone)]
pub struct ProofExtractionConfig {
    pub build_domain_id: BuildDomainId,
    pub source_file: String,
    proof_critical_unknowns: BTreeSet<String>,
}

impl ProofExtractionConfig {
    pub fn for_source(build_domain_id: BuildDomainId, source_file: impl Into<String>) -> Self {
        Self {
            build_domain_id,
            source_file: source_file.into(),
            proof_critical_unknowns: BTreeSet::new(),
        }
    }

    pub fn with_proof_critical_unknowns<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.proof_critical_unknowns = names.into_iter().map(Into::into).collect();
        self
    }

    fn unknown_is_proof_critical(&self, name: &str) -> bool {
        self.proof_critical_unknowns.contains(name)
            || name.contains("spawn")
            || name.contains("process")
            || name.contains("command")
            || name.contains("exec")
            || name.contains("system")
    }
}

/// Parse one Rust source string and emit proof-fact records for call/effect seeds.
pub fn extract_proof_facts_from_source(
    source: &str,
    config: ProofExtractionConfig,
) -> Result<Vec<ProofFactRecord>, syn1::Error> {
    let file = syn1::parse_file(source)?;
    let mut extractor = ProofEffectExtractor::new(config);
    extractor.visit_file(&file);
    Ok(extractor.records)
}

struct ProofEffectExtractor {
    config: ProofExtractionConfig,
    records: Vec<ProofFactRecord>,
    current_caller: DefinitionId,
    call_index: usize,
    boundary_index: usize,
}

impl ProofEffectExtractor {
    fn new(config: ProofExtractionConfig) -> Self {
        Self {
            current_caller: DefinitionId(format!("def:{}:<module>", config.source_file)),
            config,
            records: Vec::new(),
            call_index: 0,
            boundary_index: 0,
        }
    }

    fn source_span(&self, span: proc_macro2::Span) -> SourceSpanRecord {
        let start = span.start();
        let end = span.end();
        SourceSpanRecord {
            file: self.config.source_file.clone(),
            start_byte: 0,
            end_byte: 0,
            line_start: Some(start.line as u32),
            line_end: Some(end.line as u32),
        }
    }

    fn record_call(&mut self, name: String, span: proc_macro2::Span, classification: CallClass) {
        self.call_index += 1;
        let call_site_id = CallSiteId(format!(
            "call:{}:{}:{}",
            self.config.source_file,
            self.call_index,
            sanitize_id(&name)
        ));
        let caller = self.current_caller.clone();
        let source_span = self.source_span(span);
        let evidence_use = classification.evidence_use();
        let resolution = classification.resolution_state();
        let callee_def_id = classification
            .resolved_callee(&name)
            .map(|callee| DefinitionId(format!("def:{callee}")));
        let candidate_def_ids = classification
            .candidate_callees(&name)
            .into_iter()
            .map(|callee| DefinitionId(format!("def:{callee}")))
            .collect::<Vec<_>>();
        let blocking_reason = classification.blocking_reason();

        self.records.push(ProofFactRecord::CallSite(CallSiteFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            call_site_id: call_site_id.clone(),
            build_domain_id: self.config.build_domain_id.clone(),
            caller_def_id: caller.clone(),
            source_span,
            evidence_use,
        }));
        self.records.push(ProofFactRecord::CallEdge(CallEdgeFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            call_edge_id: CallEdgeId(format!(
                "edge:{}:{}",
                self.config.source_file, self.call_index
            )),
            call_site_id: call_site_id.clone(),
            caller_def_id: caller,
            callee_def_id: callee_def_id.clone(),
            resolution_state: resolution,
            evidence_use,
        }));
        if evidence_use.can_satisfy_proof() {
            self.records
                .push(ProofFactRecord::CallResolution(CallResolutionFact {
                    schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
                    call_site_id: call_site_id.clone(),
                    resolution_state: resolution,
                    resolved_def_id: callee_def_id,
                    candidate_def_ids,
                    external_summary_id: None,
                    blocking_reason,
                }));
        }

        if let Some(effect_class) = classification.effect_class() {
            self.records
                .push(ProofFactRecord::EffectSeed(EffectSeedFact {
                    schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
                    effect_seed_id: EffectSeedId(format!(
                        "effect:{}:{}",
                        self.config.source_file, self.call_index
                    )),
                    call_site_id,
                    effect_class,
                    confidence: classification.confidence().to_string(),
                    blocker_if_unresolved: classification.blocks_if_unresolved(),
                    evidence_use,
                }));
        }
    }

    fn record_macro_boundary(&mut self, mac: &Macro) {
        self.boundary_index += 1;
        self.records
            .push(ProofFactRecord::ExpansionBoundary(ExpansionBoundaryFact {
                schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
                boundary_id: ExpansionBoundaryId(format!(
                    "boundary:{}:macro:{}",
                    self.config.source_file, self.boundary_index
                )),
                build_domain_id: self.config.build_domain_id.clone(),
                boundary_kind: ExpansionBoundaryKind::MacroRulesInvocation,
                source_span: self.source_span(mac.span()),
                expansion_state: ExpansionState::Blocked,
                blocking_reason: Some(ProofBlockerReason::MacroExpansionNotAvailable),
                macro_def_id: None,
                proc_macro_crate_id: None,
                build_script_package_id: None,
            }));
    }

    fn classify_direct_call(&self, name: &str) -> CallClass {
        match name {
            "std::process::Command::new" | "process::Command::new" | "Command::new" => {
                CallClass::known_effect(
                    EffectClass::OperatingSystemProcessConfigure,
                    "command-configure",
                )
            }
            "tokio::process::Command::new" => CallClass::known_effect(
                EffectClass::OperatingSystemProcessConfigure,
                "tokio-command-configure",
            ),
            "tokio::spawn" => CallClass::known_effect(EffectClass::AsyncTaskSpawn, "tokio-spawn"),
            "std::thread::spawn" | "thread::spawn" => {
                CallClass::known_effect(EffectClass::AsyncTaskSpawn, "thread-spawn")
            }
            "std::fs::write" | "fs::write" => {
                CallClass::known_effect(EffectClass::DurableEvidenceWrite, "fs-write")
            }
            "std::fs::read" | "fs::read" | "std::fs::read_to_string" | "fs::read_to_string" => {
                CallClass::known_effect(EffectClass::DurableEvidenceRead, "fs-read")
            }
            other if self.config.unknown_is_proof_critical(other) => {
                CallClass::proof_critical_unknown()
            }
            _ => CallClass::navigation_unknown(),
        }
    }

    fn classify_method_call(&self, method: &str, receiver_text: &str) -> CallClass {
        let receiver = receiver_text.replace(' ', "");
        match method {
            "spawn" if receiver.contains("Command::new") && receiver_invokes_shell(&receiver) => {
                CallClass::candidate_effect(
                    EffectClass::OperatingSystemProcessCreate,
                    "shell-command-spawn",
                )
            }
            "spawn" if receiver.contains("Command::new") => CallClass::candidate_effect(
                EffectClass::OperatingSystemProcessCreate,
                "command-spawn",
            ),
            "status" | "output" if receiver.contains("Command::new") => {
                CallClass::candidate_effect(
                    EffectClass::OperatingSystemProcessCreate,
                    "command-run",
                )
            }
            "status" | "output" => CallClass::candidate_effect(
                EffectClass::OperatingSystemProcessCreate,
                "method-process-run-conservative",
            ),
            "kill" => {
                CallClass::candidate_effect(EffectClass::OperatingSystemProcessKill, "process-kill")
            }
            "wait" => {
                CallClass::candidate_effect(EffectClass::OperatingSystemProcessWait, "process-wait")
            }
            "write" | "write_all" => {
                CallClass::candidate_effect(EffectClass::DurableEvidenceWrite, "writer-write")
            }
            "read" | "read_to_string" => {
                CallClass::candidate_effect(EffectClass::DurableEvidenceRead, "reader-read")
            }
            other if self.config.unknown_is_proof_critical(other) => {
                CallClass::proof_critical_unknown()
            }
            _ => CallClass::navigation_unknown(),
        }
    }
}

impl<'ast> Visit<'ast> for ProofEffectExtractor {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        let previous = self.current_caller.clone();
        self.current_caller = DefinitionId(format!(
            "def:{}:fn:{}",
            self.config.source_file, node.sig.ident
        ));
        visit::visit_block(self, &node.block);
        self.current_caller = previous;
    }

    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        let name = expr_call_name(&node.func);
        let classification = self.classify_direct_call(&name);
        self.record_call(name, node.span(), classification);
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        let receiver = node.receiver.to_token_stream().to_string();
        let classification = self.classify_method_call(&node.method.to_string(), &receiver);
        self.record_call(node.method.to_string(), node.span(), classification);
        visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_macro(&mut self, node: &'ast ExprMacro) {
        self.record_macro_boundary(&node.mac);
        visit::visit_expr_macro(self, node);
    }

    fn visit_item_macro(&mut self, node: &'ast ItemMacro) {
        self.record_macro_boundary(&node.mac);
        visit::visit_item_macro(self, node);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CallClass {
    KnownEffect {
        effect_class: EffectClass,
        confidence: &'static str,
    },
    CandidateEffect {
        effect_class: EffectClass,
        confidence: &'static str,
    },
    ProofCriticalUnknown,
    NavigationUnknown,
}

impl CallClass {
    fn known_effect(effect_class: EffectClass, confidence: &'static str) -> Self {
        Self::KnownEffect {
            effect_class,
            confidence,
        }
    }

    fn candidate_effect(effect_class: EffectClass, confidence: &'static str) -> Self {
        Self::CandidateEffect {
            effect_class,
            confidence,
        }
    }

    fn proof_critical_unknown() -> Self {
        Self::ProofCriticalUnknown
    }

    fn navigation_unknown() -> Self {
        Self::NavigationUnknown
    }

    fn evidence_use(self) -> EvidenceUse {
        match self {
            Self::NavigationUnknown => EvidenceUse::NavigationOnly,
            _ => EvidenceUse::ProofOnly,
        }
    }

    fn resolution_state(self) -> ResolutionState {
        match self {
            Self::KnownEffect { .. } => ResolutionState::Resolved,
            Self::CandidateEffect { .. } => ResolutionState::CandidateSet,
            Self::ProofCriticalUnknown => ResolutionState::Blocked,
            Self::NavigationUnknown => ResolutionState::Unresolved,
        }
    }

    fn resolved_callee(self, name: &str) -> Option<String> {
        matches!(self, Self::KnownEffect { .. }).then(|| sanitize_id(name))
    }

    fn candidate_callees(self, name: &str) -> Vec<String> {
        if matches!(self, Self::CandidateEffect { .. }) {
            vec![sanitize_id(name)]
        } else {
            Vec::new()
        }
    }

    fn blocking_reason(self) -> Option<ProofBlockerReason> {
        match self {
            Self::CandidateEffect { .. } => Some(ProofBlockerReason::TypeResolutionMissing),
            Self::ProofCriticalUnknown => {
                Some(ProofBlockerReason::ExternalDependencySummaryMissing)
            }
            Self::NavigationUnknown => Some(ProofBlockerReason::TypeResolutionMissing),
            Self::KnownEffect { .. } => None,
        }
    }

    fn effect_class(self) -> Option<EffectClass> {
        match self {
            Self::KnownEffect { effect_class, .. } | Self::CandidateEffect { effect_class, .. } => {
                Some(effect_class)
            }
            Self::ProofCriticalUnknown => Some(EffectClass::ExternalSummaryBoundary),
            Self::NavigationUnknown => None,
        }
    }

    fn confidence(self) -> &'static str {
        match self {
            Self::KnownEffect { confidence, .. } | Self::CandidateEffect { confidence, .. } => {
                confidence
            }
            Self::ProofCriticalUnknown => "proof-critical-unknown",
            Self::NavigationUnknown => "navigation-only-unknown",
        }
    }

    fn blocks_if_unresolved(self) -> bool {
        !matches!(self, Self::NavigationUnknown)
    }
}

fn expr_call_name(func: &Expr) -> String {
    match func {
        Expr::Path(path) => path_to_string(&path.path),
        _ => func.to_token_stream().to_string().replace(' ', ""),
    }
}

fn path_to_string(path: &syn1::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

fn receiver_invokes_shell(receiver: &str) -> bool {
    [
        "\"sh\"",
        "\"bash\"",
        "\"dash\"",
        "\"zsh\"",
        "\"cmd\"",
        "\"powershell\"",
    ]
    .iter()
    .any(|needle| receiver.contains(needle))
}

fn sanitize_id(input: &str) -> String {
    input
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | ':' | '_' | '-' => ch,
            _ => '_',
        })
        .collect()
}
