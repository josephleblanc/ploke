use super::*;
use crate::define_schema;

// AI: Add documentation on this schema AI!
define_schema!(Bm25MetaSchema {
    "bm25_doc_meta",
    id: "Uuid",
    name: "String",
    version: "String",
    doc_length: "Int",
});
