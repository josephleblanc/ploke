#![cfg(test)]

use std::collections::HashSet;

use super::adapter::{
    parse_embedding_metadata, ExperimentalEmbeddingDatabaseExt, ExperimentalEmbeddingDbExt,
};
use super::schema::vector_dims::{supported_dimension_set, vector_literal};
use super::seeding::{
    init_db, insert_metadata_sample, seed_metadata_relation, seed_vector_relation_for_node,
    ExperimentalNodeSpec, SampleNodeData, EXPERIMENTAL_NODE_SPECS,
};
use super::{ExperimentalVectorRelation, HnswDistance, VECTOR_DIMENSION_SPECS};
use crate::database::Database;
use crate::error::DbError;
use cozo::{DataValue, Num, ScriptMutability, UuidWrapper};
use uuid::Uuid;

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
        dim_spec.dims(),
        dim_spec.hnsw_m(),
        dim_spec.hnsw_ef_construction(),
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
        dim_spec.dims(),
        dim_spec.hnsw_m(),
        dim_spec.hnsw_ef_construction(),
        HnswDistance::L2,
    )
    .expect("create idx");
    let query_vec = vector_literal(dim_spec.dims() as usize, dim_spec.offset());
    let rows = db
        .search_embeddings_hnsw(
            &relation.relation_name(),
            &query_vec,
            1,
            dim_spec.hnsw_search_ef(),
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
    let query_vec = vector_literal(dim_spec.dims() as usize, dim_spec.offset());
    let err = db
        .search_embeddings_hnsw(
            &relation.relation_name(),
            &query_vec,
            1,
            dim_spec.hnsw_search_ef(),
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
    relation
        .insert_row(&db, second_sample.node_id, dim_spec)
        .expect("insert second vector");
    db.create_idx(
        &relation.relation_name(),
        dim_spec.dims(),
        dim_spec.hnsw_m(),
        dim_spec.hnsw_ef_construction(),
        HnswDistance::L2,
    )
    .expect("create idx");
    let query_vec = vector_literal(dim_spec.dims() as usize, dim_spec.offset());
    let rows = db
        .search_embeddings_hnsw(
            &relation.relation_name(),
            &query_vec,
            2,
            dim_spec.hnsw_search_ef(),
        )
        .expect("search rows");
    assert!(
        rows.rows.len() >= 2,
        "expected at least two search results for k=2"
    );
}

#[test]
fn upsert_vector_values_errors_on_length_mismatch() {
    let spec = default_spec();
    let (db, sample) = seed_metadata_relation(spec).expect("seed metadata");
    let dim_spec = &VECTOR_DIMENSION_SPECS[0];
    let relation =
        ExperimentalVectorRelation::new(dim_spec.dims(), spec.base.vector_relation_base);
    relation
        .ensure_registered(&db)
        .expect("vector relation should be registered");

    let too_short = vec![0.0_f32; (dim_spec.dims() as usize).saturating_sub(1)];
    let err = relation
        .upsert_vector_values(&db, sample.node_id, dim_spec, &too_short)
        .expect_err("length mismatch should return error");
    assert!(matches!(
        err,
        DbError::ExperimentalVectorLengthMismatch { expected, actual }
            if expected == dim_spec.dims() as usize && actual == too_short.len()
    ));
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
        db.assert_vector_column_layout(&relation_name, dim_spec.dims())
            .expect("vector layout must match");

        let vector_rows = db.vector_rows(&relation_name, sample.node_id)?;
        if vector_rows.rows.len() != 1 {
            return Err(DbError::ExperimentalMetadataParse {
                reason: format!(
                    "expected single vector row for {} dimension {}",
                    spec.base.name,
                    dim_spec.dims()
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
            dims_val,
            dim_spec.dims(),
            "embedding_dims must match relation dimension"
        );
        assert!(
            provider == dim_spec.provider(),
            "provider mismatch for {}",
            spec.base.name
        );
        assert!(
            !matches!(row[3], DataValue::Null),
            "vector column must be populated for {} ({})",
            spec.base.name,
            dim_spec.dims()
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
            ExperimentalVectorRelation::new(dim_spec.dims(), spec.base.vector_relation_base);
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
            ExperimentalVectorRelation::new(dim_spec.dims(), spec.base.vector_relation_base);
        vector_relation.insert_row(&db, second_sample.node_id, &dim_spec)?;
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
            ExperimentalVectorRelation::new(dim_spec.dims(), spec.base.vector_relation_base);
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
            ExperimentalVectorRelation::new(dim_spec.dims(), spec.base.vector_relation_base);
        let relation_name = relation.relation_name();
        db.create_idx(
            &relation_name,
            dim_spec.dims(),
            dim_spec.hnsw_m(),
            dim_spec.hnsw_ef_construction(),
            HnswDistance::L2,
        )?;

        let query_vec = vector_literal(dim_spec.dims() as usize, dim_spec.offset());
        let search_rows =
            db.search_embeddings_hnsw(&relation_name, &query_vec, 1, dim_spec.hnsw_search_ef())?;
        if search_rows.rows.len() != 1 {
            return Err(DbError::ExperimentalMetadataParse {
                reason: format!(
                    "expected single HNSW match for {} ({})",
                    spec.base.name,
                    dim_spec.dims()
                ),
            });
        }
        match search_rows.rows[0][0] {
            DataValue::Uuid(UuidWrapper(id)) => {
                if id != sample.node_id && id != second_node_id {
                    return Err(DbError::ExperimentalMetadataParse {
                        reason: format!(
                            "unexpected node id {id} returned for {} ({})",
                            spec.base.name,
                            dim_spec.dims()
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
