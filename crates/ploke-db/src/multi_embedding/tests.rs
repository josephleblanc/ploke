#![cfg(test)]
use super::*;
use super::{ExperimentalEmbeddingDatabaseExt, ExperimentalEmbeddingDbExt};

pub(super) struct SampleNodeData {
    node_id: Uuid,
    params: BTreeMap<String, DataValue>,
}

pub(super) struct ExperimentalNodeSpec {
    base: &'static ExperimentalNodeRelationSpec,
    sample_builder: fn() -> SampleNodeData,
}

const EXPERIMENTAL_NODE_SPECS: &[ExperimentalNodeSpec] = &[
    ExperimentalNodeSpec {
        base: &EXPERIMENTAL_NODE_RELATION_SPECS[0],
        sample_builder: sample_function_params,
    },
    ExperimentalNodeSpec {
        base: &EXPERIMENTAL_NODE_RELATION_SPECS[1],
        sample_builder: sample_const_params,
    },
    ExperimentalNodeSpec {
        base: &EXPERIMENTAL_NODE_RELATION_SPECS[2],
        sample_builder: sample_enum_params,
    },
    ExperimentalNodeSpec {
        base: &EXPERIMENTAL_NODE_RELATION_SPECS[3],
        sample_builder: sample_impl_params,
    },
    ExperimentalNodeSpec {
        base: &EXPERIMENTAL_NODE_RELATION_SPECS[4],
        sample_builder: sample_import_params,
    },
    ExperimentalNodeSpec {
        base: &EXPERIMENTAL_NODE_RELATION_SPECS[5],
        sample_builder: sample_macro_params,
    },
    ExperimentalNodeSpec {
        base: &EXPERIMENTAL_NODE_RELATION_SPECS[6],
        sample_builder: sample_module_params,
    },
    ExperimentalNodeSpec {
        base: &EXPERIMENTAL_NODE_RELATION_SPECS[7],
        sample_builder: sample_static_params,
    },
    ExperimentalNodeSpec {
        base: &EXPERIMENTAL_NODE_RELATION_SPECS[8],
        sample_builder: sample_struct_params,
    },
    ExperimentalNodeSpec {
        base: &EXPERIMENTAL_NODE_RELATION_SPECS[9],
        sample_builder: sample_trait_params,
    },
    ExperimentalNodeSpec {
        base: &EXPERIMENTAL_NODE_RELATION_SPECS[10],
        sample_builder: sample_type_alias_params,
    },
    ExperimentalNodeSpec {
        base: &EXPERIMENTAL_NODE_RELATION_SPECS[11],
        sample_builder: sample_union_params,
    },
];

fn insert_metadata_sample(
    db: &Database,
    spec: &ExperimentalNodeSpec,
    sample: &SampleNodeData,
) -> Result<(), DbError> {
    let insert_script = spec.base.metadata_schema.script_put(&sample.params);
    db.run_script(
        &insert_script,
        sample.params.clone(),
        ScriptMutability::Mutable,
    )
    .map_err(|err| DbError::ExperimentalScriptFailure {
        action: "metadata_insert",
        relation: spec.base.metadata_schema.relation().to_string(),
        details: err.to_string(),
    })
    .map(|_| ())
}

fn default_spec() -> &'static ExperimentalNodeSpec {
    &EXPERIMENTAL_NODE_SPECS[0]
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
        .vector_metadata_rows(spec.base.metadata_schema.relation(), sample.node_id)
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
    let relation_name = spec.base.metadata_schema.relation().to_string();

    let metadata_rows = db.vector_metadata_rows(&relation_name, sample.node_id)?;
    if metadata_rows.rows.len() != 1 {
        return Err(DbError::ExperimentalMetadataParse {
            reason: format!("expected metadata row for {}", spec.base.name),
        });
    }
    let metadata_entries = parse_embedding_metadata(&metadata_rows.rows[0][0])?;
    assert_eq!(
        metadata_entries.len(),
        VECTOR_DIMENSION_SPECS.len(),
        "expected {} metadata tuples for {}",
        VECTOR_DIMENSION_SPECS.len(),
        spec.base.name
    );
    let metadata_set: HashSet<(String, i64)> = metadata_entries.into_iter().collect::<HashSet<_>>();
    let enumerated_metadata = db
        .enumerate_metadata_models(spec.base.metadata_schema.relation())
        .expect("enumerate metadata");
    assert_eq!(
        metadata_set, enumerated_metadata,
        "metadata enumeration should match parsed tuples for {}",
        spec.base.name
    );

    let mut vector_set = HashSet::new();
    let mut observed_dim_relations = HashSet::new();

    for dim_spec in VECTOR_DIMENSION_SPECS {
        let vector_relation = seed_vector_relation_for_node(&db, spec, sample.node_id, &dim_spec)?;

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
                    spec.base.name, dim_spec.dims
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
            spec.base.name
        );
        assert!(
            !matches!(row[3], DataValue::Null),
            "vector column must be populated for {} ({})",
            spec.base.name,
            dim_spec.dims
        );
        vector_set.insert((model, dims_val));
        observed_dim_relations.insert(dims_val);
    }

    assert_eq!(
        metadata_set, vector_set,
        "metadata tuples must align with vector rows for {}",
        spec.base.name
    );
    let mut enumerated_vectors = HashSet::new();
    for dim_spec in VECTOR_DIMENSION_SPECS {
        let relation =
            ExperimentalVectorRelation::new(dim_spec.dims, spec.base.vector_relation_base);
        enumerated_vectors.extend(
            db.enumerate_vector_models(&relation.relation_name())
                .expect("enumerate vectors"),
        );
    }
    assert_eq!(
        enumerated_vectors, metadata_set,
        "vector enumeration should list the same models for {}",
        spec.base.name
    );
    assert_eq!(
        observed_dim_relations,
        supported_dimension_set().clone(),
        "must observe every supported dimension relation for {}",
        spec.base.name
    );

    let second_sample = (spec.sample_builder)();
    let second_node_id = second_sample.node_id;
    let second_insert = spec.base.metadata_schema.script_put(&second_sample.params);
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
            ExperimentalVectorRelation::new(dim_spec.dims, spec.base.vector_relation_base);
        insert_vector_row(&db, &vector_relation, second_sample.node_id, &dim_spec)?;
    }

    let dedup_metadata = db
        .enumerate_metadata_models(spec.base.metadata_schema.relation())
        .expect("dedup metadata");
    assert_eq!(
        dedup_metadata, metadata_set,
        "metadata enumeration must dedupe across rows for {}",
        spec.base.name
    );
    let mut dedup_vectors = HashSet::new();
    for dim_spec in VECTOR_DIMENSION_SPECS {
        let relation =
            ExperimentalVectorRelation::new(dim_spec.dims, spec.base.vector_relation_base);
        dedup_vectors.extend(
            db.enumerate_vector_models(&relation.relation_name())
                .expect("dedup vectors"),
        );
    }
    assert_eq!(
        dedup_vectors, metadata_set,
        "vector enumeration must dedupe across rows for {}",
        spec.base.name
    );

    for dim_spec in VECTOR_DIMENSION_SPECS {
        let relation =
            ExperimentalVectorRelation::new(dim_spec.dims, spec.base.vector_relation_base);
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
                    spec.base.name, dim_spec.dims
                ),
            });
        }
        match search_rows.rows[0][0] {
            DataValue::Uuid(UuidWrapper(id)) => {
                if id != sample.node_id && id != second_node_id {
                    return Err(DbError::ExperimentalMetadataParse {
                        reason: format!(
                            "unexpected node id {id} returned for {} ({})",
                            spec.base.name, dim_spec.dims
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
