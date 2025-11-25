pub mod adapter;
pub mod schema;
pub mod vectors;

pub use adapter::{IndexDbExt, EmbeddingDbExt};
pub use schema::metadata::{CozoField, ExperimentalRelationSchema};
pub use schema::vector_dims::{
    embedding_entry, sample_vector_dimension_specs, HnswEmbedInfo, VECTOR_DIMENSION_SPECS,
};

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

impl std::fmt::Display for HnswDistance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
