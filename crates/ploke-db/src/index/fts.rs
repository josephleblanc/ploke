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
    let script = [
        format!(
            "::fts create {}:{} {{ extractor: symbol_text, extract_filter: !is_null(symbol_text), tokenizer: Simple, filters: [Lowercase] }}",
            FTS_RELATION, SYMBOLS_IDX
        ),
        format!(
            "::fts create {}:{} {{ extractor: body_text, extract_filter: !is_null(body_text), tokenizer: Simple, filters: {} }}",
            FTS_RELATION, BODY_IDX, body_filters
        ),
    ]
    .join("\n");
    db.run_script(
        &script,
        std::collections::BTreeMap::new(),
        ScriptMutability::Mutable,
    )?;
    Ok(())
}

/// Replace both FTS indexes (drop if exists and recreate with new config)
pub fn replace_fts_indexes(db: &Database, body_use_stopwords: bool) -> Result<(), DbError> {
    let body_filters = if body_use_stopwords {
        "[Lowercase, Stopwords('en')]"
    } else {
        "[Lowercase]"
    };
    let script = [
        format!(
            "::fts replace {}:{} {{ extractor: symbol_text, extract_filter: !is_null(symbol_text), tokenizer: Simple, filters: [Lowercase] }}",
            FTS_RELATION, SYMBOLS_IDX
        ),
        format!(
            "::fts replace {}:{} {{ extractor: body_text, extract_filter: !is_null(body_text), tokenizer: Simple, filters: {} }}",
            FTS_RELATION, BODY_IDX, body_filters
        ),
    ]
    .join("\n");
    db.run_script(
        &script,
        std::collections::BTreeMap::new(),
        ScriptMutability::Mutable,
    )?;
    Ok(())
}

/// Drop both FTS indexes
pub fn drop_fts_indexes(db: &Database) -> Result<(), DbError> {
    let script = [
        format!("::fts drop {}:{}", FTS_RELATION, SYMBOLS_IDX),
        format!("::fts drop {}:{}", FTS_RELATION, BODY_IDX),
    ]
    .join("\n");
    db.run_script(
        &script,
        std::collections::BTreeMap::new(),
        ScriptMutability::Mutable,
    )?;
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

    // ploke_test_utils::setup_db_full_crate("ploke-tui")
    //     .map(|d| Database::new(d))
    lazy_static! {
        pub static ref TEST_DB: Database = {
            let cozo_db = ploke_test_utils::setup_db_full_crate("ploke-tui")
                .expect("Invariant: setup_db_full_crate must successfully run on target.");
            Database::new(cozo_db)
        };

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

    #[test]
    fn test_create_fts_indicies() -> Result<(), ploke_error::Error> {
        let db = &TEST_DB;
        // AI: Write the rest of the test AI!
        Ok(())
    }

    // AI: Write the remaining tests AI!
}
