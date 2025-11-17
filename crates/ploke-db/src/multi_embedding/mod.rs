pub mod adapter;
pub mod schema;
pub mod seeding;
pub mod vectors;

use super::*;

use std::collections::{BTreeMap, HashSet};

use crate::database::Database;
use crate::error::DbError;
use crate::NodeType;
use cozo::{self, DataValue, Db, MemStorage, NamedRows, Num, ScriptMutability, UuidWrapper};
use itertools::Itertools;
use lazy_static::lazy_static;
use std::ops::Deref;
use uuid::Uuid;

#[derive(Copy, Clone, Debug)]
pub enum HnswDistance {
    L2,
    Cosine,
    Ip,
}

impl HnswDistance {
    fn as_str(&self) -> &'static str {
        match self {
            HnswDistance::L2 => "L2",
            HnswDistance::Cosine => "Cosine",
            HnswDistance::Ip => "IP",
        }
    }
}

pub fn experimental_node_relation_specs() -> &'static [ExperimentalNodeRelationSpec] {
    &EXPERIMENTAL_NODE_RELATION_SPECS
}

const ID_KEYWORDS: [&str; 9] = [
    "id",
    "function_id",
    "owner_id",
    "source_id",
    "target_id",
    "type_id",
    "node_id",
    "embedding_model",
    "provider",
];
const ID_VAL_KEYWORDS: [&str; 9] = [
    "id: Uuid",
    "function_id: Uuid",
    "owner_id: Uuid",
    "source_id: Uuid",
    "target_id: Uuid",
    "type_id: Uuid",
    "node_id: Uuid",
    "embedding_model: String",
    "provider: String",
];
