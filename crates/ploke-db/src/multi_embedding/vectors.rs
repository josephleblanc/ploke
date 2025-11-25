use std::collections::BTreeMap;
use std::ops::Deref;

use crate::database::Database;
use crate::error::DbError;
use crate::multi_embedding::adapter::EmbeddingDbExt;
use crate::multi_embedding::schema::vector_dims::{vector_literal, HnswEmbedInfo};
use cozo::{Db, MemStorage, ScriptMutability};
use ploke_core::embedding_set::EmbRelName;
use ploke_core::{ArcStr, EmbeddingModelId};
use uuid::Uuid;

pub trait CozoVectorExt {
    fn dims(&self) -> u32;

    /// Identity script for relation, with all columns.
    /// e.g.
    /// "embedding-model_dims { node_id, embedding_model, provider, at => embedding_dims, vector }
    fn script_identity(&self) -> String;

    /// Creation script for Cozo database, used to create the relation used for the embedding
    /// vector relation.
    fn script_create(&self) -> String;

    fn vector_relation(&self) -> &EmbRelName;
}
//
// fn sanitize_relation_component(raw: &str) -> String {
//     let mut out = String::with_capacity(raw.len() + 4);
//     for ch in raw.chars() {
//         if ch.is_ascii_alphanumeric() {
//             out.push(ch);
//         } else {
//             out.push('_');
//         }
//     }
//     if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
//         out.insert(0, 'v');
//     }
//     out
// }
