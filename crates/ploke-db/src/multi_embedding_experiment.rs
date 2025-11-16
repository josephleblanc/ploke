use std::collections::{BTreeMap, HashSet};

use cozo::{self, DataValue, Db, MemStorage, Num, ScriptMutability, UuidWrapper};
use itertools::Itertools;
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

fn supported_dimension_set() -> HashSet<i64> {
    VECTOR_DIMENSION_SPECS
        .iter()
        .map(|spec| spec.dims)
        .collect()
}

#[derive(Copy, Clone)]
struct ExperimentalVectorRelation {
    dims: i64,
    relation_base: &'static str,
}

impl ExperimentalVectorRelation {
    const fn new(dims: i64, relation_base: &'static str) -> Self {
        Self { dims, relation_base }
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
) -> Result<(), cozo::Error> {
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
    db.run_script(&script, BTreeMap::new(), ScriptMutability::Mutable)?;
    Ok(())
}

fn ensure_relation_registered(db: &Db<MemStorage>, relation_name: &str) {
    let rows = db
        .run_script("::relations", BTreeMap::new(), ScriptMutability::Immutable)
        .unwrap_or_else(|err| panic!("query ::relations failed for {relation_name}: {err}"));
    let mut found = false;
    for row in &rows.rows {
        if row.iter().any(|value| {
            value
                .get_str()
                .map(|name| name == relation_name)
                .unwrap_or(false)
        }) {
            found = true;
            break;
        }
    }
    assert!(
        found,
        "expected relation {} to be registered",
        relation_name
    );
}

fn assert_vector_column_layout(db: &Db<MemStorage>, relation_name: &str, dims: i64) {
    let script = format!("::columns {}", relation_name);
    let rows = db
        .run_script(&script, BTreeMap::new(), ScriptMutability::Immutable)
        .unwrap_or_else(|err| panic!("query ::columns failed for {relation_name}: {err}"));
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
    assert!(
        matches == 1,
        "expected vector column for {relation_name} with dims {dims}, rows: {:?}",
        rows.rows
    );
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn validates_multi_embedding_schema_for_all_nodes() {
        for spec in EXPERIMENTAL_NODE_SPECS {
            validate_schema_spec(spec);
        }
    }

    fn validate_schema_spec(spec: &ExperimentalNodeSpec) {
        let db = init_db();
        db.run_script(
            &spec.metadata_schema.script_create(),
            BTreeMap::new(),
            ScriptMutability::Mutable,
        )
        .unwrap_or_else(|err| panic!("create {} relation failed: {err}", spec.name));

        let sample = (spec.sample_builder)();
        let insert_script = spec.metadata_schema.script_put(&sample.params);
        db.run_script(
            &insert_script,
            sample.params.clone(),
            ScriptMutability::Mutable,
        )
        .unwrap_or_else(|err| panic!("insert {} row failed: {err}", spec.name));

        let metadata_query = format!(
            r#"
?[embeddings] :=
    *{rel}{{ id, embeddings @ 'NOW' }},
    id = to_uuid("{node_id}")
"#,
            rel = spec.metadata_schema.relation(),
            node_id = sample.node_id,
        );
        let metadata_rows = db
            .run_script(
                &metadata_query,
                BTreeMap::new(),
                ScriptMutability::Immutable,
            )
            .unwrap_or_else(|err| panic!("query {} metadata failed: {err}", spec.name));
        assert_eq!(
            metadata_rows.rows.len(),
            1,
            "expected metadata row for {}",
            spec.name
        );
        let metadata_entries = parse_embedding_metadata(&metadata_rows.rows[0][0]);
        assert_eq!(
            metadata_entries.len(),
            VECTOR_DIMENSION_SPECS.len(),
            "expected {} metadata tuples for {}",
            VECTOR_DIMENSION_SPECS.len(),
            spec.name
        );
        let metadata_set: HashSet<(String, i64)> =
            metadata_entries.into_iter().collect::<HashSet<_>>();

        let mut vector_set = HashSet::new();
        let mut observed_dim_relations = HashSet::new();

        for dim_spec in VECTOR_DIMENSION_SPECS {
            let vector_relation =
                ExperimentalVectorRelation::new(dim_spec.dims, spec.vector_relation_base);
            let create_script = vector_relation.script_create();
            db.run_script(&create_script, BTreeMap::new(), ScriptMutability::Mutable)
                .unwrap_or_else(|err| panic!("create {} vectors failed: {err}", spec.name));
            insert_vector_row(&db, &vector_relation, sample.node_id, &dim_spec)
                .unwrap_or_else(|err| panic!("insert {} vectors failed: {err}", spec.name));

            let relation_name = vector_relation.relation_name();
            ensure_relation_registered(&db, &relation_name);
            assert_vector_column_layout(&db, &relation_name, dim_spec.dims);

            let vector_query = format!(
                r#"
?[embedding_model, provider, embedding_dims, vector] :=
    *{rel}{{ node_id, embedding_model, provider, embedding_dims, vector @ 'NOW' }},
    node_id = to_uuid("{node_id}")
"#,
                rel = relation_name,
                node_id = sample.node_id,
            );
            let vector_rows = db
                .run_script(&vector_query, BTreeMap::new(), ScriptMutability::Immutable)
                .unwrap_or_else(|err| panic!("query {} vectors failed: {err}", spec.name));
            assert_eq!(
                vector_rows.rows.len(),
                1,
                "expected single vector row for {} dimension {}",
                spec.name,
                dim_spec.dims
            );
            let row = &vector_rows.rows[0];
            let model = row[0]
                .get_str()
                .expect("embedding_model must be string")
                .to_string();
            let provider = row[1]
                .get_str()
                .expect("provider must be string")
                .to_string();
            let dims_val = match &row[2] {
                DataValue::Num(Num::Int(val)) => *val,
                other => panic!("embedding_dims must be integer, got {other:?}"),
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
        assert_eq!(
            observed_dim_relations,
            supported_dimension_set(),
            "must observe every supported dimension relation for {}",
            spec.name
        );

        for dim_spec in VECTOR_DIMENSION_SPECS {
            let relation =
                ExperimentalVectorRelation::new(dim_spec.dims, spec.vector_relation_base);
            let relation_name = relation.relation_name();
            let create_idx = format!(
                r#"
::hnsw create {rel}:vector_idx {{
    fields: [vector],
    dim: {dims},
    dtype: F32,
    m: {m},
    ef_construction: {ef_construction},
    distance: L2
}}
"#,
                rel = relation_name,
                dims = dim_spec.dims,
                m = dim_spec.hnsw_m,
                ef_construction = dim_spec.hnsw_ef_construction,
            );
            db.run_script(&create_idx, BTreeMap::new(), ScriptMutability::Mutable)
                .unwrap_or_else(|err| panic!("create {} index failed: {err}", spec.name));

            let query_vec = vector_literal(dim_spec.dims as usize, dim_spec.offset);
            let search_script = format!(
                r#"
?[node_id, distance] :=
    ~{rel}:vector_idx{{ node_id |
        query: {query},
        k: 1,
        ef: {ef},
        bind_distance: distance
    }}
"#,
                rel = relation_name,
                query = query_vec,
                ef = dim_spec.hnsw_search_ef,
            );
            let search_rows = db
                .run_script(&search_script, BTreeMap::new(), ScriptMutability::Immutable)
                .unwrap_or_else(|err| panic!("search {} index failed: {err}", spec.name));
            assert_eq!(
                search_rows.rows.len(),
                1,
                "expected single HNSW match for {} ({})",
                spec.name,
                dim_spec.dims
            );
            match search_rows.rows[0][0] {
                DataValue::Uuid(UuidWrapper(id)) => assert_eq!(
                    id, sample.node_id,
                    "HNSW should return node id for {} ({})",
                    spec.name, dim_spec.dims
                ),
                _ => panic!("expected uuid result from HNSW query"),
            }
        }
    }

    fn init_db() -> Db<MemStorage> {
        let db = Db::new(MemStorage::default()).expect("create db");
        db.initialize().expect("init db");
        db
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

    fn parse_embedding_metadata(value: &DataValue) -> Vec<(String, i64)> {
        let entries = value
            .get_slice()
            .expect("embeddings column should contain a list");
        entries
            .iter()
            .map(|entry| {
                let tuple = entry
                    .get_slice()
                    .expect("embedding metadata tuple should be a list");
                assert_eq!(
                    tuple.len(),
                    2,
                    "embedding metadata tuples must be (model, dims)"
                );
                let model = tuple[0]
                    .get_str()
                    .expect("tuple[0] should be embedding model string")
                    .to_string();
                let dims = match &tuple[1] {
                    DataValue::Num(Num::Int(val)) => *val,
                    other => panic!("tuple[1] must be integer dimensions, got {other:?}"),
                };
                (model, dims)
            })
            .collect()
    }
}
