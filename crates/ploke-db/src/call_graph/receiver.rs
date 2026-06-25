use cozo::DataValue;
use serde::{Deserialize, Serialize};

use crate::{
    DbError,
    database::{to_string, to_string_list},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CallReceiver {
    SelfValue,
    SelfField {
        path: Vec<String>,
    },
    LocalBinding {
        name: String,
    },
    TypedLocalBinding {
        name: String,
        type_path: Vec<String>,
    },
    InitializedLocalBinding {
        name: String,
        init_path: Vec<String>,
    },
    BorrowedLocalBinding {
        name: String,
    },
    BorrowedTypedLocalBinding {
        name: String,
        type_path: Vec<String>,
    },
    DereferencedLocalBinding {
        name: String,
    },
    DereferencedInitializedLocalBinding {
        name: String,
        init_path: Vec<String>,
    },
    FieldLocalBinding {
        name: String,
        field_path: Vec<String>,
    },
    FieldTypedLocalBinding {
        name: String,
        type_path: Vec<String>,
        field_path: Vec<String>,
    },
    FieldInitializedLocalBinding {
        name: String,
        init_path: Vec<String>,
        field_path: Vec<String>,
    },
    PathCallResult {
        path: Vec<String>,
    },
    MethodCallResult {
        method_name: String,
    },
    AwaitResult,
    AwaitPathCallResult {
        path: Vec<String>,
    },
    TryResult,
    TryPathCallResult {
        path: Vec<String>,
    },
    Literal,
}

impl CallReceiver {
    pub(super) fn from_parts(kind: &DataValue, path: &DataValue) -> Result<Option<Self>, DbError> {
        if matches!(kind, DataValue::Null) {
            return match path {
                DataValue::Null => Ok(None),
                other => Err(DbError::Cozo(format!(
                    "method-call receiver path without receiver kind: {other:?}"
                ))),
            };
        }

        match to_string(kind)?.as_str() {
            "SelfValue" => Ok(Some(Self::SelfValue)),
            "SelfField" => Ok(Some(Self::SelfField {
                path: to_string_list(path)?,
            })),
            "LocalBinding" => {
                let path = to_string_list(path)?;
                match path.as_slice() {
                    [name] => Ok(Some(Self::LocalBinding { name: name.clone() })),
                    other => Err(DbError::Cozo(format!(
                        "local binding receiver should store exactly one name, got {other:?}"
                    ))),
                }
            }
            "TypedLocalBinding" => {
                let path = to_string_list(path)?;
                match path.as_slice() {
                    [name, type_path @ ..] if !type_path.is_empty() => {
                        Ok(Some(Self::TypedLocalBinding {
                            name: name.clone(),
                            type_path: type_path.to_vec(),
                        }))
                    }
                    other => Err(DbError::Cozo(format!(
                        "typed local binding receiver should store a name followed by a type path, got {other:?}"
                    ))),
                }
            }
            "InitializedLocalBinding" => {
                let path = to_string_list(path)?;
                match path.as_slice() {
                    [name, init_path @ ..] if !init_path.is_empty() => {
                        Ok(Some(Self::InitializedLocalBinding {
                            name: name.clone(),
                            init_path: init_path.to_vec(),
                        }))
                    }
                    other => Err(DbError::Cozo(format!(
                        "initialized local binding receiver should store a name followed by an initializer path, got {other:?}"
                    ))),
                }
            }
            "BorrowedLocalBinding" => {
                let path = to_string_list(path)?;
                match path.as_slice() {
                    [name] => Ok(Some(Self::BorrowedLocalBinding { name: name.clone() })),
                    other => Err(DbError::Cozo(format!(
                        "borrowed local binding receiver should store exactly one name, got {other:?}"
                    ))),
                }
            }
            "BorrowedTypedLocalBinding" => {
                let path = to_string_list(path)?;
                match path.as_slice() {
                    [name, type_path @ ..] if !type_path.is_empty() => {
                        Ok(Some(Self::BorrowedTypedLocalBinding {
                            name: name.clone(),
                            type_path: type_path.to_vec(),
                        }))
                    }
                    other => Err(DbError::Cozo(format!(
                        "borrowed typed local binding receiver should store a name followed by a type path, got {other:?}"
                    ))),
                }
            }
            "DereferencedLocalBinding" => {
                let path = to_string_list(path)?;
                match path.as_slice() {
                    [name] => Ok(Some(Self::DereferencedLocalBinding { name: name.clone() })),
                    other => Err(DbError::Cozo(format!(
                        "dereferenced local binding receiver should store exactly one name, got {other:?}"
                    ))),
                }
            }
            "DereferencedInitializedLocalBinding" => {
                let path = to_string_list(path)?;
                match path.as_slice() {
                    [name, init_path @ ..] if !init_path.is_empty() => {
                        Ok(Some(Self::DereferencedInitializedLocalBinding {
                            name: name.clone(),
                            init_path: init_path.to_vec(),
                        }))
                    }
                    other => Err(DbError::Cozo(format!(
                        "dereferenced initialized local binding receiver should store a name followed by an initializer path, got {other:?}"
                    ))),
                }
            }
            "FieldLocalBinding" => {
                let path = to_string_list(path)?;
                match path.as_slice() {
                    [name, field_path @ ..] if !field_path.is_empty() => {
                        Ok(Some(Self::FieldLocalBinding {
                            name: name.clone(),
                            field_path: field_path.to_vec(),
                        }))
                    }
                    other => Err(DbError::Cozo(format!(
                        "field local binding receiver should store a name followed by a field path, got {other:?}"
                    ))),
                }
            }
            "FieldTypedLocalBinding" => {
                let path = to_string_list(path)?;
                let (name, type_path, field_path) =
                    split_field_receiver_path(&path, "field typed local binding receiver")?;
                Ok(Some(Self::FieldTypedLocalBinding {
                    name,
                    type_path,
                    field_path,
                }))
            }
            "FieldInitializedLocalBinding" => {
                let path = to_string_list(path)?;
                let (name, init_path, field_path) =
                    split_field_receiver_path(&path, "field initialized local binding receiver")?;
                Ok(Some(Self::FieldInitializedLocalBinding {
                    name,
                    init_path,
                    field_path,
                }))
            }
            "PathCallResult" => {
                let path = to_string_list(path)?;
                if path.is_empty() {
                    Err(DbError::Cozo(
                        "path-call result receiver should store a non-empty path".to_string(),
                    ))
                } else {
                    Ok(Some(Self::PathCallResult { path }))
                }
            }
            "MethodCallResult" => {
                let path = to_string_list(path)?;
                match path.as_slice() {
                    [method_name] => Ok(Some(Self::MethodCallResult {
                        method_name: method_name.clone(),
                    })),
                    other => Err(DbError::Cozo(format!(
                        "method-call result receiver should store exactly one method name, got {other:?}"
                    ))),
                }
            }
            "AwaitResult" => match path {
                DataValue::Null => Ok(Some(Self::AwaitResult)),
                other => Err(DbError::Cozo(format!(
                    "await result receiver should not store a path, got {other:?}"
                ))),
            },
            "AwaitPathCallResult" => {
                let path = to_string_list(path)?;
                if path.is_empty() {
                    Err(DbError::Cozo(
                        "await path-call result receiver should store a non-empty path".to_string(),
                    ))
                } else {
                    Ok(Some(Self::AwaitPathCallResult { path }))
                }
            }
            "TryResult" => match path {
                DataValue::Null => Ok(Some(Self::TryResult)),
                other => Err(DbError::Cozo(format!(
                    "try result receiver should not store a path, got {other:?}"
                ))),
            },
            "TryPathCallResult" => {
                let path = to_string_list(path)?;
                if path.is_empty() {
                    Err(DbError::Cozo(
                        "try path-call result receiver should store a non-empty path".to_string(),
                    ))
                } else {
                    Ok(Some(Self::TryPathCallResult { path }))
                }
            }
            "Literal" => match path {
                DataValue::Null => Ok(Some(Self::Literal)),
                other => Err(DbError::Cozo(format!(
                    "literal receiver should not store a path, got {other:?}"
                ))),
            },
            other => Err(DbError::Cozo(format!(
                "unknown method-call receiver kind {other:?}"
            ))),
        }
    }
}

fn split_field_receiver_path(
    path: &[String],
    label: &str,
) -> Result<(String, Vec<String>, Vec<String>), DbError> {
    let Some((name, rest)) = path.split_first() else {
        return Err(DbError::Cozo(format!(
            "{label} should store a name, root path, separator, and field path, got {path:?}"
        )));
    };
    let Some(separator_idx) = rest.iter().position(String::is_empty) else {
        return Err(DbError::Cozo(format!(
            "{label} should include an empty separator between root path and field path, got {path:?}"
        )));
    };
    let root_path = rest[..separator_idx].to_vec();
    let field_path = rest[separator_idx + 1..].to_vec();
    if root_path.is_empty() || field_path.is_empty() {
        return Err(DbError::Cozo(format!(
            "{label} should store non-empty root and field paths, got {path:?}"
        )));
    }

    Ok((name.clone(), root_path, field_path))
}
