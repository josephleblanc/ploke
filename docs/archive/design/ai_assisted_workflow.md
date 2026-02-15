# AI-Assisted Development Workflow
*Guiding Principles for Human-AI Collaboration in Rust Systems Development*  
**Version**: 0.1.0-alpha  
**Last Updated**: 2025-03-17  

## Document Structure (IDIOMATIC_RUST-CRATE-DOC)
This document follows Rust API guidelines for documentation with:  
- Hierarchical headers mirroring [PROPOSED_FILE_ARCH1.md](#)  
- Cross-linked sections using anchor tags  
- Embedded code examples with `?` error handling  
- Hyperlinks to [IDIOMATIC_RUST.md](./ai_workflow/AI_Always_Instructions/IDIOMATIC_RUST.md) standards  

```rust
/// Documentation example following C-LINK guideline
//! #[doc = "See [Architecture Decisions](#architecture-decisions)"]
```

### Core Terminology Glossary
| Term                | Definition                                                                 | Source Reference          |
|---------------------|---------------------------------------------------------------------------|---------------------------|
| Type Stamp          | Temporal UUIDv7 identifier for code versions                              | PROPOSED_FILE_ARCH1.md    |
| Content Hash        | Blake3 hash of AST nodes for content addressing                           | syn_parser::parser::graph |
| VectorGraphDB       | Hybrid CozoDB storage combining HNSW indexes and graph relations          | PROPOSED_FILE_ARCH1.md    |
| AI-Human Interface  | Protocol governing LLM/human collaboration patterns                       | [AI/Human Interface](#ai-human-interface-patterns) |
| Embedding Provenance| Cryptographic trail ensuring model/output correspondence                  | [Security Amplification](#security-amplification) |

### Decision Tracking Methodology
1. **ADR Template** (Architecture Decision Record):
```markdown
# ADR-000: [Short Title]

## Status
PROPOSED | ACCEPTED | DEPRECATED

## Context
[Problem description]

## Decision
[Chosen solution]

## Consequences
- Positive:  
- Negative:  
- Neutral:  

## Compliance
[IDIOMATIC_RUST.md](#) Sections:  
[CONVENTIONS.md](#) Items:  
```

2. **Version Control Strategy**:
- Design documents reside in `/design` directory
- SemVer format: MAJOR.MINOR.PATCH
  - MAJOR: Breaking architectural changes
  - MINOR: New workflow patterns
  - PATCH: Corrections/typos
- ADRs stored in `/design/adrs` numbered sequentially
- Git tags correspond to document versions

### AI/Human Interface Patterns (Phase 1.5)

#### Feedback Loop Protocol - Conceptual Framework
The collaboration workflow between developer and AI follows a structured review cycle to maintain code quality while enabling rapid iteration. Key components include:

1. **Proposal Generation**: AI generates suggestions with embedded rationales
2. **Annotated Review**: Developer provides targeted feedback
3. **Revision Tracking**: Managed via cryptographic hashes
4. **Architecture Alignment**: Direct links to ADRs and design docs

#### Feedback Loop Protocol - Design Specification

/////////////////////////////////////////////////////////////
/// Design Specification: Code Review Workflow
/// Status: Proposed (RFC-1)
/// Future Implementation Path: crates/interface/src/collab.rs
/////////////////////////////////////////////////////////////

/// NOTIONAL: Code Review Workflow Structure
/// Purpose: Define human-AI collaboration protocol
#[derive(Debug)]
pub struct CodeReviewCycle<'a> {  // Lifetime enforces scope boundaries
    /// Temporal identifier (ADR-004 format)
    id: uuid::Uuid,
    
    /// Affected components from PROPOSED_FILE_ARCH1 architecture
    #[serde(rename = "TargetCrates")]
    pub crates: Vec<String>,
    
    /// Machine-readable validation requirements
    #[serde(rename = "ComplianceContract")]
    pub requirements: std::collections::HashMap<String, String>,
    
    /// Human-AI interaction history
    #[serde(rename = "DecisionTrail")]
    pub annotations: Vec<CodeAnnotation>,
}

/// Audit trail entry matching CONVENTIONS.md error handling
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct CodeAnnotation {
    /// Line references use ContentHash identifiers
    pub locations: Vec<String>,
    
    /// Categorized per Phase 9 Rust safeguards
    pub category: AnnotationCategory,
    
    /// Structured feedback preserving context
    pub comment: StructuredComment,
}

/// Enforces prioritized code review process
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum AnnotationCategory {
    TypeSafetyConcern,
    PerformanceImpact,
    ApiContractViolation,
    IdiosyncraticPreference,
}

/// Comment structure preventing ambiguous feedback
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct StructuredComment {
    /// Problem statement using RFC-2119 keywords
    pub observation: String,
    
    /// Suggested solution in patch format
    pub suggestion: String,
    
    /// Relative importance ranking
    #[serde(rename = "CriticalityLevel")]
    pub level: u8,
}

#### Explainability Requirements - Conceptual Framework
AI-generated proposals must provide machine-readable rationales linked to project standards. Key aspects include:

1. **Architecture Alignment**: Direct references to IDIOMATIC_RUST.md sections
2. **Performance Projections**: Estimated memory/throughput impacts
3. **Safety Analysis**: Thread safety and lifetime annotations
4. **Alternative Options**: Ordered list of considered alternatives

#### Explainability Requirements - Design Specification

/////////////////////////////////////////////////////////////
/// Design Specification: AI Rationale Format
/// Status: Draft
/// Future Implementation Path: crates/interface/src/rationale.rs
/////////////////////////////////////////////////////////////

/// NOTIONAL: LLM Decision Transparency
/// Implements IDIOMATIC_RUST C-FAILURE requirements
#[derive(Debug)]
struct AiRationale<'a> {  // Lifetime prevents static assumptions
    /// CONVENTIONS.md cross-refs (C-LINK compliant)
    conventions: Vec<&'a str>,
    
    /// Memory/performance estimates in MB/ns
    #[serde(rename = "ResourceProjections")]
    pub resources: ResourceEstimates,
    
    /// Thread safety and ownership guarantees
    #[serde(rename = "ConcurrencyProfile")]
    pub concurrency: ConcurrencyClaims,
    
    /// Ordered alternatives with rejection reasons  
    #[serde(rename = "ConsideredOptions")]
    pub alternatives: Vec<DesignOption>,
}

/// Hardware-aware performance predictions
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct ResourceEstimates {
    /// Worst-case memory consumption (MB)
    pub memory_ceiling: f32,
    /// 95th percentile operation latency (ns)
    pub latency_p95: u64,
    /// Projected GPU utilization ratio
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_util: Option<f32>,
}

/// Concurrency safety declarations
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct ConcurrencyClaims {
    /// Implements Send/Sync where appropriate
    pub send_sync_compliance: bool,
    /// Lifetime elision validation
    pub lifetime_safety: LifetimeStatus,
    /// Deadlock risk assessment
    pub deadlock_risk: RiskLevel,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum LifetimeStatus {
    FullyElidedSafe,
    ExplicitAnnotationsRequired,
    PotentialDanglingRisk,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")] 
pub enum RiskLevel {
    Negligible,
    Mitigated,
    RequiresReview,
}

/// Alternative design option documentation
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct DesignOption {
    /// Concrete type/methodology name
    pub option_name: String,
    /// RFC-style rejection rationale
    pub rejection_reason: String,
    /// Specific constraint violations
    pub constraint_violations: Vec<String>,
}

#### Cognitive Load Management - Conceptual Framework
Solo developers using AI assistance require rigorous attention management to prevent burnout and maintain code quality. Our strategy includes:

1. **Time Boxing**: Strict limits on continuous focus periods
2. **Context Isolation**: Domain-specific work sessions
3. **Priority Stacking**: Risk-weighted task ordering
4. **Recovery Buffers**: Mandatory break intervals

#### Cognitive Load Management - Design Specification

/////////////////////////////////////////////////////////////
/// Design Specification: Attention Management
/// Status: Draft
/// Future Implementation Path: crates/interface/src/cognitive.rs
/////////////////////////////////////////////////////////////

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct CognitivePolicy {
    /// Maximum continuous work period (minutes)
    #[serde(rename = "MaxFocusDuration")]
    pub focus_duration: u32,
    
    /// Minimum break between work sessions
    #[serde(rename = "MinRecoveryTime")]
    pub recovery_time: u32,
    
    /// Domain-specific time allocations
    #[serde(rename = "DomainBudgets")]
    pub budgets: HashMap<WorkDomain, DomainPolicy>,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema, Hash, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum WorkDomain {
    CodeIngestion,
    GraphTraversal,
    ModelInference,
    SecurityValidation,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct DomainPolicy {
    /// Time allocation in minutes
    pub time_budget: u32,
    /// Complexity multiplier (1.0 = baseline)
    pub complexity_factor: f32,
    /// Allowed context switches
    pub max_context_switches: u8,
}

### Document Index Infrastructure (Phase 1.6)

#### Taxonomy Framework - Conceptual Overview
The AI-assisted workflow taxonomy provides standardized classification for all development artifacts. Key aspects:

1. **Categorical Dimensions**:
   - Criticality (Safety Critical, Best Effort)
   - Maturity (Experimental, Stable, Deprecated)
   - Complexity (C0-C5 based on cognitive load)
2. **Hierarchical Tagging**: Taxonomies applied at crate/module/function levels
3. **Automated Analysis**: CozoDB-powered tag propagation

#### Taxonomy Framework - Design Specification

/////////////////////////////////////////////////////////////
/// Design Specification: Workflow Taxonomy
/// Status: Proposed
/// Future Implementation Path: crates/interface/src/taxonomy.rs
/////////////////////////////////////////////////////////////

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DevelopmentCriticality {
    SafetyCritical,
    OperationalEssential,
    BestEffort,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MaturityLevel {
    Experimental,  // No stability guarantees
    Provisional,   // API surface may change
    Stable,        // Following semver
    Deprecated,    // Scheduled for removal
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ComplexityTier {
    C0, // Trivial (e.g. getter methods)
    C1, // Simple control flow
    C2, // Basic algorithms 
    C3, // Nested conditionals
    C4, // Concurrency patterns
    C5, // Unsafe/FFI boundaries
}

/// Unified taxonomy tagging system
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct CodeArtifactTags {
    /// Safety impact classification
    pub criticality: DevelopmentCriticality,
    /// Stability guarantees
    pub maturity: MaturityLevel,
    /// Cognitive load estimate
    pub complexity: ComplexityTier,
    /// Linked ADR identifiers
    pub adr_links: Vec<uuid::Uuid>,
}

#### Cross-Document Linking - Implementation Strategy
1. **Anchor Schemas**: Protobuf-like message numbering
2. **Content Addressing**: Blake3 hash-based fragment identifiers
3. **Version Propagation**: Semantic version pins in URL paths
4. **Automated Indexes**: Generated via CozoDB queries

Example cross-link structure:
`/design/v0.1.0-α#hash=9aeb8f3.../section=3.2.1`

Link validation pseudocode:
```rust
fn validate_crosslink(link: &str) -> Result<VerifiedLink> {
    let parts: Vec<&str> = link.split('#').collect();
    let version = verify_semver(parts[0])?;
    let hash = blake3::Hash::from_hex(parts[1])?;
    let content = get_content_by_hash(hash)?;
    Ok(VerifiedLink::new(version, content))
}

#### Automated Index Generation - Conceptual Framework
The self-maintaining document index system ensures design decisions remain connected to implementation. Core features:

1. **Embedding Integration**: Hybrid HNSW indexes combining text and code embeddings
2. **Taxonomy-Driven Clustering**: Automatic grouping using CodeArtifactTags
3. **Real-Time Updates**: CDC pipelines from CozoDB to vector store
4. **Versioned Indexes**: Semantic version isolation for historical queries

#### Automated Index Generation - Design Specification

/////////////////////////////////////////////////////////////
/// Design Specification Code Block (Protocol Definition Only)
/// Purpose: Document index maintenance strategy pattern
/// Relation to Codebase: Will inform future crates/indexing
/// Update Protocol: Requires ADR for breaking changes
/////////////////////////////////////////////////////////////

/// Example CozoDB schema representation demonstrating
/// the index structure, not actual implementation code
::create design_index {
    content_hash: String,    // Blake3 hash
    semantic_version: String, // SemVer format
    embedding: <F32; 384>,   // Matching PROPOSED_FILE_ARCH1
    relationships: [String], // ADR links
    artifact_type: String,   // Serialized CodeArtifactTags
}

/// Hypothetical HNSW configuration showing indexing strategy
/// Note: Actual parameters will be finalized during implementation
::hnsw create doc_hnsw_idx:design_index {
    dim: 384,
    dtype: F32,
    fields: [embedding],
    filter_fields: [semantic_version, artifact_type],
    distance: Cosine,
}

/// Notional maintenance pattern - illustrates update cadence
::update-index doc_hnsw_idx [
    $hash <- [?hash, ?version, ?embed, ?rels, ?tags]
    ?[hash, version, embed, rels, tags] := ~design_index{hash, version, embed, rels, tags}
] {
    "interval": "15m",  // Batch update interval
    "version_policy": { 
        "retain": ["current", "previous"]
    }
}

/// Rust type sketch for index entries
/// Demonstrates serialization pattern, not final implementation
#[derive(Debug, serde::Serialize)]
pub struct IndexEntry {
    #[serde(rename = "contentHash")]
    pub content_hash: String,
    #[serde(rename = "semanticVersion")]
    pub version: String,
    pub embedding: Vec<f32>,
    pub relationships: Vec<String>,
    #[serde(flatten)]
    pub artifact_type: CodeArtifactTags,
}
```
