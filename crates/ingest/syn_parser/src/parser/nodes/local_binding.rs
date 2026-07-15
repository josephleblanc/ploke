use cozo::{DataValue, UuidWrapper};
use ploke_core::{IdTrait, PROJECT_NAMESPACE_UUID};
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display};
use uuid::Uuid;

use super::{AnyCallSiteId, CallBodyOwnerId, ExecutableBodyId, ToCozoUuid};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct LocalBindingId(Uuid);

impl LocalBindingId {
    #[inline]
    pub fn uuid(self) -> Uuid {
        self.0
    }
}

impl IdTrait for LocalBindingId {
    #[inline]
    fn uuid(&self) -> Uuid {
        self.0
    }

    #[inline]
    fn is_resolved(&self) -> bool {
        false
    }

    #[inline]
    fn is_synthetic(&self) -> bool {
        true
    }
}

impl ToCozoUuid for LocalBindingId {
    #[inline]
    fn to_cozo_uuid(self) -> DataValue {
        DataValue::Uuid(UuidWrapper(self.uuid()))
    }
}

impl Display for LocalBindingId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LocalBindingId({})", self.0)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct LocalBindingNode {
    pub id: LocalBindingId,
    pub owner: CallBodyOwnerId,
    pub span: (usize, usize),
    pub cfgs: Vec<String>,
    pub kind: LocalBindingKind,
    pub name: String,
    pub source: LocalBindingSource,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LocalBindingKind {
    ParameterBinding,
    LetBinding,
    FieldProjection,
    ReturnExpression,
}

impl LocalBindingKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ParameterBinding => "ParameterBinding",
            Self::LetBinding => "LetBinding",
            Self::FieldProjection => "FieldProjection",
            Self::ReturnExpression => "ReturnExpression",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum LocalBindingSource {
    Parameter,
    Constructed {
        type_path: Vec<String>,
    },
    InitializedPath {
        init_path: Vec<String>,
    },
    FieldProjection {
        base_binding_id: LocalBindingId,
        field_path: Vec<String>,
        init_path: Vec<String>,
    },
    Closure {
        body_id: ExecutableBodyId,
    },
    AsyncClosure {
        body_id: ExecutableBodyId,
    },
    PathCallResult {
        call_site_id: AnyCallSiteId,
        path: Vec<String>,
    },
    DynamicCallResult {
        call_site_id: AnyCallSiteId,
        callee_kind: String,
        callee_path: Option<Vec<String>>,
    },
}

impl LocalBindingSource {
    pub fn source_kind(&self) -> &'static str {
        match self {
            Self::Parameter => "Parameter",
            Self::Constructed { .. } => "Constructed",
            Self::InitializedPath { .. } => "InitializedPath",
            Self::FieldProjection { .. } => "FieldProjection",
            Self::Closure { .. } => "Closure",
            Self::AsyncClosure { .. } => "AsyncClosure",
            Self::PathCallResult { .. } => "PathCallResult",
            Self::DynamicCallResult { .. } => "DynamicCallResult",
        }
    }
}

pub(in crate::parser) fn generate_local_binding_id(
    owner: CallBodyOwnerId,
    name: &str,
    span: (usize, usize),
    kind: LocalBindingKind,
    cfgs: &[String],
) -> LocalBindingId {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(owner.uuid().as_bytes());
    bytes.extend_from_slice(name.as_bytes());
    bytes.extend_from_slice(&span.0.to_le_bytes());
    bytes.extend_from_slice(&span.1.to_le_bytes());
    bytes.extend_from_slice(kind.as_str().as_bytes());
    for cfg in cfgs {
        bytes.extend_from_slice(cfg.as_bytes());
        bytes.push(0);
    }
    LocalBindingId(Uuid::new_v5(&PROJECT_NAMESPACE_UUID, &bytes))
}
