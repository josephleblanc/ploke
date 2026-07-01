use crate::{
    error::SynParserError,
    parser::{
        graph::GraphAccess,
        nodes::{
            AnyCallSiteId, CallBodyOwnerId, CallNode, MethodCallNode, MethodCallReceiver,
            OrdinaryTypeSourceId, OrdinaryTypeTargetId, OrdinaryTypeUseId, StructNodeId,
        },
        relations::{CallRelation, CallResolutionKind, CallResolutionStatus, TypeRelation},
        types::TypeNode,
    },
};

use super::{
    AssocPathResolution, CallRelationResolver, LocalFunctionPathResolution, LocalTraitResolution,
    LocalTypeResolution,
};

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
        if let MethodCallReceiver::InitializedLocalBinding { init_path, .. }
        | MethodCallReceiver::BorrowedInitializedLocalBinding { init_path, .. } = &call.receiver
            && self.is_external_type_path_method(call.owner, init_path, &call.method_name)?
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
            MethodCallReceiver::BorrowedInitializedLocalBinding { init_path, .. } => {
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
            MethodCallReceiver::IfBranchPaths { paths } => {
                self.resolve_branch_receiver_method_call(call, paths, type_relations)?
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
            | MethodCallReceiver::Literal
            | MethodCallReceiver::Unsupported => AssocPathResolution::Unsupported,
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

    fn struct_field_name_matches(actual: &str, expected: &str, struct_name: &str) -> bool {
        if actual == expected {
            return true;
        }

        let marker = format!("unnamed_field{struct_name}");
        actual
            .rsplit_once(&marker)
            .is_some_and(|(prefix, _)| prefix == expected)
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

        if !matches!(method_name, "extensions_mut" | "len") {
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
        if !matches!(method_name, "extensions_mut" | "len") {
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

    fn resolve_typed_local_method_call(
        &self,
        call: &MethodCallNode,
        type_path: &[String],
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        if type_path.is_empty() || self.is_external_path(type_path) {
            return Ok(AssocPathResolution::Unsupported);
        }

        let resolution = if type_path.len() == 1 || self.is_explicit_local_path(type_path) {
            self.resolve_local_type_path(call.owner, type_path)?
        } else {
            return Ok(AssocPathResolution::Unsupported);
        };

        match resolution {
            LocalTypeResolution::Resolved(target) => self
                .resolve_type_instance_method(
                    call.owner,
                    target,
                    &call.method_name,
                    type_relations,
                )?
                .map_or(Ok(AssocPathResolution::Unsupported), Ok),
            LocalTypeResolution::Unresolved => {
                self.resolve_typed_local_trait_method_call(call.owner, type_path, &call.method_name)
            }
            LocalTypeResolution::Ambiguous => Ok(AssocPathResolution::Ambiguous),
        }
    }

    fn resolve_typed_local_trait_method_call(
        &self,
        owner: CallBodyOwnerId,
        trait_path: &[String],
        method_name: &str,
    ) -> Result<AssocPathResolution, SynParserError> {
        let [trait_segment] = trait_path else {
            return Ok(AssocPathResolution::Unsupported);
        };

        match self.resolve_local_trait_segment(owner, trait_segment)? {
            LocalTraitResolution::Resolved(trait_id) => {
                let trait_node = self.graph.get_trait_checked(trait_id)?;
                Ok(self.resolve_instance_method_in_trait(trait_node, method_name))
            }
            LocalTraitResolution::Unresolved => Ok(AssocPathResolution::Unresolved),
            LocalTraitResolution::Ambiguous => Ok(AssocPathResolution::Ambiguous),
        }
    }

    fn resolve_branch_receiver_method_call(
        &self,
        call: &MethodCallNode,
        paths: &[Vec<String>],
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        if paths.is_empty() {
            return Ok(AssocPathResolution::Unsupported);
        }

        let mut targets = Vec::new();
        let mut unresolved = false;
        for path in paths {
            match self.resolve_typed_local_method_call(call, path, type_relations)? {
                AssocPathResolution::Resolved(target) => targets.push(target),
                AssocPathResolution::Unresolved => unresolved = true,
                AssocPathResolution::Ambiguous => return Ok(AssocPathResolution::Ambiguous),
                AssocPathResolution::Unsupported => return Ok(AssocPathResolution::Unsupported),
            }
        }

        targets.sort_unstable();
        targets.dedup();

        Ok(match (targets.as_slice(), unresolved) {
            ([target], false) => AssocPathResolution::Resolved(*target),
            ([], true) => AssocPathResolution::Unresolved,
            _ => AssocPathResolution::Ambiguous,
        })
    }

    fn resolve_path_result_method_call(
        &self,
        call: &MethodCallNode,
        path: &[String],
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        if path.is_empty()
            || self.is_external_path(path)
            || self.is_external_import_path(call.owner, path)?
        {
            return Ok(AssocPathResolution::Unsupported);
        }

        let resolution = if self.is_unqualified_path(path) {
            self.resolve_unqualified_local_function_path(call.owner, path)?
        } else if self.is_explicit_local_path(path) {
            self.resolve_local_function_path(call.owner, path)?
        } else {
            self.resolve_implicit_local_function_path(call.owner, path)?
        };

        match resolution {
            LocalFunctionPathResolution::Resolved(function_id) => self
                .resolve_function_return_type_method(
                    call.owner,
                    function_id,
                    &call.method_name,
                    type_relations,
                ),
            LocalFunctionPathResolution::Unresolved => Ok(AssocPathResolution::Unresolved),
            LocalFunctionPathResolution::Ambiguous => Ok(AssocPathResolution::Ambiguous),
            LocalFunctionPathResolution::Unsupported => Ok(AssocPathResolution::Unsupported),
        }
    }

    fn resolve_try_path_result_method_call(
        &self,
        call: &MethodCallNode,
        path: &[String],
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        if path.is_empty()
            || self.is_external_path(path)
            || self.is_external_import_path(call.owner, path)?
        {
            return Ok(AssocPathResolution::Unsupported);
        }

        let resolution = if self.is_unqualified_path(path) {
            self.resolve_unqualified_local_function_path(call.owner, path)?
        } else if self.is_explicit_local_path(path) {
            self.resolve_local_function_path(call.owner, path)?
        } else {
            self.resolve_implicit_local_function_path(call.owner, path)?
        };

        match resolution {
            LocalFunctionPathResolution::Resolved(function_id) => self
                .resolve_result_ok_return_type_method(
                    call.owner,
                    function_id,
                    &call.method_name,
                    type_relations,
                ),
            LocalFunctionPathResolution::Unresolved => Ok(AssocPathResolution::Unresolved),
            LocalFunctionPathResolution::Ambiguous => Ok(AssocPathResolution::Ambiguous),
            LocalFunctionPathResolution::Unsupported => Ok(AssocPathResolution::Unsupported),
        }
    }

    fn resolve_method_result_method_call(
        &self,
        call: &MethodCallNode,
        inner_method_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        let Some(inner_call) = self.direct_inner_method_call(call, inner_method_name) else {
            return Ok(AssocPathResolution::Unsupported);
        };

        let inner_resolution = self.resolve_method_call_target(inner_call, type_relations)?;
        match inner_resolution {
            AssocPathResolution::Resolved(method_id) => self.resolve_method_return_type_method(
                call.owner,
                method_id,
                &call.method_name,
                type_relations,
            ),
            AssocPathResolution::Unresolved => Ok(AssocPathResolution::Unresolved),
            AssocPathResolution::Ambiguous => Ok(AssocPathResolution::Ambiguous),
            AssocPathResolution::Unsupported => Ok(AssocPathResolution::Unsupported),
        }
    }

    fn direct_inner_method_call(
        &self,
        call: &MethodCallNode,
        inner_method_name: &str,
    ) -> Option<&MethodCallNode> {
        let mut candidates = self
            .graph
            .call_sites()
            .iter()
            .filter_map(|candidate| match candidate {
                CallNode::MethodCall(inner)
                    if inner.owner == call.owner
                        && inner.method_name == inner_method_name
                        && inner.span.0 == call.span.0
                        && inner.span.1 < call.span.1 =>
                {
                    Some(inner)
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        let max_end = candidates.iter().map(|inner| inner.span.1).max()?;
        candidates.retain(|inner| inner.span.1 == max_end);
        match candidates.as_slice() {
            [inner] => Some(*inner),
            _ => None,
        }
    }

    fn resolve_method_call_target(
        &self,
        call: &MethodCallNode,
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        match &call.receiver {
            MethodCallReceiver::SelfValue => self.resolve_self_method_call(call),
            MethodCallReceiver::SelfField { .. } => Ok(AssocPathResolution::Unsupported),
            MethodCallReceiver::LocalBinding { name } => {
                self.resolve_param_method_call(call, name, type_relations)
            }
            MethodCallReceiver::TypedLocalBinding { type_path, .. } => {
                self.resolve_typed_local_method_call(call, type_path, type_relations)
            }
            MethodCallReceiver::InitializedLocalBinding { init_path, .. } => {
                self.resolve_typed_local_method_call(call, init_path, type_relations)
            }
            MethodCallReceiver::BorrowedInitializedLocalBinding { init_path, .. } => {
                self.resolve_typed_local_method_call(call, init_path, type_relations)
            }
            MethodCallReceiver::BorrowedTypedLocalBinding { type_path, .. } => {
                self.resolve_typed_local_method_call(call, type_path, type_relations)
            }
            MethodCallReceiver::DereferencedInitializedLocalBinding { init_path, .. } => {
                self.resolve_typed_local_method_call(call, init_path, type_relations)
            }
            MethodCallReceiver::PathCallResult { path } => {
                self.resolve_path_result_method_call(call, path, type_relations)
            }
            MethodCallReceiver::AwaitPathCallResult { path } => {
                self.resolve_path_result_method_call(call, path, type_relations)
            }
            MethodCallReceiver::TryPathCallResult { path } => {
                self.resolve_try_path_result_method_call(call, path, type_relations)
            }
            MethodCallReceiver::IfBranchPaths { paths } => {
                self.resolve_branch_receiver_method_call(call, paths, type_relations)
            }
            MethodCallReceiver::MethodCallResult { method_name } => {
                self.resolve_method_result_method_call(call, method_name, type_relations)
            }
            MethodCallReceiver::FieldTypedLocalBinding {
                type_path,
                field_path,
                ..
            } => self.resolve_field_local_method_call(call, type_path, field_path, type_relations),
            MethodCallReceiver::FieldInitializedLocalBinding {
                init_path,
                field_path,
                ..
            } => self.resolve_field_local_method_call(call, init_path, field_path, type_relations),
            MethodCallReceiver::DereferencedLocalBinding { name } => {
                self.resolve_dereferenced_param_method_call(call, name, type_relations)
            }
            MethodCallReceiver::BorrowedLocalBinding { .. }
            | MethodCallReceiver::FieldLocalBinding { .. }
            | MethodCallReceiver::AwaitResult
            | MethodCallReceiver::TryResult
            | MethodCallReceiver::Literal
            | MethodCallReceiver::Unsupported => Ok(AssocPathResolution::Unsupported),
        }
    }

    fn resolve_field_local_method_call(
        &self,
        call: &MethodCallNode,
        root_path: &[String],
        field_path: &[String],
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        if root_path.is_empty()
            || self.is_external_path(root_path)
            || self.is_external_import_path(call.owner, root_path)?
        {
            return Ok(AssocPathResolution::Unsupported);
        }

        let root_resolution = if root_path.len() == 1 || self.is_explicit_local_path(root_path) {
            self.resolve_local_type_path(call.owner, root_path)?
        } else {
            return Ok(AssocPathResolution::Unsupported);
        };

        match root_resolution {
            LocalTypeResolution::Resolved(root_target) => self.resolve_field_type_method(
                call.owner,
                root_target,
                field_path,
                &call.method_name,
                type_relations,
            ),
            LocalTypeResolution::Unresolved => Ok(AssocPathResolution::Unresolved),
            LocalTypeResolution::Ambiguous => Ok(AssocPathResolution::Ambiguous),
        }
    }

    fn resolve_field_type_method(
        &self,
        owner: CallBodyOwnerId,
        root_target: OrdinaryTypeTargetId,
        field_path: &[String],
        method_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        let Some(field_type) = self.field_type(root_target, field_path)? else {
            return Ok(AssocPathResolution::Unsupported);
        };
        let Ok(source) = OrdinaryTypeSourceId::try_from(field_type) else {
            return Ok(AssocPathResolution::Unsupported);
        };

        let mut targets = type_relations
            .iter()
            .filter_map(|relation| match relation {
                TypeRelation::Ordinary {
                    source: relation_source,
                    target,
                } if *relation_source == source => Some(*target),
                _ => None,
            })
            .collect::<Vec<_>>();
        targets.sort_unstable();
        targets.dedup();

        match targets.as_slice() {
            [target] => self
                .resolve_type_instance_method(owner, *target, method_name, type_relations)?
                .map_or(Ok(AssocPathResolution::Unsupported), Ok),
            [] => Ok(AssocPathResolution::Unsupported),
            _ => Ok(AssocPathResolution::Ambiguous),
        }
    }

    fn field_type(
        &self,
        root_target: OrdinaryTypeTargetId,
        field_path: &[String],
    ) -> Result<Option<OrdinaryTypeUseId>, SynParserError> {
        let [field_name] = field_path else {
            return Ok(None);
        };
        let Ok(struct_id) = StructNodeId::try_from(root_target) else {
            return Ok(None);
        };
        let struct_node = self.graph.get_struct_checked(struct_id)?;

        if let Ok(field_idx) = field_name.parse::<usize>() {
            return Ok(struct_node.fields.get(field_idx).map(|field| field.type_id));
        }

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
}
