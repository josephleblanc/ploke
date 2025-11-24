use std::collections::HashSet;

use cozo::{DataValue, Num};
use lazy_static::lazy_static;
use ploke_core::{EmbeddingModelId, EmbeddingProviderSlug};

use crate::DbError;

#[derive(Clone, Debug)]
pub struct VectorDimensionSpec {
    dims: i64,
    provider: EmbeddingProviderSlug,
    embedding_model: EmbeddingModelId,
    offset: f32,
    hnsw_m: i64,
    hnsw_ef_construction: i64,
    hnsw_search_ef: i64,
}

pub struct VectorDimBuilder {
    dims: Option<i64>,
    provider: Option<Option<EmbeddingProviderSlug>>,
    embedding_model: Option<EmbeddingModelId>,
    offset: Option<f32>,
    hnsw_m: Option<i64>,
    hnsw_ef_construction: Option<i64>,
    hnsw_search_ef: Option<i64>,
}

impl VectorDimBuilder {
    pub fn new() -> Self {
        Self {
            dims: None,
            provider: None,
            embedding_model: None,
            offset: None,
            hnsw_m: None,
            hnsw_ef_construction: None,
            hnsw_search_ef: None,
        }
    }

    pub fn dims(mut self, dims: i64) -> Self {
        self.dims = Some(dims);
        self
    }

    pub fn provider(mut self, provider: Option<EmbeddingProviderSlug>) -> Self {
        self.provider = Some(provider);
        self
    }

    pub fn embedding_model(mut self, embedding_model: EmbeddingModelId) -> Self {
        self.embedding_model = Some(embedding_model);
        self
    }

    pub fn offset(mut self, offset: f32) -> Self {
        self.offset = Some(offset);
        self
    }

    pub fn hnsw_m(mut self, hnsw_m: i64) -> Self {
        self.hnsw_m = Some(hnsw_m);
        self
    }

    pub fn hnsw_ef_construction(mut self, hnsw_ef_construction: i64) -> Self {
        self.hnsw_ef_construction = Some(hnsw_ef_construction);
        self
    }

    pub fn hnsw_search_ef(mut self, hnsw_search_ef: i64) -> Self {
        self.hnsw_search_ef = Some(hnsw_search_ef);
        self
    }

    pub fn build(self) -> Result<VectorDimensionSpec, DbError> {
        Ok(VectorDimensionSpec {
            dims: self.dims.ok_or(DbError::BuilderFieldRequired("dims field is required"))?,
            provider: self
                .provider
                .flatten()
                .ok_or(DbError::BuilderFieldRequired("provider field is required"))?,
            embedding_model: self.embedding_model.ok_or(DbError::BuilderFieldRequired("embedding_model field is required"))?,
            offset: self.offset.unwrap_or(0.0),
            hnsw_m: self.hnsw_m.unwrap_or(16),
            hnsw_ef_construction: self.hnsw_ef_construction.unwrap_or(64),
            hnsw_search_ef: self.hnsw_search_ef.unwrap_or(50),
        })
    }
}

impl Default for VectorDimBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl VectorDimensionSpec {
    pub fn dims(&self) -> i64 {
        self.dims
    }

    pub fn provider(&self) -> &EmbeddingProviderSlug {
        &self.provider
    }

    pub fn embedding_model(&self) -> &EmbeddingModelId {
        &self.embedding_model
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

/// Returns the statically-defined sample specs used only by tests, fixtures, and legacy docs.
///
/// Runtime embedding flows now derive `VectorDimensionSpec` instances directly from the live
/// embedder configuration so arbitrary vector lengths can be indexed without compile-time
/// registration. Avoid adding production-only specs here.
pub fn sample_vector_dimension_specs() -> &'static [VectorDimensionSpec] {
    &*VECTOR_DIMENSION_SPECS
}

/// Finds a sample spec by vector length for fixture/test scenarios.
///
/// This helper supports deterministic Cozo fixtures and regression tests. Production code
/// should not depend on this mapping because runtime vector specs are created on demand.
pub fn dimension_spec_for_length(len: usize) -> Option<&'static VectorDimensionSpec> {
    sample_vector_dimension_specs()
        .iter()
        .find(|spec| spec.dims as usize == len)
}

pub(crate) fn vector_literal(len: usize, offset: f32) -> String {
    let values = (0..len)
        .map(|idx| format!("{:.6}", offset + (idx as f32 * 0.0001)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("vec([{values}])")
}

pub fn embedding_entry(model: &str, dims: i64) -> DataValue {
    DataValue::List(vec![
        DataValue::Str(model.into()),
        DataValue::Num(Num::Int(dims)),
    ])
}

lazy_static! {
    // NOTE: This table exists solely for tests/fixtures. Production code now uses runtime-derived
    // specs and creates the required vector relations and HNSW indexes on demand.
    pub static ref VECTOR_DIMENSION_SPECS: [VectorDimensionSpec; 4] = [
        VectorDimensionSpec {
            dims: 384,
            provider: EmbeddingProviderSlug::new_from_str("local-transformers"),
            embedding_model: EmbeddingModelId::new_from_str(
                "sentence-transformers/all-MiniLM-L6-v2"
            ),
            offset: 0.01,
            hnsw_m: 16,
            hnsw_ef_construction: 64,
            hnsw_search_ef: 50,
        },
        VectorDimensionSpec {
            dims: 768,
            provider: EmbeddingProviderSlug::new_from_str("openrouter"),
            embedding_model: EmbeddingModelId::new_from_str("ploke-test-embed-768"),
            offset: 0.35,
            hnsw_m: 20,
            hnsw_ef_construction: 80,
            hnsw_search_ef: 56,
        },
        VectorDimensionSpec {
            dims: 1024,
            provider: EmbeddingProviderSlug::new_from_str("cohere"),
            embedding_model: EmbeddingModelId::new_from_str("ploke-test-embed-1024"),
            offset: 0.7,
            hnsw_m: 26,
            hnsw_ef_construction: 96,
            hnsw_search_ef: 60,
        },
        VectorDimensionSpec {
            dims: 1536,
            provider: EmbeddingProviderSlug::new_from_str("openai"),
            embedding_model: EmbeddingModelId::new_from_str("text-embedding-ada-002"),
            offset: 1.0,
            hnsw_m: 32,
            hnsw_ef_construction: 128,
            hnsw_search_ef: 64,
        },
    ];
}
