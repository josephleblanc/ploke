//! Corpus-backed call-graph coverage matrix.
//!
//! This mirrors `type_shape_matrix`: the data lives in `test-utils`, while DB,
//! RAG, and TUI layers decide which rows they can materialize.

use ploke_db::{CallRelationKind, CallStatusKind, CallTargetKind};
use uuid::Uuid;

use crate::{
    CORPUS_AXUM_CALL_GRAPH, CORPUS_CHRONO_CALL_GRAPH, CORPUS_GENERIC_ARRAY_CALL_GRAPH,
    CORPUS_MEMCHR_CALL_GRAPH, FixtureDb,
    proof_fact_fixtures::{
        generic_array_iter_summary_blocker, generic_array_size_hint_guard_blocker,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallShapeKind {
    FreeFunctionPath,
    AliasConstructorPath,
    ModuleQualifiedConstructorPath,
    GeneratedConstructorFrontier,
    DynamicCallableField,
    FunctionPointerField,
    CallableTraitObjectField,
    SelfFieldMethod,
    LocalReceiverMethod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallCorpusFixture {
    Axum,
    Chrono,
    Memchr,
    GenericArray,
}

impl CallCorpusFixture {
    pub fn fixture(self) -> &'static FixtureDb {
        match self {
            Self::Axum => &CORPUS_AXUM_CALL_GRAPH,
            Self::Chrono => &CORPUS_CHRONO_CALL_GRAPH,
            Self::Memchr => &CORPUS_MEMCHR_CALL_GRAPH,
            Self::GenericArray => &CORPUS_GENERIC_ARRAY_CALL_GRAPH,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallOwnerSelector {
    FunctionInModule {
        module_path: &'static [&'static str],
        name: &'static str,
    },
    MethodByBody {
        name: &'static str,
        body: &'static str,
        owner_type: Option<&'static str>,
        owner_trait: Option<&'static str>,
    },
    MethodByBodyFile {
        name: &'static str,
        body: &'static str,
        file_suffix: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallSiteSelector {
    Path {
        segments: &'static [&'static str],
        arg_count: Option<u32>,
    },
    Dynamic {
        arg_count: Option<u32>,
    },
    Method {
        name: &'static str,
        arg_count: Option<u32>,
        receiver: Option<CallReceiverSelector>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallReceiverSelector {
    SelfField {
        path: &'static [&'static str],
    },
    MethodResultLocalBinding {
        method_name: &'static str,
    },
    MethodResultField {
        method_name: &'static str,
        field_path: &'static [&'static str],
    },
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallTargetSelector {
    FunctionByName {
        name: &'static str,
    },
    FunctionInModule {
        module_path: &'static [&'static str],
        name: &'static str,
    },
    MethodByBody {
        name: &'static str,
        body: &'static str,
        owner_type: &'static str,
    },
    Struct {
        name: &'static str,
    },
    Variant {
        enum_name: &'static str,
        variant_name: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallExpected {
    Resolved {
        target: CallTargetSelector,
        relation: CallRelationKind,
        target_kind: CallTargetKind,
        edge_count: usize,
    },
    AmbiguousCandidates {
        candidates: &'static [CallTargetSelector],
        relation: CallRelationKind,
        target_kind: CallTargetKind,
    },
    Targetless {
        status: CallStatusKind,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallPipelineCoverage {
    Db,
    RagApi,
    RagExactApi,
    TuiTool,
}

#[derive(Debug, Clone, Copy)]
pub struct CallShapeCase {
    pub name: &'static str,
    pub kind: CallShapeKind,
    pub fixture: CallCorpusFixture,
    pub source: &'static str,
    pub owner: CallOwnerSelector,
    pub site: CallSiteSelector,
    pub expected: CallExpected,
    pub coverage: &'static [CallPipelineCoverage],
}

pub const fn call_shape_cases() -> &'static [CallShapeCase] {
    CALL_SHAPE_CASES
}

pub fn call_shape_case_blocker_reasons(case: &CallShapeCase) -> &'static [&'static str] {
    if is_unsupported_generic_array_size_hint_case(case) {
        &[
            "type_resolution_missing",
            "external_dependency_summary_missing",
        ]
    } else {
        &[]
    }
}

pub fn call_shape_case_proof_blockers(
    case: &CallShapeCase,
    call_site_id: Uuid,
) -> Vec<serde_json::Value> {
    if is_unsupported_generic_array_size_hint_case(case) {
        vec![
            generic_array_size_hint_guard_blocker(call_site_id),
            generic_array_iter_summary_blocker(call_site_id),
        ]
    } else {
        Vec::new()
    }
}

fn is_generic_array_size_hint_case(case: &CallShapeCase) -> bool {
    matches!(
        case.name,
        "generic_array_try_from_iter_size_hint_local_receiver"
            | "generic_array_try_from_fallible_iter_size_hint_local_receiver"
    )
}

fn is_unsupported_generic_array_size_hint_case(case: &CallShapeCase) -> bool {
    is_generic_array_size_hint_case(case)
        && matches!(
            case.expected,
            CallExpected::Targetless {
                status: CallStatusKind::Unsupported
            }
        )
}

const MEMCHR_SEARCHER_CALL_CANDIDATES: &[CallTargetSelector] = &[
    CallTargetSelector::FunctionByName {
        name: "searcher_kind_empty",
    },
    CallTargetSelector::FunctionByName {
        name: "searcher_kind_one_byte",
    },
    CallTargetSelector::FunctionByName {
        name: "searcher_kind_two_way",
    },
    CallTargetSelector::FunctionByName {
        name: "searcher_kind_two_way_with_prefilter",
    },
    CallTargetSelector::FunctionByName {
        name: "searcher_kind_sse2",
    },
    CallTargetSelector::FunctionByName {
        name: "searcher_kind_avx2",
    },
];

const MEMCHR_PREFILTER_CALL_CANDIDATES: &[CallTargetSelector] = &[
    CallTargetSelector::FunctionByName {
        name: "prefilter_kind_fallback",
    },
    CallTargetSelector::FunctionByName {
        name: "prefilter_kind_sse2",
    },
    CallTargetSelector::FunctionByName {
        name: "prefilter_kind_avx2",
    },
];

static CALL_SHAPE_CASES: &[CallShapeCase] = &[
    CallShapeCase {
        name: "axum_explicit_crate_path_parse_attrs",
        kind: CallShapeKind::FreeFunctionPath,
        fixture: CallCorpusFixture::Axum,
        source: "axum-macros/src/typed_path.rs:23 crate::attr_parsing::parse_attrs(...)",
        owner: CallOwnerSelector::FunctionInModule {
            module_path: &["crate", "typed_path"],
            name: "expand",
        },
        site: CallSiteSelector::Path {
            segments: &["crate", "attr_parsing", "parse_attrs"],
            arg_count: Some(2),
        },
        expected: CallExpected::Resolved {
            target: CallTargetSelector::FunctionInModule {
                module_path: &["crate", "attr_parsing"],
                name: "parse_attrs",
            },
            relation: CallRelationKind::Function,
            target_kind: CallTargetKind::Function,
            edge_count: 1,
        },
        coverage: &[
            CallPipelineCoverage::Db,
            CallPipelineCoverage::RagApi,
            CallPipelineCoverage::TuiTool,
        ],
    },
    CallShapeCase {
        name: "chrono_mapped_local_time_single_alias",
        kind: CallShapeKind::AliasConstructorPath,
        fixture: CallCorpusFixture::Chrono,
        source: "chrono/src/offset/mod.rs:143 MappedLocalTime::Single(f(v))",
        owner: CallOwnerSelector::MethodByBodyFile {
            name: "map",
            body: "MappedLocalTime::Single(f(v))",
            file_suffix: "src/offset/mod.rs",
        },
        site: CallSiteSelector::Path {
            segments: &["MappedLocalTime", "Single"],
            arg_count: Some(1),
        },
        expected: CallExpected::Resolved {
            target: CallTargetSelector::Variant {
                enum_name: "LocalResult",
                variant_name: "Single",
            },
            relation: CallRelationKind::EnumVariantConstructor,
            target_kind: CallTargetKind::Variant,
            edge_count: 1,
        },
        coverage: &[
            CallPipelineCoverage::Db,
            CallPipelineCoverage::RagApi,
            CallPipelineCoverage::TuiTool,
        ],
    },
    CallShapeCase {
        name: "axum_private_serve_future_constructor",
        kind: CallShapeKind::ModuleQualifiedConstructorPath,
        fixture: CallCorpusFixture::Axum,
        source: "axum/src/serve/mod.rs:389 private::ServeFuture(Box::pin(async move { self.run().await }))",
        owner: CallOwnerSelector::MethodByBody {
            name: "into_future",
            body: "private::ServeFuture(Box::pin(async move { self.run().await }))",
            owner_type: Some("Serve"),
            owner_trait: Some("IntoFuture"),
        },
        site: CallSiteSelector::Path {
            segments: &["private", "ServeFuture"],
            arg_count: Some(1),
        },
        expected: CallExpected::Resolved {
            target: CallTargetSelector::Struct {
                name: "ServeFuture",
            },
            relation: CallRelationKind::TupleStructConstructor,
            target_kind: CallTargetKind::Struct,
            edge_count: 1,
        },
        coverage: &[CallPipelineCoverage::Db, CallPipelineCoverage::RagApi],
    },
    CallShapeCase {
        name: "axum_generated_into_service_future_new",
        kind: CallShapeKind::GeneratedConstructorFrontier,
        fixture: CallCorpusFixture::Axum,
        source: "axum/src/handler/service.rs:174 super::future::IntoServiceFuture::new(future)",
        owner: CallOwnerSelector::MethodByBody {
            name: "call",
            body: "IntoServiceFuture::new(future)",
            owner_type: Some("HandlerService"),
            owner_trait: Some("Service<Request>"),
        },
        site: CallSiteSelector::Path {
            segments: &["super", "future", "IntoServiceFuture", "new"],
            arg_count: Some(1),
        },
        expected: CallExpected::Resolved {
            target: CallTargetSelector::MethodByBody {
                name: "new",
                body: "Self { future }",
                owner_type: "IntoServiceFuture",
            },
            relation: CallRelationKind::AssociatedFunction,
            target_kind: CallTargetKind::Method,
            edge_count: 1,
        },
        coverage: &[
            CallPipelineCoverage::Db,
            CallPipelineCoverage::RagApi,
            CallPipelineCoverage::TuiTool,
        ],
    },
    CallShapeCase {
        name: "axum_listener_tap_fn_dynamic_field",
        kind: CallShapeKind::DynamicCallableField,
        fixture: CallCorpusFixture::Axum,
        source: "axum/src/serve/listener.rs:236 (self.tap_fn)(&mut io)",
        owner: CallOwnerSelector::MethodByBody {
            name: "accept",
            body: "(self.tap_fn)(&mut io)",
            owner_type: Some("TapIo"),
            owner_trait: None,
        },
        site: CallSiteSelector::Dynamic { arg_count: Some(1) },
        expected: CallExpected::Targetless {
            status: CallStatusKind::Unsupported,
        },
        coverage: &[
            CallPipelineCoverage::Db,
            CallPipelineCoverage::RagApi,
            CallPipelineCoverage::TuiTool,
        ],
    },
    CallShapeCase {
        name: "memchr_searcher_function_pointer_field",
        kind: CallShapeKind::FunctionPointerField,
        fixture: CallCorpusFixture::Memchr,
        source: "memchr/src/memmem/searcher.rs:222 (self.call)(self, prestate, haystack, needle)",
        owner: CallOwnerSelector::MethodByBody {
            name: "find",
            body: "(self.call)(self, prestate, haystack, needle)",
            owner_type: Some("Searcher"),
            owner_trait: None,
        },
        site: CallSiteSelector::Dynamic { arg_count: Some(4) },
        expected: CallExpected::AmbiguousCandidates {
            candidates: MEMCHR_SEARCHER_CALL_CANDIDATES,
            relation: CallRelationKind::DynamicFunction,
            target_kind: CallTargetKind::Function,
        },
        coverage: &[
            CallPipelineCoverage::Db,
            CallPipelineCoverage::RagApi,
            CallPipelineCoverage::TuiTool,
        ],
    },
    CallShapeCase {
        name: "memchr_prefilter_function_pointer_field",
        kind: CallShapeKind::FunctionPointerField,
        fixture: CallCorpusFixture::Memchr,
        source: "memchr/src/memmem/searcher.rs:718 (self.call)(self, haystack)",
        owner: CallOwnerSelector::MethodByBody {
            name: "find",
            body: "(self.call)(self, haystack)",
            owner_type: Some("Prefilter"),
            owner_trait: None,
        },
        site: CallSiteSelector::Dynamic { arg_count: Some(2) },
        expected: CallExpected::AmbiguousCandidates {
            candidates: MEMCHR_PREFILTER_CALL_CANDIDATES,
            relation: CallRelationKind::DynamicFunction,
            target_kind: CallTargetKind::Function,
        },
        coverage: &[
            CallPipelineCoverage::Db,
            CallPipelineCoverage::RagApi,
            CallPipelineCoverage::TuiTool,
        ],
    },
    CallShapeCase {
        name: "memchr_runner_fwd_boxed_fnmut_field",
        kind: CallShapeKind::CallableTraitObjectField,
        fixture: CallCorpusFixture::Memchr,
        source: "memchr/src/tests/substring/mod.rs:94 fwd(t.haystack.as_bytes(), t.needle.as_bytes())",
        owner: CallOwnerSelector::MethodByBodyFile {
            name: "run",
            body: "fwd(t.haystack.as_bytes(), t.needle.as_bytes())",
            file_suffix: "src/tests/substring/mod.rs",
        },
        site: CallSiteSelector::Path {
            segments: &["fwd"],
            arg_count: Some(2),
        },
        expected: CallExpected::Targetless {
            status: CallStatusKind::Unsupported,
        },
        coverage: &[
            CallPipelineCoverage::Db,
            CallPipelineCoverage::RagApi,
            CallPipelineCoverage::TuiTool,
        ],
    },
    CallShapeCase {
        name: "memchr_runner_rev_boxed_fnmut_field",
        kind: CallShapeKind::CallableTraitObjectField,
        fixture: CallCorpusFixture::Memchr,
        source: "memchr/src/tests/substring/mod.rs:110 rev(t.haystack.as_bytes(), t.needle.as_bytes())",
        owner: CallOwnerSelector::MethodByBodyFile {
            name: "run",
            body: "fwd(t.haystack.as_bytes(), t.needle.as_bytes())",
            file_suffix: "src/tests/substring/mod.rs",
        },
        site: CallSiteSelector::Path {
            segments: &["rev"],
            arg_count: Some(2),
        },
        expected: CallExpected::Targetless {
            status: CallStatusKind::Unsupported,
        },
        coverage: &[
            CallPipelineCoverage::Db,
            CallPipelineCoverage::RagApi,
            CallPipelineCoverage::TuiTool,
        ],
    },
    CallShapeCase {
        name: "chrono_parse_next_item_queue_is_empty_self_field",
        kind: CallShapeKind::SelfFieldMethod,
        fixture: CallCorpusFixture::Chrono,
        source: "chrono/src/format/strftime.rs:635 self.queue.is_empty()",
        owner: CallOwnerSelector::MethodByBody {
            name: "parse_next_item",
            body: "self.queue.is_empty()",
            owner_type: Some("StrftimeItems"),
            owner_trait: None,
        },
        site: CallSiteSelector::Method {
            name: "is_empty",
            arg_count: Some(0),
            receiver: Some(CallReceiverSelector::SelfField { path: &["queue"] }),
        },
        expected: CallExpected::Targetless {
            status: CallStatusKind::External,
        },
        coverage: &[
            CallPipelineCoverage::Db,
            CallPipelineCoverage::RagExactApi,
            CallPipelineCoverage::TuiTool,
        ],
    },
    CallShapeCase {
        name: "generic_array_try_from_iter_size_hint_local_receiver",
        kind: CallShapeKind::LocalReceiverMethod,
        fixture: CallCorpusFixture::GenericArray,
        source: "generic-array/src/lib.rs:1239 iter.size_hint()",
        owner: CallOwnerSelector::MethodByBody {
            name: "try_from_iter",
            body: "match iter.size_hint()",
            owner_type: Some("GenericArray"),
            owner_trait: None,
        },
        site: CallSiteSelector::Method {
            name: "size_hint",
            arg_count: Some(0),
            receiver: Some(CallReceiverSelector::MethodResultLocalBinding {
                method_name: "into_iter",
            }),
        },
        expected: CallExpected::Targetless {
            status: CallStatusKind::External,
        },
        coverage: &[
            CallPipelineCoverage::Db,
            CallPipelineCoverage::RagApi,
            CallPipelineCoverage::TuiTool,
        ],
    },
    CallShapeCase {
        name: "generic_array_try_from_fallible_iter_size_hint_local_receiver",
        kind: CallShapeKind::LocalReceiverMethod,
        fixture: CallCorpusFixture::GenericArray,
        source: "generic-array/src/lib.rs:1276 iter.size_hint()",
        owner: CallOwnerSelector::MethodByBody {
            name: "try_from_fallible_iter",
            body: "match iter.size_hint()",
            owner_type: Some("GenericArray"),
            owner_trait: None,
        },
        site: CallSiteSelector::Method {
            name: "size_hint",
            arg_count: Some(0),
            receiver: Some(CallReceiverSelector::MethodResultLocalBinding {
                method_name: "into_iter",
            }),
        },
        expected: CallExpected::Targetless {
            status: CallStatusKind::External,
        },
        coverage: &[
            CallPipelineCoverage::Db,
            CallPipelineCoverage::RagApi,
            CallPipelineCoverage::TuiTool,
        ],
    },
];
