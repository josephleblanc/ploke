use serde::{Deserialize, Serialize};

use super::{CallBodyOwnerId, ExecutableBodyId, ExecutableBodyKind};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ExecutableWherePredicate {
    pub subject_path: Vec<String>,
    pub trait_bounds: Vec<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ExecutableBodyNode {
    pub id: ExecutableBodyId,
    pub kind: ExecutableBodyKind,
    pub parent: CallBodyOwnerId,
    pub span: (usize, usize),
    pub cfgs: Vec<String>,
    pub label: Option<String>,
    pub where_predicates: Vec<ExecutableWherePredicate>,
}

impl ExecutableBodyNode {
    pub fn new(
        id: ExecutableBodyId,
        parent: CallBodyOwnerId,
        span: (usize, usize),
        cfgs: Vec<String>,
        label: Option<String>,
    ) -> Self {
        Self {
            id,
            kind: id.kind(),
            parent,
            span,
            cfgs,
            label,
            where_predicates: Vec::new(),
        }
    }

    pub fn set_where_predicates(&mut self, where_predicates: Vec<ExecutableWherePredicate>) {
        self.where_predicates = where_predicates;
    }
}
