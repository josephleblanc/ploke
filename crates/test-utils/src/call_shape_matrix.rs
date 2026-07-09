//! Corpus-backed call-graph coverage matrix.
//!
//! This mirrors `type_shape_matrix`: the data lives in `test-utils`, while DB,
//! RAG, and TUI layers decide which rows they can materialize.

use ploke_db::{CallRelationKind, CallStatusKind, CallTargetKind};

use crate::{
    CORPUS_AXUM_CALL_GRAPH, CORPUS_CHRONO_CALL_GRAPH, CORPUS_GENERIC_ARRAY_CALL_GRAPH,
    CORPUS_MEMCHR_CALL_GRAPH, FixtureDb,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallShapeKind {
    FreeFunctionPath,
    AliasConstructorPath,
    GeneratedConstructorFrontier,
    DynamicCallableField,
    FunctionPointerField,
    CallableTraitObjectField,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallTargetSelector {
    FunctionInModule {
        module_path: &'static [&'static str],
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
    Targetless {
        status: CallStatusKind,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallPipelineCoverage {
    Db,
    RagApi,
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
        expected: CallExpected::Targetless {
            status: CallStatusKind::Unresolved,
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
];
