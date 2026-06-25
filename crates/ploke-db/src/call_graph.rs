use serde::{Deserialize, Serialize};
use uuid::Uuid;

use std::{collections::BTreeMap, fmt::Write as _};

use cozo::{DataValue, Num, ScriptMutability, UuidWrapper};

use crate::{
    Database, DbError,
    database::{to_string, to_string_list, to_uuid},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CallSiteKind {
    Path,
    Method,
    Dynamic,
    Macro,
}

impl CallSiteKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Path => "Path",
            Self::Method => "Method",
            Self::Dynamic => "Dynamic",
            Self::Macro => "Macro",
        }
    }

    fn from_str(value: &str) -> Result<Self, DbError> {
        match value {
            "Path" => Ok(Self::Path),
            "Method" => Ok(Self::Method),
            "Dynamic" => Ok(Self::Dynamic),
            "Macro" => Ok(Self::Macro),
            other => Err(DbError::Cozo(format!("unknown call-site kind {other:?}"))),
        }
    }
}

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
    fn from_parts(kind: &DataValue, path: &DataValue) -> Result<Option<Self>, DbError> {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CallRelationKind {
    Function,
    DynamicFunction,
    Method,
    AssociatedFunction,
    TupleStructConstructor,
    EnumVariantConstructor,
}

impl CallRelationKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Function => "Function",
            Self::DynamicFunction => "DynamicFunction",
            Self::Method => "Method",
            Self::AssociatedFunction => "AssociatedFunction",
            Self::TupleStructConstructor => "TupleStructConstructor",
            Self::EnumVariantConstructor => "EnumVariantConstructor",
        }
    }

    fn from_str(value: &str) -> Result<Self, DbError> {
        match value {
            "Function" => Ok(Self::Function),
            "DynamicFunction" => Ok(Self::DynamicFunction),
            "Method" => Ok(Self::Method),
            "AssociatedFunction" => Ok(Self::AssociatedFunction),
            "TupleStructConstructor" => Ok(Self::TupleStructConstructor),
            "EnumVariantConstructor" => Ok(Self::EnumVariantConstructor),
            other => Err(DbError::Cozo(format!(
                "unknown call relation kind {other:?}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CallTargetKind {
    Function,
    Method,
    Struct,
    Variant,
}

impl CallTargetKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Function => "Function",
            Self::Method => "Method",
            Self::Struct => "Struct",
            Self::Variant => "Variant",
        }
    }

    fn from_str(value: &str) -> Result<Self, DbError> {
        match value {
            "Function" => Ok(Self::Function),
            "Method" => Ok(Self::Method),
            "Struct" => Ok(Self::Struct),
            "Variant" => Ok(Self::Variant),
            other => Err(DbError::Cozo(format!("unknown call target kind {other:?}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CallTargetFamily {
    relation: CallRelationKind,
    source: CallSiteKind,
    target: CallTargetKind,
    target_relation: &'static str,
}

const VALID_CALL_TARGET_FAMILIES: [CallTargetFamily; 6] = [
    CallTargetFamily {
        relation: CallRelationKind::Function,
        source: CallSiteKind::Path,
        target: CallTargetKind::Function,
        target_relation: "function",
    },
    CallTargetFamily {
        relation: CallRelationKind::DynamicFunction,
        source: CallSiteKind::Dynamic,
        target: CallTargetKind::Function,
        target_relation: "function",
    },
    CallTargetFamily {
        relation: CallRelationKind::Method,
        source: CallSiteKind::Method,
        target: CallTargetKind::Method,
        target_relation: "method",
    },
    CallTargetFamily {
        relation: CallRelationKind::AssociatedFunction,
        source: CallSiteKind::Path,
        target: CallTargetKind::Method,
        target_relation: "method",
    },
    CallTargetFamily {
        relation: CallRelationKind::TupleStructConstructor,
        source: CallSiteKind::Path,
        target: CallTargetKind::Struct,
        target_relation: "struct",
    },
    CallTargetFamily {
        relation: CallRelationKind::EnumVariantConstructor,
        source: CallSiteKind::Path,
        target: CallTargetKind::Variant,
        target_relation: "variant",
    },
];

impl CallTargetFamily {
    fn matches(self, target: &CallTargetRow) -> bool {
        self.relation == target.relation
            && self.source == target.source_kind
            && self.target == target.target_kind
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CallStatusKind {
    Resolved,
    Unresolved,
    Ambiguous,
    External,
    Unsupported,
}

impl CallStatusKind {
    fn from_str(value: &str) -> Result<Self, DbError> {
        match value {
            "Resolved" => Ok(Self::Resolved),
            "Unresolved" => Ok(Self::Unresolved),
            "Ambiguous" => Ok(Self::Ambiguous),
            "External" => Ok(Self::External),
            "Unsupported" => Ok(Self::Unsupported),
            other => Err(DbError::Cozo(format!("unknown call status kind {other:?}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CallResolutionKind {
    LocalExact,
}

impl CallResolutionKind {
    fn from_str(value: &str) -> Result<Self, DbError> {
        match value {
            "LocalExact" => Ok(Self::LocalExact),
            other => Err(DbError::Cozo(format!(
                "unknown call resolution kind {other:?}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallSiteRow {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub kind: CallSiteKind,
    pub span: (u32, u32),
    pub cfgs: Vec<String>,
    pub path: Option<Vec<String>>,
    pub method: Option<String>,
    pub macro_name: Option<String>,
    pub receiver: Option<CallReceiver>,
    pub arg_count: Option<u32>,
    pub generic_arg_count: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallTargetRow {
    pub site_id: Uuid,
    pub target_id: Uuid,
    pub relation: CallRelationKind,
    pub source_kind: CallSiteKind,
    pub target_kind: CallTargetKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallResolutionRow {
    pub site_id: Uuid,
    pub site_kind: CallSiteKind,
    pub status: CallStatusKind,
    pub resolution: Option<CallResolutionKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallContextRow {
    pub site: CallSiteRow,
    pub status: CallResolutionRow,
    pub targets: Vec<CallTargetRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallCallerRow {
    pub site: CallSiteRow,
    pub status: CallResolutionRow,
    pub target: CallTargetRow,
}

/// Starting point for call-context expansion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CallContextSeed {
    Owner(Uuid),
    Target(Uuid),
}

/// Why a candidate was returned by [`Database::expand_call_context`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CallContextRelation {
    OutgoingTarget,
    IncomingCaller,
}

/// Code graph node reachable from a call-context seed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallContextCandidate {
    pub node_id: Uuid,
    pub relation: CallContextRelation,
    pub call_site_id: Uuid,
    pub target_id: Uuid,
    pub distance: u32,
}

/// Controls for call-context expansion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallContextOptions {
    pub include_outgoing_targets: bool,
    pub include_incoming_callers: bool,
    pub max_candidates: usize,
}

impl Default for CallContextOptions {
    fn default() -> Self {
        Self {
            include_outgoing_targets: true,
            include_incoming_callers: true,
            max_candidates: 64,
        }
    }
}

impl Database {
    pub fn has_call_graph_relations(&self) -> Result<bool, DbError> {
        const REQUIRED: [&str; 4] = [
            "call_site",
            "call_site_edge",
            "call_relation",
            "call_resolution_status",
        ];
        let rows = self.raw_query("::relations")?;
        let registered = rows
            .rows
            .iter()
            .filter_map(|row| row.first().and_then(|value| value.get_str()))
            .collect::<std::collections::HashSet<_>>();
        Ok(REQUIRED.iter().all(|name| registered.contains(name)))
    }

    pub fn call_sites_for_owner(&self, owner_id: Uuid) -> Result<Vec<CallSiteRow>, DbError> {
        let mut params = BTreeMap::new();
        params.insert(
            "owner_id".to_string(),
            DataValue::Uuid(UuidWrapper(owner_id)),
        );

        let rows = self.run_script(
            r#"?[
                id,
                owner_id,
                call_kind,
                span,
                cfgs,
                path,
                method_name,
                macro_name,
                receiver_kind,
                receiver_path,
                arg_count,
                generic_arg_count
            ] :=
                owner_id = $owner_id,
                (
                    *function { id: owner_id @ 'NOW' },
                    owner_kind = "Function"
                ) or (
                    *method { id: owner_id @ 'NOW' },
                    owner_kind = "Method"
                ) or (
                    *const { id: owner_id @ 'NOW' },
                    owner_kind = "Const"
                ) or (
                    *static { id: owner_id @ 'NOW' },
                    owner_kind = "Static"
                ),
                *call_site_edge {
                    source_id: owner_id,
                    target_id: id,
                    relation_kind: "BodyContainsCall",
                    source_kind: owner_kind,
                    target_kind: call_kind @ 'NOW'
                },
                *call_site {
                    id,
                    owner_id,
                    call_kind,
                    span,
                    cfgs,
                    path,
                    method_name,
                    macro_name,
                    receiver_kind,
                    receiver_path,
                    arg_count,
                    generic_arg_count @ 'NOW'
                }
            :sort span"#,
            params,
            ScriptMutability::Immutable,
        )?;

        rows.rows.iter().map(|row| decode_site(row)).collect()
    }

    pub fn call_targets_for_site(&self, site_id: Uuid) -> Result<Vec<CallTargetRow>, DbError> {
        let mut params = BTreeMap::new();
        params.insert("site_id".to_string(), DataValue::Uuid(UuidWrapper(site_id)));

        let mut script = valid_call_target_rules();
        script.push_str(
            r#"
            ?[site_id, target_id, relation_kind, source_kind, target_kind] :=
                site_id = $site_id,
                *call_site {
                    id: site_id,
                    call_kind: source_kind @ 'NOW'
                },
                *call_relation {
                    source_id: site_id,
                    target_id,
                    relation_kind,
                    source_kind,
                    target_kind @ 'NOW'
                },
                valid_target[target_id, relation_kind, source_kind, target_kind]
            :sort target_id"#,
        );

        let rows = self.run_script(&script, params, ScriptMutability::Immutable)?;

        Ok(rows
            .rows
            .iter()
            .map(|row| decode_target(row))
            .collect::<Result<Vec<CallTargetRow>, DbError>>()?
            .into_iter()
            .filter(valid_call_target)
            .collect())
    }

    pub fn call_resolution_for_site(
        &self,
        site_id: Uuid,
    ) -> Result<Option<CallResolutionRow>, DbError> {
        let mut params = BTreeMap::new();
        params.insert("site_id".to_string(), DataValue::Uuid(UuidWrapper(site_id)));

        let rows = self.run_script(
            r#"?[site_id, source_kind, status_kind, resolution_kind, call_kind] :=
                site_id = $site_id,
                *call_resolution_status {
                    source_id: site_id,
                    source_kind,
                    status_kind,
                    resolution_kind @ 'NOW'
                },
                *call_site {
                    id: site_id,
                    call_kind @ 'NOW'
                }"#,
            params,
            ScriptMutability::Immutable,
        )?;

        match rows.rows.as_slice() {
            [] => Ok(None),
            [row] => {
                let status = decode_resolution(&row[..4])?;
                let site_kind = CallSiteKind::from_str(&to_string(&row[4])?)?;
                if status.site_kind != site_kind {
                    return Err(DbError::Cozo(format!(
                        "call_resolution_status source_kind {:?} does not match call site {} kind {:?}",
                        status.site_kind, site_id, site_kind
                    )));
                }
                Ok(Some(status))
            }
            rows => Err(DbError::Cozo(format!(
                "expected at most one call_resolution_status for call site {site_id}, found {}",
                rows.len()
            ))),
        }
    }

    pub fn call_context_for_owner(&self, owner_id: Uuid) -> Result<Vec<CallContextRow>, DbError> {
        self.call_sites_for_owner(owner_id)?
            .into_iter()
            .map(|site| {
                let status = self.call_resolution_for_site(site.id)?.ok_or_else(|| {
                    DbError::Cozo(format!(
                        "missing call_resolution_status for call site {} owned by {}",
                        site.id, site.owner_id
                    ))
                })?;
                let targets = self.call_targets_for_site(site.id)?;
                let row = CallContextRow {
                    site,
                    status,
                    targets,
                };
                validate_owner_context_targets(&row)?;
                Ok(row)
            })
            .collect()
    }

    pub fn callers_for_target(&self, target_id: Uuid) -> Result<Vec<CallCallerRow>, DbError> {
        let mut params = BTreeMap::new();
        params.insert(
            "target_id".to_string(),
            DataValue::Uuid(UuidWrapper(target_id)),
        );

        let mut script = valid_call_target_rules();
        script.push_str(
            r#"
            ?[
                id,
                owner_id,
                call_kind,
                span,
                cfgs,
                path,
                method_name,
                macro_name,
                receiver_kind,
                receiver_path,
                arg_count,
                generic_arg_count,
                relation_site_id,
                relation_target_id,
                relation_kind,
                source_kind,
                target_kind
            ] :=
                relation_target_id = $target_id,
                *call_relation {
                    source_id: relation_site_id,
                    target_id: relation_target_id,
                    relation_kind,
                    source_kind,
                    target_kind @ 'NOW'
                },
                valid_target[relation_target_id, relation_kind, source_kind, target_kind],
                *call_site_edge {
                    source_id: owner_id,
                    target_id: relation_site_id,
                    relation_kind: "BodyContainsCall",
                    source_kind: owner_kind,
                    target_kind: call_kind @ 'NOW'
                },
                *call_site {
                    id: relation_site_id,
                    owner_id,
                    call_kind,
                    span,
                    cfgs,
                    path,
                    method_name,
                    macro_name,
                    receiver_kind,
                    receiver_path,
                    arg_count,
                    generic_arg_count @ 'NOW'
                },
                (
                    *function { id: owner_id @ 'NOW' },
                    owner_kind = "Function"
                ) or (
                    *method { id: owner_id @ 'NOW' },
                    owner_kind = "Method"
                ) or (
                    *const { id: owner_id @ 'NOW' },
                    owner_kind = "Const"
                ) or (
                    *static { id: owner_id @ 'NOW' },
                    owner_kind = "Static"
                ),
                id = relation_site_id,
                call_kind = source_kind
            :sort owner_id, span, relation_kind"#,
        );

        let rows = self.run_script(&script, params, ScriptMutability::Immutable)?;

        let callers = rows
            .rows
            .iter()
            .map(|row| {
                let site = decode_site(&row[..12])?;
                let target = decode_target(&row[12..])?;
                let status = self.call_resolution_for_site(site.id)?.ok_or_else(|| {
                    DbError::Cozo(format!(
                        "missing call_resolution_status for call site {} targeting {}",
                        site.id, target_id
                    ))
                })?;
                Ok(CallCallerRow {
                    site,
                    status,
                    target,
                })
            })
            .collect::<Result<Vec<CallCallerRow>, DbError>>()?;

        let mut valid_callers = Vec::new();
        for caller in callers {
            if !valid_call_target(&caller.target) {
                continue;
            }
            self.validate_target_centered_caller_targets(&caller)?;
            valid_callers.push(caller);
        }

        Ok(valid_callers)
    }

    fn validate_target_centered_caller_targets(
        &self,
        caller: &CallCallerRow,
    ) -> Result<(), DbError> {
        let targets = self.call_targets_for_site(caller.site.id)?;
        let row = CallContextRow {
            site: caller.site.clone(),
            status: caller.status,
            targets,
        };
        validate_owner_context_targets(&row)
    }

    /// Expands an owner or target seed into graphRAG call-context candidates.
    ///
    /// This is the application-facing layer over the lower-level call-site and
    /// target-centered helpers. It only promotes resolved call edges as context
    /// candidates; unsupported, external, unresolved, or ambiguous rows remain
    /// visible through [`Self::call_context_for_owner`] but do not become
    /// traversal targets.
    pub fn expand_call_context(
        &self,
        seed: CallContextSeed,
        options: CallContextOptions,
    ) -> Result<Vec<CallContextCandidate>, DbError> {
        if options.max_candidates == 0 {
            return Ok(Vec::new());
        }

        let mut candidates = Vec::new();

        match seed {
            CallContextSeed::Owner(owner_id) if options.include_outgoing_targets => {
                for row in self.call_context_for_owner(owner_id)? {
                    if row.status.status != CallStatusKind::Resolved {
                        continue;
                    }
                    candidates.extend(row.targets.into_iter().map(|target| CallContextCandidate {
                        node_id: target.target_id,
                        relation: CallContextRelation::OutgoingTarget,
                        call_site_id: row.site.id,
                        target_id: target.target_id,
                        distance: 1,
                    }));
                }
            }
            CallContextSeed::Target(target_id) if options.include_incoming_callers => {
                for caller in self.callers_for_target(target_id)? {
                    if caller.status.status != CallStatusKind::Resolved {
                        continue;
                    }
                    candidates.push(CallContextCandidate {
                        node_id: caller.site.owner_id,
                        relation: CallContextRelation::IncomingCaller,
                        call_site_id: caller.site.id,
                        target_id: caller.target.target_id,
                        distance: 1,
                    });
                }
            }
            CallContextSeed::Owner(_) | CallContextSeed::Target(_) => {}
        }

        candidates.sort_by_key(|candidate| {
            (
                candidate.distance,
                candidate.relation,
                candidate.node_id.as_u128(),
                candidate.call_site_id.as_u128(),
            )
        });
        candidates.truncate(options.max_candidates);
        Ok(candidates)
    }
}

fn decode_site(row: &[DataValue]) -> Result<CallSiteRow, DbError> {
    let site = CallSiteRow {
        id: to_uuid(&row[0])?,
        owner_id: to_uuid(&row[1])?,
        kind: CallSiteKind::from_str(&to_string(&row[2])?)?,
        span: span_pair(&row[3])?,
        cfgs: to_string_list(&row[4])?,
        path: optional_string_list(&row[5])?,
        method: optional_string(&row[6])?,
        macro_name: optional_string(&row[7])?,
        receiver: CallReceiver::from_parts(&row[8], &row[9])?,
        arg_count: optional_index(&row[10])?,
        generic_arg_count: optional_index(&row[11])?,
    };
    validate_call_site_shape(&site)?;
    Ok(site)
}

fn validate_call_site_shape(site: &CallSiteRow) -> Result<(), DbError> {
    let valid = match site.kind {
        CallSiteKind::Path => {
            non_empty_path(site.path.as_deref())
                && site.method.is_none()
                && site.macro_name.is_none()
                && site.receiver.is_none()
                && site.arg_count.is_some()
                && site.generic_arg_count.is_some()
        }
        CallSiteKind::Method => {
            site.path.is_none()
                && non_empty_string(site.method.as_deref())
                && site.macro_name.is_none()
                && site.receiver.is_some()
                && site.arg_count.is_some()
                && site.generic_arg_count.is_some()
        }
        CallSiteKind::Dynamic => {
            optional_non_empty_path(site.path.as_deref())
                && site.method.is_none()
                && site.macro_name.is_none()
                && site.receiver.is_none()
                && site.arg_count.is_some()
                && site.generic_arg_count.is_none()
        }
        CallSiteKind::Macro => {
            site.path.is_none()
                && site.method.is_none()
                && non_empty_string(site.macro_name.as_deref())
                && site.receiver.is_none()
                && site.arg_count.is_none()
                && site.generic_arg_count.is_none()
        }
    };

    if valid {
        Ok(())
    } else {
        Err(DbError::Cozo(format!(
            "malformed {:?} call_site {}",
            site.kind, site.id
        )))
    }
}

fn non_empty_path(path: Option<&[String]>) -> bool {
    path.is_some_and(|path| !path.is_empty() && path.iter().all(|segment| !segment.is_empty()))
}

fn optional_non_empty_path(path: Option<&[String]>) -> bool {
    path.is_none_or(|path| !path.is_empty() && path.iter().all(|segment| !segment.is_empty()))
}

fn non_empty_string(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.is_empty())
}

fn valid_call_target_rules() -> String {
    let mut rules = String::new();
    for family in VALID_CALL_TARGET_FAMILIES {
        let relation = family.relation.as_str();
        let source = family.source.as_str();
        let target = family.target.as_str();
        let target_relation = family.target_relation;
        writeln!(
            &mut rules,
            r#"
            valid_target[target_id, relation_kind, source_kind, target_kind] :=
                relation_kind = "{relation}",
                source_kind = "{source}",
                target_kind = "{target}",
                *{target_relation} {{ id: target_id @ 'NOW' }}
"#
        )
        .expect("writing call target rule to String should not fail");
    }
    rules
}

fn decode_target(row: &[DataValue]) -> Result<CallTargetRow, DbError> {
    Ok(CallTargetRow {
        site_id: to_uuid(&row[0])?,
        target_id: to_uuid(&row[1])?,
        relation: CallRelationKind::from_str(&to_string(&row[2])?)?,
        source_kind: CallSiteKind::from_str(&to_string(&row[3])?)?,
        target_kind: CallTargetKind::from_str(&to_string(&row[4])?)?,
    })
}

fn valid_call_target(target: &CallTargetRow) -> bool {
    VALID_CALL_TARGET_FAMILIES
        .iter()
        .any(|family| family.matches(target))
}

fn decode_resolution(row: &[DataValue]) -> Result<CallResolutionRow, DbError> {
    let status = CallResolutionRow {
        site_id: to_uuid(&row[0])?,
        site_kind: CallSiteKind::from_str(&to_string(&row[1])?)?,
        status: CallStatusKind::from_str(&to_string(&row[2])?)?,
        resolution: optional_resolution(&row[3])?,
    };
    validate_resolution_shape(&status)?;
    Ok(status)
}

fn validate_resolution_shape(status: &CallResolutionRow) -> Result<(), DbError> {
    let valid = match status.status {
        CallStatusKind::Resolved => status.resolution == Some(CallResolutionKind::LocalExact),
        CallStatusKind::Unresolved
        | CallStatusKind::Ambiguous
        | CallStatusKind::External
        | CallStatusKind::Unsupported => status.resolution.is_none(),
    };

    if valid {
        Ok(())
    } else {
        Err(DbError::Cozo(format!(
            "call_resolution_status resolution_kind {:?} is invalid for {:?} call site {}",
            status.resolution, status.status, status.site_id
        )))
    }
}

fn validate_owner_context_targets(row: &CallContextRow) -> Result<(), DbError> {
    if row.status.status == CallStatusKind::Resolved && row.targets.len() != 1 {
        return Err(DbError::Cozo(format!(
            "resolved call site {} expected exactly one target, found {}",
            row.site.id,
            row.targets.len()
        )));
    }
    Ok(())
}

fn optional_string(value: &DataValue) -> Result<Option<String>, DbError> {
    match value {
        DataValue::Null => Ok(None),
        other => Ok(Some(to_string(other)?)),
    }
}

fn optional_string_list(value: &DataValue) -> Result<Option<Vec<String>>, DbError> {
    match value {
        DataValue::Null => Ok(None),
        other => Ok(Some(to_string_list(other)?)),
    }
}

fn optional_index(value: &DataValue) -> Result<Option<u32>, DbError> {
    match value {
        DataValue::Null => Ok(None),
        other => Ok(Some(required_index(other)?)),
    }
}

fn optional_resolution(value: &DataValue) -> Result<Option<CallResolutionKind>, DbError> {
    match value {
        DataValue::Null => Ok(None),
        other => Ok(Some(CallResolutionKind::from_str(&to_string(other)?)?)),
    }
}

fn required_index(value: &DataValue) -> Result<u32, DbError> {
    match value {
        DataValue::Num(Num::Int(value)) => u32::try_from(*value)
            .map_err(|err| DbError::Cozo(format!("call graph integer out of u32 range: {err}"))),
        other => Err(DbError::Cozo(format!(
            "expected call graph integer, found {other:?}"
        ))),
    }
}

fn span_pair(value: &DataValue) -> Result<(u32, u32), DbError> {
    match value {
        DataValue::List(items) if items.len() == 2 => {
            Ok((required_index(&items[0])?, required_index(&items[1])?))
        }
        other => Err(DbError::Cozo(format!(
            "expected call graph span pair, found {other:?}"
        ))),
    }
}
