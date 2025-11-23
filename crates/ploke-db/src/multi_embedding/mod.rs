pub mod adapter;
pub mod schema;
pub mod vectors;

pub use adapter::{ExperimentalEmbeddingDatabaseExt, ExperimentalEmbeddingDbExt};
pub use schema::metadata::{CozoField, ExperimentalRelationSchema};
pub use schema::vector_dims::{
    embedding_entry, vector_dimension_specs, VectorDimensionSpec, VECTOR_DIMENSION_SPECS,
};
pub use vectors::ExperimentalVectorRelation;

#[derive(Copy, Clone, Debug)]
pub enum HnswDistance {
    L2,
    Cosine,
    Ip,
}

impl HnswDistance {
    pub fn as_str(&self) -> &'static str {
        match self {
            HnswDistance::L2 => "L2",
            HnswDistance::Cosine => "Cosine",
            HnswDistance::Ip => "IP",
        }
    }
}
