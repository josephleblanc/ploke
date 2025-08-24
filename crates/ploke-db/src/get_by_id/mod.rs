use itertools::Itertools;
use lazy_static::lazy_static;
use cozo::NamedRows;
use ploke_core::{rag_types::ContextPart, TrackingHash};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{database::{to_string, to_uuid, ImmutQuery}, result::{get_byte_offsets, get_pos}, Database, DbError, NodeType, QueryResult};

lazy_static! {
    pub static ref COMMON_FIELDS_EMBEDDED: String = NodeType::primary_nodes().iter().map(|ty| {
        let rel = ty.relation_str();
            format!(r#"
            has_embedding[id, name, hash, span] := *{rel}{{id, name, tracking_hash: hash, span, embedding @ 'NOW' }}, !is_null(embedding)
            "#)
        }).join("\n");

}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommonFields {
    id: Uuid,
    name: String,
    hash: TrackingHash,
    span: [i64; 2]
}


impl CommonFields {
    fn try_from_query_result(value: QueryResult) -> Result<Vec< Self >, ploke_error::Error> {
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
                let hash =
                    TrackingHash(to_uuid(&row[node_th_index]).map_err(map_err)?);
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
    file: String,
    canon: String,
}

impl TryFrom<cozo::NamedRows> for NodePaths {
    type Error = crate::DbError;

    fn try_from(value: NamedRows) -> Result<Self, Self::Error> {
        let mut row = match value.rows.first() {
            Some(r) => r,
            None => {return Err(DbError::QueryExecution("No results found for id".to_string()))}
        }.iter();
        let name = match row.next().and_then(|n| n.get_str()) {
            Some(n) => n.to_string(),
            None => return Err(DbError::QueryExecution("No name returned for id".to_string())),
        };
        let mut canon = match row.next().and_then(|c| c.get_slice()) {
            Some(c) => c.iter().filter_map(|p| p.get_str()).join("::"),
            None => return Err(DbError::QueryExecution("No canon_path returned for id".to_string())),
        };
        let file = match row.next().and_then(|f| f.get_str()) {
            Some(f) => f.to_string(),
            None => return Err(DbError::QueryExecution("No file_path returned for id".to_string())),
        };
        canon.push_str("::");
        canon.push_str(&name);
        Ok(NodePaths {
            file,
            canon
        })
            //[1].get_slice().expect("Error returning node path")
    }
}

pub trait GetNodeInfo: ImmutQuery {
    /// Gets the file and cannonical paths for the target node id.
    // TODO: Make a batch transaction version of this. Maybe we can abstract the rules `parent_of`,
    // `ancestor`, etc and then just run it all at once for a list of Uuids somehow. Need to figure
    // out better how cozo works.
    fn paths_from_id(&self, to_find_node_id: Uuid) -> Result<NamedRows, DbError> {
        let common_fields_embedded: &str = COMMON_FIELDS_EMBEDDED.as_ref();
        let query: String = format!(r#"
        {common_fields_embedded}

        parent_of[child, parent] := *syntax_edge{{source_id: parent, target_id: child, relation_kind: "Contains" @ 'NOW'}}

        ancestor[desc, asc] := parent_of[desc, asc]
        ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]
        is_file_module[id, file_path] := *module{{id}}, *file_mod {{ owner_id: id, file_path}}

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
        "#);
        // let error_query = query.clone().split_off(1653);
        // eprintln!("Error in query (starting 10 chars back) at:\n{error_query}\n");
        self.raw_query(&query)
    }
}

impl GetNodeInfo for Database {}

#[cfg(test)]
mod tests {
    use ploke_error::Error;

    use crate::{get_by_id::NodePaths, utils::test_utils::TEST_DB_NODES};

    use super::{CommonFields, GetNodeInfo};

    #[test]
    fn test_canon_path() -> Result<(), Error> {
        let db_arc = TEST_DB_NODES.clone().expect("problem loading fixture_nodes from cold start");
        let db = db_arc.lock().expect("problem getting lock on test db for fixture_nodes");
        let common_nodes = db.get_common_nodes()?;
        let common_field_nodes = CommonFields::try_from_query_result(common_nodes)?;
        let expect_headers = vec![
            String::from("name"), 
            String::from("canon_path"),
            String::from("file_path")
        ];
        for node in common_field_nodes {
            let node_name = node.name;
            eprintln!("Checking node with name: {node_name}");

            let db_res = db.paths_from_id(node.id)?;
            assert_eq!(expect_headers, db_res.headers);

            let node_res_name = db_res.rows[0][0].get_str().expect("Error returning node name");
            assert_eq!(node_name, node_res_name);
            let node_res_path = db_res.rows[0][1].get_slice().expect("Error returning node path");
            eprintln!("  Found canon_path: {node_res_path:?}");
            let node_res_file_path= db_res.rows[0][2].get_str().expect("Error returning file_path");
            eprintln!("  Found file_path: {node_res_file_path:?}");
            let node_paths: NodePaths = db_res.try_into()?;
                // .expect("Could not parse NamedRows into NodePaths");
            eprintln!("  Found file_path: {node_paths:#?}");
        }

        
        Ok(())
    }
}
