use std::collections::{BTreeMap, HashSet};

use crate::database::Database;
use crate::error::DbError;
use cozo::{self, DataValue, Db, MemStorage, NamedRows, Num, ScriptMutability, UuidWrapper};
use itertools::Itertools;
use lazy_static::lazy_static;
use std::ops::Deref;
use uuid::Uuid;

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

macro_rules! define_relation_schema {
    ($const_name:ident {
        $relation:literal,
        $($field_name:ident: $dv:literal),+ $(,)?
    }) => {
        pub const $const_name: ExperimentalRelationSchema = ExperimentalRelationSchema {
            relation: $relation,
            fields: &[
                $(CozoField::new(stringify!($field_name), $dv)),+
            ],
        };
    };
}

#[derive(Copy, Clone)]
struct VectorDimensionSpec {
    dims: i64,
    provider: &'static str,
    embedding_model: &'static str,
    offset: f32,
    hnsw_m: i64,
    hnsw_ef_construction: i64,
    hnsw_search_ef: i64,
}

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

const VECTOR_DIMENSION_SPECS: [VectorDimensionSpec; 4] = [
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

lazy_static! {
    static ref SUPPORTED_DIMENSION_SET: HashSet<i64> = VECTOR_DIMENSION_SPECS
        .iter()
        .map(|spec| spec.dims)
        .collect();
}

fn supported_dimension_set() -> &'static HashSet<i64> {
    &SUPPORTED_DIMENSION_SET
}

pub trait ExperimentalEmbeddingDbExt {
    fn ensure_relation_registered(&self, relation_name: &str) -> Result<(), DbError>;
    fn assert_vector_column_layout(
        &self,
        relation_name: &str,
        dims: i64,
    ) -> Result<(), DbError>;
    fn enumerate_metadata_models(
        &self,
        relation_name: &str,
    ) -> Result<HashSet<(String, i64)>, DbError>;
    fn enumerate_vector_models(
        &self,
        relation_name: &str,
    ) -> Result<HashSet<(String, i64)>, DbError>;
}

impl ExperimentalEmbeddingDbExt for Db<MemStorage> {
    fn ensure_relation_registered(&self, relation_name: &str) -> Result<(), DbError> {
        let rows = self
            .run_script("::relations", BTreeMap::new(), ScriptMutability::Immutable)
            .map_err(|err| DbError::ExperimentalScriptFailure {
                action: "relations_lookup",
                relation: relation_name.to_string(),
                details: err.to_string(),
            })?;
        let found = rows.rows.iter().any(|row| {
            row.iter().any(|value| {
                value
                    .get_str()
                    .map(|name| name == relation_name)
                    .unwrap_or(false)
            })
        });
        if found {
            Ok(())
        } else {
            Err(DbError::ExperimentalRelationMissing {
                relation: relation_name.to_string(),
            })
        }
    }

    fn assert_vector_column_layout(
        &self,
        relation_name: &str,
        dims: i64,
    ) -> Result<(), DbError> {
        let script = format!("::columns {}", relation_name);
        let rows = self
            .run_script(&script, BTreeMap::new(), ScriptMutability::Immutable)
            .map_err(|err| DbError::ExperimentalScriptFailure {
                action: "columns_lookup",
                relation: relation_name.to_string(),
                details: err.to_string(),
            })?;
        let mut matches = 0;
        for row in &rows.rows {
            let column_name = row
                .get(0)
                .and_then(DataValue::get_str)
                .map(|s| s == "vector")
                .unwrap_or(false);
            let column_type = row
                .get(3)
                .and_then(DataValue::get_str)
                .map(|s| s == format!("<F32;{dims}>"))
                .unwrap_or(false);
            if column_name && column_type {
                matches += 1;
            }
        }
        if matches == 1 {
            Ok(())
        } else {
            Err(DbError::ExperimentalVectorLayoutMismatch {
                relation: relation_name.to_string(),
                dims,
            })
        }
    }

    fn enumerate_metadata_models(
        &self,
        relation_name: &str,
    ) -> Result<HashSet<(String, i64)>, DbError> {
        let query = format!(
            r#"
?[embeddings] :=
    *{rel}{{ embeddings @ 'NOW' }}
"#,
            rel = relation_name,
        );
        let rows = self
            .run_script(&query, BTreeMap::new(), ScriptMutability::Immutable)
            .map_err(|err| DbError::ExperimentalScriptFailure {
                action: "metadata_query",
                relation: relation_name.to_string(),
                details: err.to_string(),
            })?;
        let mut values = HashSet::new();
        for row in &rows.rows {
            for entry in parse_embedding_metadata(&row[0])? {
                values.insert(entry);
            }
        }
        Ok(values)
    }

    fn enumerate_vector_models(
        &self,
        relation_name: &str,
    ) -> Result<HashSet<(String, i64)>, DbError> {
        let query = format!(
            r#"
?[embedding_model, embedding_dims] :=
    *{rel}{{ embedding_model, embedding_dims @ 'NOW' }}
"#,
            rel = relation_name,
        );
        let rows = self
            .run_script(&query, BTreeMap::new(), ScriptMutability::Immutable)
            .map_err(|err| DbError::ExperimentalScriptFailure {
                action: "vector_query",
                relation: relation_name.to_string(),
                details: err.to_string(),
            })?;
        let mut entries = HashSet::new();
        for row in &rows.rows {
            let model = row[0]
                .get_str()
                .ok_or_else(|| DbError::ExperimentalMetadataParse {
                    reason: format!(
                        "embedding_model should be string for relation {relation_name}"
                    ),
                })?
                .to_string();
            let dims = match &row[1] {
                DataValue::Num(Num::Int(val)) => *val,
                other => {
                    return Err(DbError::ExperimentalMetadataParse {
                        reason: format!(
                            "embedding_dims must be integer for relation {relation_name}, got {other:?}"
                        ),
                    })
                }
            };
            entries.insert((model, dims));
        }
        Ok(entries)
    }
}

impl ExperimentalEmbeddingDbExt for Database {
    fn ensure_relation_registered(&self, relation_name: &str) -> Result<(), DbError> {
        <Db<MemStorage> as ExperimentalEmbeddingDbExt>::ensure_relation_registered(
            self.deref(),
            relation_name,
        )
    }

    fn assert_vector_column_layout(
        &self,
        relation_name: &str,
        dims: i64,
    ) -> Result<(), DbError> {
        <Db<MemStorage> as ExperimentalEmbeddingDbExt>::assert_vector_column_layout(
            self.deref(),
            relation_name,
            dims,
        )
    }

    fn enumerate_metadata_models(
        &self,
        relation_name: &str,
    ) -> Result<HashSet<(String, i64)>, DbError> {
        <Db<MemStorage> as ExperimentalEmbeddingDbExt>::enumerate_metadata_models(
            self.deref(),
            relation_name,
        )
    }

    fn enumerate_vector_models(
        &self,
        relation_name: &str,
    ) -> Result<HashSet<(String, i64)>, DbError> {
        <Db<MemStorage> as ExperimentalEmbeddingDbExt>::enumerate_vector_models(
            self.deref(),
            relation_name,
        )
    }
}

pub trait ExperimentalEmbeddingDatabaseExt: ExperimentalEmbeddingDbExt {
    fn create_idx(
        &self,
        relation_name: &str,
        dims: i64,
        m: i64,
        ef_construction: i64,
        distance: HnswDistance,
    ) -> Result<(), DbError>;
    fn search_embeddings_hnsw(
        &self,
        relation_name: &str,
        query_literal: &str,
        k: i64,
        ef: i64,
    ) -> Result<NamedRows, DbError>;
    fn vector_rows(&self, relation_name: &str, node_id: Uuid) -> Result<NamedRows, DbError>;
    fn vector_metadata_rows(
        &self,
        relation_name: &str,
        node_id: Uuid,
    ) -> Result<NamedRows, DbError>;
}

impl ExperimentalEmbeddingDatabaseExt for Database {
    fn create_idx(
        &self,
        relation_name: &str,
        dims: i64,
        m: i64,
        ef_construction: i64,
        distance: HnswDistance,
    ) -> Result<(), DbError> {
        let script = format!(
            r#"
::hnsw create {rel}:vector_idx {{
    fields: [vector],
    dim: {dims},
    dtype: F32,
    m: {m},
    ef_construction: {ef_construction},
    distance: {distance}
}}
"#,
            rel = relation_name,
            dims = dims,
            m = m,
            ef_construction = ef_construction,
            distance = distance.as_str(),
        );
        self.run_script(&script, BTreeMap::new(), ScriptMutability::Mutable)
            .map(|_| ())
            .map_err(|err| DbError::ExperimentalScriptFailure {
                action: "create_idx",
                relation: relation_name.to_string(),
                details: err.to_string(),
            })
    }

    fn search_embeddings_hnsw(
        &self,
        relation_name: &str,
        query_literal: &str,
        k: i64,
        ef: i64,
    ) -> Result<NamedRows, DbError> {
        let script = format!(
            r#"
?[node_id, distance] :=
    ~{rel}:vector_idx{{ node_id |
        query: {query},
        k: {k},
        ef: {ef},
        bind_distance: distance
    }}
"#,
            rel = relation_name,
            query = query_literal,
            k = k,
            ef = ef,
        );
        self.run_script(&script, BTreeMap::new(), ScriptMutability::Immutable)
            .map_err(|err| DbError::ExperimentalScriptFailure {
                action: "search_embeddings_hnsw",
                relation: relation_name.to_string(),
                details: err.to_string(),
            })
    }

    fn vector_rows(&self, relation_name: &str, node_id: Uuid) -> Result<NamedRows, DbError> {
        let script = format!(
            r#"
?[embedding_model, provider, embedding_dims, vector] :=
    *{rel}{{ node_id, embedding_model, provider, embedding_dims, vector @ 'NOW' }},
    node_id = to_uuid("{node_id}")
"#,
            rel = relation_name,
            node_id = node_id,
        );
        self.run_script(&script, BTreeMap::new(), ScriptMutability::Immutable)
            .map_err(|err| DbError::ExperimentalScriptFailure {
                action: "vector_rows",
                relation: relation_name.to_string(),
                details: err.to_string(),
            })
    }

    fn vector_metadata_rows(
        &self,
        relation_name: &str,
        node_id: Uuid,
    ) -> Result<NamedRows, DbError> {
        let script = format!(
            r#"
?[embeddings] :=
    *{rel}{{ id, embeddings @ 'NOW' }},
    id = to_uuid("{node_id}")
"#,
            rel = relation_name,
            node_id = node_id,
        );
        self.run_script(&script, BTreeMap::new(), ScriptMutability::Immutable)
            .map_err(|err| DbError::ExperimentalScriptFailure {
                action: "vector_metadata_rows",
                relation: relation_name.to_string(),
                details: err.to_string(),
            })
    }
}
#[derive(Copy, Clone, Debug)]
struct ExperimentalVectorRelation {
    dims: i64,
    relation_base: &'static str,
}

impl ExperimentalVectorRelation {
    fn try_new(dims: i64, relation_base: &'static str) -> Result<Self, DbError> {
        if supported_dimension_set().contains(&dims) {
            Ok(Self { dims, relation_base })
        } else {
            Err(DbError::UnsupportedEmbeddingDimension { dims })
        }
    }

    fn new(dims: i64, relation_base: &'static str) -> Self {
        Self::try_new(dims, relation_base).expect("dimension must be supported")
    }

    fn dims(&self) -> i64 {
        self.dims
    }

    fn relation_name(&self) -> String {
        format!("{}_{}", self.relation_base, self.dims)
    }

    fn script_identity(&self) -> String {
        format!(
            "{} {{ node_id, embedding_model, provider, at => embedding_dims, vector }}",
            self.relation_name()
        )
    }

    fn script_create(&self) -> String {
        format!(
            ":create {} {{ node_id: Uuid, embedding_model: String, provider: String, at: Validity => embedding_dims: Int, vector: <F32; {}> }}",
            self.relation_name(),
            self.dims
        )
    }
}

define_relation_schema!(FUNCTION_MULTI_EMBEDDING_SCHEMA {
    "function_multi_embedding",
    id: "Uuid",
    name: "String",
    docstring: "String?",
    vis_kind: "String",
    vis_path: "[String]?",
    span: "[Int; 2]",
    tracking_hash: "Uuid",
    cfgs: "[String]",
    return_type_id: "Uuid?",
    body: "String?",
    module_id: "Uuid",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(CONST_MULTI_EMBEDDING_SCHEMA {
    "const_multi_embedding",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    ty_id: "Uuid",
    value: "String?",
    docstring: "String?",
    tracking_hash: "Uuid",
    cfgs: "[String]",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(ENUM_MULTI_EMBEDDING_SCHEMA {
    "enum_multi_embedding",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    tracking_hash: "Uuid",
    cfgs: "[String]?",
    variants: "[Uuid]",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(IMPL_MULTI_EMBEDDING_SCHEMA {
    "impl_multi_embedding",
    id: "Uuid",
    self_type: "Uuid",
    span: "[Int; 2]",
    trait_type: "Uuid?",
    methods: "[Uuid]?",
    cfgs: "[String]",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(IMPORT_MULTI_EMBEDDING_SCHEMA {
    "import_multi_embedding",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String?",
    vis_path: "[String]?",
    cfgs: "[String]",
    source_path: "[String]",
    visible_name: "String",
    original_name: "String?",
    is_glob: "Bool",
    is_self_import: "Bool",
    import_kind: "String",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(MACRO_MULTI_EMBEDDING_SCHEMA {
    "macro_multi_embedding",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    body: "String?",
    tracking_hash: "Uuid",
    cfgs: "[String]",
    kind: "String",
    proc_kind: "String?",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(MODULE_MULTI_EMBEDDING_SCHEMA {
    "module_multi_embedding",
    id: "Uuid",
    name: "String",
    path: "[String]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    span: "[Int; 2]",
    tracking_hash: "Uuid",
    module_kind: "String",
    cfgs: "[String]",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(STATIC_MULTI_EMBEDDING_SCHEMA {
    "static_multi_embedding",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    ty_id: "Uuid",
    is_mutable: "Bool",
    value: "String?",
    docstring: "String?",
    tracking_hash: "Uuid",
    cfgs: "[String]",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(STRUCT_MULTI_EMBEDDING_SCHEMA {
    "struct_multi_embedding",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    tracking_hash: "Uuid",
    cfgs: "[String]?",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(TRAIT_MULTI_EMBEDDING_SCHEMA {
    "trait_multi_embedding",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    tracking_hash: "Uuid",
    cfgs: "[String]?",
    methods: "[Uuid]?",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(TYPE_ALIAS_MULTI_EMBEDDING_SCHEMA {
    "type_alias_multi_embedding",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    tracking_hash: "Uuid",
    cfgs: "[String]?",
    ty_id: "Uuid",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(UNION_MULTI_EMBEDDING_SCHEMA {
    "union_multi_embedding",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    tracking_hash: "Uuid",
    cfgs: "[String]?",
    embeddings: "[(String, Int)]"
});

fn embedding_entry(model: &str, dims: i64) -> DataValue {
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

fn insert_vector_row(
    db: &Db<MemStorage>,
    relation: &ExperimentalVectorRelation,
    node_id: Uuid,
    dim_spec: &VectorDimensionSpec,
) -> Result<(), DbError> {
    let identity = relation.script_identity();
    let literal = vector_literal(dim_spec.dims as usize, dim_spec.offset);
    let script = format!(
        r#"
?[node_id, embedding_model, provider, at, embedding_dims, vector] <- [[
    to_uuid("{node_id}"),
    "{embedding_model}",
    "{provider}",
    'ASSERT',
    {embedding_dims},
    {vector_literal}
]] :put {identity}
"#,
        node_id = node_id,
        embedding_model = dim_spec.embedding_model,
        provider = dim_spec.provider,
        embedding_dims = dim_spec.dims,
        vector_literal = literal,
        identity = identity,
    );
    db.run_script(&script, BTreeMap::new(), ScriptMutability::Mutable)
        .map_err(|err| DbError::ExperimentalScriptFailure {
            action: "insert_vector_row",
            relation: relation.relation_name(),
            details: err.to_string(),
        })?;
    Ok(())
}

fn parse_embedding_metadata(value: &DataValue) -> Result<Vec<(String, i64)>, DbError> {
    let entries = value.get_slice().ok_or_else(|| DbError::ExperimentalMetadataParse {
        reason: "embeddings column should contain a list".into(),
    })?;
    let mut parsed = Vec::new();
    for entry in entries {
        let tuple = entry.get_slice().ok_or_else(|| DbError::ExperimentalMetadataParse {
            reason: "embedding metadata tuple should be a list".into(),
        })?;
        if tuple.len() != 2 {
            return Err(DbError::ExperimentalMetadataParse {
                reason: "embedding metadata tuples must be (model, dims)".into(),
            });
        }
        let model = tuple[0].get_str().ok_or_else(|| DbError::ExperimentalMetadataParse {
            reason: "tuple[0] should be embedding model string".into(),
        })?;
        let dims = match &tuple[1] {
            DataValue::Num(Num::Int(val)) => *val,
            other => {
                return Err(DbError::ExperimentalMetadataParse {
                    reason: format!("tuple[1] must be integer dimensions, got {other:?}"),
                })
            }
        };
        parsed.push((model.to_string(), dims));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::{ExperimentalEmbeddingDatabaseExt, ExperimentalEmbeddingDbExt};

    struct SampleNodeData {
        node_id: Uuid,
        params: BTreeMap<String, DataValue>,
    }

    struct ExperimentalNodeSpec {
        name: &'static str,
        metadata_schema: &'static ExperimentalRelationSchema,
        vector_relation_base: &'static str,
        sample_builder: fn() -> SampleNodeData,
    }

    const EXPERIMENTAL_NODE_SPECS: &[ExperimentalNodeSpec] = &[
        ExperimentalNodeSpec {
            name: "function",
            metadata_schema: &FUNCTION_MULTI_EMBEDDING_SCHEMA,
            vector_relation_base: "function_embedding_vectors",
            sample_builder: sample_function_params,
        },
        ExperimentalNodeSpec {
            name: "const",
            metadata_schema: &CONST_MULTI_EMBEDDING_SCHEMA,
            vector_relation_base: "const_embedding_vectors",
            sample_builder: sample_const_params,
        },
        ExperimentalNodeSpec {
            name: "enum",
            metadata_schema: &ENUM_MULTI_EMBEDDING_SCHEMA,
            vector_relation_base: "enum_embedding_vectors",
            sample_builder: sample_enum_params,
        },
        ExperimentalNodeSpec {
            name: "impl",
            metadata_schema: &IMPL_MULTI_EMBEDDING_SCHEMA,
            vector_relation_base: "impl_embedding_vectors",
            sample_builder: sample_impl_params,
        },
        ExperimentalNodeSpec {
            name: "import",
            metadata_schema: &IMPORT_MULTI_EMBEDDING_SCHEMA,
            vector_relation_base: "import_embedding_vectors",
            sample_builder: sample_import_params,
        },
        ExperimentalNodeSpec {
            name: "macro",
            metadata_schema: &MACRO_MULTI_EMBEDDING_SCHEMA,
            vector_relation_base: "macro_embedding_vectors",
            sample_builder: sample_macro_params,
        },
        ExperimentalNodeSpec {
            name: "module",
            metadata_schema: &MODULE_MULTI_EMBEDDING_SCHEMA,
            vector_relation_base: "module_embedding_vectors",
            sample_builder: sample_module_params,
        },
        ExperimentalNodeSpec {
            name: "static",
            metadata_schema: &STATIC_MULTI_EMBEDDING_SCHEMA,
            vector_relation_base: "static_embedding_vectors",
            sample_builder: sample_static_params,
        },
        ExperimentalNodeSpec {
            name: "struct",
            metadata_schema: &STRUCT_MULTI_EMBEDDING_SCHEMA,
            vector_relation_base: "struct_embedding_vectors",
            sample_builder: sample_struct_params,
        },
        ExperimentalNodeSpec {
            name: "trait",
            metadata_schema: &TRAIT_MULTI_EMBEDDING_SCHEMA,
            vector_relation_base: "trait_embedding_vectors",
            sample_builder: sample_trait_params,
        },
        ExperimentalNodeSpec {
            name: "type_alias",
            metadata_schema: &TYPE_ALIAS_MULTI_EMBEDDING_SCHEMA,
            vector_relation_base: "type_alias_embedding_vectors",
            sample_builder: sample_type_alias_params,
        },
        ExperimentalNodeSpec {
            name: "union",
            metadata_schema: &UNION_MULTI_EMBEDDING_SCHEMA,
            vector_relation_base: "union_embedding_vectors",
            sample_builder: sample_union_params,
        },
    ];

    fn seed_metadata_relation(
        spec: &ExperimentalNodeSpec,
    ) -> Result<(Database, SampleNodeData), DbError> {
        let db = init_db();
        let relation_name = spec.metadata_schema.relation().to_string();
        db.run_script(
            &spec.metadata_schema.script_create(),
            BTreeMap::new(),
            ScriptMutability::Mutable,
        )
        .map_err(|err| DbError::ExperimentalScriptFailure {
            action: "schema_create",
            relation: relation_name,
            details: err.to_string(),
        })?;

        let sample = (spec.sample_builder)();
        insert_metadata_sample(&db, spec, &sample)?;
        Ok((db, sample))
    }

    fn insert_metadata_sample(
        db: &Database,
        spec: &ExperimentalNodeSpec,
        sample: &SampleNodeData,
    ) -> Result<(), DbError> {
        let insert_script = spec.metadata_schema.script_put(&sample.params);
        db.run_script(
            &insert_script,
            sample.params.clone(),
            ScriptMutability::Mutable,
        )
        .map_err(|err| DbError::ExperimentalScriptFailure {
            action: "metadata_insert",
            relation: spec.metadata_schema.relation().to_string(),
            details: err.to_string(),
        })
        .map(|_| ())
    }

    fn default_spec() -> &'static ExperimentalNodeSpec {
        &EXPERIMENTAL_NODE_SPECS[0]
    }

    fn seed_vector_relation_for_node(
        db: &Database,
        spec: &ExperimentalNodeSpec,
        node_id: Uuid,
        dim_spec: &VectorDimensionSpec,
    ) -> Result<ExperimentalVectorRelation, DbError> {
        let vector_relation =
            ExperimentalVectorRelation::new(dim_spec.dims, spec.vector_relation_base);
        let relation_name = vector_relation.relation_name();
        match db.ensure_relation_registered(&relation_name) {
            Ok(()) => {}
            Err(DbError::ExperimentalRelationMissing { .. }) => {
                let create_script = vector_relation.script_create();
                db.run_script(&create_script, BTreeMap::new(), ScriptMutability::Mutable)
                    .map_err(|err| DbError::ExperimentalScriptFailure {
                        action: "vector_relation_create",
                        relation: relation_name.clone(),
                        details: err.to_string(),
                    })?;
            }
            Err(other) => return Err(other),
        }
        insert_vector_row(db, &vector_relation, node_id, dim_spec)?;
        Ok(vector_relation)
    }

    #[test]
    fn validates_multi_embedding_schema_for_all_nodes() {
        for spec in EXPERIMENTAL_NODE_SPECS {
            validate_schema_spec(spec).expect("multi embedding schema validation");
        }
    }

    #[test]
    fn rejects_unsupported_embedding_dimension() {
        let err = ExperimentalVectorRelation::try_new(999, "function_embedding_vectors")
            .expect_err("unsupported dims should return error");
        assert!(matches!(
            err,
            DbError::UnsupportedEmbeddingDimension { dims } if dims == 999
        ));
    }

    #[test]
    fn adapter_vector_metadata_rows_returns_row() {
        let spec = default_spec();
        let (db, sample) = seed_metadata_relation(spec).expect("seed metadata");
        let rows = db
            .vector_metadata_rows(spec.metadata_schema.relation(), sample.node_id)
            .expect("metadata rows");
        assert_eq!(rows.rows.len(), 1, "expected single metadata row");
    }

    #[test]
    fn adapter_vector_metadata_rows_errors_when_relation_missing() {
        let db = init_db();
        let err = db
            .vector_metadata_rows("unknown_relation", Uuid::new_v4())
            .expect_err("missing metadata relation should error");
        assert!(matches!(
            err,
            DbError::ExperimentalScriptFailure { action, .. } if action == "vector_metadata_rows"
        ));
    }

    #[test]
    fn adapter_vector_rows_returns_row() {
        let spec = default_spec();
        let (db, sample) = seed_metadata_relation(spec).expect("seed metadata");
        let dim_spec = &VECTOR_DIMENSION_SPECS[0];
        let relation =
            seed_vector_relation_for_node(&db, spec, sample.node_id, dim_spec).expect("vector");
        let rows = db
            .vector_rows(&relation.relation_name(), sample.node_id)
            .expect("vector rows");
        assert_eq!(rows.rows.len(), 1, "expected vector row");
    }

    #[test]
    fn adapter_vector_rows_errors_when_relation_missing() {
        let db = init_db();
        let err = db
            .vector_rows("unknown_relation", Uuid::new_v4())
            .expect_err("missing vector relation should error");
        assert!(matches!(
            err,
            DbError::ExperimentalScriptFailure { action, .. } if action == "vector_rows"
        ));
    }

    #[test]
    fn adapter_create_idx_succeeds_for_existing_relation() {
        let spec = default_spec();
        let (db, sample) = seed_metadata_relation(spec).expect("seed metadata");
        let dim_spec = &VECTOR_DIMENSION_SPECS[0];
        let relation =
            seed_vector_relation_for_node(&db, spec, sample.node_id, dim_spec).expect("vector");
        db.create_idx(
            &relation.relation_name(),
            dim_spec.dims,
            dim_spec.hnsw_m,
            dim_spec.hnsw_ef_construction,
            HnswDistance::L2,
        )
        .expect("create idx");
    }

    #[test]
    fn adapter_create_idx_errors_when_relation_missing() {
        let db = init_db();
        let err = db
            .create_idx("unknown_relation", 384, 16, 64, HnswDistance::L2)
            .expect_err("missing relation should error");
        assert!(matches!(
            err,
            DbError::ExperimentalScriptFailure { action, .. } if action == "create_idx"
        ));
    }

    #[test]
    fn adapter_search_embeddings_hnsw_returns_match() {
        let spec = default_spec();
        let (db, sample) = seed_metadata_relation(spec).expect("seed metadata");
        let dim_spec = &VECTOR_DIMENSION_SPECS[0];
        let relation =
            seed_vector_relation_for_node(&db, spec, sample.node_id, dim_spec).expect("vector");
        db.create_idx(
            &relation.relation_name(),
            dim_spec.dims,
            dim_spec.hnsw_m,
            dim_spec.hnsw_ef_construction,
            HnswDistance::L2,
        )
        .expect("create idx");
        let query_vec = vector_literal(dim_spec.dims as usize, dim_spec.offset);
        let rows = db
            .search_embeddings_hnsw(
                &relation.relation_name(),
                &query_vec,
                1,
                dim_spec.hnsw_search_ef,
            )
            .expect("search rows");
        assert_eq!(rows.rows.len(), 1, "expected single search result");
    }

    #[test]
    fn adapter_search_embeddings_hnsw_errors_without_index() {
        let spec = default_spec();
        let (db, sample) = seed_metadata_relation(spec).expect("seed metadata");
        let dim_spec = &VECTOR_DIMENSION_SPECS[0];
        let relation =
            seed_vector_relation_for_node(&db, spec, sample.node_id, dim_spec).expect("vector");
        let query_vec = vector_literal(dim_spec.dims as usize, dim_spec.offset);
        let err = db
            .search_embeddings_hnsw(
                &relation.relation_name(),
                &query_vec,
                1,
                dim_spec.hnsw_search_ef,
            )
            .expect_err("missing index should error");
        assert!(matches!(
            err,
            DbError::ExperimentalScriptFailure { action, .. } if action == "search_embeddings_hnsw"
        ));
    }

    #[test]
    fn adapter_vector_rows_returns_empty_for_unknown_node() {
        let spec = default_spec();
        let (db, sample) = seed_metadata_relation(spec).expect("seed metadata");
        let dim_spec = &VECTOR_DIMENSION_SPECS[0];
        let relation =
            seed_vector_relation_for_node(&db, spec, sample.node_id, dim_spec).expect("vector");
        let random_node = Uuid::new_v4();
        let rows = db
            .vector_rows(&relation.relation_name(), random_node)
            .expect("vector rows");
        assert!(
            rows.rows.is_empty(),
            "expected empty rows for unknown node id"
        );
    }

    #[test]
    fn parse_embedding_metadata_errors_on_invalid_tuple() {
        let bad_value = DataValue::List(vec![DataValue::List(vec![
            DataValue::Str("test-model".into()),
            DataValue::Str("not-an-int".into()),
        ])]);
        let err = parse_embedding_metadata(&bad_value)
            .expect_err("invalid tuple should raise metadata parse error");
        assert!(matches!(
            err,
            DbError::ExperimentalMetadataParse { reason } if reason.contains("tuple[1]")
        ));
    }

    #[test]
    fn adapter_search_embeddings_hnsw_returns_multiple_results_when_k_is_two() {
        let spec = default_spec();
        let (db, sample) = seed_metadata_relation(spec).expect("seed metadata");
        let dim_spec = &VECTOR_DIMENSION_SPECS[0];
        let relation =
            seed_vector_relation_for_node(&db, spec, sample.node_id, dim_spec).expect("vector");
        let second_sample = (spec.sample_builder)();
        insert_metadata_sample(&db, spec, &second_sample).expect("insert second metadata");
        insert_vector_row(&db, &relation, second_sample.node_id, dim_spec)
            .expect("insert second vector");
        db.create_idx(
            &relation.relation_name(),
            dim_spec.dims,
            dim_spec.hnsw_m,
            dim_spec.hnsw_ef_construction,
            HnswDistance::L2,
        )
        .expect("create idx");
        let query_vec = vector_literal(dim_spec.dims as usize, dim_spec.offset);
        let rows = db
            .search_embeddings_hnsw(
                &relation.relation_name(),
                &query_vec,
                2,
                dim_spec.hnsw_search_ef,
            )
            .expect("search rows");
        assert!(
            rows.rows.len() >= 2,
            "expected at least two search results for k=2"
        );
    }

    fn validate_schema_spec(spec: &ExperimentalNodeSpec) -> Result<(), DbError> {
        let (db, sample) = seed_metadata_relation(spec)?;
        let relation_name = spec.metadata_schema.relation().to_string();

        let metadata_rows = db.vector_metadata_rows(&relation_name, sample.node_id)?;
        if metadata_rows.rows.len() != 1 {
            return Err(DbError::ExperimentalMetadataParse {
                reason: format!("expected metadata row for {}", spec.name),
            });
        }
        let metadata_entries = parse_embedding_metadata(&metadata_rows.rows[0][0])?;
        assert_eq!(
            metadata_entries.len(),
            VECTOR_DIMENSION_SPECS.len(),
            "expected {} metadata tuples for {}",
            VECTOR_DIMENSION_SPECS.len(),
            spec.name
        );
        let metadata_set: HashSet<(String, i64)> =
            metadata_entries.into_iter().collect::<HashSet<_>>();
        let enumerated_metadata = db
            .enumerate_metadata_models(spec.metadata_schema.relation())
            .expect("enumerate metadata");
        assert_eq!(
            metadata_set, enumerated_metadata,
            "metadata enumeration should match parsed tuples for {}",
            spec.name
        );

        let mut vector_set = HashSet::new();
        let mut observed_dim_relations = HashSet::new();

        for dim_spec in VECTOR_DIMENSION_SPECS {
            let vector_relation =
                seed_vector_relation_for_node(&db, spec, sample.node_id, &dim_spec)?;

            let relation_name = vector_relation.relation_name();
            db.ensure_relation_registered(&relation_name)
                .expect("relation should exist");
            db.assert_vector_column_layout(&relation_name, dim_spec.dims)
                .expect("vector layout must match");

            let vector_rows = db.vector_rows(&relation_name, sample.node_id)?;
            if vector_rows.rows.len() != 1 {
                return Err(DbError::ExperimentalMetadataParse {
                    reason: format!(
                        "expected single vector row for {} dimension {}",
                        spec.name, dim_spec.dims
                    ),
                });
            }
            let row = &vector_rows.rows[0];
            let model = row[0]
                .get_str()
                .ok_or_else(|| DbError::ExperimentalMetadataParse {
                    reason: "embedding_model must be string".into(),
                })?
                .to_string();
            let provider = row[1]
                .get_str()
                .ok_or_else(|| DbError::ExperimentalMetadataParse {
                    reason: "provider must be string".into(),
                })?
                .to_string();
            let dims_val = match &row[2] {
                DataValue::Num(Num::Int(val)) => *val,
                other => {
                    return Err(DbError::ExperimentalMetadataParse {
                        reason: format!("embedding_dims must be integer, got {other:?}"),
                    })
                }
            };
            assert_eq!(
                dims_val, dim_spec.dims,
                "embedding_dims must match relation dimension"
            );
            assert!(
                provider == dim_spec.provider,
                "provider mismatch for {}",
                spec.name
            );
            assert!(
                !matches!(row[3], DataValue::Null),
                "vector column must be populated for {} ({})",
                spec.name,
                dim_spec.dims
            );
            vector_set.insert((model, dims_val));
            observed_dim_relations.insert(dims_val);
        }

        assert_eq!(
            metadata_set, vector_set,
            "metadata tuples must align with vector rows for {}",
            spec.name
        );
        let mut enumerated_vectors = HashSet::new();
        for dim_spec in VECTOR_DIMENSION_SPECS {
            let relation =
                ExperimentalVectorRelation::new(dim_spec.dims, spec.vector_relation_base);
            enumerated_vectors.extend(
                db.enumerate_vector_models(&relation.relation_name())
                    .expect("enumerate vectors"),
            );
        }
        assert_eq!(
            enumerated_vectors, metadata_set,
            "vector enumeration should list the same models for {}",
            spec.name
        );
        assert_eq!(
            observed_dim_relations,
            supported_dimension_set().clone(),
            "must observe every supported dimension relation for {}",
            spec.name
        );

        let second_sample = (spec.sample_builder)();
        let second_node_id = second_sample.node_id;
        let second_insert = spec.metadata_schema.script_put(&second_sample.params);
        db.run_script(
            &second_insert,
            second_sample.params.clone(),
            ScriptMutability::Mutable,
        )
        .map_err(|err| DbError::ExperimentalScriptFailure {
            action: "metadata_insert",
            relation: relation_name.clone(),
            details: err.to_string(),
        })?;
        for dim_spec in VECTOR_DIMENSION_SPECS {
            let vector_relation =
                ExperimentalVectorRelation::new(dim_spec.dims, spec.vector_relation_base);
            insert_vector_row(&db, &vector_relation, second_sample.node_id, &dim_spec)?;
        }

        let dedup_metadata = db
            .enumerate_metadata_models(spec.metadata_schema.relation())
            .expect("dedup metadata");
        assert_eq!(
            dedup_metadata, metadata_set,
            "metadata enumeration must dedupe across rows for {}",
            spec.name
        );
        let mut dedup_vectors = HashSet::new();
        for dim_spec in VECTOR_DIMENSION_SPECS {
            let relation =
                ExperimentalVectorRelation::new(dim_spec.dims, spec.vector_relation_base);
            dedup_vectors.extend(
                db.enumerate_vector_models(&relation.relation_name())
                    .expect("dedup vectors"),
            );
        }
        assert_eq!(
            dedup_vectors, metadata_set,
            "vector enumeration must dedupe across rows for {}",
            spec.name
        );

        for dim_spec in VECTOR_DIMENSION_SPECS {
            let relation =
                ExperimentalVectorRelation::new(dim_spec.dims, spec.vector_relation_base);
            let relation_name = relation.relation_name();
            db.create_idx(
                &relation_name,
                dim_spec.dims,
                dim_spec.hnsw_m,
                dim_spec.hnsw_ef_construction,
                HnswDistance::L2,
            )?;

            let query_vec = vector_literal(dim_spec.dims as usize, dim_spec.offset);
            let search_rows =
                db.search_embeddings_hnsw(&relation_name, &query_vec, 1, dim_spec.hnsw_search_ef)?;
            if search_rows.rows.len() != 1 {
                return Err(DbError::ExperimentalMetadataParse {
                    reason: format!(
                        "expected single HNSW match for {} ({})",
                        spec.name, dim_spec.dims
                    ),
                });
            }
            match search_rows.rows[0][0] {
                DataValue::Uuid(UuidWrapper(id)) => {
                    if id != sample.node_id && id != second_node_id {
                        return Err(DbError::ExperimentalMetadataParse {
                            reason: format!(
                                "unexpected node id {id} returned for {} ({})",
                                spec.name, dim_spec.dims
                            ),
                        });
                    }
                }
                _ => {
                    return Err(DbError::ExperimentalMetadataParse {
                        reason: "expected uuid result from HNSW query".into(),
                    })
                }
            }
        }
        Ok(())
    }

    fn init_db() -> Database {
        let db = Db::new(MemStorage::default()).expect("create db");
        db.initialize().expect("init db");
        Database::new(db)
    }

    fn metadata_embeddings() -> DataValue {
        DataValue::List(
            VECTOR_DIMENSION_SPECS
                .iter()
                .map(|spec| embedding_entry(spec.embedding_model, spec.dims))
                .collect(),
        )
    }

    fn sample_span() -> DataValue {
        DataValue::List(vec![
            DataValue::Num(Num::Int(0)),
            DataValue::Num(Num::Int(42)),
        ])
    }

    fn cfg_list(values: &[&str]) -> DataValue {
        DataValue::List(values.iter().map(|v| DataValue::Str((*v).into())).collect())
    }

    fn string_list(values: &[&str]) -> DataValue {
        DataValue::List(values.iter().map(|v| DataValue::Str((*v).into())).collect())
    }

    fn sample_function_params() -> SampleNodeData {
        let function_id = Uuid::new_v4();
        let module_id = Uuid::new_v4();
        let mut params = BTreeMap::new();
        params.insert("id".into(), DataValue::Uuid(UuidWrapper(function_id)));
        params.insert(
            "name".into(),
            DataValue::Str("experimental_function".into()),
        );
        params.insert("docstring".into(), DataValue::Null);
        params.insert("vis_kind".into(), DataValue::Str("public".into()));
        params.insert("vis_path".into(), DataValue::Null);
        params.insert("span".into(), sample_span());
        params.insert(
            "tracking_hash".into(),
            DataValue::Uuid(UuidWrapper(Uuid::new_v4())),
        );
        params.insert("cfgs".into(), cfg_list(&["default"]));
        params.insert("return_type_id".into(), DataValue::Null);
        params.insert("body".into(), DataValue::Null);
        params.insert("module_id".into(), DataValue::Uuid(UuidWrapper(module_id)));
        params.insert("embeddings".into(), metadata_embeddings());

        SampleNodeData {
            node_id: function_id,
            params,
        }
    }

    fn sample_const_params() -> SampleNodeData {
        let const_id = Uuid::new_v4();
        let ty_id = Uuid::new_v4();
        let mut params = BTreeMap::new();
        params.insert("id".into(), DataValue::Uuid(UuidWrapper(const_id)));
        params.insert("name".into(), DataValue::Str("CONST_VALUE".into()));
        params.insert("span".into(), sample_span());
        params.insert("vis_kind".into(), DataValue::Str("public".into()));
        params.insert("vis_path".into(), DataValue::Null);
        params.insert("ty_id".into(), DataValue::Uuid(UuidWrapper(ty_id)));
        params.insert("value".into(), DataValue::Str("42".into()));
        params.insert("docstring".into(), DataValue::Null);
        params.insert(
            "tracking_hash".into(),
            DataValue::Uuid(UuidWrapper(Uuid::new_v4())),
        );
        params.insert("cfgs".into(), cfg_list(&["default"]));
        params.insert("embeddings".into(), metadata_embeddings());

        SampleNodeData {
            node_id: const_id,
            params,
        }
    }

    fn sample_enum_params() -> SampleNodeData {
        let enum_id = Uuid::new_v4();
        let mut params = BTreeMap::new();
        params.insert("id".into(), DataValue::Uuid(UuidWrapper(enum_id)));
        params.insert("name".into(), DataValue::Str("ExampleEnum".into()));
        params.insert("span".into(), sample_span());
        params.insert("vis_kind".into(), DataValue::Str("public".into()));
        params.insert("vis_path".into(), DataValue::Null);
        params.insert("docstring".into(), DataValue::Null);
        params.insert(
            "tracking_hash".into(),
            DataValue::Uuid(UuidWrapper(Uuid::new_v4())),
        );
        params.insert("cfgs".into(), DataValue::Null);
        params.insert(
            "variants".into(),
            DataValue::List(vec![DataValue::Uuid(UuidWrapper(Uuid::new_v4()))]),
        );
        params.insert("embeddings".into(), metadata_embeddings());

        SampleNodeData {
            node_id: enum_id,
            params,
        }
    }

    fn sample_impl_params() -> SampleNodeData {
        let impl_id = Uuid::new_v4();
        let mut params = BTreeMap::new();
        params.insert("id".into(), DataValue::Uuid(UuidWrapper(impl_id)));
        params.insert(
            "self_type".into(),
            DataValue::Uuid(UuidWrapper(Uuid::new_v4())),
        );
        params.insert("span".into(), sample_span());
        params.insert("trait_type".into(), DataValue::Null);
        params.insert("methods".into(), DataValue::Null);
        params.insert("cfgs".into(), cfg_list(&["default"]));
        params.insert("embeddings".into(), metadata_embeddings());

        SampleNodeData {
            node_id: impl_id,
            params,
        }
    }

    fn sample_import_params() -> SampleNodeData {
        let import_id = Uuid::new_v4();
        let mut params = BTreeMap::new();
        params.insert("id".into(), DataValue::Uuid(UuidWrapper(import_id)));
        params.insert("name".into(), DataValue::Str("imported_item".into()));
        params.insert("span".into(), sample_span());
        params.insert("vis_kind".into(), DataValue::Null);
        params.insert("vis_path".into(), DataValue::Null);
        params.insert("cfgs".into(), cfg_list(&["test"]));
        params.insert("source_path".into(), string_list(&["crate", "module"]));
        params.insert("visible_name".into(), DataValue::Str("visible_item".into()));
        params.insert(
            "original_name".into(),
            DataValue::Str("original_item".into()),
        );
        params.insert("is_glob".into(), DataValue::Bool(false));
        params.insert("is_self_import".into(), DataValue::Bool(false));
        params.insert("import_kind".into(), DataValue::Str("Named".into()));
        params.insert("embeddings".into(), metadata_embeddings());

        SampleNodeData {
            node_id: import_id,
            params,
        }
    }

    fn sample_macro_params() -> SampleNodeData {
        let macro_id = Uuid::new_v4();
        let mut params = BTreeMap::new();
        params.insert("id".into(), DataValue::Uuid(UuidWrapper(macro_id)));
        params.insert("name".into(), DataValue::Str("example_macro".into()));
        params.insert("span".into(), sample_span());
        params.insert("vis_kind".into(), DataValue::Str("public".into()));
        params.insert("vis_path".into(), DataValue::Null);
        params.insert("docstring".into(), DataValue::Null);
        params.insert("body".into(), DataValue::Str("body".into()));
        params.insert(
            "tracking_hash".into(),
            DataValue::Uuid(UuidWrapper(Uuid::new_v4())),
        );
        params.insert("cfgs".into(), cfg_list(&["macro"]));
        params.insert("kind".into(), DataValue::Str("Declarative".into()));
        params.insert("proc_kind".into(), DataValue::Null);
        params.insert("embeddings".into(), metadata_embeddings());

        SampleNodeData {
            node_id: macro_id,
            params,
        }
    }

    fn sample_module_params() -> SampleNodeData {
        let module_id = Uuid::new_v4();
        let mut params = BTreeMap::new();
        params.insert("id".into(), DataValue::Uuid(UuidWrapper(module_id)));
        params.insert("name".into(), DataValue::Str("module".into()));
        params.insert("path".into(), string_list(&["crate", "module"]));
        params.insert("vis_kind".into(), DataValue::Str("public".into()));
        params.insert("vis_path".into(), DataValue::Null);
        params.insert("docstring".into(), DataValue::Null);
        params.insert("span".into(), sample_span());
        params.insert(
            "tracking_hash".into(),
            DataValue::Uuid(UuidWrapper(Uuid::new_v4())),
        );
        params.insert("module_kind".into(), DataValue::Str("Inline".into()));
        params.insert("cfgs".into(), cfg_list(&["module"]));
        params.insert("embeddings".into(), metadata_embeddings());

        SampleNodeData {
            node_id: module_id,
            params,
        }
    }

    fn sample_static_params() -> SampleNodeData {
        let static_id = Uuid::new_v4();
        let mut params = BTreeMap::new();
        params.insert("id".into(), DataValue::Uuid(UuidWrapper(static_id)));
        params.insert("name".into(), DataValue::Str("STATIC_VAL".into()));
        params.insert("span".into(), sample_span());
        params.insert("vis_kind".into(), DataValue::Str("public".into()));
        params.insert("vis_path".into(), DataValue::Null);
        params.insert("ty_id".into(), DataValue::Uuid(UuidWrapper(Uuid::new_v4())));
        params.insert("is_mutable".into(), DataValue::Bool(true));
        params.insert("value".into(), DataValue::Str("value".into()));
        params.insert("docstring".into(), DataValue::Null);
        params.insert(
            "tracking_hash".into(),
            DataValue::Uuid(UuidWrapper(Uuid::new_v4())),
        );
        params.insert("cfgs".into(), cfg_list(&["static"]));
        params.insert("embeddings".into(), metadata_embeddings());

        SampleNodeData {
            node_id: static_id,
            params,
        }
    }

    fn sample_struct_params() -> SampleNodeData {
        let struct_id = Uuid::new_v4();
        let mut params = BTreeMap::new();
        params.insert("id".into(), DataValue::Uuid(UuidWrapper(struct_id)));
        params.insert("name".into(), DataValue::Str("ExampleStruct".into()));
        params.insert("span".into(), sample_span());
        params.insert("vis_kind".into(), DataValue::Str("public".into()));
        params.insert("vis_path".into(), DataValue::Null);
        params.insert("docstring".into(), DataValue::Null);
        params.insert(
            "tracking_hash".into(),
            DataValue::Uuid(UuidWrapper(Uuid::new_v4())),
        );
        params.insert("cfgs".into(), DataValue::Null);
        params.insert("embeddings".into(), metadata_embeddings());

        SampleNodeData {
            node_id: struct_id,
            params,
        }
    }

    fn sample_trait_params() -> SampleNodeData {
        let trait_id = Uuid::new_v4();
        let mut params = BTreeMap::new();
        params.insert("id".into(), DataValue::Uuid(UuidWrapper(trait_id)));
        params.insert("name".into(), DataValue::Str("ExampleTrait".into()));
        params.insert("span".into(), sample_span());
        params.insert("vis_kind".into(), DataValue::Str("public".into()));
        params.insert("vis_path".into(), DataValue::Null);
        params.insert("docstring".into(), DataValue::Null);
        params.insert(
            "tracking_hash".into(),
            DataValue::Uuid(UuidWrapper(Uuid::new_v4())),
        );
        params.insert("cfgs".into(), DataValue::Null);
        params.insert("methods".into(), DataValue::Null);
        params.insert("embeddings".into(), metadata_embeddings());

        SampleNodeData {
            node_id: trait_id,
            params,
        }
    }

    fn sample_type_alias_params() -> SampleNodeData {
        let alias_id = Uuid::new_v4();
        let mut params = BTreeMap::new();
        params.insert("id".into(), DataValue::Uuid(UuidWrapper(alias_id)));
        params.insert("name".into(), DataValue::Str("Alias".into()));
        params.insert("span".into(), sample_span());
        params.insert("vis_kind".into(), DataValue::Str("public".into()));
        params.insert("vis_path".into(), DataValue::Null);
        params.insert("docstring".into(), DataValue::Null);
        params.insert(
            "tracking_hash".into(),
            DataValue::Uuid(UuidWrapper(Uuid::new_v4())),
        );
        params.insert("cfgs".into(), DataValue::Null);
        params.insert("ty_id".into(), DataValue::Uuid(UuidWrapper(Uuid::new_v4())));
        params.insert("embeddings".into(), metadata_embeddings());

        SampleNodeData {
            node_id: alias_id,
            params,
        }
    }

    fn sample_union_params() -> SampleNodeData {
        let union_id = Uuid::new_v4();
        let mut params = BTreeMap::new();
        params.insert("id".into(), DataValue::Uuid(UuidWrapper(union_id)));
        params.insert("name".into(), DataValue::Str("ExampleUnion".into()));
        params.insert("span".into(), sample_span());
        params.insert("vis_kind".into(), DataValue::Str("public".into()));
        params.insert("vis_path".into(), DataValue::Null);
        params.insert("docstring".into(), DataValue::Null);
        params.insert(
            "tracking_hash".into(),
            DataValue::Uuid(UuidWrapper(Uuid::new_v4())),
        );
        params.insert("cfgs".into(), DataValue::Null);
        params.insert("embeddings".into(), metadata_embeddings());

        SampleNodeData {
            node_id: union_id,
            params,
        }
    }

}
