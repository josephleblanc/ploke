use ploke_error::Error as PlokeError;

use crate::{Database, DbError, NodeType, QueryResult};

/// An extension trait for [`Database`] that provides convenience methods
/// for common embedding-related queries.
pub trait DbEmbedUtils {
    /// Gets rows from a specific node relation where the embedding is null,
    /// filtered by the node's `name`.
    ///
    /// This is a helper to quickly check the embedding status of a specific item
    /// without writing a full CozoScript query.
    fn get_null_embedding_rows(
        &self,
        name: &str,
        node_ty: NodeType,
    ) -> Result<QueryResult, PlokeError>;
}

impl DbEmbedUtils for Database {
    fn get_null_embedding_rows(
        &self,
        name: &str,
        node_ty: NodeType,
    ) -> std::result::Result<QueryResult, ploke_error::Error> {
        let script = build_null_embed_script(name, node_ty);

        let rows = self.raw_query(&script).map_err(PlokeError::from)?;
        Ok(rows)
    }
}

/// Constructs the CozoScript query to check for a null embedding on a named item.
fn build_null_embed_script(name: &str, node_ty: NodeType) -> String {
    let ty = node_ty.relation_str();
    format!(
        "?[name, is_null_embedding] :=
        *{ty}{{name, embedding @ 'NOW' }},
        name = \"{name}\",
        is_null_embedding = is_null(embedding)
        "
    )
}

/// An extension trait for [`QueryResult`] that provides helper methods for
/// interpreting results from embedding-related queries.
pub trait HasNullEmbeds {
    /// Checks if the query result indicates a null embedding.
    ///
    /// This is designed to work with the specific output of `get_null_embedding_rows`,
    /// which returns a boolean in the second column of the first row.
    ///
    /// # Returns
    ///
    /// `Ok(true)` if the embedding is null, `Ok(false)` if it is not.
    ///
    /// # Errors
    ///
    /// Returns a `ploke_error::Error` wrapping `DbError::NotFound` if the query
    /// result is empty or does not have the expected shape.
    fn is_null_embedding(&self) -> Result<bool, PlokeError>;
}

impl HasNullEmbeds for QueryResult {
    fn is_null_embedding(&self) -> Result<bool, PlokeError> {
        let is_null_embed = self
            .rows
            .first() // Use .first() to avoid panicking on empty rows
            .and_then(|row| row.get(1))
            .and_then(|v| v.get_bool())
            .ok_or(DbError::NotFound)?;
        Ok(is_null_embed)
    }
}

pub struct NewQueryBuilder {
    // e.g. some kind of join/select
    first_set: Vec<String>,
    // e.g. some kind of limit on # of returned items
    second_set: Vec<String>,
    // ..etc
}
