use lazy_static::lazy_static;

use std::collections::{BTreeMap, HashSet};

use cozo::{DataValue, Num};
use itertools::Itertools;

use crate::{multi_embedding::definitions::*, NodeType};

#[derive(Copy, Clone, Debug)]
pub enum HnswDistance {
    L2,
    Cosine,
    Ip,
}

impl HnswDistance {
    fn as_str(&self) -> &'static str {
        match self {
            HnswDistance::L2 => "L2",
            HnswDistance::Cosine => "Cosine",
            HnswDistance::Ip => "IP",
        }
    }
}


#[derive(Copy, Clone)]
pub struct CozoField {
    st: &'static str,
    dv: &'static str,
}

impl CozoField {
    const fn new(st: &'static str, dv: &'static str) -> Self {
        Self { st, dv }
    }

    fn st(&self) -> &str {
        self.st
    }

    fn dv(&self) -> &str {
        self.dv
    }
}

pub struct ExperimentalRelationSchema {
    relation: &'static str,
    fields: &'static [CozoField],
}

impl ExperimentalRelationSchema {
    pub fn relation(&self) -> &'static str {
        self.relation
    }

    pub fn field_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.fields.iter().map(CozoField::st)
    }

    fn script_identity(&self) -> String {
        let fields = self.fields.iter().map(CozoField::st).collect::<Vec<_>>();
        let keys = fields
            .iter()
            .copied()
            .filter(|f| ID_KEYWORDS.contains(f))
            .join(", ");
        let vals = fields
            .iter()
            .copied()
            .filter(|f| !ID_KEYWORDS.contains(f))
            .join(", ");
        format!("{} {{ {keys}, at => {vals} }}", self.relation)
    }

    pub fn script_create(&self) -> String {
        let fields = self
            .fields
            .iter()
            .map(|field| format!("{}: {}", field.st(), field.dv()))
            .collect::<Vec<_>>();
        let keys = fields
            .iter()
            .filter(|f| ID_VAL_KEYWORDS.contains(&f.as_str()))
            .join(", ");
        let vals = fields
            .iter()
            .filter(|f| !ID_VAL_KEYWORDS.contains(&f.as_str()))
            .join(", ");
        format!(
            ":create {} {{ {}, at: Validity => {} }}",
            self.relation, keys, vals
        )
    }

    pub fn script_put(&self, params: &BTreeMap<String, DataValue>) -> String {
        let lhs_keys = params
            .keys()
            .filter(|k| ID_KEYWORDS.contains(&k.as_str()))
            .join(", ");
        let lhs_entries = params
            .keys()
            .filter(|k| !ID_KEYWORDS.contains(&k.as_str()))
            .join(", ");
        let rhs_keys = params
            .keys()
            .filter(|k| ID_KEYWORDS.contains(&k.as_str()))
            .map(|k| format!("${}", k))
            .join(", ");
        let rhs_entries = params
            .keys()
            .filter(|k| !ID_KEYWORDS.contains(&k.as_str()))
            .map(|k| format!("${}", k))
            .join(", ");

        format!(
            "?[{lhs_keys}, at, {lhs_entries}] <- [[{rhs_keys}, 'ASSERT', {rhs_entries}]] :put {}",
            self.script_identity()
        )
    }
}

#[derive(Copy, Clone, Debug)]
pub struct VectorDimensionSpec {
    dims: i64,
    provider: &'static str,
    embedding_model: &'static str,
    offset: f32,
    hnsw_m: i64,
    hnsw_ef_construction: i64,
    hnsw_search_ef: i64,
}

impl VectorDimensionSpec {
    pub fn dims(&self) -> i64 {
        self.dims
    }

    pub fn provider(&self) -> &'static str {
        self.provider
    }

    pub fn embedding_model(&self) -> &'static str {
        self.embedding_model
    }

    pub fn offset(&self) -> f32 {
        self.offset
    }

    pub fn hnsw_m(&self) -> i64 {
        self.hnsw_m
    }

    pub fn hnsw_ef_construction(&self) -> i64 {
        self.hnsw_ef_construction
    }

    pub fn hnsw_search_ef(&self) -> i64 {
        self.hnsw_search_ef
    }
}

pub fn vector_dimension_specs() -> &'static [VectorDimensionSpec] {
    &VECTOR_DIMENSION_SPECS
}

pub const EXPERIMENTAL_NODE_RELATION_SPECS: [ExperimentalNodeRelationSpec; 12] = [
    ExperimentalNodeRelationSpec {
        name: "function",
        node_type: NodeType::Function,
        metadata_schema: &FUNCTION_MULTI_EMBEDDING_SCHEMA,
        vector_relation_base: "function_embedding_vectors",
    },
    ExperimentalNodeRelationSpec {
        name: "const",
        node_type: NodeType::Const,
        metadata_schema: &CONST_MULTI_EMBEDDING_SCHEMA,
        vector_relation_base: "const_embedding_vectors",
    },
    ExperimentalNodeRelationSpec {
        name: "enum",
        node_type: NodeType::Enum,
        metadata_schema: &ENUM_MULTI_EMBEDDING_SCHEMA,
        vector_relation_base: "enum_embedding_vectors",
    },
    ExperimentalNodeRelationSpec {
        name: "impl",
        node_type: NodeType::Impl,
        metadata_schema: &IMPL_MULTI_EMBEDDING_SCHEMA,
        vector_relation_base: "impl_embedding_vectors",
    },
    ExperimentalNodeRelationSpec {
        name: "import",
        node_type: NodeType::Import,
        metadata_schema: &IMPORT_MULTI_EMBEDDING_SCHEMA,
        vector_relation_base: "import_embedding_vectors",
    },
    ExperimentalNodeRelationSpec {
        name: "macro",
        node_type: NodeType::Macro,
        metadata_schema: &MACRO_MULTI_EMBEDDING_SCHEMA,
        vector_relation_base: "macro_embedding_vectors",
    },
    ExperimentalNodeRelationSpec {
        name: "module",
        node_type: NodeType::Module,
        metadata_schema: &MODULE_MULTI_EMBEDDING_SCHEMA,
        vector_relation_base: "module_embedding_vectors",
    },
    ExperimentalNodeRelationSpec {
        name: "static",
        node_type: NodeType::Static,
        metadata_schema: &STATIC_MULTI_EMBEDDING_SCHEMA,
        vector_relation_base: "static_embedding_vectors",
    },
    ExperimentalNodeRelationSpec {
        name: "struct",
        node_type: NodeType::Struct,
        metadata_schema: &STRUCT_MULTI_EMBEDDING_SCHEMA,
        vector_relation_base: "struct_embedding_vectors",
    },
    ExperimentalNodeRelationSpec {
        name: "trait",
        node_type: NodeType::Trait,
        metadata_schema: &TRAIT_MULTI_EMBEDDING_SCHEMA,
        vector_relation_base: "trait_embedding_vectors",
    },
    ExperimentalNodeRelationSpec {
        name: "type_alias",
        node_type: NodeType::TypeAlias,
        metadata_schema: &TYPE_ALIAS_MULTI_EMBEDDING_SCHEMA,
        vector_relation_base: "type_alias_embedding_vectors",
    },
    ExperimentalNodeRelationSpec {
        name: "union",
        node_type: NodeType::Union,
        metadata_schema: &UNION_MULTI_EMBEDDING_SCHEMA,
        vector_relation_base: "union_embedding_vectors",
    },
];

pub fn experimental_node_relation_specs() -> &'static [ExperimentalNodeRelationSpec] {
    &EXPERIMENTAL_NODE_RELATION_SPECS
}

pub fn experimental_spec_for_node(
    node_type: NodeType,
) -> Option<&'static ExperimentalNodeRelationSpec> {
    EXPERIMENTAL_NODE_RELATION_SPECS
        .iter()
        .find(|spec| spec.node_type == node_type)
}

pub fn embedding_entry(model: &str, dims: i64) -> DataValue {
    DataValue::List(vec![
        DataValue::Str(model.into()),
        DataValue::Num(Num::Int(dims)),
    ])
}

fn vector_literal(len: usize, offset: f32) -> String {
    let values = (0..len)
        .map(|idx| format!("{:.6}", offset + (idx as f32 * 0.0001)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("vec([{values}])")
}

lazy_static! {
    static ref SUPPORTED_DIMENSION_SET: HashSet<i64> = VECTOR_DIMENSION_SPECS
        .iter()
        .map(|spec| spec.dims)
        .collect();
}

fn supported_dimension_set() -> &'static HashSet<i64> {
    &SUPPORTED_DIMENSION_SET
}


const ID_KEYWORDS: [&str; 9] = [
    "id",
    "function_id",
    "owner_id",
    "source_id",
    "target_id",
    "type_id",
    "node_id",
    "embedding_model",
    "provider",
];
const ID_VAL_KEYWORDS: [&str; 9] = [
    "id: Uuid",
    "function_id: Uuid",
    "owner_id: Uuid",
    "source_id: Uuid",
    "target_id: Uuid",
    "type_id: Uuid",
    "node_id: Uuid",
    "embedding_model: String",
    "provider: String",
];

pub const VECTOR_DIMENSION_SPECS: [VectorDimensionSpec; 4] = [
    VectorDimensionSpec {
        dims: 384,
        provider: "local-transformers",
        embedding_model: "sentence-transformers/all-MiniLM-L6-v2",
        offset: 0.01,
        hnsw_m: 16,
        hnsw_ef_construction: 64,
        hnsw_search_ef: 50,
    },
    VectorDimensionSpec {
        dims: 768,
        provider: "openrouter",
        embedding_model: "ploke-test-embed-768",
        offset: 0.35,
        hnsw_m: 20,
        hnsw_ef_construction: 80,
        hnsw_search_ef: 56,
    },
    VectorDimensionSpec {
        dims: 1024,
        provider: "cohere",
        embedding_model: "ploke-test-embed-1024",
        offset: 0.7,
        hnsw_m: 26,
        hnsw_ef_construction: 96,
        hnsw_search_ef: 60,
    },
    VectorDimensionSpec {
        dims: 1536,
        provider: "openai",
        embedding_model: "text-embedding-ada-002",
        offset: 1.0,
        hnsw_m: 32,
        hnsw_ef_construction: 128,
        hnsw_search_ef: 64,
    },
];



#[derive(Copy, Clone)]
pub struct ExperimentalNodeRelationSpec {
    pub name: &'static str,
    pub node_type: NodeType,
    pub metadata_schema: &'static ExperimentalRelationSchema,
    pub vector_relation_base: &'static str,
}

impl ExperimentalNodeRelationSpec {
    pub fn relation_name(&self) -> &'static str {
        self.metadata_schema.relation()
    }
}


