use cozo::NamedRows;
use itertools::Itertools;
use lazy_static::lazy_static;
use ploke_core::{rag_types::ContextPart, TrackingHash};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    database::{to_string, to_uuid, ImmutQuery},
    result::{get_byte_offsets, get_pos},
    Database, DbError, NodeType, QueryResult,
};

lazy_static! {
    /// Query rule to find primary nodes (multi-embedding schema version).
    /// In multi-embedding schema, embeddings are stored in separate relations,
    /// so we don't filter by embedding field here.
    pub static ref COMMON_FIELDS_EMBEDDED: String = NodeType::primary_nodes().iter().map(|ty| {
        let rel = ty.relation_str();
            format!(r#"
            has_embedding[id, name, hash, span] := *{rel}{{id, name, tracking_hash: hash, span @ 'NOW'}}
            "#)
        }).join("\n");
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommonFields {
    id: Uuid,
    name: String,
    hash: TrackingHash,
    span: [i64; 2],
}

impl CommonFields {
    fn try_from_query_result(value: QueryResult) -> Result<Vec<Self>, ploke_error::Error> {
        let id_index: usize = get_pos(&value.headers, "id")?;
        let name_index: usize = get_pos(&value.headers, "name")?;
        let node_th_index: usize = get_pos(&value.headers, "hash")?;
        let span_index = get_pos(&value.headers, "span")?;

        let map_err = |e: DbError| {
            ploke_error::Error::Internal(ploke_error::InternalError::CompilerError(e.to_string()))
        };

        let common_fields = value
            .rows
            .into_iter()
            .map(|row| {
                let id = to_uuid(&row[id_index]).map_err(map_err)?;
                let name = to_string(&row[name_index]).map_err(map_err)?;
                let hash = TrackingHash(to_uuid(&row[node_th_index]).map_err(map_err)?);
                let span = &row[span_index].get_slice().unwrap();

                let (start_byte, end_byte) = get_byte_offsets(span);

                Ok(Self {
                    id,
                    name,
                    hash,
                    span: [start_byte as i64, end_byte as i64],
                })
            })
            .collect::<Result<Vec<_>, ploke_error::Error>>()?;
        Ok(common_fields)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodePaths {
    pub file: String,
    pub canon: String,
}

impl TryFrom<cozo::NamedRows> for NodePaths {
    type Error = crate::DbError;

    fn try_from(value: NamedRows) -> Result<Self, Self::Error> {
        let mut row = match value.rows.first() {
            Some(r) => r,
            None => {
                return Err(DbError::QueryExecution(
                    "No results found for id".to_string(),
                ))
            }
        }
        .iter();
        let name = match row.next().and_then(|n| n.get_str()) {
            Some(n) => n.to_string(),
            None => {
                return Err(DbError::QueryExecution(
                    "No name returned for id".to_string(),
                ))
            }
        };
        let mut canon = match row.next().and_then(|c| c.get_slice()) {
            Some(c) => c.iter().filter_map(|p| p.get_str()).join("::"),
            None => {
                return Err(DbError::QueryExecution(
                    "No canon_path returned for id".to_string(),
                ))
            }
        };
        let file = match row.next().and_then(|f| f.get_str()) {
            Some(f) => f.to_string(),
            None => {
                return Err(DbError::QueryExecution(
                    "No file_path returned for id".to_string(),
                ))
            }
        };
        canon.push_str("::");
        canon.push_str(&name);
        Ok(NodePaths { file, canon })
        //[1].get_slice().expect("Error returning node path")
    }
}

pub trait GetNodeInfo: ImmutQuery {
    /// Gets the file and cannonical paths for the target node id (multi-embedding schema version).
    /// Uses `@ 'NOW'` time-travel annotations consistent with the multi-embedding schema.
    fn paths_from_id(&self, to_find_node_id: Uuid) -> Result<NamedRows, DbError> {
        let common_fields_embedded: &str = COMMON_FIELDS_EMBEDDED.as_ref();
        let query: String = format!(
            r#"
        {common_fields_embedded}

        parent_of[child, parent] := *syntax_edge{{source_id: parent, target_id: child, relation_kind: "Contains" @ 'NOW'}}

        ancestor[desc, asc] := parent_of[desc, asc]
        ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]
        is_file_module[id, file_path] := *module{{id @ 'NOW'}}, *file_mod {{ owner_id: id, file_path @ 'NOW'}}

        containing_file[file_path, target_id] := ancestor[target_id, containing_id],
            is_file_module[containing_id, file_path]

        new_data[name, canon_path, file_path] := 
            *module{{ id: module_node_id, path: canon_path @ 'NOW' }},
            parent_of[node_id, module_node_id],
            has_embedding[node_id, name, hash, span],
            to_find_node_id = to_uuid("{to_find_node_id}"),
            to_find_node_id == node_id,
            containing_file[file_path, to_find_node_id]

        ?[name, canon_path, file_path] := new_data[name, canon_path, file_path]
        "#
        );
        self.raw_query(&query)
    }
}

impl GetNodeInfo for Database {}
