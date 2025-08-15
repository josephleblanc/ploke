// ploke_bm25_indexer.rs
// BM25 integration for Ploke using the `bm25` crate v2.3.1
// - In-memory indexer that accepts batches of (Uuid, snippet)
// - Uses a code-aware tokenizer (implements bm25::Tokenizer)
// - Uses bm25::Embedder and bm25::Scorer to build sparse embeddings and score queries
// - Adds Cozo client trait + an index_batch_with_cozo method that upserts doc metadata into Cozo
// - Adds `new_from_corpus` constructor that consumes a Vec<(Uuid, String)> to compute avgdl

use std::{collections::HashMap, sync::Arc};

use bm25::{EmbedderBuilder, Scorer, Tokenizer};
use ploke_core::{EmbeddingData, TrackingHash};
use uuid::Uuid;

use crate::{Database, DbError};

pub mod bm25_service;

pub const TOKENIZER_VERSION: &str = "code_version_v1";

// ------------------------- Code-aware tokenizer -------------------------
// Implements bm25::Tokenizer by producing a Vec<String> of tokens from code.
#[derive(Default, Clone)]
/// Tokenizer for source code that:
/// - splits identifiers into subtokens (snake_case, camelCase, PascalCase, digits, acronyms)
/// - includes words from comments (line and block)
/// - treats non-alphanumeric symbols as separate tokens
///
/// Example:
/// ```
/// use ploke_db::bm25_index::CodeTokenizer;
/// let src = "/// docs\nfn parseJSON_v2(x: i32) { x += 10; }";
/// let t = CodeTokenizer::default();
/// let toks = t.tokenize(src);
/// assert!(toks.iter().any(|t| t == "docs"));
/// assert!(toks.iter().any(|t| t == "parse"));
/// assert!(toks.iter().any(|t| t == "json"));
/// assert!(toks.iter().any(|t| t == "v2"));
/// ```
pub struct CodeTokenizer;

impl CodeTokenizer {
    /// Split an identifier into subtokens (snake_case, camelCase, PascalCase, digits, acronyms)
    fn split_identifier(ident: &str) -> Vec<String> {
        let mut parts: Vec<String> = Vec::new();
        for chunk in ident.split('_') {
            if chunk.is_empty() {
                continue;
            }
            let mut cur = String::new();
            let chars: Vec<char> = chunk.chars().collect();
            for (i, &ch) in chars.iter().enumerate() {
                if i > 0 {
                    let prev = chars[i - 1];
                    let next = chars.get(i + 1).copied();
                    let lower_to_upper = prev.is_lowercase() && ch.is_uppercase();
                    let upper_seq_then_lower = prev.is_uppercase()
                        && ch.is_uppercase()
                        && next.map_or_else(|| false, |n| n.is_lowercase());
                    let digit_boundary = (prev.is_ascii_digit() && !ch.is_ascii_digit())
                        || (!prev.is_ascii_digit() && ch.is_ascii_digit());
                    if (lower_to_upper || upper_seq_then_lower || digit_boundary) && !cur.is_empty()
                    {
                        parts.push(cur.to_lowercase());
                        cur.clear();
                    }
                }
                cur.push(ch);
            }
            if !cur.is_empty() {
                parts.push(cur.to_lowercase());
            }
        }
        parts
    }

    /// Extract tokens from a code string. Includes identifier subtokens, comment words, and symbols
    fn tokens_from_code(s: &str) -> Vec<String> {
        let mut out = Vec::new();
        let bytes = s.as_bytes();
        let mut i = 0usize;
        let mut code_start = 0usize;
        let len = bytes.len();

        while i < len {
            if bytes[i] == b'/' {
                if i + 1 < len && bytes[i + 1] == b'/' {
                    // Emit code before line comment
                    if code_start < i {
                        let code_part = &s[code_start..i];
                        Self::tokenize_code_part(code_part, &mut out);
                    }
                    // Extract and tokenize the comment (skip the leading slashes, handle "///")
                    let mut j = i + 2;
                    while j < len && bytes[j] != b'\n' {
                        j += 1;
                    }
                    let mut comment_slice = &s[i + 2..j];
                    while comment_slice.starts_with('/') {
                        comment_slice = &comment_slice[1..];
                    }
                    for tok in comment_slice.split(|ch: char| !ch.is_alphanumeric()) {
                        if !tok.is_empty() {
                            out.push(tok.to_lowercase());
                        }
                    }
                    // Advance past newline if present
                    if j < len && bytes[j] == b'\n' {
                        i = j + 1;
                        code_start = i;
                        continue;
                    } else {
                        i = j;
                        code_start = i;
                        break;
                    }
                } else if i + 1 < len && bytes[i + 1] == b'*' {
                    // Emit code before block comment
                    if code_start < i {
                        let code_part = &s[code_start..i];
                        Self::tokenize_code_part(code_part, &mut out);
                    }
                    // Find end of block comment
                    let mut j = i + 2;
                    let mut found_end = false;
                    while j + 1 < len {
                        if bytes[j] == b'*' && bytes[j + 1] == b'/' {
                            found_end = true;
                            break;
                        }
                        j += 1;
                    }
                    let comment_end = if found_end { j } else { len };
                    let comment_slice = &s[i + 2..comment_end];
                    for tok in comment_slice.split(|ch: char| !ch.is_alphanumeric()) {
                        if !tok.is_empty() {
                            out.push(tok.to_lowercase());
                        }
                    }
                    if found_end {
                        i = j + 2;
                        code_start = i;
                        continue;
                    } else {
                        // Reached EOF inside block comment
                        i = len;
                        code_start = i;
                        break;
                    }
                }
            }
            i += 1;
        }

        // Emit any trailing code after the last comment
        if code_start < len {
            let code_part = &s[code_start..len];
            Self::tokenize_code_part(code_part, &mut out);
        }

        out
    }

    /// Count tokens in the entire code string without allocating per-token Strings.
    /// This is guaranteed to match the length of `tokenize()` for the same input.
    /// ```
    /// use ploke_db::bm25_index::CodeTokenizer;
    /// let s = "/// docs\nfn parseJSON_v2(x: i32) { x += 10; }";
    /// let t = CodeTokenizer::default();
    /// let toks = t.tokenize(s);
    /// let count = CodeTokenizer::count_tokens_in_code(s);
    /// assert_eq!(toks.len(), count);
    /// ```
    pub fn count_tokens_in_code(s: &str) -> usize {
        let bytes = s.as_bytes();
        let mut i = 0usize;
        let mut code_start = 0usize;
        let len = bytes.len();
        let mut count = 0usize;

        while i < len {
            if bytes[i] == b'/' {
                if i + 1 < len && bytes[i + 1] == b'/' {
                    // Count tokens in code before line comment
                    if code_start < i {
                        count += Self::token_count_in_code_part(&s[code_start..i]);
                    }
                    // Count comment tokens
                    let mut j = i + 2;
                    while j < len && bytes[j] != b'\n' {
                        j += 1;
                    }
                    let mut comment_slice = &s[i + 2..j];
                    while comment_slice.starts_with('/') {
                        comment_slice = &comment_slice[1..];
                    }
                    count += comment_slice
                        .split(|ch: char| !ch.is_alphanumeric())
                        .filter(|t| !t.is_empty())
                        .count();

                    // Advance
                    if j < len && bytes[j] == b'\n' {
                        i = j + 1;
                        code_start = i;
                        continue;
                    } else {
                        i = j;
                        code_start = i;
                        break;
                    }
                } else if i + 1 < len && bytes[i + 1] == b'*' {
                    // Count tokens in code before block comment
                    if code_start < i {
                        count += Self::token_count_in_code_part(&s[code_start..i]);
                    }
                    // Find end of block comment
                    let mut j = i + 2;
                    let mut found_end = false;
                    while j + 1 < len {
                        if bytes[j] == b'*' && bytes[j + 1] == b'/' {
                            found_end = true;
                            break;
                        }
                        j += 1;
                    }
                    let comment_end = if found_end { j } else { len };
                    let comment_slice = &s[i + 2..comment_end];
                    count += comment_slice
                        .split(|ch: char| !ch.is_alphanumeric())
                        .filter(|t| !t.is_empty())
                        .count();

                    if found_end {
                        i = j + 2;
                        code_start = i;
                        continue;
                    } else {
                        i = len;
                        code_start = i;
                        break;
                    }
                }
            }
            i += 1;
        }

        if code_start < len {
            count += Self::token_count_in_code_part(&s[code_start..len]);
        }

        count
    }

    /// Count tokens in a code segment (no comments), mirroring tokenize_code_part rules.
    fn token_count_in_code_part(line: &str) -> usize {
        let mut count = 0usize;
        let mut id_start: Option<usize> = None;

        for (i, ch) in line.char_indices() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                if id_start.is_none() {
                    id_start = Some(i);
                }
            } else if ch.is_whitespace() {
                if let Some(start) = id_start.take() {
                    count += Self::split_identifier_count(&line[start..i]);
                }
            } else {
                if let Some(start) = id_start.take() {
                    count += Self::split_identifier_count(&line[start..i]);
                }
                // symbol token
                count += 1;
            }
        }
        if let Some(start) = id_start.take() {
            count += Self::split_identifier_count(&line[start..]);
        }
        count
    }

    /// Count subtokens for an identifier (snake_case, camelCase, PascalCase, digits, acronyms)
    fn split_identifier_count(ident: &str) -> usize {
        let mut total = 0usize;
        for chunk in ident.split('_') {
            if chunk.is_empty() {
                continue;
            }
            let chars: Vec<char> = chunk.chars().collect();
            if chars.is_empty() {
                continue;
            }
            let mut part_len = 0usize;
            for i in 0..chars.len() {
                if i > 0 {
                    let prev = chars[i - 1];
                    let next = chars.get(i + 1).copied();
                    let ch = chars[i];
                    let lower_to_upper = prev.is_lowercase() && ch.is_uppercase();
                    let upper_seq_then_lower = prev.is_uppercase()
                        && ch.is_uppercase()
                        && next.map_or_else(|| false, |n| n.is_lowercase());
                    let digit_boundary = (prev.is_ascii_digit() && !ch.is_ascii_digit())
                        || (!prev.is_ascii_digit() && ch.is_ascii_digit());
                    if (lower_to_upper || upper_seq_then_lower || digit_boundary) && part_len > 0 {
                        total += 1;
                        part_len = 0;
                    }
                }
                part_len += 1;
            }
            if part_len > 0 {
                total += 1;
            }
        }
        total
    }

    fn tokenize_code_part(line: &str, out: &mut Vec<String>) {
        let mut cur = String::new();
        let mut cur_is_id = false;

        let push_cur = |out: &mut Vec<String>, cur: &mut String, cur_is_id: &mut bool| {
            if cur.is_empty() {
                return;
            }
            if *cur_is_id {
                for sub in Self::split_identifier(cur) {
                    out.push(sub);
                }
            } else {
                // push symbol as-is (but lowercased)
                out.push(cur.to_lowercase());
            }
            cur.clear();
            *cur_is_id = false;
        };

        for ch in line.chars() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                if !cur_is_id {
                    push_cur(out, &mut cur, &mut cur_is_id);
                }
                cur.push(ch);
                cur_is_id = true;
            } else if ch.is_whitespace() {
                push_cur(out, &mut cur, &mut cur_is_id);
            } else {
                if cur_is_id {
                    push_cur(out, &mut cur, &mut cur_is_id);
                }
                out.push(ch.to_string());
            }
        }
        push_cur(out, &mut cur, &mut cur_is_id);
    }
}

impl Tokenizer for CodeTokenizer {
    fn tokenize(&self, input_text: &str) -> Vec<String> {
        Self::tokens_from_code(input_text)
    }
}

// ------------------------- Cozo client trait + DocMeta -------------------------

#[derive(Debug, Clone, Copy)]
pub struct DocMeta {
    pub token_length: usize,
    pub tracking_hash: TrackingHash,
}

#[derive(Debug, Clone)]
pub struct DocData {
    pub id: Uuid,
    pub meta: DocMeta,
    pub snippet: String,
}

impl From<(&EmbeddingData, &String)> for DocData {
    fn from(value: (&EmbeddingData, &String)) -> Self {
        let token_length = CodeTokenizer::count_tokens_in_code(value.1);
        let tracking_hash = value.0.node_tracking_hash; // Use node tracking hash from EmbeddingData
        DocData {
            id: value.0.id,
            snippet: value.1.clone(),
            meta: DocMeta {
                token_length,
                tracking_hash,
            },
        }
    }
}

// ------------------------- BM25 Indexer -------------------------

    /// In-memory BM25 indexer that uses bm25::Embedder + Scorer.
    ///
    /// Example:
    /// ```
    /// use ploke_db::bm25_index::{Bm25Indexer, DocData, DocMeta, CodeTokenizer};
    /// use ploke_core::TrackingHash;
    /// use uuid::Uuid;
    ///
    /// let mut idx = Bm25Indexer::new(10.0);
    /// let id = Uuid::new_v4();
    /// let snippet = String::from("fn add_one(x: i32) -> i32 { x + 1 }");
    /// let meta = DocMeta {
    ///     token_length: CodeTokenizer::count_tokens_in_code(&snippet),
    ///     tracking_hash: TrackingHash(Uuid::new_v5(&Uuid::NAMESPACE_DNS, snippet.as_bytes())),
    /// };
    /// let doc = DocData { id, meta, snippet };
    /// idx.upsert_batch(std::iter::once(doc));
    /// let res = idx.search("add_one", 5);
    /// assert!(!res.is_empty());
    /// ```
pub struct Bm25Indexer {
    embedder: bm25::Embedder<u32, CodeTokenizer>,
    scorer: Scorer<Uuid, u32>,
    staged_meta: HashMap<Uuid, DocMeta>,
    version: &'static str,
}

impl Bm25Indexer {
    /// Create a new indexer. `avgdl` should be an estimate or a fitted value for your corpus.
    pub fn new(avgdl: f32) -> Self {
        let embedder = EmbedderBuilder::<u32, CodeTokenizer>::with_avgdl(avgdl).build();
        let scorer = Scorer::<Uuid, u32>::new();
        Self {
            embedder,
            scorer,
            staged_meta: HashMap::new(),
            version: TOKENIZER_VERSION,
        }
    }

    pub fn stage_doc_meta(&mut self, id: Uuid, meta: DocMeta) {
        self.staged_meta.insert(id, meta);
    }

    pub fn extend_staged(&mut self, docs: impl IntoIterator<Item = (Uuid, DocMeta)>) {
        self.staged_meta.extend(docs);
    }

    /// Construct a new Bm25Indexer from a corpus Vec<(Uuid, String)>.
    /// This computes average document length (avgdl) from the corpus tokens, builds an
    /// embedder with that avgdl, and indexes all documents. The corpus is consumed.
    pub fn new_from_corpus(corpus: Vec<(Uuid, String)>) -> Self {
        // compute token lengths for each doc first
        let mut total_tokens: usize = 0;
        let mut doc_token_counts: Vec<(Uuid, usize, String)> = Vec::with_capacity(corpus.len());
        for (id, snippet) in corpus.into_iter() {
            let len = CodeTokenizer::count_tokens_in_code(&snippet);
            total_tokens += len;
            // store snippet string temporarily so we can re-embed after building embedder
            doc_token_counts.push((id, len, snippet));
        }
        let n = doc_token_counts.len();
        let avgdl = if n > 0 {
            (total_tokens as f32) / (n as f32)
        } else {
            0.0
        };

        let embedder = EmbedderBuilder::<u32, CodeTokenizer>::with_avgdl(avgdl).build();
        let mut scorer = Scorer::<Uuid, u32>::new();

        // now embed and upsert
        for (id, _len, snippet) in doc_token_counts.into_iter() {
            let embedding = embedder.embed(&snippet);
            scorer.upsert(&id, embedding);
        }

        Self {
            embedder,
            scorer,
            staged_meta: HashMap::new(),
            version: TOKENIZER_VERSION,
        }
    }

    /// Index a batch of (uuid, snippet) pairs.
    /// Returns the number of items indexed.
    pub fn index_batch(&mut self, batch: Vec<DocData>) -> usize {
        let inserted = batch.len();
        for DocData { id, meta, snippet } in batch {
            let tokens = CodeTokenizer::tokens_from_code(&snippet);
            tracing::debug!("index_batch tokens: {:?}", tokens);
            let embedding = self.embedder.embed(&snippet);
            self.scorer.upsert(&id, embedding);
            // Stage per-doc metadata for atomic Finalize
            let tracking_hash = meta.tracking_hash;
            let token_len = meta.token_length;
            self.staged_meta.insert(
                id,
                DocMeta {
                    token_length: token_len,
                    tracking_hash,
                },
            );
        }
        inserted
    }

    /// Index a batch of (uuid, snippet) pairs.
    /// Returns the number of items indexed.
    pub fn upsert_batch(&mut self, batch: impl IntoIterator<Item = DocData>) -> usize {
        let mut inserted = 0;
        for DocData { id, meta, snippet } in batch {
            let embedding = self.embedder.embed(&snippet);
            self.scorer.upsert(&id, embedding);
            // Stage per-doc metadata for atomic Finalize and avgdl computation
            self.staged_meta.insert(id, meta);
            inserted += 1;
        }
        inserted
    }

    /// Index a batch of (uuid, snippet) pairs.
    /// Returns the number of items indexed.
    pub fn upsert_batch_with_cozo(
        &mut self,
        cozo: &Database,
        batch: impl IntoIterator<Item = DocData>,
    ) -> Result<usize, DbError> {
        let mut inserted = 0;
        let new_iter = batch.into_iter().map(|d| {
            let DocData { id, meta, snippet } = d;
            let embedding = self.embedder.embed(&snippet);
            self.scorer.upsert(&id, embedding);
            // Stage per-doc metadata (keeps parity with index_batch and supports avgdl/finalize)
            self.staged_meta.insert(id, meta);
            inserted += 1;
            (id, meta)
        });
        cozo.upsert_bm25_doc_meta_batch(new_iter)?;
        Ok(inserted)
    }

    ///// Index a batch and upsert document metadata into the provided Cozo client.
    ///// This demonstrates action (A): write doc metadata to Cozo while indexing.
    //pub fn index_batch_with_cozo(
    //    &mut self,
    //    batch: Vec<(Uuid, String)>,
    //    cozo: Arc<Database>,
    //) {
    //    for (id, snippet) in batch {
    //        let embedding = self.embedder.embed(&snippet);
    //        self.scorer.upsert(&id, embedding);
    //        // compute a stable tracking hash (UUID v5 over DNS namespace) for the snippet)
    //        // NOTE: This wraps the UUID v5 into the project's TrackingHash newtype.
    //        // In the future, prefer TrackingHash::generate(...) when token/context data is available.
    //        let tracking_hash = TrackingHash(Uuid::new_v5(&Uuid::NAMESPACE_DNS, snippet.as_bytes()));
    //        // compute token length using tokenizer
    //        let token_len = CodeTokenizer::count_tokens_in_code(&snippet);
    //
    //        // stage for Finalize
    //        self.staged_meta.insert(
    //            id,
    //            DocMeta {
    //                token_length: token_len,
    //                tracking_hash,
    //            },
    //        );
    //
    //        // upsert to cozo
    //        cozo.upsert_bm25_doc_meta_batch(
    //            id,
    //            DocMeta {
    //                token_length: token_len,
    //                tracking_hash,
    //            },
    //        );
    //    }
    //}

    /// Remove a document by id (used when file changes and nodes are pruned)
    pub fn remove(&mut self, id: &Uuid) {
        self.scorer.remove(id);
    }

    /// Search with a query string, returning top-k results as ScoredDocument<Uuid>
    pub fn search(&self, query: &str, top_k: usize) -> Vec<bm25::ScoredDocument<Uuid>> {
        let qemb = self.embedder.embed(query);
        tracing::debug!("qemb: {qemb:?}");
        let mut matches = self.scorer.matches(&qemb);
        tracing::debug!("matches: {matches:?}");
        if matches.len() > top_k {
            matches.truncate(top_k);
        }
        matches
    }

    /// Compute average document length (avgdl) from staged metadata.
    /// Returns 0.0 if no documents are staged.
    pub fn compute_avgdl_from_staged(&self) -> f32 {
        let n = self.staged_meta.len();
        if n == 0 {
            return 0.0;
        }
        let total: usize = self.staged_meta.values().map(|m| m.token_length).sum();
        (total as f32) / (n as f32)
    }

    /// Drain staged metadata for persistence during Finalize.
    pub fn drain_staged_meta(&mut self) -> Vec<(Uuid, DocMeta)> {
        self.staged_meta.drain().collect()
    }
}

// ------------------------- Tests -------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{create_index_primary, DbError};
    use lazy_static::lazy_static;
    use ploke_error::Error as PlokeError;
    use ploke_test_utils::workspace_root;
    use std::collections::HashMap;

    struct MockCozo {
        pub store: HashMap<Uuid, DocMeta>,
    }

    impl MockCozo {
        pub fn new() -> Self {
            Self {
                store: HashMap::new(),
            }
        }
    }

    lazy_static! {
        /// This test db is restored from the backup of an earlier parse of the `fixture_nodes`
        /// crate located in `tests/fixture_crates/fixture_nodes`, and has a decent sampling of all
        /// rust code items. It provides a good target for other tests because it has already been
        /// extensively tested in `syn_parser`, with each item individually verified to have all
        /// fields correctly parsed for expected values.
        ///
        /// One "gotcha" of laoding the Cozo database is that the hnsw items are not retained
        /// between backups, so they must be recalculated each time. However, by restoring the
        /// backup database we do retain the dense vector embeddings, allowing our tests to be
        /// significantly sped up by using a lazy loader here and making calls to the same backup.
        ///
        /// If needed, other tests can re-implement the load from this file, which may become a
        /// factor for some tests that need to alter the database, but as long as things are
        /// cleaned up afterwards it should be OK.
        // TODO: Add a mutex guard to avoid cross-contamination of tests.
        pub static ref TEST_DB_NODES: Result<Arc< Database >, PlokeError> = {
            let db = Database::init_with_schema()?;

            let mut target_file = workspace_root();
            target_file.push("tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92");
            let prior_rels_vec = db.relations_vec()?;
            db.import_from_backup(&target_file, &prior_rels_vec)
                .map_err(DbError::from)
                .map_err(ploke_error::Error::from)?;
            create_index_primary(&db)?;
            Ok(Arc::new( db ))
        };
    }

    #[test]
    fn tokenizer_splits_identifiers_and_comments() {
        let t = CodeTokenizer;
        let src = r#"// leading comment
fn FooBar_baz(x: i32) -> i32 { /* block comment */ x + 1 }"#;
        let toks = t.tokenize(src);
        // should include comment words and identifier subtokens
        assert!(toks.iter().any(|s| s == "leading"));
        assert!(toks.iter().any(|s| s == "comment"));
        assert!(toks.iter().any(|s| s == "foo"));
        assert!(toks.iter().any(|s| s == "bar"));
        assert!(toks.iter().any(|s| s == "baz"));
    }

    #[test]
    fn indexer_indexes_and_searches_basic() {
        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        let a = String::from("fn add_one(x: i32) -> i32 { x + 1 }");
        let b = String::from(
            "/// does something
fn compute_answer() -> i32 { 42 }",
        );

        // Use corpus constructor to fit avgdl and index both docs
        let idx = Bm25Indexer::new_from_corpus(vec![(id_a, a.clone()), (id_b, b.clone())]);

        // query for 'compute' should return id_b first
        let results = idx.search("compute", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].id, id_b);

        // query for 'add_one' (identifier) should return id_a first
        let results2 = idx.search("add_one", 10);
        assert!(!results2.is_empty());
        assert_eq!(results2[0].id, id_a);
    }

    #[test]
    fn scorer_scores_higher_for_matching_document() {
        let mut idx = Bm25Indexer::new(10.0);
        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        let a = String::from("fn alpha() { println!(\"hello\"); }");
        let b = String::from("fn beta() { println!(\"compute\"); }");

        // Index both docs into the scorer
        let docs = vec![
            DocData {
                id: id_a,
                meta: DocMeta {
                    token_length: CodeTokenizer::count_tokens_in_code(&a),
                    tracking_hash: TrackingHash(Uuid::new_v5(&Uuid::NAMESPACE_DNS, a.as_bytes())),
                },
                snippet: a.clone(),
            },
            DocData {
                id: id_b,
                meta: DocMeta {
                    token_length: CodeTokenizer::count_tokens_in_code(&b),
                    tracking_hash: TrackingHash(Uuid::new_v5(&Uuid::NAMESPACE_DNS, b.as_bytes())),
                },
                snippet: b.clone(),
            },
        ];
        idx.index_batch(docs);

        let qemb = idx.embedder.embed("compute");
        let score_a = idx.scorer.score(&id_a, &qemb).unwrap_or(0.0);
        let score_b = idx.scorer.score(&id_b, &qemb).unwrap_or(0.0);
        assert!(
            score_b > score_a,
            "expected matching doc to score higher ({} > {})",
            score_b,
            score_a
        );
    }

    #[test]
    fn index_batch_with_cozo_writes_doc_meta() {
        let mut idx = Bm25Indexer::new(10.0);
        let db = TEST_DB_NODES.as_ref().expect("test db init").clone();
        let id = Uuid::new_v4();
        let snippet = String::from(
            "/// docs
fn hello() { println!(\"hi\"); }",
        );
        let meta = DocMeta {
            token_length: CodeTokenizer::count_tokens_in_code(&snippet),
            tracking_hash: TrackingHash(Uuid::new_v5(&Uuid::NAMESPACE_DNS, snippet.as_bytes())),
        };
        let docs = vec![DocData {
            id,
            meta,
            snippet: snippet.clone(),
        }];
        let inserted = idx
            .upsert_batch_with_cozo(db.as_ref(), docs)
            .expect("upsert to cozo");
        assert_eq!(inserted, 1);

        // Ensure the in-memory staged metadata captured what we sent
        let staged = idx.staged_meta.get(&id).expect("staged meta missing");
        assert_eq!(staged.token_length, meta.token_length);
        assert_eq!(staged.tracking_hash, meta.tracking_hash);
    }

    #[test]
    fn new_from_corpus_consumes_vec_and_indexes() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let corpus: Vec<(Uuid, String)> = vec![
            (id1, String::from("fn a() {}")),
            (id2, String::from("fn b() {}")),
        ];
        // new_from_corpus takes ownership
        let idx = Bm25Indexer::new_from_corpus(corpus);
        // ensure docs are indexed by searching
        let res = idx.search("a", 10);
        assert!(!res.is_empty());
    }

    #[test]
    fn token_count_matches_tokenize_len_on_simple_input() {
        let t = CodeTokenizer;
        let s = "/// docs\nfn parseJSON_v2(x: i32) { x += 10; }";
        let toks = t.tokenize(s);
        let count = CodeTokenizer::count_tokens_in_code(s);
        assert_eq!(toks.len(), count, "tokenize() and count_tokens_in_code() should agree");
    }

    #[test]
    fn compute_avgdl_from_staged_meta() {
        let mut idx = Bm25Indexer::new(10.0);
        let s1 = "fn first_token() { let x = 1; }";
        let s2 = "fn second_token_longer_name() { let y = 2; }";
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let m1 = DocMeta {
            token_length: CodeTokenizer::count_tokens_in_code(s1),
            tracking_hash: TrackingHash(Uuid::new_v5(&Uuid::NAMESPACE_DNS, s1.as_bytes())),
        };
        let m2 = DocMeta {
            token_length: CodeTokenizer::count_tokens_in_code(s2),
            tracking_hash: TrackingHash(Uuid::new_v5(&Uuid::NAMESPACE_DNS, s2.as_bytes())),
        };
        let docs = vec![
            DocData { id: id1, meta: m1, snippet: s1.to_string() },
            DocData { id: id2, meta: m2, snippet: s2.to_string() },
        ];
        idx.index_batch(docs);
        let expected = (m1.token_length as f32 + m2.token_length as f32) / 2.0;
        let got = idx.compute_avgdl_from_staged();
        assert!((got - expected).abs() < f32::EPSILON, "avgdl mismatch: got {got}, expected {expected}");
    }

    #[tokio::test]
    async fn bm25_service_index_search_finalize_roundtrip() {
        let db = TEST_DB_NODES.as_ref().expect("test db init").clone();
        let tx = bm25_service::start_default(db).expect("start bm25 service");

        // Prepare a document with a unique token to avoid accidental collisions
        let id = Uuid::new_v4();
        let snippet = String::from("fn unique_xylophone_token() { let compute = 1; }");
        let meta = DocMeta {
            token_length: CodeTokenizer::count_tokens_in_code(&snippet),
            tracking_hash: TrackingHash(Uuid::new_v5(&Uuid::NAMESPACE_DNS, snippet.as_bytes())),
        };
        let doc = DocData { id, meta, snippet: snippet.clone() };

        // Index
        tx.send(bm25_service::Bm25Cmd::IndexBatch { docs: vec![doc] })
            .await
            .expect("send index batch");

        // Search
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        tx.send(bm25_service::Bm25Cmd::Search {
            query: "xylophone".to_string(),
            top_k: 5,
            resp: resp_tx,
        })
        .await
        .expect("send search");
        let results = resp_rx.await.expect("await search");
        assert!(!results.is_empty(), "expected non-empty search results");
        assert_eq!(results[0].0, id, "expected the indexed doc to rank first");

        // Finalize
        let (f_tx, f_rx) = tokio::sync::oneshot::channel();
        tx.send(bm25_service::Bm25Cmd::FinalizeSeed { resp: f_tx })
            .await
            .expect("send finalize");
        let ack = f_rx.await.expect("await finalize ack");
        assert!(ack.is_ok(), "finalize ack should be Ok");
    }
}
