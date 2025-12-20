//! Query result handling and formatting

mod formatter;
mod snippet;

use std::path::PathBuf;

pub use formatter::ResultFormatter;
use ploke_core::{
    rag_types::CanonPath, EmbeddingData, FileData, TrackingHash,
};
pub use snippet::CodeSnippet;
use uuid::Uuid;
pub mod typed_rows;

use crate::{error::DbError, result::typed_rows::ResolvedEdgeData};
use cozo::{DataValue, NamedRows};

/// Result of a database query
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub rows: Vec<Vec<DataValue>>,
    pub headers: Vec<String>,
}

// // TODO: Make these Typed Ids, and put the typed id definitions into ploke-core
// #[derive(Debug, Clone)]
// pub struct FileData {
//     pub id: Uuid,
//     pub namespace: Uuid,
//     pub file_tracking_hash: TrackingHash,
//     pub file_path: PathBuf,
// }

impl QueryResult {
    /// Convert query results into code snippets
    pub fn into_snippets(self) -> Result<Vec<CodeSnippet>, DbError> {
        self.rows
            .iter()
            .map(|row| CodeSnippet::from_db_row(row))
            .collect()
    }

    pub fn row_refs(&self) -> impl Iterator<Item = Row<'_>> {
        self.rows
            .iter()
            .map(|row| Row::new(self.headers.as_slice(), row.as_slice()))
    }

    pub fn try_into_file_data(self) -> Result<Vec<FileData>, ploke_error::Error> {
        let map_err = |e: DbError| {
            ploke_error::Error::Internal(ploke_error::InternalError::CompilerError(e.to_string()))
        };

        let file_data = self
            .row_refs()
            .map(|row| {
                let id = row.get::<Uuid>("id").map_err(map_err)?;
                let file_path_str = row.get::<String>("file_path").map_err(map_err)?;
                let file_tracking_hash =
                    TrackingHash(row.get::<Uuid>("tracking_hash").map_err(map_err)?);
                let namespace = row.get::<Uuid>("namespace").map_err(map_err)?;

                Ok(FileData {
                    id,
                    file_path: PathBuf::from(file_path_str),
                    file_tracking_hash,
                    namespace,
                })
            })
            .collect::<Result<Vec<_>, ploke_error::Error>>()?;

        Ok(file_data)
    }

    // TODO: Delete namespace and file_path, maybe also file_th
    pub fn to_embedding_nodes(self) -> Result<Vec<EmbeddingData>, ploke_error::Error> {
        let map_err = |e: DbError| {
            ploke_error::Error::Internal(ploke_error::InternalError::CompilerError(e.to_string()))
        };

        let span_index = get_pos(&self.headers, "span")?;

        let embeddings = self
            .row_refs()
            .map(|row| {
                let id = row.get::<Uuid>("id").map_err(map_err)?;
                let name = row.get::<String>("name").map_err(map_err)?;
                let file_path_str = row.get::<String>("file_path").map_err(map_err)?;
                let node_tracking_hash = TrackingHash(row.get::<Uuid>("hash").map_err(map_err)?);
                let file_tracking_hash =
                    TrackingHash(row.get::<Uuid>("file_hash").map_err(map_err)?);
                let namespace = row.get::<Uuid>("namespace").map_err(map_err)?;
                let span_value = row.data_value(span_index).map_err(map_err)?;
                let span_slice = span_value
                    .get_slice()
                    .ok_or_else(|| {
                        DbError::Cozo(format!("Expected span to be a list, found {span_value:?}"))
                    })
                    .map_err(map_err)?;

                let (start_byte, end_byte) = get_byte_offsets(&span_slice);

                Ok(EmbeddingData {
                    id,
                    name,
                    file_path: PathBuf::from(file_path_str),
                    start_byte,
                    end_byte,
                    node_tracking_hash,
                    file_tracking_hash,
                    namespace,
                })
            })
            .collect::<Result<Vec<_>, ploke_error::Error>>()?;

        Ok(embeddings)
    }

    pub fn to_resolved_edges(self) -> Result<Vec<ResolvedEdgeData>, ploke_error::Error> {
        let map_err = |e: DbError| {
            ploke_error::Error::Internal(ploke_error::InternalError::CompilerError(e.to_string()))
        };

        let canon_path_index: usize = get_pos(&self.headers, "canon_path")?;

        let canon_path_to_string = |val: &DataValue| -> Result<String, DbError> {
            match val {
                DataValue::Str(s) => Ok(s.to_string()),
                DataValue::List(parts) => {
                    let mut buf = Vec::with_capacity(parts.len());
                    for part in parts {
                        if let Some(s) = part.get_str() {
                            buf.push(s.to_string());
                        } else {
                            return Err(DbError::Cozo(format!(
                                "Expected canon_path list of strings, found {part:?}"
                            )));
                        }
                    }
                    Ok(buf.join("::"))
                }
                other => Err(DbError::Cozo(format!(
                    "Expected canon_path as string or list, found {other:?}"
                ))),
            }
        };

        let embeddings = self
            .row_refs()
            .map(|row| {
                let source_id = row.get::<Uuid>("source_id").map_err(map_err)?;
                let target_id = row.get::<Uuid>("target_id").map_err(map_err)?;
                let target_name = row.get::<String>("target_name").map_err(map_err)?;
                let source_name = row.get::<String>("source_name").map_err(map_err)?;
                let canon_path =
                    canon_path_to_string(row.data_value(canon_path_index).map_err(map_err)?)
                        .map_err(map_err)?;
                let relation_kind = row.get::<String>("relation_kind").map_err(map_err)?;
                let file_path_str = row.get::<String>("file_path").map_err(map_err)?;

                Ok(ResolvedEdgeData {
                    file_path: PathBuf::from(file_path_str),
                    source_id,
                    source_name,
                    target_id,
                    target_name,
                    canon_path: CanonPath::new(canon_path),
                    relation_kind,
                })
            })
            .collect::<Result<Vec<_>, ploke_error::Error>>()?;

        Ok(embeddings)
    }

    // pub fn to_embedding_vector(self, embedding_set: &EmbeddingSet) -> Result<Vec<EmbeddingVector>, ploke_error::Error> {
    //     let map_err = |e: DbError| {
    //         ploke_error::Error::Internal(ploke_error::InternalError::CompilerError(e.to_string()))
    //     };
    //
    //     let node_id_index: usize = get_pos(&self.headers, "node_id")?;
    //     let embedding_set_id_index: usize = get_pos(&self.headers, "embedding_set_id")?;
    //     let vector_index: usize = get_pos(&self.headers, "vector")?;
    //
    //     let embeddings = self
    //         .rows
    //         .into_iter()
    //         .map(
    //             |row| {
    //             let node_id = to_uuid(&row[node_id_index]).map_err(map_err)?;
    //             let embedding_set_id = to_u64(&row[embedding_set_id_index]).map_err(map_err)?;
    //             let vector = to_vector(&row[vector_index], embedding_set).map_err(map_err)?;
    //
    //             Ok(EmbeddingVector {
    //                 node_id,
    //                 vector,
    //                 embedding_set_id: EmbeddingSetId::from_db_raw(embedding_set_id),
    //             })
    //         })
    //         .collect::<Result<Vec<_>, ploke_error::Error>>()?;
    //
    //     Ok(embeddings)
    // }

    pub fn iter_col<'a>(&'a self, col_title: &str) -> Option<impl Iterator<Item = &'a DataValue>> {
        use std::ops::Index;
        let col_idx = self
            .headers
            .iter()
            .enumerate()
            .find(|(idx, col)| col.as_str() == col_title)
            .map(|(idx, col)| idx)?;
        Some(self.rows.iter().map(move |r| r.index(col_idx)))
    }

    /// Converts the headers and row to debug string format.
    ///
    /// All the headers are in debug format, then each row on a new line following the debug
    /// header.
    pub fn debug_string_all(&self) -> String {
        let header = &self.headers;
        let mut s = format!("{:?}", header);

        for row in &self.rows {
            s.push('\n');
            let row_debug_str = format!("{row:?}");
            s.push_str(&row_debug_str);
        }
        s
    }
}

pub(crate) fn get_byte_offsets(span: &&[DataValue]) -> (usize, usize) {
    let error_msg = "Invariant Violated: All Nodes must have a start/end byte";
    let start_byte = span.first().expect(error_msg).get_int().expect(error_msg) as usize;
    let end_byte = span.last().expect(error_msg).get_int().expect(error_msg) as usize;
    (start_byte, end_byte)
}
pub(crate) fn get_pos(v: &[String], field: &str) -> Result<usize, DbError> {
    v.iter()
        .position(|s| s == field)
        .ok_or_else(|| DbError::Cozo(format!("Could not locate field {} in NamedRows", field)))
}

impl From<NamedRows> for QueryResult {
    fn from(named_rows: NamedRows) -> Self {
        Self {
            rows: named_rows.rows,
            headers: named_rows.headers,
        }
    }
}

impl TryFrom<QueryResult> for Vec<ResolvedEdgeData> {
    type Error = DbError;

    fn try_from(value: QueryResult) -> Result<Self, Self::Error> {
        let canon_path_index = get_pos(&value.headers, "canon_path")?;

        value
            .rows
            .iter()
            .map(|raw_row| {
                let row = Row::new(&value.headers, raw_row.as_slice());

                let canon_path = match row.data_value(canon_path_index)? {
                    DataValue::Str(s) => CanonPath::new(s.to_string()),
                    DataValue::List(parts) => {
                        let mut buf = Vec::with_capacity(parts.len());
                        for part in parts {
                            if let Some(s) = part.get_str() {
                                buf.push(s.to_string());
                            } else {
                                return Err(DbError::Cozo(format!(
                                    "Expected canon_path list of strings, found {part:?}"
                                )));
                            }
                        }
                        CanonPath::new(buf.join("::"))
                    }
                    other => {
                        return Err(DbError::Cozo(format!(
                            "Expected canon_path as string or list, found {other:?}"
                        )))
                    }
                };

                Ok(ResolvedEdgeData {
                    file_path: PathBuf::from(row.get::<String>("file_path")?),
                    source_id: row.get::<Uuid>("source_id")?,
                    source_name: row.get::<String>("source_name")?,
                    target_id: row.get::<Uuid>("target_id")?,
                    target_name: row.get::<String>("target_name")?,
                    canon_path,
                    relation_kind: row.get::<String>("relation_kind")?,
                })
            })
            .collect()
    }
}

impl TryFrom<QueryResult> for ResolvedEdgeData {
    type Error = DbError;

    fn try_from(value: QueryResult) -> Result<Self, Self::Error> {
        if value.rows.len() != 1 {
            return Err(DbError::Cozo(format!(
                "Expected exactly one row for edge, found {}",
                value.rows.len()
            )));
        }

        let canon_path_index = get_pos(&value.headers, "canon_path")?;
        let row = Row::new(
            &value.headers,
            value.rows.first().expect("checked len; row exists"),
        );

        let canon_path = match row.data_value(canon_path_index)? {
            DataValue::Str(s) => CanonPath::new(s.to_string()),
            DataValue::List(parts) => {
                let mut buf = Vec::with_capacity(parts.len());
                for part in parts {
                    if let Some(s) = part.get_str() {
                        buf.push(s.to_string());
                    } else {
                        return Err(DbError::Cozo(format!(
                            "Expected canon_path list of strings, found {part:?}"
                        )));
                    }
                }
                CanonPath::new(buf.join("::"))
            }
            other => {
                return Err(DbError::Cozo(format!(
                    "Expected canon_path as string or list, found {other:?}"
                )))
            }
        };

        Ok(ResolvedEdgeData {
            file_path: PathBuf::from(row.get::<String>("file_path")?),
            source_id: row.get::<Uuid>("source_id")?,
            source_name: row.get::<String>("source_name")?,
            target_id: row.get::<Uuid>("target_id")?,
            target_name: row.get::<String>("target_name")?,
            canon_path,
            relation_kind: row.get::<String>("relation_kind")?,
        })
    }
}

pub trait CozoDecode: Sized {
    type Error;

    fn try_decode(value: DataValue) -> Result<Self, Self::Error>;
}

pub trait CozoBorrow<'a> {
    type Error;

    fn try_borrow(value: &'a DataValue) -> Result<&'a Self, Self::Error>;
}

pub trait CozoEncode {
    fn encode(self) -> DataValue;
}

#[derive(Clone, Copy, Debug)]
pub struct Row<'a> {
    headers: &'a [String],
    row: &'a [DataValue],
}

impl<'a> Row<'a> {
    pub fn new(headers: &'a [String], row: &'a [DataValue]) -> Self {
        Self { headers, row }
    }

    pub fn get<T: CozoDecode<Error = DbError>>(&self, column: &str) -> Result<T, DbError> {
        let idx = get_pos(self.headers, column)?;
        self.get_idx(idx)
    }

    pub fn get_idx<T: CozoDecode<Error = DbError>>(&self, idx: usize) -> Result<T, DbError> {
        let value = self
            .row
            .get(idx)
            .ok_or_else(|| DbError::Cozo(format!("Missing column at position {idx}")))?;
        T::try_decode(value.clone())
    }

    pub fn data_value(&self, idx: usize) -> Result<&DataValue, DbError> {
        self.row
            .get(idx)
            .ok_or_else(|| DbError::Cozo(format!("Missing column at position {idx}")))
    }

    pub fn borrow<'b, T>(&'b self, column: &str) -> Result<&'b T, DbError>
    where
        T: CozoBorrow<'b, Error = DbError> + ?Sized,
    {
        let idx = get_pos(self.headers, column)?;
        self.borrow_idx(idx)
    }

    pub fn borrow_idx<'b, T>(&'b self, idx: usize) -> Result<&'b T, DbError>
    where
        T: CozoBorrow<'b, Error = DbError> + ?Sized,
    {
        let value = self
            .row
            .get(idx)
            .ok_or_else(|| DbError::Cozo(format!("Missing column at position {idx}")))?;
        T::try_borrow(value)
    }
}

fn type_mismatch(expected: &str, found: &DataValue) -> DbError {
    DbError::Cozo(format!("Expected {expected}, found {found:?}"))
}

impl CozoDecode for bool {
    type Error = DbError;

    fn try_decode(dv: DataValue) -> Result<Self, Self::Error> {
        match dv {
            DataValue::Bool(b) => Ok(b),
            other => Err(type_mismatch("Bool", &other)),
        }
    }
}

impl CozoBorrow<'_> for bool {
    type Error = DbError;

    fn try_borrow(value: &DataValue) -> Result<&Self, Self::Error> {
        match value {
            DataValue::Bool(b) => Ok(b),
            other => Err(type_mismatch("Bool", other)),
        }
    }
}

impl CozoDecode for f32 {
    type Error = DbError;

    fn try_decode(dv: DataValue) -> Result<Self, Self::Error> {
        match dv {
            DataValue::Num(cozo::Num::Float(f)) => Ok(f as f32),
            DataValue::Num(cozo::Num::Int(i)) => Ok(i as f32),
            other => Err(type_mismatch("Float", &other)),
        }
    }
}

impl CozoDecode for f64 {
    type Error = DbError;

    fn try_decode(dv: DataValue) -> Result<Self, Self::Error> {
        match dv {
            DataValue::Num(cozo::Num::Float(f)) => Ok(f),
            DataValue::Num(cozo::Num::Int(i)) => Ok(i as f64),
            other => Err(type_mismatch("Float", &other)),
        }
    }
}

impl CozoDecode for i64 {
    type Error = DbError;

    fn try_decode(dv: DataValue) -> Result<Self, Self::Error> {
        match dv {
            DataValue::Num(cozo::Num::Int(i)) => Ok(i),
            other => Err(type_mismatch("Integer", &other)),
        }
    }
}

impl CozoBorrow<'_> for i64 {
    type Error = DbError;

    fn try_borrow(value: &DataValue) -> Result<&Self, Self::Error> {
        match value {
            DataValue::Num(cozo::Num::Int(i)) => Ok(i),
            other => Err(type_mismatch("Integer", other)),
        }
    }
}

impl CozoDecode for u64 {
    type Error = DbError;

    fn try_decode(dv: DataValue) -> Result<Self, Self::Error> {
        match dv {
            DataValue::Num(cozo::Num::Int(i)) => u64::try_from(i)
                .map_err(|e| DbError::Cozo(format!("Expected unsigned integer, found {i}: {e}"))),
            other => Err(type_mismatch("Unsigned integer", &other)),
        }
    }
}

impl CozoDecode for usize {
    type Error = DbError;

    fn try_decode(dv: DataValue) -> Result<Self, Self::Error> {
        match dv {
            DataValue::Num(cozo::Num::Int(i)) => usize::try_from(i).map_err(|e| {
                DbError::Cozo(format!("Expected usize-compatible integer, found {i}: {e}"))
            }),
            other => Err(type_mismatch("Integer", &other)),
        }
    }
}

impl CozoDecode for String {
    type Error = DbError;

    fn try_decode(dv: DataValue) -> Result<Self, Self::Error> {
        match dv {
            DataValue::Str(s) => Ok(s.to_string()),
            other => Err(type_mismatch("String", &other)),
        }
    }
}

impl<'a> CozoBorrow<'a> for str {
    type Error = DbError;

    fn try_borrow(value: &'a DataValue) -> Result<&'a Self, Self::Error> {
        match value {
            DataValue::Str(s) => Ok(s.as_str()),
            other => Err(type_mismatch("String", other)),
        }
    }
}

impl CozoDecode for Vec<u8> {
    type Error = DbError;

    fn try_decode(dv: DataValue) -> Result<Self, Self::Error> {
        match dv {
            DataValue::Bytes(bytes) => Ok(bytes),
            other => Err(type_mismatch("Bytes", &other)),
        }
    }
}

impl<'a> CozoBorrow<'a> for [u8] {
    type Error = DbError;

    fn try_borrow(value: &'a DataValue) -> Result<&'a Self, Self::Error> {
        match value {
            DataValue::Bytes(bytes) => Ok(bytes.as_slice()),
            other => Err(type_mismatch("Bytes", other)),
        }
    }
}

impl CozoDecode for Uuid {
    type Error = DbError;

    fn try_decode(dv: DataValue) -> Result<Self, Self::Error> {
        match dv {
            DataValue::Uuid(uuid_wrapper) => Ok(uuid_wrapper.0),
            other => Err(type_mismatch("Uuid", &other)),
        }
    }
}

impl CozoDecode for CanonPath {
    type Error = DbError;

    fn try_decode(dv: DataValue) -> Result<Self, Self::Error> {
        match dv {
            DataValue::Str(s) => Ok(CanonPath::new(s.to_string())),
            DataValue::List(parts) => {
                let mut buf = Vec::with_capacity(parts.len());
                for part in parts {
                    if let Some(s) = part.get_str() {
                        buf.push(s.to_string());
                    } else {
                        return Err(DbError::Cozo(format!(
                            "Expected canon_path list of strings, found {part:?}"
                        )));
                    }
                }
                Ok(CanonPath::new(buf.join("::")))
            }
            other => Err(type_mismatch("canon_path string or list", &other)),
        }
    }
}

impl<T> CozoDecode for Option<T>
where
    T: CozoDecode<Error = DbError>,
{
    type Error = DbError;

    fn try_decode(dv: DataValue) -> Result<Self, Self::Error> {
        match dv {
            DataValue::Null => Ok(None),
            other => T::try_decode(other).map(Some),
        }
    }
}

impl<T> CozoDecode for Vec<T>
where
    T: CozoDecode<Error = DbError>,
{
    type Error = DbError;

    fn try_decode(dv: DataValue) -> Result<Self, Self::Error> {
        match dv {
            DataValue::List(values) => values
                .into_iter()
                .map(T::try_decode)
                .collect::<Result<Vec<_>, _>>(),
            other => Err(type_mismatch("List", &other)),
        }
    }
}

impl CozoEncode for bool {
    fn encode(self) -> DataValue {
        DataValue::Bool(self)
    }
}

impl CozoEncode for i64 {
    fn encode(self) -> DataValue {
        DataValue::Num(cozo::Num::Int(self))
    }
}

impl CozoEncode for usize {
    fn encode(self) -> DataValue {
        DataValue::Num(cozo::Num::Int(self as i64))
    }
}

impl CozoEncode for f64 {
    fn encode(self) -> DataValue {
        DataValue::Num(cozo::Num::Float(self))
    }
}

impl CozoEncode for f32 {
    fn encode(self) -> DataValue {
        DataValue::Num(cozo::Num::Float(self as f64))
    }
}

impl CozoEncode for String {
    fn encode(self) -> DataValue {
        DataValue::Str(self.into())
    }
}

impl<'a> CozoEncode for &'a str {
    fn encode(self) -> DataValue {
        DataValue::Str(self.into())
    }
}

impl CozoEncode for Vec<u8> {
    fn encode(self) -> DataValue {
        DataValue::Bytes(self)
    }
}

impl<'a> CozoEncode for &'a [u8] {
    fn encode(self) -> DataValue {
        DataValue::Bytes(self.to_vec())
    }
}

impl CozoEncode for Uuid {
    fn encode(self) -> DataValue {
        DataValue::Uuid(cozo::UuidWrapper(self))
    }
}

impl<T> CozoEncode for Option<T>
where
    T: CozoEncode,
{
    fn encode(self) -> DataValue {
        match self {
            Some(v) => v.encode(),
            None => DataValue::Null,
        }
    }
}

impl<T> CozoEncode for Vec<T>
where
    T: CozoEncode,
{
    fn encode(self) -> DataValue {
        DataValue::List(self.into_iter().map(CozoEncode::encode).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{CozoBorrow, CozoDecode, QueryResult};
    use ploke_core::rag_types::CanonPath;
    use ploke_db_derive::CozoRow;
    use cozo::{DataValue, Num, UuidWrapper};
    use uuid::Uuid;

    use crate::DbError;

    #[test]
    fn cozo_uuid_to_uuid() -> Result<(), DbError> {
        let cozo_id = DataValue::Uuid(UuidWrapper(Uuid::nil()));
        assert_eq!(Uuid::nil(), <Uuid as CozoDecode>::try_decode(cozo_id)?);
        Ok(())
    }

    #[test]
    fn option_handles_null() -> Result<(), DbError> {
        let decoded: Option<String> = <Option<String> as CozoDecode>::try_decode(DataValue::Null)?;
        assert_eq!(None, decoded);
        Ok(())
    }

    #[test]
    fn vec_of_ints_decodes() -> Result<(), DbError> {
        let dv = DataValue::List(vec![
            DataValue::Num(Num::Int(1)),
            DataValue::Num(Num::Int(2)),
        ]);
        let decoded: Vec<i64> = <Vec<i64> as CozoDecode>::try_decode(dv)?;
        assert_eq!(vec![1_i64, 2_i64], decoded);
        Ok(())
    }

    #[test]
    fn row_accessors_work() -> Result<(), DbError> {
        let headers = vec!["id".into(), "name".into()];
        let rows = vec![vec![
            DataValue::Uuid(UuidWrapper(Uuid::nil())),
            DataValue::Str("hello".into()),
        ]];
        let qr = QueryResult { rows, headers };
        let row = qr.row_refs().next().expect("row present");
        assert_eq!(Uuid::nil(), row.get::<Uuid>("id")?);
        assert_eq!("hello", row.borrow::<str>("name")?);
        Ok(())
    }

    #[test]
    fn row_missing_column_errors() {
        let headers = vec!["id".into()];
        let rows = vec![vec![DataValue::Null]];
        let qr = QueryResult { rows, headers };
        let row = qr.row_refs().next().expect("row present");
        assert!(row.get::<Uuid>("missing").is_err());
    }

    #[test]
    fn row_handles_all_datavalue_variants() -> Result<(), DbError> {
        use std::cmp::Reverse;
        use std::collections::BTreeSet;
        use std::convert::TryInto;

        use serde_json::json;

        let headers = (0..15).map(|i| format!("c{i}")).collect::<Vec<_>>();

        let mut set = BTreeSet::new();
        set.insert(DataValue::Num(Num::Int(42)));

        let row = vec![
            DataValue::Null,
            DataValue::Bool(true),
            DataValue::Num(Num::Int(1)),
            DataValue::Num(Num::Float(1.5)),
            DataValue::Str("hello".into()),
            DataValue::Bytes(vec![1, 2, 3]),
            DataValue::Uuid(UuidWrapper(Uuid::nil())),
            DataValue::Regex(cozo::RegexWrapper("foo".try_into().unwrap())),
            DataValue::List(vec![DataValue::Bool(false)]),
            DataValue::Set(set),
            DataValue::Vec(cozo::Vector::F32(Default::default())),
            DataValue::Vec(cozo::Vector::F64(Default::default())),
            DataValue::Json(cozo::JsonData(json!({"a": 1}))),
            DataValue::Validity(cozo::Validity {
                timestamp: cozo::ValidityTs(Reverse(0)),
                is_assert: Reverse(true),
            }),
            DataValue::Bot,
        ];

        let qr = QueryResult {
            headers,
            rows: vec![row],
        };

        let row_len = qr.rows[0].len();
        let row = qr.row_refs().next().expect("row present");
        for idx in 0..row_len {
            row.data_value(idx)?;
        }

        // Ensure debug formatting works with all variants present.
        let _ = qr.debug_string_all();
        Ok(())
    }

    #[derive(Debug, CozoRow, PartialEq)]
    struct DerivedEdge {
        #[cozo(col = "source_id")]
        source: Uuid,
        #[cozo(col = "target_id")]
        target: Uuid,
        #[cozo(col = "source_name")]
        source_name: String,
        #[cozo(col = "target_name")]
        target_name: String,
        relation_kind: String,
        file_path: String,
        canon_path: CanonPath,
    }

    #[test]
    fn cozo_row_derive_handles_vec_and_single() -> Result<(), DbError> {
        let headers = vec![
            "source_id".into(),
            "target_id".into(),
            "source_name".into(),
            "target_name".into(),
            "relation_kind".into(),
            "file_path".into(),
            "canon_path".into(),
        ];

        let row1 = vec![
            DataValue::Uuid(UuidWrapper(Uuid::nil())),
            DataValue::Uuid(UuidWrapper(Uuid::nil())),
            DataValue::Str("s".into()),
            DataValue::Str("t".into()),
            DataValue::Str("rel".into()),
            DataValue::Str("/tmp/file".into()),
            DataValue::List(vec![DataValue::Str("a".into()), DataValue::Str("b".into())]),
        ];

        let row2 = vec![
            DataValue::Uuid(UuidWrapper(Uuid::nil())),
            DataValue::Uuid(UuidWrapper(Uuid::nil())),
            DataValue::Str("s2".into()),
            DataValue::Str("t2".into()),
            DataValue::Str("rel2".into()),
            DataValue::Str("/tmp/file2".into()),
            DataValue::Str("a::b".into()),
        ];

        let qr = QueryResult {
            headers: headers.clone(),
            rows: vec![row1.clone(), row2.clone()],
        };

        let v = Vec::<DerivedEdge>::try_from(qr.clone())?;
        assert_eq!(2, v.len());
        assert_eq!(CanonPath::new("a::b".to_string()), v[0].canon_path);
        assert_eq!("s2", v[1].source_name);

        let single = DerivedEdge::try_from(QueryResult {
            headers,
            rows: vec![row2],
        })?;
        assert_eq!("rel2", single.relation_kind);
        Ok(())
    }
}
