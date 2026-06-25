use serde::{Deserialize, Serialize};

use crate::DbError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CallSiteKind {
    Path,
    Method,
    Dynamic,
    Macro,
}

impl CallSiteKind {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Path => "Path",
            Self::Method => "Method",
            Self::Dynamic => "Dynamic",
            Self::Macro => "Macro",
        }
    }

    pub(super) fn from_str(value: &str) -> Result<Self, DbError> {
        match value {
            "Path" => Ok(Self::Path),
            "Method" => Ok(Self::Method),
            "Dynamic" => Ok(Self::Dynamic),
            "Macro" => Ok(Self::Macro),
            other => Err(DbError::Cozo(format!("unknown call-site kind {other:?}"))),
        }
    }
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
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Function => "Function",
            Self::DynamicFunction => "DynamicFunction",
            Self::Method => "Method",
            Self::AssociatedFunction => "AssociatedFunction",
            Self::TupleStructConstructor => "TupleStructConstructor",
            Self::EnumVariantConstructor => "EnumVariantConstructor",
        }
    }

    pub(super) fn from_str(value: &str) -> Result<Self, DbError> {
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
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Function => "Function",
            Self::Method => "Method",
            Self::Struct => "Struct",
            Self::Variant => "Variant",
        }
    }

    pub(super) fn from_str(value: &str) -> Result<Self, DbError> {
        match value {
            "Function" => Ok(Self::Function),
            "Method" => Ok(Self::Method),
            "Struct" => Ok(Self::Struct),
            "Variant" => Ok(Self::Variant),
            other => Err(DbError::Cozo(format!("unknown call target kind {other:?}"))),
        }
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
    pub(super) fn from_str(value: &str) -> Result<Self, DbError> {
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
    pub(super) fn from_str(value: &str) -> Result<Self, DbError> {
        match value {
            "LocalExact" => Ok(Self::LocalExact),
            other => Err(DbError::Cozo(format!(
                "unknown call resolution kind {other:?}"
            ))),
        }
    }
}
