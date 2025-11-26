use std::collections::HashSet;

use cozo::{DataValue, Num};
use lazy_static::lazy_static;
use ploke_core::{
    arcstr_wrapper, embedding_set::EmbRelName, ArcStr, EmbeddingModelId, EmbeddingProviderSlug,
    EmbeddingSetId, EmbeddingShape,
};

use crate::{database::HNSW_SUFFIX, multi_embedding::{vectors::CozoVectorExt, HnswDistance}, DbError};

#[derive(Clone, Debug)]
pub struct HnswEmbedInfo {
    emb_set: EmbeddingSetId,
    hnsw_m: i64,
    hnsw_ef_construction: i64,
    hnsw_search_ef: i64,
    hnsw_rel: HnswRelName,
    distance: HnswDistance,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default, Debug)]
pub struct HnswRelName(ArcStr);

arcstr_wrapper!(HnswRelName);

impl HnswRelName {
    pub fn ad_hoc_from_set(emb_set: EmbeddingSetId) -> Self {
        let hnsw_rel = format!("{}{}_0", emb_set.relation_name(), HNSW_SUFFIX);
        // TODO: Add a new_from_string method to the macro for acrstr_wrapper! like we have with
        // EmbeddingModelId
        Self::new_from_str(&hnsw_rel)
    }
}

impl From<EmbeddingSetId> for HnswRelName {
    fn from(value: EmbeddingSetId) -> Self {
        let base_name = format!("{}:{}_0", value.relation_name(), HNSW_SUFFIX);
        HnswRelName::new_from_str(&base_name)
    }
}

pub struct HnswEmbedInfoBuilder {
    emb_set: Option<EmbeddingSetId>,
    offset: Option<f32>,
    hnsw_m: Option<i64>,
    hnsw_ef_construction: Option<i64>,
    hnsw_search_ef: Option<i64>,
    distance: Option< HnswDistance >,
}

impl CozoVectorExt for EmbeddingSetId {
    fn dims(&self) -> u32 {
        self.shape.dimension
    }

    fn vector_relation(&self) -> &EmbRelName {
        &self.rel_name
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
            self.dims()
        )
    }
}

impl CozoVectorExt for HnswEmbedInfo {
    fn dims(&self) -> u32 {
        self.emb_set.dims()
    }

    fn script_identity(&self) -> String {
        self.emb_set.script_identity()
    }

    fn script_create(&self) -> String {
        self.emb_set.script_create()
    }

    fn vector_relation(&self) -> &EmbRelName {
        self.emb_set.vector_relation()
    }
}

impl HnswEmbedInfoBuilder {
    pub fn new() -> Self {
        Self {
            emb_set: None,
            offset: None,
            hnsw_m: None,
            hnsw_ef_construction: None,
            hnsw_search_ef: None,
            distance: None,
        }
    }

    pub fn dims(mut self, dims: u32) -> Self {
        if let Some(emb_set) = self.emb_set.as_mut() {
            emb_set.shape.dimension = dims;
        }
        self
    }

    pub fn emb_set(mut self, emb_set: EmbeddingSetId) -> Self {
        self.emb_set = Some(emb_set);
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

    pub fn distance(mut self, distance: HnswDistance) -> Self {
        self.distance = Some(distance);
        self
    }
    //
    // pub fn provider(mut self, provider: EmbeddingProviderSlug) -> Self {
    //     if let Some(emb_set) = self.emb_set {
    //         emb_set.provider = provider;
    //     }
    // }

    fn generate_hnsw_name(&self) -> Option<HnswRelName> {
        if let Some(emb_set) = &self.emb_set {
            let base_name = format!("{}{}_0", emb_set.relation_name(), HNSW_SUFFIX);
            Some(HnswRelName::new_from_str(&base_name))
        } else {
            None
        }
    }

    pub fn build(self) -> Result<HnswEmbedInfo, DbError> {
        let hnsw_rel = self
            .generate_hnsw_name()
            .ok_or(DbError::BuilderFieldRequired(
            "emb_set field for embedding set is required, not found while constructing HnswRelName",
        ))?;
        Ok(HnswEmbedInfo {
            emb_set: self.emb_set.ok_or(DbError::BuilderFieldRequired(
                "emb_set field for embedding set is required",
            ))?,
            hnsw_m: self.hnsw_m.unwrap_or(16),
            hnsw_ef_construction: self.hnsw_ef_construction.unwrap_or(64),
            hnsw_search_ef: self.hnsw_search_ef.unwrap_or(50),
            hnsw_rel,
            distance: self.distance.unwrap_or(HnswDistance::L2)
        })
    }
}

impl Default for HnswEmbedInfoBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl HnswEmbedInfo {
    pub fn dims(&self) -> u32 {
        self.emb_set.shape.dimension
    }

    pub fn emb_set(&self) -> &EmbeddingSetId {
        &self.emb_set
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

    pub fn embedding_model(&self) -> &EmbeddingModelId {
        &self.emb_set.model
    }

    pub fn provider(&self) -> &EmbeddingProviderSlug {
        &self.emb_set.provider
    }

    pub fn emb_rel_name(&self) -> &EmbRelName {
        &self.emb_set.rel_name
    }

    pub fn hnsw_rel_name(&self) -> &HnswRelName {
        &self.hnsw_rel
    }

    pub fn distance(&self) -> HnswDistance {
        self.distance
    }
}

/// Returns the statically-defined sample specs used only by tests, fixtures, and legacy docs.
///
/// Runtime embedding flows now derive `HnswEmbedInfo` instances directly from the live
/// embedder configuration so arbitrary vector lengths can be indexed without compile-time
/// registration. Avoid adding production-only specs here.
pub fn sample_vector_dimension_specs() -> &'static [HnswEmbedInfo] {
    &*VECTOR_DIMENSION_SPECS
}

/// Finds a sample spec by vector length for fixture/test scenarios.
///
/// This helper supports deterministic Cozo fixtures and regression tests. Production code
/// should not depend on this mapping because runtime vector specs are created on demand.
pub fn dimension_spec_for_length(len: usize) -> Option<&'static HnswEmbedInfo> {
    sample_vector_dimension_specs()
        .iter()
        .find(|spec| spec.dims() as usize == len)
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
    pub static ref VECTOR_DIMENSION_SPECS: [HnswEmbedInfo; 4] = [
        HnswEmbedInfo {
            emb_set:
            EmbeddingSetId::new(
                EmbeddingProviderSlug::new_from_str("local-transformers"),
                EmbeddingModelId::new_from_str("sentence-transformers/all-MiniLM-L6-v2"),
                EmbeddingShape::new_dims_default(384),
            ),
            hnsw_m: 16,
            hnsw_ef_construction: 64,
            hnsw_search_ef: 50,
            hnsw_rel: HnswRelName::from(
                EmbeddingSetId::new(
                    EmbeddingProviderSlug::new_from_str("local-transformers"),
                    EmbeddingModelId::new_from_str("sentence-transformers/all-MiniLM-L6-v2"),
                    EmbeddingShape::new_dims_default(384),
                )
            ),
            distance: HnswDistance::L2,
        },
        HnswEmbedInfo {
            emb_set: EmbeddingSetId::new(
                EmbeddingProviderSlug::new_from_str("openrouter"),
                EmbeddingModelId::new_from_str("ploke-test-embed-768"),
                EmbeddingShape::new_dims_default(768),
            ),
            hnsw_m: 20,
            hnsw_ef_construction: 80,
            hnsw_search_ef: 56,
            hnsw_rel: HnswRelName::from(
                EmbeddingSetId::new(
                EmbeddingProviderSlug::new_from_str("openrouter"),
                EmbeddingModelId::new_from_str("ploke-test-embed-768"),
                EmbeddingShape::new_dims_default(768),
                )
            ),
            distance: HnswDistance::L2,
        },
        HnswEmbedInfo {
            emb_set: EmbeddingSetId::new(
                EmbeddingProviderSlug::new_from_str("cohere"),
                EmbeddingModelId::new_from_str("ploke-test-embed-1024"),
                EmbeddingShape::new_dims_default(1024),
            ),
            hnsw_m: 26,
            hnsw_ef_construction: 96,
            hnsw_search_ef: 60,
            hnsw_rel: HnswRelName::from(
                EmbeddingSetId::new(
                EmbeddingProviderSlug::new_from_str("cohere"),
                EmbeddingModelId::new_from_str("ploke-test-embed-1024"),
                EmbeddingShape::new_dims_default(1024),
                )
            ),
            distance: HnswDistance::L2,
        },
        HnswEmbedInfo {
            emb_set: EmbeddingSetId::new(
                EmbeddingProviderSlug::new_from_str("openai"),
                EmbeddingModelId::new_from_str("text-embedding-ada-002"),
                EmbeddingShape::new_dims_default(1536),
            ),
            hnsw_m: 32,
            hnsw_ef_construction: 128,
            hnsw_search_ef: 64,
            hnsw_rel: HnswRelName::from(
                EmbeddingSetId::new(
                EmbeddingProviderSlug::new_from_str("openai"),
                EmbeddingModelId::new_from_str("text-embedding-ada-002"),
                EmbeddingShape::new_dims_default(1536),
                )
            ),
            distance: HnswDistance::L2,
        },
    ];
}
