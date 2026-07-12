use crate::parser::nodes::ExecutableBodyId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum LocalBindingProof {
    Typed {
        name: String,
        type_path: Vec<String>,
        init_path: Option<Vec<String>>,
    },
    TraitObject {
        name: String,
        trait_path: Vec<String>,
        init_path: Option<Vec<String>>,
    },
    Initialized {
        name: String,
        init_path: Vec<String>,
    },
    AmbiguousInitialized {
        name: String,
        init_paths: Vec<Vec<String>>,
    },
    TupleReturn {
        name: String,
        path: Vec<String>,
        index: usize,
    },
    TupleMethodReturn {
        name: String,
        method_name: String,
        method_span: (usize, usize),
        index: usize,
    },
    MethodResult {
        name: String,
        method_name: String,
        method_span: (usize, usize),
    },
    EnumVariantField {
        name: String,
        enum_path: Vec<String>,
        variant_name: String,
        field_index: usize,
    },
    Closure {
        name: String,
        closure_id: ExecutableBodyId,
        is_async: bool,
    },
    LocalFunction {
        name: String,
        body_id: ExecutableBodyId,
    },
    ValueAlias {
        name: String,
        source_path: Vec<String>,
    },
    Constructed {
        name: String,
        type_path: Vec<String>,
        fields: ConstructedFields,
    },
    Array {
        name: String,
        element_init_paths: Vec<Option<Vec<String>>>,
    },
    Referenced {
        name: String,
        type_path: Vec<String>,
    },
    Untyped {
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ConstructedFields {
    Tuple(Vec<Option<FieldInitProof>>),
    Named(Vec<(String, Option<FieldInitProof>)>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum FieldInitProof {
    Path(Vec<String>),
    Array(Vec<Option<Vec<String>>>),
}

impl LocalBindingProof {
    pub(super) fn name(&self) -> &str {
        match self {
            Self::Typed { name, .. }
            | Self::TraitObject { name, .. }
            | Self::Initialized { name, .. }
            | Self::AmbiguousInitialized { name, .. }
            | Self::TupleReturn { name, .. }
            | Self::TupleMethodReturn { name, .. }
            | Self::MethodResult { name, .. }
            | Self::EnumVariantField { name, .. }
            | Self::Closure { name, .. }
            | Self::LocalFunction { name, .. }
            | Self::ValueAlias { name, .. }
            | Self::Constructed { name, .. }
            | Self::Array { name, .. }
            | Self::Referenced { name, .. }
            | Self::Untyped { name } => name,
        }
    }
}
