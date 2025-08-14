use super::*;
use crate::define_schema;

// BM25 document metadata stored per primary node.
///
/// Fields:
/// - id: UUID of the node/document.
/// - tracking_hash: Stable content hash (UUID v5 over DNS namespace) for the snippet.
/// - tokenizer_version: Tokenizer/version tag (e.g. "code_tokenizer_v1") used when the snippet was tokenized.
/// - token_length: Token length of the snippet as computed by the code tokenizer.
define_schema!(Bm25MetaSchema {
    "bm25_doc_meta",
    id: "Uuid",
    tracking_hash: "Uuid",
    tokenizer_version: "String",
    token_length: "Int",
});
