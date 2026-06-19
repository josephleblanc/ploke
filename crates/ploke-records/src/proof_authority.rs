//! Proof-oriented authority and typestate seed extractor.
//!
//! This module is intentionally conservative. It recognizes explicit authority
//! boundary calls and records structurally similar or unchecked calls as blocked
//! evidence. It emits passive proof facts only; it does not admit History, grant
//! Crown authority, execute a handoff, or validate a proof.

use std::collections::{BTreeMap, BTreeSet};

use crate::proof_facts::{
    AuthorityFact, AuthorityFactId, AuthorityTerm, BlockerFactId, BuildDomainId, EvidenceUse,
    ObligationStatus, PROOF_FACT_SCHEMA_VERSION, ProofBlockerFact, ProofBlockerReason,
    ProofFactRecord, SourceSpanRecord,
};
use quote::ToTokens;
use syn1::spanned::Spanned;
use syn1::visit::{self, Visit};
use syn1::{
    ExprCall, ExprClosure, ExprMethodCall, FnArg, ImplItemMethod, ItemFn, Local, Pat,
    TraitItemMethod,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AuthorityAdmissionSite {
    canonical_call: String,
    line_start: u32,
}

impl AuthorityAdmissionSite {
    pub fn from_canonical_call_site(canonical_call: impl AsRef<str>, line_start: u32) -> Self {
        Self {
            canonical_call: canonical_call_name(canonical_call.as_ref()),
            line_start,
        }
    }
}

/// Configuration for extracting authority proof facts from one source file.
#[derive(Debug, Clone)]
pub struct AuthorityExtractionConfig {
    pub build_domain_id: BuildDomainId,
    pub source_file: String,
    admitted_sites: BTreeSet<AuthorityAdmissionSite>,
}

impl AuthorityExtractionConfig {
    pub fn for_source(build_domain_id: BuildDomainId, source_file: impl Into<String>) -> Self {
        Self {
            build_domain_id,
            source_file: source_file.into(),
            admitted_sites: BTreeSet::new(),
        }
    }

    /// Mark exact source-site authority surfaces as trusted proof boundaries.
    ///
    /// This is deliberately opt-in. The extractor can identify authority-shaped
    /// calls from source text, but only a caller that has already bound those
    /// strings and line sites to trusted definition/provenance evidence may
    /// admit them. All exact-but-untrusted or malformed authority-like calls
    /// remain blocked.
    pub fn with_admitted_sites<I>(mut self, sites: I) -> Self
    where
        I: IntoIterator<Item = AuthorityAdmissionSite>,
    {
        self.admitted_sites = sites.into_iter().collect();
        self
    }

    fn status_for_site(&self, canonical: &str, line_start: u32) -> ObligationStatus {
        let site = AuthorityAdmissionSite::from_canonical_call_site(canonical, line_start);
        if self.admitted_sites.contains(&site) {
            ObligationStatus::Admitted
        } else {
            ObligationStatus::Blocked
        }
    }
}

/// Parse one Rust source string and emit passive authority proof facts.
pub fn extract_authority_facts_from_source(
    source: &str,
    config: AuthorityExtractionConfig,
) -> Result<Vec<ProofFactRecord>, syn1::Error> {
    let file = syn1::parse_file(source)?;
    let mut extractor = AuthorityExtractor::new(config);
    extractor.visit_file(&file);
    Ok(extractor.records)
}

struct AuthorityExtractor {
    config: AuthorityExtractionConfig,
    records: Vec<ProofFactRecord>,
    current_caller: String,
    receiver_types: BTreeMap<String, String>,
    authority_index: usize,
}

impl AuthorityExtractor {
    fn new(config: AuthorityExtractionConfig) -> Self {
        Self {
            current_caller: format!("def:{}:<module>", config.source_file),
            receiver_types: BTreeMap::new(),
            config,
            records: Vec::new(),
            authority_index: 0,
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

    fn remember_typed_pattern(&mut self, pat: &Pat, ty: &impl ToTokens) {
        match pat {
            Pat::Ident(ident) => {
                self.receiver_types.insert(
                    ident.ident.to_string(),
                    canonical_call_name(&ty.to_token_stream().to_string()),
                );
            }
            Pat::Type(pat_type) => self.remember_typed_pattern(&pat_type.pat, &pat_type.ty),
            _ => {}
        }
    }

    fn remember_fn_inputs(
        &mut self,
        inputs: &syn1::punctuated::Punctuated<FnArg, syn1::token::Comma>,
    ) {
        for input in inputs {
            if let FnArg::Typed(pat_type) = input {
                self.remember_typed_pattern(&pat_type.pat, &pat_type.ty);
            }
        }
    }

    fn record_authority(
        &mut self,
        call_name: String,
        span: proc_macro2::Span,
        class: AuthorityClass,
    ) {
        self.authority_index += 1;
        let slug = sanitize_id(&call_name);
        let source_span = self.source_span(span);
        let fact_id = AuthorityFactId(format!(
            "authority:{}:{}:{}",
            self.config.source_file, self.authority_index, slug
        ));
        self.records.push(ProofFactRecord::Authority(AuthorityFact {
            schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
            authority_fact_id: fact_id.clone(),
            build_domain_id: self.config.build_domain_id.clone(),
            source_span,
            authority_term: class.term,
            status: class.status,
            evidence_use: EvidenceUse::ProofOnly,
        }));

        if class.status.blocks_until_evidence() || class.status.is_terminal_failure() {
            self.records
                .push(ProofFactRecord::ProofBlocker(ProofBlockerFact {
                    schema_version: PROOF_FACT_SCHEMA_VERSION.to_string(),
                    blocker_id: BlockerFactId(format!(
                        "blocker:{}:{}",
                        self.config.source_file, self.authority_index
                    )),
                    reason: ProofBlockerReason::AuthorityEvidenceMissing,
                    status: ObligationStatus::Blocked,
                    build_domain_id: Some(self.config.build_domain_id.clone()),
                    call_site_id: None,
                    detail: format!(
                        "authority-like call {call_name} is not an admitted typestate boundary"
                    ),
                }));
        }
    }
}

impl<'ast> Visit<'ast> for AuthorityExtractor {
    fn visit_item_fn(&mut self, item: &'ast ItemFn) {
        let previous = self.current_caller.clone();
        let previous_receiver_types = std::mem::take(&mut self.receiver_types);
        self.current_caller = format!("def:{}::{}", self.config.source_file, item.sig.ident);
        self.remember_fn_inputs(&item.sig.inputs);
        visit::visit_item_fn(self, item);
        self.current_caller = previous;
        self.receiver_types = previous_receiver_types;
    }

    fn visit_impl_item_method(&mut self, item: &'ast ImplItemMethod) {
        let previous = self.current_caller.clone();
        let previous_receiver_types = std::mem::take(&mut self.receiver_types);
        self.current_caller = format!("def:{}::{}", self.config.source_file, item.sig.ident);
        self.remember_fn_inputs(&item.sig.inputs);
        visit::visit_impl_item_method(self, item);
        self.current_caller = previous;
        self.receiver_types = previous_receiver_types;
    }

    fn visit_trait_item_method(&mut self, item: &'ast TraitItemMethod) {
        let previous = self.current_caller.clone();
        let previous_receiver_types = std::mem::take(&mut self.receiver_types);
        self.current_caller = format!("def:{}::{}", self.config.source_file, item.sig.ident);
        self.remember_fn_inputs(&item.sig.inputs);
        visit::visit_trait_item_method(self, item);
        self.current_caller = previous;
        self.receiver_types = previous_receiver_types;
    }

    fn visit_expr_closure(&mut self, closure: &'ast ExprClosure) {
        let previous_receiver_types = self.receiver_types.clone();
        for input in &closure.inputs {
            if let Pat::Type(pat_type) = input {
                self.remember_typed_pattern(&pat_type.pat, &pat_type.ty);
            }
        }
        visit::visit_expr_closure(self, closure);
        self.receiver_types = previous_receiver_types;
    }

    fn visit_expr_call(&mut self, call: &'ast ExprCall) {
        let name = call.func.to_token_stream().to_string();
        if let Some(class) =
            classify_authority_call(&name, &self.config, call.span().start().line as u32)
        {
            self.record_authority(name, call.span(), class);
        }
        visit::visit_expr_call(self, call);
    }

    fn visit_local(&mut self, local: &'ast Local) {
        if let Pat::Type(pat_type) = &local.pat {
            if let Pat::Ident(ident) = pat_type.pat.as_ref() {
                self.receiver_types.insert(
                    ident.ident.to_string(),
                    canonical_call_name(&pat_type.ty.to_token_stream().to_string()),
                );
            }
        }
        visit::visit_local(self, local);
    }

    fn visit_expr_method_call(&mut self, call: &'ast ExprMethodCall) {
        let receiver = call.receiver.to_token_stream().to_string();
        let receiver_canonical = canonical_call_name(&receiver);
        let method = call.method.to_string();
        let name = self
            .receiver_types
            .get(&receiver_canonical)
            .map(|receiver_type| format!("{receiver_type}.{method}"))
            .unwrap_or_else(|| format!("{receiver}.{method}"));
        if let Some(class) =
            classify_authority_call(&name, &self.config, call.span().start().line as u32)
        {
            self.record_authority(name, call.span(), class);
        }
        visit::visit_expr_method_call(self, call);
    }
}

#[derive(Debug, Clone, Copy)]
struct AuthorityClass {
    term: AuthorityTerm,
    status: ObligationStatus,
}

impl AuthorityClass {
    fn new(term: AuthorityTerm, status: ObligationStatus) -> Self {
        Self { term, status }
    }

    fn blocked(term: AuthorityTerm) -> Self {
        Self {
            term,
            status: ObligationStatus::Blocked,
        }
    }
}

fn classify_authority_call(
    raw_name: &str,
    config: &AuthorityExtractionConfig,
    line_start: u32,
) -> Option<AuthorityClass> {
    let canonical = canonical_call_name(raw_name);

    let exact_term = match canonical.as_str() {
        call if call == "ParentLineage::admit_parent"
            || call == "<ParentLineage>::admit_parent"
            || call.ends_with("::ParentLineage::admit_parent") =>
        {
            Some(AuthorityTerm::ParentLineage)
        }
        call if matches!(
            call,
            "Crown::<Ruling>::admit_authority"
                | "Crown::<Ruling>::admit_ruling"
                | "Crown::<Ruling>::admit_crown_ruling"
                | "<Crown<Ruling>>::admit_authority"
                | "<Crown<Ruling>>::admit_ruling"
                | "<Crown<Ruling>>::admit_crown_ruling"
        ) || call.ends_with("::Crown::<Ruling>::admit_authority")
            || call.ends_with("::Crown::<Ruling>::admit_ruling")
            || call.ends_with("::Crown::<Ruling>::admit_crown_ruling") =>
        {
            Some(AuthorityTerm::CrownRuling)
        }
        call if matches!(
            call,
            "AuthorityToken::construct"
                | "AuthorityToken::mint"
                | "AuthorityToken::admit"
                | "<AuthorityToken>::construct"
                | "<AuthorityToken>::mint"
                | "<AuthorityToken>::admit"
        ) || call.ends_with("::AuthorityToken::construct")
            || call.ends_with("::AuthorityToken::mint")
            || call.ends_with("::AuthorityToken::admit") =>
        {
            Some(AuthorityTerm::AuthorityTokenConstructor)
        }
        call if matches!(
            call,
            "Handoff::admit_successor_parent"
                | "Handoff::admit_successor"
                | "<Handoff>::admit_successor_parent"
                | "<Handoff>::admit_successor"
        ) || call.ends_with("::Handoff::admit_successor_parent")
            || call.ends_with("::Handoff::admit_successor") =>
        {
            Some(AuthorityTerm::Successor)
        }
        call if matches!(
            call,
            "Predecessor::retire_authority"
                | "Predecessor::lock_authority"
                | "<Predecessor>::retire_authority"
                | "<Predecessor>::lock_authority"
        ) || call.ends_with("::Predecessor::retire_authority")
            || call.ends_with("::Predecessor::lock_authority") =>
        {
            Some(AuthorityTerm::PredecessorRetired)
        }
        call if matches!(
            call,
            "ImmutableSurfaceDigest::admit"
                | "ImmutableSurfaceDigest::verify"
                | "ImmutableSurfaceDigest::compare"
                | "<ImmutableSurfaceDigest>::admit"
                | "<ImmutableSurfaceDigest>::verify"
                | "<ImmutableSurfaceDigest>::compare"
        ) || call.ends_with("::ImmutableSurfaceDigest::admit")
            || call.ends_with("::ImmutableSurfaceDigest::verify")
            || call.ends_with("::ImmutableSurfaceDigest::compare") =>
        {
            Some(AuthorityTerm::ImmutableSurfaceDigestAdmission)
        }
        _ => None,
    };

    if let Some(term) = exact_term {
        return Some(AuthorityClass::new(
            term,
            config.status_for_site(&canonical, line_start),
        ));
    }

    if canonical.starts_with("ParentLineage::")
        || canonical.starts_with("<ParentLineage>::")
        || ufcs_authority_method(&canonical, "ParentLineage", &["admit_parent"])
        || canonical.contains("::ParentLineage::")
        || receiver_authority_method(&canonical, &["lineage", "parent"], &["admit_parent"])
    {
        return Some(AuthorityClass::blocked(AuthorityTerm::ParentLineage));
    }
    if canonical.starts_with("Crown::<Ruling>::")
        || canonical.starts_with("Crown::")
        || canonical.contains("::Crown::<Ruling>::")
        || canonical.starts_with("<Crown<Ruling>>::")
        || ufcs_authority_method(
            &canonical,
            "Crown<Ruling>",
            &["admit_authority", "admit_ruling", "admit_crown_ruling"],
        )
        || receiver_authority_method(
            &canonical,
            &["crown"],
            &["admit_authority", "admit_ruling", "admit_crown_ruling"],
        )
    {
        return Some(AuthorityClass::blocked(AuthorityTerm::CrownRuling));
    }
    if canonical.starts_with("AuthorityToken::")
        || canonical.starts_with("<AuthorityToken>::")
        || ufcs_authority_method(
            &canonical,
            "AuthorityToken",
            &["construct", "mint", "admit"],
        )
        || canonical.contains("::AuthorityToken::")
        || receiver_authority_method(
            &canonical,
            &["token", "authority"],
            &[
                "construct",
                "mint",
                "admit",
                "mint_authority_token",
                "admit_authority_token",
            ],
        )
    {
        return Some(AuthorityClass::blocked(
            AuthorityTerm::AuthorityTokenConstructor,
        ));
    }
    if canonical.starts_with("Handoff::")
        || canonical.starts_with("<Handoff>::")
        || ufcs_authority_method(
            &canonical,
            "Handoff",
            &["admit_successor_parent", "admit_successor"],
        )
        || canonical.contains("::Handoff::")
        || receiver_authority_method(
            &canonical,
            &["handoff"],
            &["admit_successor_parent", "admit_successor"],
        )
    {
        return Some(AuthorityClass::blocked(AuthorityTerm::Successor));
    }
    if canonical.starts_with("Predecessor::")
        || canonical.starts_with("<Predecessor>::")
        || ufcs_authority_method(
            &canonical,
            "Predecessor",
            &["retire_authority", "lock_authority"],
        )
        || canonical.contains("::Predecessor::")
        || receiver_authority_method(
            &canonical,
            &["predecessor"],
            &["retire_authority", "lock_authority"],
        )
    {
        return Some(AuthorityClass::blocked(AuthorityTerm::PredecessorRetired));
    }
    if canonical.starts_with("ImmutableSurfaceDigest::")
        || canonical.starts_with("<ImmutableSurfaceDigest>::")
        || ufcs_authority_method(
            &canonical,
            "ImmutableSurfaceDigest",
            &["admit", "verify", "compare"],
        )
        || canonical.contains("::ImmutableSurfaceDigest::")
        || receiver_authority_method(
            &canonical,
            &["digest", "immutablesurface"],
            &["admit", "verify", "compare"],
        )
    {
        return Some(AuthorityClass::blocked(
            AuthorityTerm::ImmutableSurfaceDigestAdmission,
        ));
    }

    None
}

fn canonical_call_name(raw_name: &str) -> String {
    raw_name.chars().filter(|ch| !ch.is_whitespace()).collect()
}

fn ufcs_authority_method(canonical: &str, type_name: &str, method_names: &[&str]) -> bool {
    let Some(rest) = canonical.strip_prefix('<') else {
        return false;
    };
    let Some((qualified_self_ty, method)) = rest.split_once(">::") else {
        return false;
    };
    if !method_names.contains(&method) {
        return false;
    }

    let canonical_type = canonical_call_name(type_name);
    let Some(self_ty_last_segment) = qualified_self_ty.rsplit("::").next() else {
        return false;
    };
    self_ty_last_segment == canonical_type
        || self_ty_last_segment.starts_with(&format!("{canonical_type}as"))
}

fn receiver_authority_method(
    canonical: &str,
    receiver_keywords: &[&str],
    method_names: &[&str],
) -> bool {
    let Some((receiver, method)) = canonical.rsplit_once('.') else {
        return false;
    };
    let receiver = receiver.to_ascii_lowercase();
    receiver_keywords
        .iter()
        .any(|keyword| receiver.contains(keyword))
        && method_names.contains(&method)
}

fn sanitize_id(input: &str) -> String {
    input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
