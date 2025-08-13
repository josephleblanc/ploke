use crate::{Database, DbError};
use cozo::{DataValue, Num, ScriptMutability, UuidWrapper};
use itertools::Itertools;
use uuid::Uuid;

/// Relation and index names
pub const FTS_RELATION: &str = "node_fts";
pub const SYMBOLS_IDX: &str = "symbols_idx";
pub const BODY_IDX: &str = "body_idx";

/// Create the FTS backing relation:
/// node_fts { id: Uuid => symbol_text: String?, body_text: String? }
pub fn create_fts_relation(db: &Database) -> Result<(), DbError> {
    let script = format!(
        ":create {} {{ id: Uuid => symbol_text: String?, body_text: String? }}",
        FTS_RELATION
    );
    db.run_script(
        &script,
        std::collections::BTreeMap::new(),
        ScriptMutability::Mutable,
    )?;
    Ok(())
}

/// Drop the FTS backing relation
pub fn drop_fts_relation(db: &Database) -> Result<(), DbError> {
    // Drop relation; ':drop' is the correct directive to remove a relation definition.
    let script = format!(":drop {}", FTS_RELATION);
    db.run_script(
        &script,
        std::collections::BTreeMap::new(),
        ScriptMutability::Mutable,
    )?;
    Ok(())
}

/// Create both FTS indexes:
/// - symbols_idx over symbol_text with Simple + Lowercase
/// - body_idx over body_text with Simple + Lowercase (+ optional Stopwords('en'))
pub fn create_fts_indexes(db: &Database, body_use_stopwords: bool) -> Result<(), DbError> {
    let body_filters = if body_use_stopwords {
        "[Lowercase, Stopwords('en')]"
    } else {
        "[Lowercase]"
    };
    let cmds = [
        format!(
            "::fts create {}:{} {{ extractor: symbol_text, extract_filter: !is_null(symbol_text), tokenizer: Simple, filters: [Lowercase] }}",
            FTS_RELATION, SYMBOLS_IDX
        ),
        format!(
            "::fts create {}:{} {{ extractor: body_text, extract_filter: !is_null(body_text), tokenizer: Simple, filters: {} }}",
            FTS_RELATION, BODY_IDX, body_filters
        ),
    ];
    for cmd in cmds {
        db.run_script(
            &cmd,
            std::collections::BTreeMap::new(),
            ScriptMutability::Mutable,
        )?;
    }
    Ok(())
}

/// Replace both FTS indexes (drop if exists and recreate with new config)
pub fn replace_fts_indexes(db: &Database, body_use_stopwords: bool) -> Result<(), DbError> {
    let body_filters = if body_use_stopwords {
        "[Lowercase, Stopwords('en')]"
    } else {
        "[Lowercase]"
    };
    let cmds = [
        format!(
            "::fts replace {}:{} {{ extractor: symbol_text, extract_filter: !is_null(symbol_text), tokenizer: Simple, filters: [Lowercase] }}",
            FTS_RELATION, SYMBOLS_IDX
        ),
        format!(
            "::fts replace {}:{} {{ extractor: body_text, extract_filter: !is_null(body_text), tokenizer: Simple, filters: {} }}",
            FTS_RELATION, BODY_IDX, body_filters
        ),
    ];
    for cmd in cmds {
        db.run_script(
            &cmd,
            std::collections::BTreeMap::new(),
            ScriptMutability::Mutable,
        )?;
    }
    Ok(())
}

/// Drop both FTS indexes
pub fn drop_fts_indexes(db: &Database) -> Result<(), DbError> {
    let cmds = [
        format!("::fts drop {}:{}", FTS_RELATION, SYMBOLS_IDX),
        format!("::fts drop {}:{}", FTS_RELATION, BODY_IDX),
    ];
    for cmd in cmds {
        db.run_script(
            &cmd,
            std::collections::BTreeMap::new(),
            ScriptMutability::Mutable,
        )?;
    }
    Ok(())
}

/// Upsert a row in node_fts for a given node ID.
/// Pass None to clear a field; pass Some(text) to set/update.
pub fn upsert_node_fts(
    db: &Database,
    id: Uuid,
    symbol_text: Option<String>,
    body_text: Option<String>,
) -> Result<(), DbError> {
    let mut params = std::collections::BTreeMap::new();
    params.insert("id".to_string(), DataValue::Uuid(UuidWrapper(id)));
    params.insert(
        "symbol_text".to_string(),
        match symbol_text {
            Some(s) => DataValue::Str(s.into()),
            None => DataValue::Null,
        },
    );
    params.insert(
        "body_text".to_string(),
        match body_text {
            Some(s) => DataValue::Str(s.into()),
            None => DataValue::Null,
        },
    );

    // Insert or replace the row
    let script = format!(
        r#"
        ?[id, symbol_text, body_text] <- [[ $id, $symbol_text, $body_text ]]
        :put {} {{ id => symbol_text, body_text }}
        "#,
        FTS_RELATION
    );
    db.run_script(&script, params, ScriptMutability::Mutable)?;
    Ok(())
}

/// Search the symbols index. Returns Vec<(score, id, symbol_text)>
pub fn search_symbols(
    db: &Database,
    q: &str,
    k: usize,
) -> Result<Vec<(f64, Uuid, String)>, DbError> {
    let mut params = std::collections::BTreeMap::new();
    params.insert("q".into(), DataValue::Str(q.into()));
    params.insert("k".into(), DataValue::Num(Num::Int(k as i64)));

    let script = format!(
        r#"
        ?[s, id, symbol_text] := ~{}:{} {{ id, symbol_text |
            query: $q,
            k: $k,
            score_kind: 'tf_idf',
            bind_score: s
        }}
        :order -s
        "#,
        FTS_RELATION, SYMBOLS_IDX
    );

    let result = db.run_script(&script, params, ScriptMutability::Immutable)?;
    let mut out = Vec::with_capacity(result.rows.len());
    for row in result.rows {
        let score = row[0].get_float().unwrap_or(0.0);
        let id = match row[1] {
            DataValue::Uuid(UuidWrapper(u)) => u,
            _ => Uuid::nil(),
        };
        let text = row[2].get_str().unwrap_or_default().to_string();
        out.push((score, id, text));
    }
    Ok(out)
}

/// Search the body index. Returns Vec<(score, id, body_text)>
pub fn search_body(
    db: &Database,
    q: &str,
    k: usize,
) -> Result<Vec<(f64, Uuid, String)>, DbError> {
    let mut params = std::collections::BTreeMap::new();
    params.insert("q".into(), DataValue::Str(q.into()));
    params.insert("k".into(), DataValue::Num(Num::Int(k as i64)));

    let script = format!(
        r#"
        ?[s, id, body_text] := ~{}:{} {{ id, body_text |
            query: $q,
            k: $k,
            score_kind: 'tf_idf',
            bind_score: s
        }}
        :order -s
        "#,
        FTS_RELATION, BODY_IDX
    );

    let result = db.run_script(&script, params, ScriptMutability::Immutable)?;
    let mut out = Vec::with_capacity(result.rows.len());
    for row in result.rows {
        let score = row[0].get_float().unwrap_or(0.0);
        let id = match row[1] {
            DataValue::Uuid(UuidWrapper(u)) => u,
            _ => Uuid::nil(),
        };
        let text = row[2].get_str().unwrap_or_default().to_string();
        out.push((score, id, text));
    }
    Ok(out)
}

/// Split identifiers on underscores and CamelCase; include original & lowercase variants.
/// Returns a deduplicated list of tokens preserving insertion order of first occurrence.
pub fn identifier_tokens<S: AsRef<str>>(idents: impl IntoIterator<Item = S>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for s in idents {
        let s = s.as_ref();
        for tok in split_ident_keep_original(s) {
            let lc = tok.to_lowercase();
            if seen.insert(lc.clone()) {
                out.push(lc);
            }
        }
    }
    out
}

/// Build the symbols text blob from various code-centric fields.
///
/// - name: item name (e.g., "HashMap")
/// - path_segments: ["crate", "collections", "hash_map"]
/// - signature: function/type signature string
/// - doc_aliases: #[doc(alias = "...")] values
/// - cargo_features: enabled feature names
pub fn build_symbols_text(
    name: Option<&str>,
    path_segments: &[String],
    signature: Option<&str>,
    doc_aliases: &[String],
    cargo_features: &[String],
) -> String {
    let mut pieces: Vec<String> = Vec::new();

    // Name tokens (original + split)
    if let Some(n) = name {
        pieces.push(n.to_string());
        pieces.extend(identifier_tokens([n]));
    }

    // Path segments (original + split)
    pieces.extend(path_segments.iter().cloned());
    pieces.extend(identifier_tokens(path_segments.iter().map(|s| s.as_str())));

    // Signature included raw; tokenizer will split on punctuation/whitespace
    if let Some(sig) = signature {
        pieces.push(sig.to_string());
        // Include identifier-split tokens extracted naively by splitting on non-alphanum
        let sig_ident_like = sig
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_' )
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect_vec();
        pieces.extend(identifier_tokens(sig_ident_like.iter().map(|s| s.as_str())));
    }

    // Doc aliases
    pieces.extend(doc_aliases.iter().cloned());
    pieces.extend(identifier_tokens(doc_aliases.iter().map(|s| s.as_str())));

    // Cargo features (prefix for clarity) + tokens
    pieces.extend(
        cargo_features
            .iter()
            .map(|f| format!("feature:{f}"))
            .collect_vec(),
    );
    pieces.extend(identifier_tokens(cargo_features.iter().map(|s| s.as_str())));

    // Dedup while preserving order
    let mut seen = std::collections::HashSet::new();
    pieces
        .into_iter()
        .filter_map(|p| {
            let lc = p.to_lowercase();
            if seen.insert(lc.clone()) {
                Some(lc)
            } else {
                None
            }
        })
        .join(" ")
}

/// Build the body text blob from doc comments, summaries and short excerpts.
/// Keep it conservative to avoid indexing entire files.
pub fn build_body_text(
    doc_comments: &[String],
    summary: Option<&str>,
    short_excerpts: &[String],
) -> String {
    let mut pieces: Vec<String> = Vec::new();
    if let Some(s) = summary {
        pieces.push(s.to_string());
    }
    pieces.extend(doc_comments.iter().cloned());
    pieces.extend(short_excerpts.iter().cloned());

    pieces.join("\n")
}

/// Internal: split one identifier into tokens, including original string.
/// - Splits on underscores
/// - Splits CamelCase like "HashMapCapacity" -> ["Hash", "Map", "Capacity"]
fn split_ident_keep_original(s: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    if s.is_empty() {
        return tokens;
    }
    tokens.push(s.to_string()); // keep original
    for part in s.split('_') {
        if part.is_empty() {
            continue;
        }
        tokens.extend(split_camel(part));
    }
    tokens
}

/// Split a CamelCase-ish token into constituent parts without lowercasing.
/// Examples:
/// - "HashMap" -> ["Hash", "Map"]
/// - "XMLHttp" -> ["XML", "Http"]
/// - "hash2Map" -> ["hash", "2", "Map"]
fn split_camel(s: &str) -> Vec<String> {
    if s.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<String> = Vec::new();
    let mut start = 0;
    let mut chars = s.char_indices().peekable();

    while let Some((i, c)) = chars.next() {
        if i == 0 {
            continue;
        }
        let prev = s[..i].chars().last().unwrap();
        let next_opt = chars.peek().map(|(_, ch)| *ch);

        // Boundary rules:
        // - lower -> upper (e.g., hM)
        // - letter -> digit or digit -> letter
        // - acronym boundary: ABCd -> AB, Cd (upper followed by lower and previous was upper)
        let is_boundary = (prev.is_lowercase() && c.is_uppercase())
            || (prev.is_ascii_alphabetic() && c.is_ascii_digit())
            || (prev.is_ascii_digit() && c.is_ascii_alphabetic())
            || (prev.is_uppercase()
                && c.is_uppercase()
                && next_opt.map(|n| n.is_lowercase()).unwrap_or(false));

        if is_boundary {
            if start < i {
                out.push(s[start..i].to_string());
            }
            start = i;
        }
    }
    // push tail
    if start < s.len() {
        out.push(s[start..].to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use lazy_static::lazy_static;
    use tokio::sync::Mutex;

    // ploke_test_utils::setup_db_full_crate("ploke-tui")
    //     .map(|d| Database::new(d))
    lazy_static! {
        pub static ref TEST_DB: Database = {
            let cozo_db = ploke_test_utils::setup_db_full_crate("ploke-tui")
                .expect("Invariant: setup_db_full_crate must successfully run on target.");
            Database::new(cozo_db)
        };
        // Serialize FTS tests to avoid races on the shared DB and FTS objects.
        pub static ref TEST_GUARD: Mutex<()> = Mutex::new(());
    }

    // Accept only benign "not found" errors, fail otherwise.
    fn assert_ok_or_not_found<T>(res: Result<T, DbError>) {
        if let Err(e) = res {
            let msg = e.to_string().to_lowercase();
            assert!(
                msg.contains("not found")
                    || msg.contains("does not exist")
                    || msg.contains("no such")
                    || msg.contains("not exist"),
                "Unexpected error: {}",
                e
            );
        }
    }

    // Best-effort cleanup that still asserts only-not-found errors.
    // Drop the relation first; if it does not exist, indexes are irrelevant.
    // This avoids parser errors from ::fts drop when the relation is absent.
    fn clean_fts(db: &Database) {
        assert_ok_or_not_found(drop_fts_indexes(db));
        assert_ok_or_not_found(drop_fts_relation(db));
    }

    #[test]
    fn test_split_camel() {
        assert_eq!(split_camel("HashMap"), vec!["Hash", "Map"]);
        assert_eq!(split_camel("XMLHttp"), vec!["XML", "Http"]);
        assert_eq!(split_camel("hash2Map"), vec!["hash", "2", "Map"]);
        assert_eq!(split_camel("hashmap"), vec!["hashmap"]);
        assert_eq!(split_camel(""), Vec::<String>::new());
    }

    #[test]
    fn test_identifier_tokens() {
        let tokens = identifier_tokens(["HashMap_Capacity"]);
        assert!(tokens.contains(&"hash".into()));
        assert!(tokens.contains(&"map".into()));
        assert!(tokens.contains(&"capacity".into()));
    }

    #[test]
    fn test_build_symbols_text() {
        let text = build_symbols_text(
            Some("HashMap"),
            &["crate".into(), "collections".into(), "hash_map".into()],
            Some("fn capacity(&self) -> usize"),
            &["hashmap".into()],
            &["std".into()],
        );
        assert!(text.contains("hash"));
        assert!(text.contains("map"));
        assert!(text.contains("capacity"));
        assert!(text.contains("feature:std"));
        assert!(text.contains("collections"));
    }

    #[tokio::test]
    async fn test_create_fts_indicies() -> Result<(), ploke_error::Error> {
        let _g = TEST_GUARD.lock().await;
        let db = &TEST_DB;

        // Ensure a clean slate with explicit checks
        clean_fts(db);

        // Create relation and indexes
        create_fts_relation(db).unwrap();
        create_fts_indexes(db, true).unwrap();

        // Upsert two sample nodes
        let id1 = uuid::Uuid::new_v4();
        let id2 = uuid::Uuid::new_v4();

        upsert_node_fts(
            db,
            id1,
            Some("HashMap capacity".to_string()),
            Some("A hash map grows capacity".to_string()),
        )
        .unwrap();

        upsert_node_fts(
            db,
            id2,
            Some("Vec push pop".to_string()),
            Some("A growable vector".to_string()),
        )
        .unwrap();

        // Search symbols: should find id1 for "hashmap"
        let sym_results = search_symbols(db, "hashmap", 10).unwrap();
        assert!(sym_results.iter().any(|(_, id, _)| *id == id1));

        // Search body: should find id1 for "capacity"
        let body_results = search_body(db, "capacity", 10).unwrap();
        assert!(body_results.iter().any(|(_, id, _)| *id == id1));

        // Replace indexes with different config and ensure search still works
        replace_fts_indexes(db, false).unwrap();

        let sym_results_2 = search_symbols(db, "vec", 10).unwrap();
        assert!(sym_results_2.iter().any(|(_, id, _)| *id == id2));

        // Cleanup with explicit checks
        clean_fts(db);

        Ok(())
    }

    #[tokio::test]
    async fn test_drop_fts_indexes() {
        let _g = TEST_GUARD.lock().await;
        let db = &TEST_DB;

        clean_fts(db);

        create_fts_relation(db).unwrap();
        create_fts_indexes(db, true).unwrap();

        let id = uuid::Uuid::new_v4();
        upsert_node_fts(
            db,
            id,
            Some("AlphaBeta".to_string()),
            Some("Body text about alpha".to_string()),
        )
        .unwrap();

        // Drop only indexes; relation remains
        drop_fts_indexes(db).unwrap();

        // Searches should now error due to missing indexes
        assert!(search_symbols(db, "alpha", 5).is_err());
        assert!(search_body(db, "body", 5).is_err());

        // Recreate indexes and verify searches work again
        create_fts_indexes(db, false).unwrap();
        let sym_ok = search_symbols(db, "alpha", 5).unwrap();
        assert!(sym_ok.iter().any(|(_, i, _)| *i == id));

        clean_fts(db);
    }

    #[tokio::test]
    async fn test_drop_fts_relation() {
        let _g = TEST_GUARD.lock().await;
        let db = &TEST_DB;

        clean_fts(db);

        create_fts_relation(db).unwrap();
        create_fts_indexes(db, false).unwrap();

        let id = uuid::Uuid::new_v4();
        upsert_node_fts(db, id, Some("GammaDelta".into()), Some("Some body".into())).unwrap();

        // Drop the relation; subsequent upserts and searches should fail
        drop_fts_relation(db).unwrap();

        assert!(upsert_node_fts(db, id, Some("x".into()), None).is_err());
        assert!(search_symbols(db, "gamma", 5).is_err());
        assert!(search_body(db, "body", 5).is_err());

        // Recreate relation + indexes and verify end-to-end again
        create_fts_relation(db).unwrap();
        create_fts_indexes(db, true).unwrap();

        upsert_node_fts(db, id, Some("Gamma".into()), Some("Body".into())).unwrap();
        let sym_ok = search_symbols(db, "gamma", 5).unwrap();
        assert!(sym_ok.iter().any(|(_, i, _)| *i == id));

        clean_fts(db);
    }

    #[tokio::test]
    async fn test_replace_requires_relation() {
        let _g = TEST_GUARD.lock().await;
        let db = &TEST_DB;

        clean_fts(db);

        // Without relation, replace should fail
        assert!(replace_fts_indexes(db, true).is_err());

        // After creating relation, replace should succeed
        create_fts_relation(db).unwrap();
        assert!(replace_fts_indexes(db, false).is_ok());

        clean_fts(db);
    }

}
