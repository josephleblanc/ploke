//! Prototype 1 evidence inventory schema.
//!
//! This module defines the row schema used by human-facing documentation and
//! machine-facing CLI output (`ploke-eval history inventory`).
//!
//! The inventory is a *reporting* surface: it must not upgrade authority. It
//! names evidence surfaces, where they live, how they are discovered, and what
//! their current authority treatment is.

use serde::Serialize;

use super::history_preview::EvidenceClass;

/// One inventory row describing an evidence surface (file/directory/pattern).
///
/// This is intentionally a narrow, stable schema suitable for both:
/// - markdown table rendering (humans)
/// - `--format json` output (diffing / tooling)
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct InventoryRow {
    /// Stable logical identifier for the surface (diff-friendly).
    pub(crate) id: &'static str,

    /// Prototype 1 evidence class when the surface is part of the campaign-local
    /// `prototype1/` importer inventory.
    ///
    /// External referenced evidence may not map to an `EvidenceClass`.
    pub(crate) class: Option<EvidenceClass>,

    /// One or more location patterns for the same logical surface.
    pub(crate) locations: Vec<InventoryLocation>,

    /// Record container/encoding (JSON, JSONL, directory of JSON, etc.).
    pub(crate) record_format: RecordFormat,

    /// Record schema/version identifier where known.
    pub(crate) schema: Option<&'static str>,

    /// Current authority treatment category.
    pub(crate) treatment: AuthorityTreatment,

    /// How this surface is discovered/enumerated by current read-only paths.
    pub(crate) discovery: DiscoveryMethod,

    /// Producers (writers) known to create the surface.
    pub(crate) producers: Vec<&'static str>,

    /// Consumers (readers) known to use the surface.
    pub(crate) consumers: Vec<&'static str>,

    /// Minimum provenance keys expected in the payload or recoverable from the
    /// surrounding location.
    pub(crate) provenance_keys: Vec<&'static str>,

    /// Known gaps and diagnostics notes (kept short; detailed prose belongs in
    /// the doc generator).
    pub(crate) notes: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct InventoryLocation {
    /// Which logical root the path pattern is relative to.
    pub(crate) root: InventoryRoot,
    /// A display pattern (may include placeholders like `<campaign-id>`).
    pub(crate) pattern: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InventoryRoot {
    /// `~/.ploke-eval/campaigns/<campaign-id>/prototype1/`
    CampaignPrototype1,
    /// `~/.ploke-eval/campaigns/<campaign-id>/`
    CampaignRoot,
    /// The active checkout (repo root) currently hosting `.ploke/...` state.
    ActiveCheckout,
    /// `~/.ploke-eval/instances/prototype1/<campaign-id>/...`
    InstancesPrototype1,
    /// A referenced location outside the above roots (rare; keep explicit).
    OtherExternal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RecordFormat {
    Json,
    Jsonl,
    DirOfJson,
    DirTree,
    Log,
    Archive,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AuthorityTreatment {
    /// Candidate for admission under explicit policy (not automatically authority).
    IngressCandidate,
    /// Read-only admitted evidence (preview/import boundary) with clear provenance.
    AdmittedPreview,
    /// Mutable projection; must not be treated as sealed authority.
    Projection,
    /// A catalog that primarily references other evidence surfaces.
    RefOnly,
    /// Mixed or conditional treatment (e.g. class-conditional fallbacks).
    Conditional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DiscoveryMethod {
    /// A single explicit path.
    ExplicitPath,
    /// Enumerated by scanning a directory.
    DirectoryScan,
    /// Enumerated by scanning nested per-node directories.
    NestedDirectoryScan,
    /// Recovered/inferred from other records (e.g. node id inferred from path).
    Inferred,
    /// Documented but not currently discoverable via code enumeration.
    DocumentedOnly,
}

