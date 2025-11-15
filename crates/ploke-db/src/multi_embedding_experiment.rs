use std::collections::BTreeMap;

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

macro_rules! define_schema_experimental {
    ($schema_name:ident {
        $relation:literal,
        $($field_name:ident: $dv:literal),+
        $(,)?
    }) => {
        pub struct $schema_name {
            pub relation: &'static str,
            $(pub $field_name: CozoField),+
        }

        impl $schema_name {
            pub const SCHEMA: Self = Self {
                relation: $relation,
                $($field_name: CozoField::new(stringify!($field_name), $dv)),+
            };

            pub const SCHEMA_FIELDS: &'static [&'static str] = &[
                $( stringify!($field_name) ),+
            ];

            fn script_identity(&self) -> String {
                let fields = vec![
                    $( self.$field_name.st().to_string() ),+
                ];
                let keys = fields.iter().filter(|f| ID_KEYWORDS.contains(&f.as_str())).join(", ");
                let vals = fields.iter().filter(|f| !ID_KEYWORDS.contains(&f.as_str())).join(", ");
                format!("{} {{ {keys}, at => {vals} }}", self.relation)
            }

            pub fn script_create(&self) -> String {
                let fields = vec![
                    $( format!("{}: {}", self.$field_name.st(), self.$field_name.dv()) ),+
                ];
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

            pub fn script_put(&self, params: &BTreeMap<String, cozo::DataValue>) -> String {
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
    };
}

define_schema_experimental!(ExperimentalFunctionSchema {
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

impl ExperimentalFunctionSchema {
    pub fn create_relation(db: &Db<MemStorage>) -> Result<(), cozo::Error> {
        let script = Self::SCHEMA.script_create();
        db.run_script(&script, BTreeMap::new(), ScriptMutability::Mutable)?;
        Ok(())
    }
}

define_schema_experimental!(ExperimentalEmbeddingVectorsSchema {
    "function_embedding_vectors",
    node_id: "Uuid",
    embedding_model: "String",
    provider: "String",
    embedding_dims: "Int",
    vector_dim384: "<F32; 384>?",
    vector_dim1536: "<F32; 1536>?"
});

impl ExperimentalEmbeddingVectorsSchema {
    pub fn create_relation(db: &Db<MemStorage>) -> Result<(), cozo::Error> {
        let script = Self::SCHEMA.script_create();
        db.run_script(&script, BTreeMap::new(), ScriptMutability::Mutable)?;
        Ok(())
    }
}

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

fn insert_embedding_vectors(db: &Db<MemStorage>, function_id: Uuid) -> Result<(), cozo::Error> {
    let rel = ExperimentalEmbeddingVectorsSchema::SCHEMA.relation;
    let identity = ExperimentalEmbeddingVectorsSchema::SCHEMA.script_identity();
    let vector_local = vector_literal(384, 0.01);
    let script_local = format!(
        r#"
?[node_id, embedding_model, provider, at, embedding_dims, vector_dim384, vector_dim1536] <- [[
    to_uuid("{function_id}"),
    "sentence-transformers/all-MiniLM-L6-v2",
    "local-transformers",
    'ASSERT',
    384,
    {vector_local},
    null
]] :put {identity}
"#,
        vector_local = vector_local,
        identity = identity,
    );
    db.run_script(&script_local, BTreeMap::new(), ScriptMutability::Mutable)?;

    let vector_remote = vector_literal(1536, 1.0);
    let script_remote = format!(
        r#"
?[node_id, embedding_model, provider, at, embedding_dims, vector_dim384, vector_dim1536] <- [[
    to_uuid("{function_id}"),
    "text-embedding-ada-002",
    "openai",
    'ASSERT',
    1536,
    null,
    {vector_remote}
]] :put {identity}
"#,
        vector_remote = vector_remote,
        identity = identity,
    );
    db.run_script(&script_remote, BTreeMap::new(), ScriptMutability::Mutable)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn init_db() -> Db<MemStorage> {
        let db = Db::new(MemStorage::default()).expect("create db");
        db.initialize().expect("init db");
        db
    }

    fn sample_params() -> (BTreeMap<String, DataValue>, (Uuid, Uuid)) {
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
        params.insert(
            "span".into(),
            DataValue::List(vec![
                DataValue::Num(Num::Int(0)),
                DataValue::Num(Num::Int(42)),
            ]),
        );
        params.insert(
            "tracking_hash".into(),
            DataValue::Uuid(UuidWrapper(Uuid::new_v4())),
        );
        params.insert(
            "cfgs".into(),
            DataValue::List(vec![DataValue::Str("default".into())]),
        );
        params.insert("return_type_id".into(), DataValue::Null);
        params.insert("body".into(), DataValue::Null);
        params.insert("module_id".into(), DataValue::Uuid(UuidWrapper(module_id)));
        params.insert(
            "embeddings".into(),
            DataValue::List(vec![
                embedding_entry("sentence-transformers/all-MiniLM-L6-v2", 384),
                embedding_entry("text-embedding-ada-002", 1536),
            ]),
        );

        (params, (function_id, module_id))
    }

    #[test]
    fn creates_function_relation_with_embedding_metadata() {
        let db = init_db();
        ExperimentalFunctionSchema::create_relation(&db).expect("create relation");

        let (params, (function_id, _)) = sample_params();
        let script = ExperimentalFunctionSchema::SCHEMA.script_put(&params);
        db.run_script(&script, params.clone(), ScriptMutability::Mutable)
            .expect("insert function row");

        let mut query_params = BTreeMap::new();
        query_params.insert(
            "function_id".into(),
            DataValue::Uuid(UuidWrapper(function_id)),
        );
        let rows = db
            .run_script(
                r#"
                    ?[name, embeddings] :=
                        *function_multi_embedding{
                            id,
                            name,
                            embeddings @ 'NOW'
                        },
                        id = $function_id
                "#,
                query_params,
                ScriptMutability::Immutable,
            )
            .expect("query function relation");

        assert_eq!(rows.rows.len(), 1);
        let row = &rows.rows[0];
        match &row[0] {
            DataValue::Str(name) => assert_eq!(name, "experimental_function"),
            other => panic!("expected row[0] to be string, got {other:?}"),
        }
        let metadata_entries = parse_embedding_metadata(&row[1]);
        assert_eq!(metadata_entries.len(), 2, "expected two metadata tuples");
        let metadata_set: HashSet<(String, i64)> = metadata_entries.into_iter().collect();

        ExperimentalEmbeddingVectorsSchema::create_relation(&db)
            .expect("create embedding relation");
        insert_embedding_vectors(&db, function_id).expect("insert embedding vectors");

        let rel = ExperimentalEmbeddingVectorsSchema::SCHEMA.relation;
        let verify_embeddings = format!(
            r#"
                ?[embedding_model, provider, embedding_dims, vector_dim384, vector_dim1536] :=
                    *{rel}{{ node_id, embedding_model, provider, embedding_dims, vector_dim384, vector_dim1536 @ 'NOW' }},
                    node_id = to_uuid("{function_id}")
            "#,
            rel = rel,
            function_id = function_id
        );
        let verification_rows = db
            .run_script(&verify_embeddings, BTreeMap::new(), ScriptMutability::Immutable)
            .expect("query embedding rows");
        assert_eq!(
            verification_rows.rows.len(),
            2,
            "expect two embedding rows for the sample function"
        );
        let vector_rows = parse_vector_rows(&verification_rows.rows);
        let vector_set: HashSet<(String, i64)> = vector_rows
            .iter()
            .map(|info| (info.embedding_model.clone(), info.embedding_dims))
            .collect();
        assert_eq!(
            metadata_set, vector_set,
            "metadata tuples must align with stored vector rows"
        );
        for info in &vector_rows {
            match info.embedding_dims {
                384 => {
                    assert!(
                        info.has_vector_384,
                        "384-dim entry should populate vector_dim384"
                    );
                    assert!(
                        !info.has_vector_1536,
                        "384-dim entry must not populate vector_dim1536"
                    );
                    assert_eq!(info.provider, "local-transformers");
                }
                1536 => {
                    assert!(
                        info.has_vector_1536,
                        "1536-dim entry should populate vector_dim1536"
                    );
                    assert!(
                        !info.has_vector_384,
                        "1536-dim entry must not populate vector_dim384"
                    );
                    assert_eq!(info.provider, "openai");
                }
                other => panic!("unexpected embedding dims {other}"),
            }
        }

        let create_idx_384 = format!(
            r#"
                ::hnsw create {rel}:vector_dim384_idx {{
                    fields: [vector_dim384],
                    dim: 384,
                    dtype: F32,
                    m: 16,
                    ef_construction: 64,
                    distance: L2,
                    filter: embedding_dims == 384
                }}
            "#,
            rel = rel
        );
        db.run_script(&create_idx_384, BTreeMap::new(), ScriptMutability::Mutable)
            .expect("create 384-dim index");

        let create_idx_1536 = format!(
            r#"
                ::hnsw create {rel}:vector_dim1536_idx {{
                    fields: [vector_dim1536],
                    dim: 1536,
                    dtype: F32,
                    m: 32,
                    ef_construction: 128,
                    distance: L2,
                    filter: embedding_dims == 1536
                }}
            "#,
            rel = rel
        );
        db.run_script(&create_idx_1536, BTreeMap::new(), ScriptMutability::Mutable)
            .expect("create 1536-dim index");

        let local_query_vec = vector_literal(384, 0.01);
        let search_local = format!(
            r#"
                ?[node_id, distance] :=
                    ~{rel}:vector_dim384_idx{{ node_id |
                        query: {local_query_vec},
                        k: 1,
                        ef: 50,
                        bind_distance: distance
                    }}
            "#,
            rel = rel,
            local_query_vec = local_query_vec
        );
        let local_rows = db
            .run_script(&search_local, BTreeMap::new(), ScriptMutability::Immutable)
            .expect("search local embedding");
        assert_eq!(local_rows.rows.len(), 1, "expect one local embedding match");
        if let DataValue::Uuid(UuidWrapper(id)) = local_rows.rows[0][0] {
            assert_eq!(id, function_id);
        } else {
            panic!("expected uuid from local HNSW query");
        }

        let remote_query_vec = vector_literal(1536, 1.0);
        let search_remote = format!(
            r#"
                ?[node_id, distance] :=
                    ~{rel}:vector_dim1536_idx{{ node_id |
                        query: {remote_query_vec},
                        k: 1,
                        ef: 64,
                        bind_distance: distance
                    }}
            "#,
            rel = rel,
            remote_query_vec = remote_query_vec
        );
        let remote_rows = db
            .run_script(&search_remote, BTreeMap::new(), ScriptMutability::Immutable)
            .expect("search remote embedding");
        assert_eq!(
            remote_rows.rows.len(),
            1,
            "expect one remote embedding match"
        );
        if let DataValue::Uuid(UuidWrapper(id)) = remote_rows.rows[0][0] {
            assert_eq!(id, function_id);
        } else {
            panic!("expected uuid from remote HNSW query");
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

    #[derive(Debug)]
    struct VectorRowInfo {
        embedding_model: String,
        provider: String,
        embedding_dims: i64,
        has_vector_384: bool,
        has_vector_1536: bool,
    }

    fn parse_vector_rows(rows: &[Vec<DataValue>]) -> Vec<VectorRowInfo> {
        rows.iter()
            .map(|row| {
                assert_eq!(row.len(), 5, "vector query should return five columns");
                let embedding_model = row[0]
                    .get_str()
                    .expect("embedding_model must be string")
                    .to_string();
                let provider = row[1]
                    .get_str()
                    .expect("provider must be string")
                    .to_string();
                let embedding_dims = match &row[2] {
                    DataValue::Num(Num::Int(val)) => *val,
                    other => panic!("embedding_dims must be integer, got {other:?}"),
                };
                let has_vector_384 = !matches!(row[3], DataValue::Null);
                let has_vector_1536 = !matches!(row[4], DataValue::Null);
                VectorRowInfo {
                    embedding_model,
                    provider,
                    embedding_dims,
                    has_vector_384,
                    has_vector_1536,
                }
            })
            .collect()
    }
}
