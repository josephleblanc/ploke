use crate::{
    error::SynParserError,
    parser::{
        nodes::{
            AnyCallSiteId, CallBodyOwnerId, DynamicCallCallee, DynamicCallNode, FunctionNodeId,
        },
        relations::{CallRelation, CallResolutionKind, CallResolutionStatus},
    },
};

use super::{CallRelationResolver, LocalFunctionPathResolution};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DynamicPathResolution {
    Resolved(FunctionNodeId),
    Unresolved,
    Ambiguous,
    External,
    Unsupported,
}

impl CallRelationResolver<'_> {
    pub(super) fn resolve_dynamic_call(
        &self,
        call: &DynamicCallNode,
        relations: &mut Vec<CallRelation>,
        statuses: &mut Vec<CallResolutionStatus>,
    ) -> Result<(), SynParserError> {
        let source = AnyCallSiteId::Dynamic(call.id);
        if let DynamicCallCallee::IfBranchPaths { paths } = &call.callee {
            self.resolve_if_branch_dynamic_call(call, paths, relations, statuses)?;
            return Ok(());
        }
        if let DynamicCallCallee::MatchArmPaths { paths } = &call.callee {
            self.resolve_if_branch_dynamic_call(call, paths, relations, statuses)?;
            return Ok(());
        }

        if let DynamicCallCallee::InitializedLocalBinding { init_path, .. }
        | DynamicCallCallee::FnPointerCastInitializedLocalBinding { init_path, .. }
        | DynamicCallCallee::DereferencedInitializedLocalBinding { init_path, .. }
        | DynamicCallCallee::FieldInitializedLocalBinding { init_path, .. }
        | DynamicCallCallee::IndexedInitializedLocalBinding { init_path, .. } = &call.callee
        {
            self.resolve_initialized_dynamic_binding_call(call, init_path, relations, statuses)?;
            return Ok(());
        }

        let path = match &call.callee {
            DynamicCallCallee::Path { path } | DynamicCallCallee::FnPointerCastPath { path } => {
                path
            }
            DynamicCallCallee::LocalBinding { .. }
            | DynamicCallCallee::InitializedLocalBinding { .. }
            | DynamicCallCallee::FnPointerCastInitializedLocalBinding { .. }
            | DynamicCallCallee::FnPointerCastLocalBinding { .. }
            | DynamicCallCallee::DereferencedInitializedLocalBinding { .. }
            | DynamicCallCallee::FieldLocalBinding { .. }
            | DynamicCallCallee::FieldInitializedLocalBinding { .. }
            | DynamicCallCallee::IndexedInitializedLocalBinding { .. }
            | DynamicCallCallee::IfBranchPaths { .. }
            | DynamicCallCallee::MatchArmPaths { .. }
            | DynamicCallCallee::Other => {
                statuses.push(CallResolutionStatus::Unsupported { source });
                return Ok(());
            }
        };

        if self.is_external_path(path) || self.is_external_import_path(call.owner, path)? {
            statuses.push(CallResolutionStatus::External { source });
            return Ok(());
        }

        let resolution = if self.is_unqualified_path(path) {
            self.resolve_unqualified_local_function_path(call.owner, path)?
        } else if self.is_explicit_local_path(path) {
            self.resolve_local_function_path(call.owner, path)?
        } else {
            self.resolve_implicit_local_function_path(call.owner, path)?
        };

        match resolution {
            LocalFunctionPathResolution::Resolved(target) => {
                relations.push(CallRelation::DynamicFunction {
                    source: call.id,
                    target,
                });
                statuses.push(CallResolutionStatus::Resolved {
                    source,
                    kind: CallResolutionKind::LocalExact,
                });
            }
            LocalFunctionPathResolution::Unresolved => {
                statuses.push(CallResolutionStatus::Unresolved { source });
            }
            LocalFunctionPathResolution::Ambiguous => {
                statuses.push(CallResolutionStatus::Ambiguous { source });
            }
            LocalFunctionPathResolution::Unsupported => {
                statuses.push(CallResolutionStatus::Unsupported { source });
            }
        }

        Ok(())
    }

    fn resolve_initialized_dynamic_binding_call(
        &self,
        call: &DynamicCallNode,
        init_path: &[String],
        relations: &mut Vec<CallRelation>,
        statuses: &mut Vec<CallResolutionStatus>,
    ) -> Result<(), SynParserError> {
        let source = AnyCallSiteId::Dynamic(call.id);

        if self.is_external_path(init_path)
            || self.is_external_import_path(call.owner, init_path)?
        {
            statuses.push(CallResolutionStatus::External { source });
            return Ok(());
        }

        let resolution = if self.is_unqualified_path(init_path) {
            self.resolve_unqualified_local_function_path(call.owner, init_path)?
        } else if self.is_explicit_local_path(init_path) {
            self.resolve_local_function_path(call.owner, init_path)?
        } else {
            self.resolve_implicit_local_function_path(call.owner, init_path)?
        };

        match resolution {
            LocalFunctionPathResolution::Resolved(target) => {
                relations.push(CallRelation::DynamicFunction {
                    source: call.id,
                    target,
                });
                statuses.push(CallResolutionStatus::Resolved {
                    source,
                    kind: CallResolutionKind::LocalExact,
                });
            }
            LocalFunctionPathResolution::Unresolved => {
                statuses.push(CallResolutionStatus::Unresolved { source });
            }
            LocalFunctionPathResolution::Ambiguous => {
                statuses.push(CallResolutionStatus::Ambiguous { source });
            }
            LocalFunctionPathResolution::Unsupported => {
                statuses.push(CallResolutionStatus::Unsupported { source });
            }
        }

        Ok(())
    }

    fn resolve_if_branch_dynamic_call(
        &self,
        call: &DynamicCallNode,
        paths: &[Vec<String>],
        relations: &mut Vec<CallRelation>,
        statuses: &mut Vec<CallResolutionStatus>,
    ) -> Result<(), SynParserError> {
        let source = AnyCallSiteId::Dynamic(call.id);
        if paths.is_empty() {
            statuses.push(CallResolutionStatus::Unsupported { source });
            return Ok(());
        }

        let mut targets = Vec::new();
        let mut has_external = false;
        let mut has_unresolved = false;

        for path in paths {
            match self.resolve_dynamic_path(call.owner, path)? {
                DynamicPathResolution::Resolved(target) => targets.push(target),
                DynamicPathResolution::Unresolved => has_unresolved = true,
                DynamicPathResolution::Ambiguous => {
                    statuses.push(CallResolutionStatus::Ambiguous { source });
                    return Ok(());
                }
                DynamicPathResolution::External => has_external = true,
                DynamicPathResolution::Unsupported => {
                    statuses.push(CallResolutionStatus::Unsupported { source });
                    return Ok(());
                }
            }
        }

        targets.sort_unstable();
        targets.dedup();

        match (targets.as_slice(), has_external, has_unresolved) {
            ([target], false, false) => {
                relations.push(CallRelation::DynamicFunction {
                    source: call.id,
                    target: *target,
                });
                statuses.push(CallResolutionStatus::Resolved {
                    source,
                    kind: CallResolutionKind::LocalExact,
                });
            }
            ([], true, false) => {
                statuses.push(CallResolutionStatus::External { source });
            }
            ([], false, true) => {
                statuses.push(CallResolutionStatus::Unresolved { source });
            }
            _ => {
                relations.extend(targets.iter().copied().map(|target| {
                    CallRelation::DynamicFunction {
                        source: call.id,
                        target,
                    }
                }));
                statuses.push(CallResolutionStatus::Ambiguous { source });
            }
        }

        Ok(())
    }

    fn resolve_dynamic_path(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
    ) -> Result<DynamicPathResolution, SynParserError> {
        if self.is_external_path(path) || self.is_external_import_path(owner, path)? {
            return Ok(DynamicPathResolution::External);
        }

        let resolution = if self.is_unqualified_path(path) {
            self.resolve_unqualified_local_function_path(owner, path)?
        } else if self.is_explicit_local_path(path) {
            self.resolve_local_function_path(owner, path)?
        } else {
            self.resolve_implicit_local_function_path(owner, path)?
        };

        Ok(match resolution {
            LocalFunctionPathResolution::Resolved(target) => {
                DynamicPathResolution::Resolved(target)
            }
            LocalFunctionPathResolution::Unresolved => DynamicPathResolution::Unresolved,
            LocalFunctionPathResolution::Ambiguous => DynamicPathResolution::Ambiguous,
            LocalFunctionPathResolution::Unsupported => DynamicPathResolution::Unsupported,
        })
    }
}
