use super::*;
use crate::define_schema;

/// BM25 document metadata stored per primary node.
///
/// Fields:
/// - id: UUID of the node/document.
/// - name: Human-readable name or path for the document/node.
/// - version: Tokenizer/version tag (e.g. "code_tokenizer_v1") used when the snippet was tokenized.
/// - doc_length: Token length of the snippet as computed by the code tokenizer.
define_schema!(Bm25MetaSchema {
    "bm25_doc_meta",
    id: "Uuid",
    name: "String",
    version: "String",
    doc_length: "Int",
});
