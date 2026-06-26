use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPart {
    pub id: Uuid,
    pub file_path: NodeFilepath,
    pub canon_path: CanonPath,
    pub ranges: Vec<(usize, usize)>,
    pub kind: ContextPartKind,
    pub text: String,
    pub score: f32,
    pub modality: Modality,
    #[serde(default)]
    pub type_context: Option<TypeContextInfo>,
    #[serde(default)]
    pub call_expansion: Option<CallExpansionInfo>,
    #[serde(default)]
    pub call_context: Vec<CallContextInfo>,
    #[serde(default)]
    pub proof_context: Vec<ProofContextInfo>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextStats {
    pub total_tokens: usize,
    pub files: usize,
    pub parts: usize,
    pub truncated_parts: usize,
    pub dedup_removed: usize,
    #[serde(default)]
    pub skipped_io_errors: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssembledContext {
    pub parts: Vec<ContextPart>,
    pub stats: ContextStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ContextPartKind {
    Code,
    Doc,
    Signature,
    Metadata,
}

impl From<ContextPartKind> for &'static str {
    fn from(v: ContextPartKind) -> Self {
        use ContextPartKind::*;
        match v {
            Code => "Code",
            Doc => "Doc",
            Signature => "Signature",
            Metadata => "Metadata",
        }
    }
}

impl ContextPartKind {
    pub fn to_static_str(self) -> &'static str {
        self.into()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Modality {
    Dense,
    Sparse,
    HybridFused,
}

impl From<Modality> for &'static str {
    fn from(v: Modality) -> Self {
        use Modality::*;
        match v {
            Dense => "Dense",
            Sparse => "Sparse",
            HybridFused => "HybridFused",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum TypeContextKind {
    SameResolvedType,
    UsesTypeNested,
    TypeDefinitionImpact,
    ImplOfTrait,
    ImplSelfType,
    AliasExpansion,
    TraitBound,
    IteratorSurface,
    ConstGenericAlias,
}

impl TypeContextKind {
    pub fn to_static_str(self) -> &'static str {
        use TypeContextKind::*;
        match self {
            SameResolvedType => "SameResolvedType",
            UsesTypeNested => "UsesTypeNested",
            TypeDefinitionImpact => "TypeDefinitionImpact",
            ImplOfTrait => "ImplOfTrait",
            ImplSelfType => "ImplSelfType",
            AliasExpansion => "AliasExpansion",
            TraitBound => "TraitBound",
            IteratorSurface => "IteratorSurface",
            ConstGenericAlias => "ConstGenericAlias",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct TypeContextInfo {
    pub seed_id: Uuid,
    pub relation: TypeContextKind,
    pub distance: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CallExpansionKind {
    OutgoingTarget,
    IncomingCaller,
}

impl CallExpansionKind {
    pub fn to_static_str(self) -> &'static str {
        match self {
            Self::OutgoingTarget => "OutgoingTarget",
            Self::IncomingCaller => "IncomingCaller",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct CallExpansionInfo {
    pub seed_id: Uuid,
    pub relation: CallExpansionKind,
    pub call_site_id: Uuid,
    pub target_id: Uuid,
    pub distance: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CallSiteKind {
    Path,
    Method,
    Dynamic,
    Macro,
}

impl CallSiteKind {
    pub fn to_static_str(&self) -> &'static str {
        match self {
            Self::Path => "Path",
            Self::Method => "Method",
            Self::Dynamic => "Dynamic",
            Self::Macro => "Macro",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CallReceiverInfo {
    SelfValue,
    SelfField {
        path: Vec<String>,
    },
    LocalBinding {
        name: String,
    },
    TypedLocalBinding {
        name: String,
        type_path: Vec<String>,
    },
    InitializedLocalBinding {
        name: String,
        init_path: Vec<String>,
    },
    BorrowedLocalBinding {
        name: String,
    },
    BorrowedTypedLocalBinding {
        name: String,
        type_path: Vec<String>,
    },
    DereferencedLocalBinding {
        name: String,
    },
    DereferencedInitializedLocalBinding {
        name: String,
        init_path: Vec<String>,
    },
    FieldLocalBinding {
        name: String,
        field_path: Vec<String>,
    },
    FieldTypedLocalBinding {
        name: String,
        type_path: Vec<String>,
        field_path: Vec<String>,
    },
    FieldInitializedLocalBinding {
        name: String,
        init_path: Vec<String>,
        field_path: Vec<String>,
    },
    PathCallResult {
        path: Vec<String>,
    },
    MethodCallResult {
        method_name: String,
    },
    AwaitResult,
    AwaitPathCallResult {
        path: Vec<String>,
    },
    TryResult,
    TryPathCallResult {
        path: Vec<String>,
    },
    Literal,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CallCalleeInfo {
    Path {
        path: Vec<String>,
    },
    Method {
        name: String,
        receiver: Option<CallReceiverInfo>,
    },
    Macro {
        name: String,
    },
    Dynamic,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CallTargetKind {
    Function,
    DynamicFunction,
    Method,
    AssociatedFunction,
    TupleStructConstructor,
    EnumVariantConstructor,
}

impl CallTargetKind {
    pub fn to_static_str(&self) -> &'static str {
        match self {
            Self::Function => "Function",
            Self::DynamicFunction => "DynamicFunction",
            Self::Method => "Method",
            Self::AssociatedFunction => "AssociatedFunction",
            Self::TupleStructConstructor => "TupleStructConstructor",
            Self::EnumVariantConstructor => "EnumVariantConstructor",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct CallTargetInfo {
    pub target_id: Uuid,
    pub relation: CallTargetKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CallStatusKind {
    Resolved,
    Unresolved,
    Ambiguous,
    External,
    Unsupported,
}

impl CallStatusKind {
    pub fn to_static_str(&self) -> &'static str {
        match self {
            Self::Resolved => "Resolved",
            Self::Unresolved => "Unresolved",
            Self::Ambiguous => "Ambiguous",
            Self::External => "External",
            Self::Unsupported => "Unsupported",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CallResolutionKind {
    LocalExact,
}

impl CallResolutionKind {
    pub fn to_static_str(&self) -> &'static str {
        match self {
            Self::LocalExact => "LocalExact",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct CallContextInfo {
    pub site_id: Uuid,
    pub kind: CallSiteKind,
    pub span: (u32, u32),
    pub callee: CallCalleeInfo,
    pub status: CallStatusKind,
    #[serde(default)]
    pub resolution: Option<CallResolutionKind>,
    #[serde(default)]
    pub targets: Vec<CallTargetInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialOrd, Ord, Hash, PartialEq)]
pub struct ProofContextInfo {
    pub fact_id: String,
    pub kind: String,
    #[serde(default)]
    pub build_domain_id: Option<String>,
    #[serde(default)]
    pub call_site_id: Option<String>,
    #[serde(default)]
    pub call_edge_id: Option<String>,
    #[serde(default)]
    pub caller_def_id: Option<String>,
    #[serde(default)]
    pub callee_def_id: Option<String>,
    #[serde(default)]
    pub resolution_state: Option<String>,
    #[serde(default)]
    pub resolved_def_id: Option<String>,
    #[serde(default)]
    pub candidate_def_ids: Vec<String>,
    #[serde(default)]
    pub external_summary_id: Option<String>,
    #[serde(default)]
    pub evidence_use: Option<String>,
    #[serde(default)]
    pub source_file: Option<String>,
    #[serde(default)]
    pub start_byte: Option<u32>,
    #[serde(default)]
    pub end_byte: Option<u32>,
    #[serde(default)]
    pub line_start: Option<u32>,
    #[serde(default)]
    pub line_end: Option<u32>,
    #[serde(default)]
    pub effect_class: Option<String>,
    #[serde(default)]
    pub blocker_reason: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
}

impl Modality {
    pub fn to_static_str(self) -> &'static str {
        self.into()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestCodeContextArgs {
    pub search_term: String,
    #[serde(default, alias = "token_budget")]
    pub token_budget_per_result: Option<u32>,
    #[serde(default)]
    pub token_budget_total: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestCodeContextResult {
    pub ok: bool,
    pub search_term: String,
    pub top_k: usize,
    pub kind: ContextPartKind,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub next_steps: Vec<String>,
    pub context: Vec<ConciseContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssembledMeta {
    pub search_term: String,
    pub top_k: usize,
    pub kind: ContextPartKind,
}

impl RequestCodeContextResult {
    pub fn from_assembled(parts: Vec<ContextPart>, m: AssembledMeta) -> Self {
        let context: Vec<ConciseContext> = parts.into_iter().map(ConciseContext::from).collect();
        Self {
            ok: true,
            search_term: m.search_term,
            top_k: m.top_k,
            kind: m.kind,
            note: None,
            next_steps: Vec::new(),
            context,
        }
    }
}

impl From<ContextPart> for ConciseContext {
    fn from(value: ContextPart) -> Self {
        Self {
            id: value.id,
            file_path: value.file_path.clone(),
            canon_path: value.canon_path.clone(),
            snippet: value.text,
            type_context: value.type_context,
            call_expansion: value.call_expansion,
            call_context: value.call_context,
            proof_context: value.proof_context,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialOrd, Ord, Hash, PartialEq)]
#[serde(transparent)]
pub struct NodeFilepath(pub String);

impl AsRef<str> for NodeFilepath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl NodeFilepath {
    pub fn new(s: String) -> Self {
        Self(s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialOrd, Ord, Hash, PartialEq)]
#[serde(transparent)]
pub struct CanonPath(pub String);

impl AsRef<str> for CanonPath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl CanonPath {
    pub fn new(s: String) -> Self {
        Self(s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, PartialOrd, Ord)]
pub struct ConciseContext {
    pub id: Uuid,
    pub file_path: NodeFilepath,
    pub canon_path: CanonPath,
    pub snippet: String,
    #[serde(default)]
    pub type_context: Option<TypeContextInfo>,
    #[serde(default)]
    pub call_expansion: Option<CallExpansionInfo>,
    #[serde(default)]
    pub call_context: Vec<CallContextInfo>,
    #[serde(default)]
    pub proof_context: Vec<ProofContextInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetFileMetadataResult {
    pub ok: bool,
    pub file_path: String,
    pub exists: bool,
    pub byte_len: u64,
    pub modified_ms: Option<i64>,
    pub file_hash: String,
    pub tracking_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyCodeEditResult {
    pub ok: bool,
    /// Number of edits staged into an EditProposal
    pub staged: usize,
    /// Number of edits applied immediately (0 unless auto-confirm is enabled and synchronous)
    pub applied: usize,
    /// Display-friendly file paths included in this proposal
    pub files: Vec<String>,
    /// Preview mode used for the summary ("diff" or "codeblock")
    pub preview_mode: String,
    /// Whether auto-confirm is enabled in config (application may proceed asynchronously)
    pub auto_confirmed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFileResult {
    pub ok: bool,
    /// Number of creations staged
    pub staged: usize,
    /// Number of creations applied immediately (0 unless auto-confirm is enabled and synchronous)
    pub applied: usize,
    /// Display-friendly file paths included in this proposal
    pub files: Vec<String>,
    /// Preview mode used for the summary ("diff" or "codeblock")
    pub preview_mode: String,
    /// Whether auto-confirm is enabled in config (application may proceed asynchronously)
    pub auto_confirmed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadFileResult {
    pub ok: bool,
    pub file_path: String,
    pub exists: bool,
    pub byte_len: u64,
    pub content: String,
}

#[cfg(test)]
mod read_file_result_tests {
    use super::*;

    #[test]
    fn serde_roundtrip_utf8() {
        let v = ReadFileResult {
            ok: true,
            file_path: "/tmp/test.rs".to_string(),
            exists: true,
            byte_len: 12,
            content: "fn main(){}".to_string(),
        };
        let s = serde_json::to_string(&v).expect("serialize");
        let de: ReadFileResult = serde_json::from_str(&s).expect("deserialize");
        assert!(de.ok);
        assert_eq!(de.file_path, v.file_path);
        assert!(de.exists);
        assert_eq!(de.byte_len, 12u64);
        assert_eq!(de.content, v.content);
    }

    #[test]
    fn serde_roundtrip_base64() {
        let v = ReadFileResult {
            ok: true,
            file_path: "/tmp/a.bin".to_string(),
            exists: true,
            byte_len: 4,
            content: "AAECAw==".to_string(),
        };
        let s = serde_json::to_string(&v).expect("serialize");
        let de: ReadFileResult = serde_json::from_str(&s).expect("deserialize");
        assert!(de.ok);
        assert_eq!(de.byte_len, 4u64);
    }
}
