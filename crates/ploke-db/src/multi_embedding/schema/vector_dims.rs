use std::collections::HashSet;

use cozo::{DataValue, Num};
use lazy_static::lazy_static;
use ploke_core::{EmbeddingModelId, EmbeddingProviderSlug};

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

pub fn sample_vector_dimension_specs() -> &'static [VectorDimensionSpec] {
    &*VECTOR_DIMENSION_SPECS
}

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
