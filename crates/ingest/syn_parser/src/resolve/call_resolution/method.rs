use crate::{
    error::SynParserError,
    parser::{
        nodes::{AnyCallSiteId, MethodCallNode, MethodCallReceiver},
        relations::{CallRelation, CallResolutionKind, CallResolutionStatus, TypeRelation},
    },
};

use super::{AssocPathResolution, CallRelationResolver};

impl CallRelationResolver<'_> {
    pub(super) fn resolve_method_call(
        &self,
        call: &MethodCallNode,
        type_relations: &[TypeRelation],
        relations: &mut Vec<CallRelation>,
        statuses: &mut Vec<CallResolutionStatus>,
    ) -> Result<(), SynParserError> {
        let source = AnyCallSiteId::Method(call.id);

        if matches!(call.receiver, MethodCallReceiver::Literal)
            && Self::is_external_literal_method(&call.method_name)
        {
            statuses.push(CallResolutionStatus::External { source });
            return Ok(());
        }
        if let MethodCallReceiver::PathCallResult { path } = &call.receiver
            && self.is_external_path_result_method(call.owner, path, &call.method_name)?
        {
            statuses.push(CallResolutionStatus::External { source });
            return Ok(());
        }
        if let MethodCallReceiver::SelfField { field_path } = &call.receiver
            && self.is_external_self_field_method(call, field_path, type_relations)?
        {
            statuses.push(CallResolutionStatus::External { source });
            return Ok(());
        }
        if let MethodCallReceiver::TypedLocalBinding { type_path, .. }
        | MethodCallReceiver::BorrowedTypedLocalBinding { type_path, .. } = &call.receiver
            && self.is_external_type_path_method(call.owner, type_path, &call.method_name)?
        {
            statuses.push(CallResolutionStatus::External { source });
            return Ok(());
        }

        let resolution = match &call.receiver {
            MethodCallReceiver::SelfValue => self.resolve_self_method_call(call)?,
            MethodCallReceiver::SelfField { field_path } => {
                self.resolve_self_field_method_call(call, field_path, type_relations)?
            }
            MethodCallReceiver::LocalBinding { name } => {
                self.resolve_param_method_call(call, name, type_relations)?
            }
            MethodCallReceiver::TypedLocalBinding { type_path, .. } => {
                self.resolve_typed_local_method_call(call, type_path, type_relations)?
            }
            MethodCallReceiver::InitializedLocalBinding { init_path, .. } => {
                self.resolve_typed_local_method_call(call, init_path, type_relations)?
            }
            MethodCallReceiver::BorrowedTypedLocalBinding { type_path, .. } => {
                self.resolve_typed_local_method_call(call, type_path, type_relations)?
            }
            MethodCallReceiver::DereferencedInitializedLocalBinding { init_path, .. } => {
                self.resolve_typed_local_method_call(call, init_path, type_relations)?
            }
            MethodCallReceiver::PathCallResult { path } => {
                self.resolve_path_result_method_call(call, path, type_relations)?
            }
            MethodCallReceiver::AwaitPathCallResult { path } => {
                self.resolve_path_result_method_call(call, path, type_relations)?
            }
            MethodCallReceiver::TryPathCallResult { path } => {
                self.resolve_try_path_result_method_call(call, path, type_relations)?
            }
            MethodCallReceiver::MethodCallResult { method_name } => {
                self.resolve_method_result_method_call(call, method_name, type_relations)?
            }
            MethodCallReceiver::FieldTypedLocalBinding {
                type_path,
                field_path,
                ..
            } => {
                self.resolve_field_local_method_call(call, type_path, field_path, type_relations)?
            }
            MethodCallReceiver::FieldInitializedLocalBinding {
                init_path,
                field_path,
                ..
            } => {
                self.resolve_field_local_method_call(call, init_path, field_path, type_relations)?
            }
            MethodCallReceiver::DereferencedLocalBinding { name } => {
                self.resolve_dereferenced_param_method_call(call, name, type_relations)?
            }
            MethodCallReceiver::BorrowedLocalBinding { .. }
            | MethodCallReceiver::FieldLocalBinding { .. }
            | MethodCallReceiver::AwaitResult
            | MethodCallReceiver::TryResult
            | MethodCallReceiver::Literal => AssocPathResolution::Unsupported,
        };

        match resolution {
            AssocPathResolution::Resolved(target) => {
                relations.push(CallRelation::Method {
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

        Ok(())
    }
}
