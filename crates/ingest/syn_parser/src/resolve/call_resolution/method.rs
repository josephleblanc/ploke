use crate::{
    error::SynParserError,
    parser::{
        graph::GraphAccess,
        nodes::{
            AnyCallSiteId, CallBodyOwnerId, MethodCallNode, MethodCallReceiver, OrdinaryTypeUseId,
            StructNodeId,
        },
        relations::{CallRelation, CallResolutionKind, CallResolutionStatus, TypeRelation},
        types::TypeNode,
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

    fn is_external_literal_method(method_name: &str) -> bool {
        matches!(method_name, "to_string")
    }

    fn is_external_self_field_method(
        &self,
        call: &MethodCallNode,
        field_path: &[String],
        type_relations: &[TypeRelation],
    ) -> Result<bool, SynParserError> {
        let [field_name] = field_path else {
            return Ok(false);
        };
        let Some(field_type) = self.self_field_type(call.owner, field_name, type_relations)? else {
            return Ok(false);
        };
        self.is_external_type_method(call.owner, field_type, &call.method_name)
    }

    fn resolve_self_field_method_call(
        &self,
        call: &MethodCallNode,
        field_path: &[String],
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        let [field_name] = field_path else {
            return Ok(AssocPathResolution::Unsupported);
        };
        let Some(field_type) = self.self_field_type(call.owner, field_name, type_relations)? else {
            return Ok(AssocPathResolution::Unsupported);
        };
        self.resolve_type_use_method(call.owner, &[field_type], &call.method_name, type_relations)?
            .map_or(Ok(AssocPathResolution::Unsupported), Ok)
    }

    fn self_field_type(
        &self,
        owner: CallBodyOwnerId,
        field_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<Option<OrdinaryTypeUseId>, SynParserError> {
        let CallBodyOwnerId::Method(owner_method_id) = owner else {
            return Ok(None);
        };

        let Some(impl_id) = self.impl_for_owner_method(owner_method_id)? else {
            return Ok(None);
        };
        let Some(impl_node) = self.maybe_impl_node(impl_id) else {
            return Ok(None);
        };
        let Some(self_target) = self.impl_self_target(impl_node, type_relations)? else {
            return Ok(None);
        };
        let Ok(struct_id) = StructNodeId::try_from(self_target) else {
            return Ok(None);
        };
        let struct_node = self.graph.get_struct_checked(struct_id)?;

        let mut matches = struct_node
            .fields
            .iter()
            .filter(|field| {
                field.name.as_deref().is_some_and(|name| {
                    Self::struct_field_name_matches(name, field_name, &struct_node.name)
                })
            })
            .map(|field| field.type_id)
            .collect::<Vec<_>>();
        matches.sort_unstable();
        matches.dedup();

        Ok(match matches.as_slice() {
            [type_id] => Some(*type_id),
            _ => None,
        })
    }

    fn is_external_type_method(
        &self,
        owner: CallBodyOwnerId,
        type_id: OrdinaryTypeUseId,
        method_name: &str,
    ) -> Result<bool, SynParserError> {
        let TypeNode::Named(type_node) = self.type_node(type_id)? else {
            return Ok(false);
        };

        if !matches!(method_name, "len") {
            return Ok(false);
        }

        if self.is_external_path(&type_node.path) {
            return Ok(true);
        }

        match type_node.path.as_slice() {
            [segment] if matches!(segment.as_str(), "String" | "Vec") => {
                Ok(!self.local_segment_visible(owner, segment)?)
            }
            _ => Ok(false),
        }
    }

    fn is_external_type_path_method(
        &self,
        owner: CallBodyOwnerId,
        type_path: &[String],
        method_name: &str,
    ) -> Result<bool, SynParserError> {
        if !matches!(method_name, "len") {
            return Ok(false);
        }

        if self.is_external_path(type_path) || self.is_external_import_path(owner, type_path)? {
            return Ok(true);
        }

        match type_path {
            [segment] if matches!(segment.as_str(), "String" | "Vec") => {
                Ok(!self.local_segment_visible(owner, segment)?)
            }
            _ => Ok(false),
        }
    }

    fn is_external_path_result_method(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
        method_name: &str,
    ) -> Result<bool, SynParserError> {
        if !matches!(method_name, "unwrap" | "uuid") {
            return Ok(false);
        }

        Ok(self.is_external_path(path) || self.is_external_import_path(owner, path)?)
    }

    pub(super) fn resolve_self_method_call(
        &self,
        call: &MethodCallNode,
    ) -> Result<AssocPathResolution, SynParserError> {
        let CallBodyOwnerId::Method(owner_method_id) = call.owner else {
            return Ok(AssocPathResolution::Unsupported);
        };

        if let Some(impl_id) = self.impl_for_owner_method(owner_method_id)? {
            let Some(impl_node) = self.maybe_impl_node(impl_id) else {
                return Ok(AssocPathResolution::Unsupported);
            };
            return Ok(self.resolve_method_in_impl(impl_node, &call.method_name));
        }

        if let Some(trait_id) = self.trait_for_owner_method(owner_method_id)? {
            let trait_node = self.graph.get_trait_checked(trait_id)?;
            return Ok(self.resolve_instance_method_in_trait(trait_node, &call.method_name));
        }

        Ok(AssocPathResolution::Unsupported)
    }

    pub(super) fn resolve_param_method_call(
        &self,
        call: &MethodCallNode,
        name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        let Some(parameters) = self.owner_parameters(call.owner)? else {
            return Ok(AssocPathResolution::Unsupported);
        };

        let params = parameters
            .iter()
            .filter(|param| !param.is_self && param.name.as_deref() == Some(name))
            .collect::<Vec<_>>();
        if params.is_empty() {
            return Ok(AssocPathResolution::Unsupported);
        }

        let type_ids = params.iter().map(|param| param.type_id).collect::<Vec<_>>();
        if let Some(resolution) =
            self.resolve_type_use_method(call.owner, &type_ids, &call.method_name, type_relations)?
        {
            return Ok(resolution);
        }

        let mut dereferenced_ids = Vec::new();
        for type_id in &type_ids {
            if let Some(type_id) = self.dereferenced_type_use(*type_id)? {
                dereferenced_ids.push(type_id);
            }
        }
        dereferenced_ids.sort_unstable();
        dereferenced_ids.dedup();
        if let Some(resolution) = self.resolve_type_use_method(
            call.owner,
            &dereferenced_ids,
            &call.method_name,
            type_relations,
        )? {
            return Ok(resolution);
        }

        self.resolve_bound_method_from_types(&type_ids, &call.method_name, type_relations)?
            .map_or(Ok(AssocPathResolution::Unresolved), Ok)
    }

    pub(super) fn resolve_dereferenced_param_method_call(
        &self,
        call: &MethodCallNode,
        name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        let Some(parameters) = self.owner_parameters(call.owner)? else {
            return Ok(AssocPathResolution::Unsupported);
        };

        let params = parameters
            .iter()
            .filter(|param| !param.is_self && param.name.as_deref() == Some(name))
            .collect::<Vec<_>>();
        if params.is_empty() {
            return Ok(AssocPathResolution::Unsupported);
        }

        let mut dereferenced_types = Vec::new();
        for param in params {
            if let Some(type_id) = self.dereferenced_type_use(param.type_id)? {
                dereferenced_types.push(type_id);
            }
        }
        dereferenced_types.sort_unstable();
        dereferenced_types.dedup();
        if dereferenced_types.is_empty() {
            return Ok(AssocPathResolution::Unsupported);
        }

        if let Some(resolution) = self.resolve_type_use_method(
            call.owner,
            &dereferenced_types,
            &call.method_name,
            type_relations,
        )? {
            return Ok(resolution);
        }

        self.resolve_bound_method_from_types(
            &dereferenced_types,
            &call.method_name,
            type_relations,
        )?
        .map_or(Ok(AssocPathResolution::Unresolved), Ok)
    }

    fn dereferenced_type_use(
        &self,
        type_id: OrdinaryTypeUseId,
    ) -> Result<Option<OrdinaryTypeUseId>, SynParserError> {
        match self.type_node(type_id)? {
            TypeNode::Reference(node) => Ok(Some(node.referenced)),
            TypeNode::Paren(node) => self.dereferenced_type_use(node.inner),
            _ => Ok(None),
        }
    }
}
