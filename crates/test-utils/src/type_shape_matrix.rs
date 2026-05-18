use ploke_db::{TypeContextRelation, TypeRelationKind, TypeUseRole};

use crate::{
    CORPUS_AXUM_OPENROUTER_EMBEDDINGS, CORPUS_AXUM_TYPE_GRAPH, CORPUS_CHRONO_OPENROUTER_EMBEDDINGS,
    CORPUS_CHRONO_TYPE_GRAPH, CORPUS_GENERIC_ARRAY_OPENROUTER_EMBEDDINGS,
    CORPUS_GENERIC_ARRAY_TYPE_GRAPH, CORPUS_MEMCHR_OPENROUTER_EMBEDDINGS, CORPUS_MEMCHR_TYPE_GRAPH,
    CORPUS_SEMVER_OPENROUTER_EMBEDDINGS, CORPUS_SEMVER_TYPE_GRAPH, FixtureDb,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeShapeKind {
    Named,
    NamedGenericArgument,
    QualifiedProjection,
    Reference,
    Slice,
    Array,
    Tuple,
    FunctionPointer,
    RawPointer,
    TraitObject,
    ImplTrait,
    TraitBound,
    AssociatedTypeBound,
    TraitSuper,
    GenericBound,
    GenericParamBound,
    WhereSubject,
    WhereBound,
    WhereGenericParamBound,
    Never,
    Inferred,
    Macro,
    Unknown,
    Paren,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CorpusFixture {
    Semver,
    Memchr,
    GenericArray,
    Chrono,
    Axum,
}

impl CorpusFixture {
    pub fn fixture(self) -> &'static FixtureDb {
        match self {
            Self::Semver => &CORPUS_SEMVER_TYPE_GRAPH,
            Self::Memchr => &CORPUS_MEMCHR_TYPE_GRAPH,
            Self::GenericArray => &CORPUS_GENERIC_ARRAY_TYPE_GRAPH,
            Self::Chrono => &CORPUS_CHRONO_TYPE_GRAPH,
            Self::Axum => &CORPUS_AXUM_TYPE_GRAPH,
        }
    }

    pub fn searchable_fixture(self) -> &'static FixtureDb {
        match self {
            Self::Semver => &CORPUS_SEMVER_OPENROUTER_EMBEDDINGS,
            Self::Memchr => &CORPUS_MEMCHR_OPENROUTER_EMBEDDINGS,
            Self::GenericArray => &CORPUS_GENERIC_ARRAY_OPENROUTER_EMBEDDINGS,
            Self::Chrono => &CORPUS_CHRONO_OPENROUTER_EMBEDDINGS,
            Self::Axum => &CORPUS_AXUM_OPENROUTER_EMBEDDINGS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContainingOwnerSelector {
    StructByName {
        name: &'static str,
    },
    StructInModule {
        module_path: &'static [&'static str],
        name: &'static str,
    },
    MethodByImplSelf {
        self_type: &'static str,
        method: &'static str,
    },
    MethodByImplTraitAndSelf {
        trait_name: &'static str,
        self_type: &'static str,
        method: &'static str,
    },
    TraitInModule {
        module_path: &'static [&'static str],
        name: &'static str,
    },
    ImplByTraitInFile {
        file_suffix: &'static str,
        trait_name: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OwnerSelector {
    FunctionInModule {
        module_path: &'static [&'static str],
        name: &'static str,
    },
    FunctionInFile {
        file_suffix: &'static str,
        name: &'static str,
    },
    MethodByImplSelf {
        self_type: &'static str,
        method: &'static str,
    },
    MethodByImplTraitAndSelf {
        trait_name: &'static str,
        self_type: &'static str,
        method: &'static str,
    },
    MethodByRawPointerImpl {
        file_suffix: &'static str,
        trait_name: &'static str,
        mutable: bool,
        method: &'static str,
    },
    FieldByStructInModule {
        module_path: &'static [&'static str],
        struct_name: &'static str,
        field_index: u32,
    },
    FieldByStructInFile {
        file_suffix: &'static str,
        struct_name: &'static str,
        field_index: u32,
    },
    TypeAlias {
        name: &'static str,
    },
    ConstInFile {
        file_suffix: &'static str,
        name: &'static str,
    },
    StaticInFile {
        file_suffix: &'static str,
        name: &'static str,
    },
    StructByName {
        name: &'static str,
    },
    TraitInModule {
        module_path: &'static [&'static str],
        name: &'static str,
    },
    GenericTypeParamByContainingOwner {
        containing_owner: ContainingOwnerSelector,
        param_name: &'static str,
    },
    ImplByTraitInFile {
        file_suffix: &'static str,
        trait_name: &'static str,
    },
    WhereGenericParamBoundOwner {
        containing_owner: ContainingOwnerSelector,
        predicate_index: u32,
        bound_index: u32,
        target_trait: TargetSelector,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetSelector {
    StructByName {
        name: &'static str,
    },
    StructInModule {
        module_path: &'static [&'static str],
        name: &'static str,
    },
    EnumByName {
        name: &'static str,
    },
    TraitInModule {
        module_path: &'static [&'static str],
        name: &'static str,
    },
    TraitInFile {
        file_suffix: &'static str,
        name: &'static str,
    },
    GenericParamReachableByName {
        name: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoordinateSpec {
    None,
    ParamSlot(u32),
    FieldSlot(u32),
    TraitSuperSlot(u32),
    GenericBoundSlot {
        generic_param_index: u32,
        bound_index: u32,
    },
    GenericParamBoundSlot {
        containing_owner: ContainingOwnerSelector,
        generic_param_index: u32,
        bound_index: u32,
    },
    WhereSubjectSlot {
        predicate_index: u32,
    },
    WhereBoundSlot {
        predicate_index: u32,
        bound_index: u32,
    },
    WhereGenericParamBoundSlot {
        containing_owner: ContainingOwnerSelector,
        predicate_index: u32,
        bound_index: u32,
    },
    AssociatedTypeBoundSlot {
        associated_type_index: u32,
        associated_type_name: &'static str,
        bound_index: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShapePipelineCoverage {
    DbOnly,
    RagApi,
    TuiTool,
    LiveIgnored,
}

#[derive(Debug, Clone, Copy)]
pub struct TypeShapeCase {
    pub name: &'static str,
    pub kind: TypeShapeKind,
    pub fixture: CorpusFixture,
    pub source: &'static str,
    pub owner: OwnerSelector,
    pub role: TypeUseRole,
    pub coordinate: CoordinateSpec,
    pub terminal: TargetSelector,
    pub relation_kind: TypeRelationKind,
    pub depth: u32,
    pub type_context_relation: TypeContextRelation,
    pub coverage: &'static [ShapePipelineCoverage],
    pub search_term: &'static str,
    pub live_prompt: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct TypeShapeNoTargetCase {
    pub name: &'static str,
    pub kind: TypeShapeKind,
    pub fixture: CorpusFixture,
    pub source: &'static str,
    pub owner: OwnerSelector,
    pub role: TypeUseRole,
    pub coordinate: CoordinateSpec,
    pub root_relation: &'static str,
    pub nested_relation: Option<TypeShapeNestedNoTarget>,
}

#[derive(Debug, Clone, Copy)]
pub struct TypeShapeNestedNoTarget {
    pub relation: &'static str,
    pub containment_path: &'static [&'static str],
}

pub const fn positive_type_shape_cases() -> &'static [TypeShapeCase] {
    POSITIVE_TYPE_SHAPE_CASES
}

pub const fn no_target_type_shape_cases() -> &'static [TypeShapeNoTargetCase] {
    NO_TARGET_TYPE_SHAPE_CASES
}

pub const fn absent_type_shape_cases() -> &'static [(TypeShapeKind, &'static str)] {
    ABSENT_TYPE_SHAPE_CASES
}

const PIPELINE: &[ShapePipelineCoverage] = &[
    ShapePipelineCoverage::DbOnly,
    ShapePipelineCoverage::RagApi,
    ShapePipelineCoverage::TuiTool,
    ShapePipelineCoverage::LiveIgnored,
];
const RAG_API: &[ShapePipelineCoverage] =
    &[ShapePipelineCoverage::DbOnly, ShapePipelineCoverage::RagApi];

static POSITIVE_TYPE_SHAPE_CASES: &[TypeShapeCase] = &[
    TypeShapeCase {
        name: "named_return_memchr_iter",
        kind: TypeShapeKind::Named,
        fixture: CorpusFixture::Memchr,
        source: "BurntSushi/memchr src/memchr.rs: memchr_iter(...) -> Memchr<'h>",
        owner: OwnerSelector::FunctionInModule {
            module_path: &["crate", "memchr"],
            name: "memchr_iter",
        },
        role: TypeUseRole::FunctionReturn,
        coordinate: CoordinateSpec::None,
        terminal: TargetSelector::StructByName { name: "Memchr" },
        relation_kind: TypeRelationKind::Ordinary,
        depth: 0,
        type_context_relation: TypeContextRelation::TypeDefinitionImpact,
        coverage: PIPELINE,
        search_term: "memchr_iter",
        live_prompt: "Use request_code_context to find the memchr_iter return type and include its type_context.",
    },
    TypeShapeCase {
        name: "reference_param_semver_matches_req",
        kind: TypeShapeKind::Reference,
        fixture: CorpusFixture::Semver,
        source: "dtolnay/semver src/eval.rs: matches_req(req: &VersionReq, ...)",
        owner: OwnerSelector::FunctionInModule {
            module_path: &["crate", "eval"],
            name: "matches_req",
        },
        role: TypeUseRole::FunctionParam,
        coordinate: CoordinateSpec::ParamSlot(0),
        terminal: TargetSelector::StructInModule {
            module_path: &["crate"],
            name: "VersionReq",
        },
        relation_kind: TypeRelationKind::Ordinary,
        depth: 1,
        type_context_relation: TypeContextRelation::UsesTypeNested,
        coverage: PIPELINE,
        search_term: "matches_req",
        live_prompt: "Use request_code_context to find matches_req and include the VersionReq type_context.",
    },
    TypeShapeCase {
        name: "named_generic_argument_chrono_weekday_set_single_day",
        kind: TypeShapeKind::NamedGenericArgument,
        fixture: CorpusFixture::Chrono,
        source: "chronotope/chrono src/weekday_set.rs: WeekdaySet::single_day(...) -> Option<Weekday>",
        owner: OwnerSelector::MethodByImplSelf {
            self_type: "WeekdaySet",
            method: "single_day",
        },
        role: TypeUseRole::MethodReturn,
        coordinate: CoordinateSpec::None,
        terminal: TargetSelector::EnumByName { name: "Weekday" },
        relation_kind: TypeRelationKind::Ordinary,
        depth: 1,
        type_context_relation: TypeContextRelation::UsesTypeNested,
        coverage: PIPELINE,
        search_term: "single_day method WeekdaySet Weekday",
        live_prompt: "Use request_code_context to find WeekdaySet::single_day and include the Weekday type_context.",
    },
    TypeShapeCase {
        name: "qualified_projection_generic_array_const_array_length",
        kind: TypeShapeKind::QualifiedProjection,
        fixture: CorpusFixture::GenericArray,
        source: "fizyk20/generic-array src/typenum.rs: <Const<N> as IntoArrayLength>::ArrayLength",
        owner: OwnerSelector::TypeAlias {
            name: "ConstArrayLength",
        },
        role: TypeUseRole::TypeAliasTarget,
        coordinate: CoordinateSpec::None,
        terminal: TargetSelector::TraitInModule {
            module_path: &["crate"],
            name: "IntoArrayLength",
        },
        relation_kind: TypeRelationKind::Trait,
        depth: 1,
        type_context_relation: TypeContextRelation::AliasExpansion,
        coverage: RAG_API,
        search_term: "ConstArrayLength IntoArrayLength",
        live_prompt: "Use request_code_context to find ConstArrayLength and include the IntoArrayLength type_context.",
    },
    TypeShapeCase {
        name: "function_pointer_memchr_searcher_kind",
        kind: TypeShapeKind::FunctionPointer,
        fixture: CorpusFixture::Memchr,
        source: "BurntSushi/memchr src/memmem/searcher.rs: type SearcherKindFn = unsafe fn(&Searcher, &mut PrefilterState, ...) -> Option<usize>",
        owner: OwnerSelector::TypeAlias {
            name: "SearcherKindFn",
        },
        role: TypeUseRole::TypeAliasTarget,
        coordinate: CoordinateSpec::None,
        terminal: TargetSelector::StructInModule {
            module_path: &["crate", "memmem", "searcher"],
            name: "Searcher",
        },
        relation_kind: TypeRelationKind::Ordinary,
        depth: 2,
        type_context_relation: TypeContextRelation::AliasExpansion,
        coverage: RAG_API,
        search_term: "SearcherKindFn Searcher PrefilterState",
        live_prompt: "Use request_code_context to find SearcherKindFn and include the Searcher type_context.",
    },
    TypeShapeCase {
        name: "slice_static_chrono_d_fmt",
        kind: TypeShapeKind::Slice,
        fixture: CorpusFixture::Chrono,
        source: "chronotope/chrono src/format/strftime.rs: static D_FMT: &[Item<'static>]",
        owner: OwnerSelector::StaticInFile {
            file_suffix: "src/format/strftime.rs",
            name: "D_FMT",
        },
        role: TypeUseRole::StaticType,
        coordinate: CoordinateSpec::None,
        terminal: TargetSelector::EnumByName { name: "Item" },
        relation_kind: TypeRelationKind::Ordinary,
        depth: 2,
        type_context_relation: TypeContextRelation::UsesTypeNested,
        coverage: RAG_API,
        search_term: "D_FMT Item strftime",
        live_prompt: "Use request_code_context to find D_FMT and include the Item type_context.",
    },
    TypeShapeCase {
        name: "array_param_chrono_weekday_set",
        kind: TypeShapeKind::Array,
        fixture: CorpusFixture::Chrono,
        source: "chronotope/chrono src/weekday.rs: WeekdaySet::from_array(days: [Weekday; C])",
        owner: OwnerSelector::MethodByImplSelf {
            self_type: "WeekdaySet",
            method: "from_array",
        },
        role: TypeUseRole::MethodParam,
        coordinate: CoordinateSpec::ParamSlot(0),
        terminal: TargetSelector::EnumByName { name: "Weekday" },
        relation_kind: TypeRelationKind::Ordinary,
        depth: 1,
        type_context_relation: TypeContextRelation::UsesTypeNested,
        coverage: RAG_API,
        search_term: "WeekdaySet from_array Weekday",
        live_prompt: "Use request_code_context to find WeekdaySet::from_array and include the Weekday type_context.",
    },
    TypeShapeCase {
        name: "tuple_return_chrono_weekday_set_split_at",
        kind: TypeShapeKind::Tuple,
        fixture: CorpusFixture::Chrono,
        source: "chronotope/chrono src/weekday.rs: WeekdaySet::split_at(...) -> (Self, Self)",
        owner: OwnerSelector::MethodByImplSelf {
            self_type: "WeekdaySet",
            method: "split_at",
        },
        role: TypeUseRole::MethodReturn,
        coordinate: CoordinateSpec::None,
        terminal: TargetSelector::StructByName { name: "WeekdaySet" },
        relation_kind: TypeRelationKind::Ordinary,
        depth: 1,
        type_context_relation: TypeContextRelation::UsesTypeNested,
        coverage: RAG_API,
        search_term: "WeekdaySet split_at",
        live_prompt: "Use request_code_context to find WeekdaySet::split_at and include the tuple return type_context.",
    },
    TypeShapeCase {
        name: "raw_pointer_memchr_pointer_distance",
        kind: TypeShapeKind::RawPointer,
        fixture: CorpusFixture::Memchr,
        source: "BurntSushi/memchr src/ext.rs: impl<T> Pointer for *const T { distance(self, origin: *const T) }",
        owner: OwnerSelector::MethodByRawPointerImpl {
            file_suffix: "src/ext.rs",
            trait_name: "Pointer",
            mutable: false,
            method: "distance",
        },
        role: TypeUseRole::MethodParam,
        coordinate: CoordinateSpec::ParamSlot(1),
        terminal: TargetSelector::GenericParamReachableByName { name: "T" },
        relation_kind: TypeRelationKind::Ordinary,
        depth: 1,
        type_context_relation: TypeContextRelation::UsesTypeNested,
        coverage: RAG_API,
        search_term: "Pointer distance raw pointer T",
        live_prompt: "Use request_code_context to find Pointer::distance and include the raw pointer type_context.",
    },
    TypeShapeCase {
        name: "trait_object_axum_boxed_into_route",
        kind: TypeShapeKind::TraitObject,
        fixture: CorpusFixture::Axum,
        source: "tokio-rs/axum axum/src/boxed.rs: BoxedIntoRoute<S, E>(Box<dyn ErasedIntoRoute<S, E>>)",
        owner: OwnerSelector::FieldByStructInFile {
            file_suffix: "axum/src/boxed.rs",
            struct_name: "BoxedIntoRoute",
            field_index: 0,
        },
        role: TypeUseRole::FieldType,
        coordinate: CoordinateSpec::FieldSlot(0),
        terminal: TargetSelector::TraitInFile {
            file_suffix: "axum/src/boxed.rs",
            name: "ErasedIntoRoute",
        },
        relation_kind: TypeRelationKind::Trait,
        depth: 2,
        type_context_relation: TypeContextRelation::UsesTypeNested,
        coverage: PIPELINE,
        search_term: "BoxedIntoRoute struct",
        live_prompt: "Use request_code_context to find BoxedIntoRoute and include the ErasedIntoRoute type_context.",
    },
    TypeShapeCase {
        name: "trait_object_axum_map_layer_fn_field",
        kind: TypeShapeKind::TraitObject,
        fixture: CorpusFixture::Axum,
        source: "tokio-rs/axum axum/src/boxed.rs: Map.layer: Box<dyn LayerFn<E, E2>>",
        owner: OwnerSelector::FieldByStructInFile {
            file_suffix: "axum/src/boxed.rs",
            struct_name: "Map",
            field_index: 1,
        },
        role: TypeUseRole::FieldType,
        coordinate: CoordinateSpec::FieldSlot(1),
        terminal: TargetSelector::TraitInFile {
            file_suffix: "axum/src/boxed.rs",
            name: "LayerFn",
        },
        relation_kind: TypeRelationKind::Trait,
        depth: 2,
        type_context_relation: TypeContextRelation::UsesTypeNested,
        coverage: PIPELINE,
        search_term: "Map struct axum src boxed rs",
        live_prompt: "Use request_code_context to find Map.layer and include the LayerFn type_context.",
    },
    TypeShapeCase {
        name: "trait_object_axum_make_erased_handler_clone_box",
        kind: TypeShapeKind::TraitObject,
        fixture: CorpusFixture::Axum,
        source: "tokio-rs/axum axum/src/boxed.rs: MakeErasedHandler::clone_box(...) -> Box<dyn ErasedIntoRoute<S, Infallible>>",
        owner: OwnerSelector::MethodByImplTraitAndSelf {
            trait_name: "ErasedIntoRoute",
            self_type: "MakeErasedHandler",
            method: "clone_box",
        },
        role: TypeUseRole::MethodReturn,
        coordinate: CoordinateSpec::None,
        terminal: TargetSelector::TraitInFile {
            file_suffix: "axum/src/boxed.rs",
            name: "ErasedIntoRoute",
        },
        relation_kind: TypeRelationKind::Trait,
        depth: 2,
        type_context_relation: TypeContextRelation::UsesTypeNested,
        coverage: PIPELINE,
        search_term: "method clone_box boxed MakeErasedHandler",
        live_prompt: "Use request_code_context to find MakeErasedHandler::clone_box and include the ErasedIntoRoute type_context.",
    },
    TypeShapeCase {
        name: "impl_trait_axum_strip_prefix_zip_longest_item",
        kind: TypeShapeKind::ImplTrait,
        fixture: CorpusFixture::Axum,
        source: "tokio-rs/axum axum/src/routing/strip_prefix.rs: zip_longest(...) -> impl Iterator<Item = Item<I::Item>>",
        owner: OwnerSelector::FunctionInFile {
            file_suffix: "axum/src/routing/strip_prefix.rs",
            name: "zip_longest",
        },
        role: TypeUseRole::FunctionReturn,
        coordinate: CoordinateSpec::None,
        terminal: TargetSelector::EnumByName { name: "Item" },
        relation_kind: TypeRelationKind::Ordinary,
        depth: 2,
        type_context_relation: TypeContextRelation::UsesTypeNested,
        coverage: PIPELINE,
        search_term: "zip_longest",
        live_prompt: "Use request_code_context to find zip_longest and include the Item type_context.",
    },
    TypeShapeCase {
        name: "impl_trait_axum_strip_prefix_layer",
        kind: TypeShapeKind::ImplTrait,
        fixture: CorpusFixture::Axum,
        source: "tokio-rs/axum axum/src/routing/strip_prefix.rs: StripPrefix::layer(...) -> impl Layer<S, Service = Self> + Clone",
        owner: OwnerSelector::MethodByImplSelf {
            self_type: "StripPrefix",
            method: "layer",
        },
        role: TypeUseRole::MethodReturn,
        coordinate: CoordinateSpec::None,
        terminal: TargetSelector::StructByName {
            name: "StripPrefix",
        },
        relation_kind: TypeRelationKind::Ordinary,
        depth: 2,
        type_context_relation: TypeContextRelation::UsesTypeNested,
        coverage: PIPELINE,
        search_term: "method layer StripPrefix",
        live_prompt: "Use request_code_context to find StripPrefix::layer and include the StripPrefix type_context.",
    },
    TypeShapeCase {
        name: "impl_trait_generic_array_array_builder_extend_source",
        kind: TypeShapeKind::ImplTrait,
        fixture: CorpusFixture::GenericArray,
        source: "fizyk20/generic-array src/internal.rs: ArrayBuilder::extend(..., source: impl Iterator<Item = T>)",
        owner: OwnerSelector::MethodByImplSelf {
            self_type: "ArrayBuilder",
            method: "extend",
        },
        role: TypeUseRole::MethodParam,
        coordinate: CoordinateSpec::ParamSlot(1),
        terminal: TargetSelector::GenericParamReachableByName { name: "T" },
        relation_kind: TypeRelationKind::Ordinary,
        depth: 2,
        type_context_relation: TypeContextRelation::UsesTypeNested,
        coverage: RAG_API,
        search_term: "ArrayBuilder extend Iterator Item T",
        live_prompt: "Use request_code_context to find ArrayBuilder::extend and include the Iterator item type_context.",
    },
    TypeShapeCase {
        name: "trait_bound_generic_array_array_length",
        kind: TypeShapeKind::TraitBound,
        fixture: CorpusFixture::GenericArray,
        source: "fizyk20/generic-array src/lib.rs: pub struct GenericArray<T, N: ArrayLength>",
        owner: OwnerSelector::StructByName {
            name: "GenericArray",
        },
        role: TypeUseRole::GenericBound,
        coordinate: CoordinateSpec::GenericBoundSlot {
            generic_param_index: 1,
            bound_index: 0,
        },
        terminal: TargetSelector::TraitInModule {
            module_path: &["crate"],
            name: "ArrayLength",
        },
        relation_kind: TypeRelationKind::Trait,
        depth: 0,
        type_context_relation: TypeContextRelation::TypeDefinitionImpact,
        coverage: RAG_API,
        search_term: "GenericArray ArrayLength bound",
        live_prompt: "Use request_code_context to find GenericArray and include the ArrayLength type_context.",
    },
    TypeShapeCase {
        name: "associated_type_bound_chrono_timezone_offset",
        kind: TypeShapeKind::AssociatedTypeBound,
        fixture: CorpusFixture::Chrono,
        source: "chronotope/chrono src/offset/mod.rs: trait TimeZone { type Offset: Offset; }",
        owner: OwnerSelector::TraitInModule {
            module_path: &["crate", "offset"],
            name: "TimeZone",
        },
        role: TypeUseRole::AssociatedTypeBound,
        coordinate: CoordinateSpec::AssociatedTypeBoundSlot {
            associated_type_index: 0,
            associated_type_name: "Offset",
            bound_index: 0,
        },
        terminal: TargetSelector::TraitInModule {
            module_path: &["crate", "offset"],
            name: "Offset",
        },
        relation_kind: TypeRelationKind::Trait,
        depth: 0,
        type_context_relation: TypeContextRelation::TypeDefinitionImpact,
        coverage: RAG_API,
        search_term: "TimeZone Offset associated type bound",
        live_prompt: "Use request_code_context to find TimeZone::Offset and include the Offset type_context.",
    },
    TypeShapeCase {
        name: "trait_super_generic_array_concat",
        kind: TypeShapeKind::TraitSuper,
        fixture: CorpusFixture::GenericArray,
        source: "fizyk20/generic-array src/sequence.rs: unsafe trait Concat<T, M: ArrayLength>: GenericSequence<T>",
        owner: OwnerSelector::TraitInModule {
            module_path: &["crate", "sequence"],
            name: "Concat",
        },
        role: TypeUseRole::TraitSuper,
        coordinate: CoordinateSpec::TraitSuperSlot(0),
        terminal: TargetSelector::TraitInModule {
            module_path: &["crate", "sequence"],
            name: "GenericSequence",
        },
        relation_kind: TypeRelationKind::Trait,
        depth: 0,
        type_context_relation: TypeContextRelation::TypeDefinitionImpact,
        coverage: RAG_API,
        search_term: "Concat GenericSequence supertrait",
        live_prompt: "Use request_code_context to find Concat and include the GenericSequence supertrait type_context.",
    },
    TypeShapeCase {
        name: "generic_param_bound_chrono_datetime_tz",
        kind: TypeShapeKind::GenericParamBound,
        fixture: CorpusFixture::Chrono,
        source: "chronotope/chrono src/datetime/mod.rs: pub struct DateTime<Tz: TimeZone>",
        owner: OwnerSelector::GenericTypeParamByContainingOwner {
            containing_owner: ContainingOwnerSelector::StructByName { name: "DateTime" },
            param_name: "Tz",
        },
        role: TypeUseRole::GenericParamBound,
        coordinate: CoordinateSpec::GenericParamBoundSlot {
            containing_owner: ContainingOwnerSelector::StructByName { name: "DateTime" },
            generic_param_index: 0,
            bound_index: 0,
        },
        terminal: TargetSelector::TraitInModule {
            module_path: &["crate", "offset"],
            name: "TimeZone",
        },
        relation_kind: TypeRelationKind::Trait,
        depth: 0,
        type_context_relation: TypeContextRelation::TypeDefinitionImpact,
        coverage: RAG_API,
        search_term: "DateTime Tz TimeZone",
        live_prompt: "Use request_code_context to find DateTime<Tz> and include the TimeZone type_context.",
    },
    TypeShapeCase {
        name: "where_bound_chrono_subsec_round",
        kind: TypeShapeKind::WhereBound,
        fixture: CorpusFixture::Chrono,
        source: "chronotope/chrono src/round.rs: impl<T> SubsecRound for T where T: Timelike + ...",
        owner: OwnerSelector::ImplByTraitInFile {
            file_suffix: "src/round.rs",
            trait_name: "SubsecRound",
        },
        role: TypeUseRole::WherePredicateBound,
        coordinate: CoordinateSpec::WhereBoundSlot {
            predicate_index: 0,
            bound_index: 0,
        },
        terminal: TargetSelector::TraitInModule {
            module_path: &["crate", "traits"],
            name: "Timelike",
        },
        relation_kind: TypeRelationKind::Trait,
        depth: 0,
        type_context_relation: TypeContextRelation::TypeDefinitionImpact,
        coverage: RAG_API,
        search_term: "SubsecRound Timelike where",
        live_prompt: "Use request_code_context to find SubsecRound and include the Timelike where-bound type_context.",
    },
    TypeShapeCase {
        name: "where_subject_generic_array_mapped_sequence",
        kind: TypeShapeKind::WhereSubject,
        fixture: CorpusFixture::GenericArray,
        source: "fizyk20/generic-array src/lib.rs: where GenericArray<U, N>: GenericSequence<...>",
        owner: OwnerSelector::ImplByTraitInFile {
            file_suffix: "src/lib.rs",
            trait_name: "MappedGenericSequence",
        },
        role: TypeUseRole::WherePredicateSubject,
        coordinate: CoordinateSpec::WhereSubjectSlot { predicate_index: 0 },
        terminal: TargetSelector::StructByName {
            name: "GenericArray",
        },
        relation_kind: TypeRelationKind::Ordinary,
        depth: 0,
        type_context_relation: TypeContextRelation::TypeDefinitionImpact,
        coverage: RAG_API,
        search_term: "MappedGenericSequence GenericArray where subject",
        live_prompt: "Use request_code_context to find MappedGenericSequence and include the GenericArray where-subject type_context.",
    },
    TypeShapeCase {
        name: "where_generic_param_bound_generic_array_zip_rhs",
        kind: TypeShapeKind::WhereGenericParamBound,
        fixture: CorpusFixture::GenericArray,
        source: "fizyk20/generic-array src/functional.rs: FunctionalSequence::zip where Rhs: GenericSequence<B, ...>",
        owner: OwnerSelector::WhereGenericParamBoundOwner {
            containing_owner: ContainingOwnerSelector::MethodByImplTraitAndSelf {
                trait_name: "FunctionalSequence",
                self_type: "GenericArray",
                method: "zip",
            },
            predicate_index: 2,
            bound_index: 0,
            target_trait: TargetSelector::TraitInModule {
                module_path: &["crate", "sequence"],
                name: "GenericSequence",
            },
        },
        role: TypeUseRole::WhereGenericParamBound,
        coordinate: CoordinateSpec::WhereGenericParamBoundSlot {
            containing_owner: ContainingOwnerSelector::MethodByImplTraitAndSelf {
                trait_name: "FunctionalSequence",
                self_type: "GenericArray",
                method: "zip",
            },
            predicate_index: 2,
            bound_index: 0,
        },
        terminal: TargetSelector::TraitInModule {
            module_path: &["crate", "sequence"],
            name: "GenericSequence",
        },
        relation_kind: TypeRelationKind::Trait,
        depth: 0,
        type_context_relation: TypeContextRelation::TypeDefinitionImpact,
        coverage: RAG_API,
        search_term: "FunctionalSequence zip Rhs GenericSequence",
        live_prompt: "Use request_code_context to find FunctionalSequence::zip and include the GenericSequence type_context.",
    },
];

static NO_TARGET_TYPE_SHAPE_CASES: &[TypeShapeNoTargetCase] = &[
    TypeShapeNoTargetCase {
        name: "never_return_generic_array_from_iter_length_fail",
        kind: TypeShapeKind::Never,
        fixture: CorpusFixture::GenericArray,
        source: "fizyk20/generic-array src/lib.rs: from_iter_length_fail(...) -> !",
        owner: OwnerSelector::FunctionInFile {
            file_suffix: "src/lib.rs",
            name: "from_iter_length_fail",
        },
        role: TypeUseRole::FunctionReturn,
        coordinate: CoordinateSpec::None,
        root_relation: "never_type",
        nested_relation: None,
    },
    TypeShapeNoTargetCase {
        name: "macro_argument_axum_punctuated_token",
        kind: TypeShapeKind::Macro,
        fixture: CorpusFixture::Axum,
        source: "tokio-rs/axum axum-macros/src/from_request/mod.rs: impl_enum_by_extracting_all_at_once(..., variants: Punctuated<syn::Variant, Token![,]>, ...)",
        owner: OwnerSelector::FunctionInFile {
            file_suffix: "axum-macros/src/from_request/mod.rs",
            name: "impl_enum_by_extracting_all_at_once",
        },
        role: TypeUseRole::FunctionParam,
        coordinate: CoordinateSpec::ParamSlot(1),
        root_relation: "named_type",
        nested_relation: Some(TypeShapeNestedNoTarget {
            relation: "macro_type",
            containment_path: &["Argument"],
        }),
    },
    TypeShapeNoTargetCase {
        name: "paren_nested_axum_core_error_source",
        kind: TypeShapeKind::Paren,
        fixture: CorpusFixture::Axum,
        source: "tokio-rs/axum axum-core/src/error.rs: StdError for Error::source() -> Option<&(dyn StdError + 'static)>",
        owner: OwnerSelector::MethodByImplSelf {
            self_type: "Error",
            method: "source",
        },
        role: TypeUseRole::MethodReturn,
        coordinate: CoordinateSpec::None,
        root_relation: "named_type",
        nested_relation: Some(TypeShapeNestedNoTarget {
            relation: "paren_type",
            containment_path: &["Argument", "Referenced"],
        }),
    },
];

static ABSENT_TYPE_SHAPE_CASES: &[(TypeShapeKind, &str)] = &[
    (TypeShapeKind::Inferred, "inferred_type"),
    (TypeShapeKind::Unknown, "unknown_type"),
];
