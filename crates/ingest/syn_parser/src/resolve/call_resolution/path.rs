use crate::{
    error::SynParserError,
    parser::{
        nodes::{AnyCallSiteId, PathCallCallee, PathCallNode},
        relations::{CallRelation, CallResolutionKind, CallResolutionStatus, TypeRelation},
    },
};

use super::{
    AssocPathResolution, CallRelationResolver, ConstructorPathResolution,
    LocalFunctionPathResolution,
};

impl CallRelationResolver<'_> {
    pub(super) fn resolve_path_call(
        &self,
        call: &PathCallNode,
        type_relations: &[TypeRelation],
        relations: &mut Vec<CallRelation>,
        statuses: &mut Vec<CallResolutionStatus>,
    ) -> Result<(), SynParserError> {
        let source = AnyCallSiteId::Path(call.id);

        match &call.callee {
            PathCallCallee::ItemPath => {}
            PathCallCallee::ValueBinding { .. } => {
                statuses.push(CallResolutionStatus::Unsupported { source });
                return Ok(());
            }
            PathCallCallee::InitializedValueBinding { init_path, .. } => {
                self.resolve_initialized_value_binding_call(call, init_path, relations, statuses)?;
                return Ok(());
            }
        }

        if self.is_external_path(&call.path)
            || self.is_external_import_path(call.owner, &call.path)?
        {
            statuses.push(CallResolutionStatus::External { source });
            return Ok(());
        }

        if let Some(resolution) =
            self.resolve_associated_function_path(call.owner, &call.path, type_relations)?
        {
            match resolution {
                AssocPathResolution::Resolved(target) => {
                    relations.push(CallRelation::AssociatedFunction {
                        source: call.id,
                        target,
                    });
                    statuses.push(CallResolutionStatus::Resolved {
                        source,
                        kind: CallResolutionKind::LocalExact,
                    });
                }
                AssocPathResolution::Unresolved => {
                    statuses.push(CallResolutionStatus::Unresolved { source });
                }
                AssocPathResolution::Ambiguous => {
                    statuses.push(CallResolutionStatus::Ambiguous { source });
                }
                AssocPathResolution::Unsupported => {
                    statuses.push(CallResolutionStatus::Unsupported { source });
                }
            }
            return Ok(());
        }

        if let Some(resolution) = self.resolve_constructor_path(call)? {
            match resolution {
                ConstructorPathResolution::TupleStruct(target) => {
                    relations.push(CallRelation::TupleStructConstructor {
                        source: call.id,
                        target,
                    });
                    statuses.push(CallResolutionStatus::Resolved {
                        source,
                        kind: CallResolutionKind::LocalExact,
                    });
                }
                ConstructorPathResolution::EnumVariant(target) => {
                    relations.push(CallRelation::EnumVariantConstructor {
                        source: call.id,
                        target,
                    });
                    statuses.push(CallResolutionStatus::Resolved {
                        source,
                        kind: CallResolutionKind::LocalExact,
                    });
                }
                ConstructorPathResolution::Unresolved => {
                    statuses.push(CallResolutionStatus::Unresolved { source });
                }
                ConstructorPathResolution::Ambiguous => {
                    statuses.push(CallResolutionStatus::Ambiguous { source });
                }
            }
            return Ok(());
        }

        let resolution = if self.is_unqualified_path(&call.path) {
            self.resolve_unqualified_local_function_path(call.owner, &call.path)?
        } else if self.is_explicit_local_path(&call.path) {
            self.resolve_local_function_path(call.owner, &call.path)?
        } else {
            self.resolve_implicit_local_function_path(call.owner, &call.path)?
        };

        let external_prelude = self.is_external_prelude_path(call.owner, &call.path)?;

        match resolution {
            LocalFunctionPathResolution::Resolved(target) => {
                relations.push(CallRelation::Function {
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
            LocalFunctionPathResolution::Unsupported if external_prelude => {
                statuses.push(CallResolutionStatus::External { source });
            }
            LocalFunctionPathResolution::Unsupported => {
                statuses.push(CallResolutionStatus::Unsupported { source });
            }
        }

        Ok(())
    }

    fn resolve_initialized_value_binding_call(
        &self,
        call: &PathCallNode,
        init_path: &[String],
        relations: &mut Vec<CallRelation>,
        statuses: &mut Vec<CallResolutionStatus>,
    ) -> Result<(), SynParserError> {
        let source = AnyCallSiteId::Path(call.id);

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
                relations.push(CallRelation::Function {
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
}
