use crate::{
    error::SynParserError,
    parser::{
        graph::GraphAccess,
        nodes::{
            AnyCallSiteId, AsAnyNodeId, CallBodyOwnerId, CallNode, FieldNode, FunctionNodeId,
            MethodCallNode, MethodCallReceiver, OrdinaryTypeSourceId, OrdinaryTypeTargetId,
            OrdinaryTypeUseId, StructNodeId, TraitTypeSourceId, TypeAliasNodeId,
            TypeGenericParamNodeId,
        },
        relations::{CallRelation, CallResolutionKind, CallResolutionStatus, TypeRelation},
        types::TypeNode,
    },
};

use super::{
    AssocPathResolution, CallRelationResolver, LocalFunctionPathResolution, LocalTraitResolution,
    LocalTypeResolution, WorkspaceTypeResolution, trait_declares_instance_method,
};

#[derive(Debug, Clone, Copy)]
struct SelfFieldTypeInfo {
    declared: OrdinaryTypeUseId,
    impl_arg: Option<OrdinaryTypeUseId>,
}

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
        if let MethodCallReceiver::MethodCallResult { method_name } = &call.receiver
            && self.is_external_method_result_method(call.owner, method_name, &call.method_name)?
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
        if let MethodCallReceiver::LocalBinding { name }
        | MethodCallReceiver::BorrowedLocalBinding { name } = &call.receiver
            && self.is_external_param_method_call(call, name, type_relations)?
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
            MethodCallReceiver::BorrowedLocalBinding { name } => {
                self.resolve_param_method_call(call, name, type_relations)?
            }
            MethodCallReceiver::TypedLocalBinding { type_path, .. } => {
                self.resolve_typed_local_method_call(call, type_path, type_relations)?
            }
            MethodCallReceiver::InitializedLocalBinding { init_path, .. } => {
                self.resolve_typed_local_method_call(call, init_path, type_relations)?
            }
            MethodCallReceiver::TupleReturnBinding { path, index, .. } => {
                self.resolve_tuple_return_method_call(call, path, *index, type_relations)?
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
            MethodCallReceiver::TryMethodCallResult { method_name } => {
                self.resolve_try_method_result_method_call(call, method_name, type_relations)?
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
            MethodCallReceiver::FieldLocalBinding { .. }
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
        if self.is_external_executable_self_field_method(
            call.owner,
            field_path,
            &call.method_name,
        )? {
            return Ok(true);
        }

        let [field_name] = field_path else {
            return Ok(false);
        };
        let Some(field_type) = self.self_field_type_info(call.owner, field_name, type_relations)?
        else {
            return Ok(false);
        };
        if self.is_external_type_method(
            call.owner,
            field_type.declared,
            &call.method_name,
            type_relations,
        )? {
            return Ok(true);
        }

        let mut candidates = vec![field_type.declared];
        if let Some(impl_arg) = field_type.impl_arg {
            candidates.push(impl_arg);
        }

        for field_type in candidates {
            if self.is_external_bound_self_field_method(
                call.owner,
                field_type,
                &call.method_name,
                type_relations,
            )? {
                return Ok(true);
            }
        }

        Ok(false)
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
        Ok(self
            .self_field_type_info(owner, field_name, type_relations)?
            .map(|field_type| field_type.declared))
    }

    fn self_field_type_info(
        &self,
        owner: CallBodyOwnerId,
        field_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<Option<SelfFieldTypeInfo>, SynParserError> {
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

        let Some(declared) =
            Self::struct_field_type(&struct_node.fields, field_name, &struct_node.name)
        else {
            return Ok(None);
        };

        Ok(Some(SelfFieldTypeInfo {
            declared,
            impl_arg: self.impl_self_arg_for_struct_field(
                impl_node,
                struct_node,
                declared,
                type_relations,
            )?,
        }))
    }

    fn impl_self_arg_for_struct_field(
        &self,
        impl_node: &crate::parser::nodes::ImplNode,
        struct_node: &crate::parser::nodes::StructNode,
        field_type: OrdinaryTypeUseId,
        type_relations: &[TypeRelation],
    ) -> Result<Option<OrdinaryTypeUseId>, SynParserError> {
        let Some(field_target) = self.single_ordinary_target(field_type, type_relations)? else {
            return Ok(None);
        };
        let Ok(field_param) = TypeGenericParamNodeId::try_from(field_target) else {
            return Ok(None);
        };
        let Some(field_index) = struct_node.generic_params.iter().position(|param| {
            TypeGenericParamNodeId::try_refine(param.id, &param.kind).ok() == Some(field_param)
        }) else {
            return Ok(None);
        };

        let TypeNode::Named(self_type) = self.type_node(impl_node.self_type)? else {
            return Ok(None);
        };
        Ok(self_type.arguments.get(field_index).copied())
    }

    fn struct_field_type(
        fields: &[FieldNode],
        field_name: &str,
        struct_name: &str,
    ) -> Option<OrdinaryTypeUseId> {
        if let Ok(field_idx) = field_name.parse::<usize>() {
            return fields.get(field_idx).map(|field| field.type_id);
        }

        let mut matches = fields
            .iter()
            .filter(|field| {
                field.name.as_deref().is_some_and(|name| {
                    Self::struct_field_name_matches(name, field_name, struct_name)
                })
            })
            .map(|field| field.type_id)
            .collect::<Vec<_>>();
        matches.sort_unstable();
        matches.dedup();

        match matches.as_slice() {
            [type_id] => Some(*type_id),
            _ => None,
        }
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
        type_relations: &[TypeRelation],
    ) -> Result<bool, SynParserError> {
        if method_name == "is_empty" {
            return self.type_use_is_slice(type_id);
        }

        let TypeNode::Named(type_node) = self.type_node(type_id)? else {
            return Ok(false);
        };

        if !matches!(
            method_name,
            "extensions_mut" | "len" | "poll_ready" | "size_hint" | "oneshot"
        ) {
            return Ok(false);
        }

        if method_name == "oneshot" && !self.has_external_service_ext_import(owner)? {
            return Ok(false);
        }

        if self.receiver_type_is_external(owner, type_id, type_relations)? {
            return Ok(true);
        }

        if method_name == "size_hint" {
            return Ok(false);
        }

        match type_node.path.as_slice() {
            [segment] if matches!(segment.as_str(), "String" | "Vec") => {
                Ok(!self.local_segment_visible(owner, segment)?)
            }
            _ => Ok(false),
        }
    }

    fn type_use_is_slice(&self, type_id: OrdinaryTypeUseId) -> Result<bool, SynParserError> {
        match self.type_node(type_id)? {
            TypeNode::Slice(_) => Ok(true),
            TypeNode::Reference(node) => self.type_use_is_slice(node.referenced),
            TypeNode::Paren(node) => self.type_use_is_slice(node.inner),
            _ => Ok(false),
        }
    }

    fn is_external_bound_self_field_method(
        &self,
        owner: CallBodyOwnerId,
        field_type: OrdinaryTypeUseId,
        method_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<bool, SynParserError> {
        if method_name != "poll_ready" {
            return Ok(false);
        }

        let Some(target) = self.single_ordinary_target(field_type, type_relations)? else {
            return Ok(false);
        };
        let Ok(param_id) = TypeGenericParamNodeId::try_from(target) else {
            return Ok(false);
        };

        let sources = self.generic_bound_sources(owner, target, Some(param_id), type_relations)?;
        for source in sources {
            if self.is_external_service_trait_bound(owner, source)? {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn has_external_service_ext_import(
        &self,
        owner: CallBodyOwnerId,
    ) -> Result<bool, SynParserError> {
        self.is_external_import_path(owner, &["ServiceExt".to_string()])
    }

    fn is_external_service_trait_bound(
        &self,
        owner: CallBodyOwnerId,
        source: TraitTypeSourceId,
    ) -> Result<bool, SynParserError> {
        let path = match self.type_node(source)? {
            TypeNode::Named(node) => &node.path,
            TypeNode::TraitBound(node) => &node.path,
            _ => return Ok(false),
        };

        self.is_external_service_path(owner, path)
    }

    fn is_external_service_path(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
    ) -> Result<bool, SynParserError> {
        if !path.last().is_some_and(|segment| segment == "Service") {
            return Ok(false);
        }
        if self.is_external_path(path) {
            return Ok(true);
        }
        if self.is_external_import_path(owner, path)? {
            return Ok(!self.service_trait_is_local(owner, path)?);
        }

        let [segment] = path else {
            return Ok(false);
        };
        let Some(module_id) = self.containing_module_for_owner(owner) else {
            return Ok(false);
        };
        let module_id = self.import_scope_module(module_id)?;
        let paths = self.segment_external_paths(module_id, segment, 0)?;
        if paths.is_empty() || self.service_trait_is_local(owner, path)? {
            return Ok(false);
        }

        Ok(paths
            .iter()
            .all(|path| path.last().is_some_and(|segment| segment == "Service")))
    }

    fn service_trait_is_local(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
    ) -> Result<bool, SynParserError> {
        let [segment] = path else {
            return Ok(false);
        };

        Ok(!matches!(
            self.resolve_local_trait_segment(owner, segment)?,
            LocalTraitResolution::Unresolved
        ))
    }

    fn is_external_executable_self_field_method(
        &self,
        owner: CallBodyOwnerId,
        field_path: &[String],
        method_name: &str,
    ) -> Result<bool, SynParserError> {
        if method_name != "poll_ready" || field_path != ["0"] {
            return Ok(false);
        }

        let CallBodyOwnerId::Executable(id) = owner else {
            return Ok(false);
        };
        let body = self
            .graph
            .executable_bodies()
            .iter()
            .find(|body| body.id == id)
            .ok_or_else(|| {
                SynParserError::InternalState(format!(
                    "call resolution found missing executable body {id} during external self-field method lookup"
                ))
            })?;
        if !body
            .label
            .as_deref()
            .is_some_and(|label| label.starts_with("local_impl_method:"))
        {
            return Ok(false);
        }

        for predicate in &body.where_predicates {
            if predicate.subject_path.len() != 1 {
                continue;
            }
            for bound in &predicate.trait_bounds {
                if !bound.last().is_some_and(|segment| segment == "Service") {
                    continue;
                }
                if self.is_external_service_path(owner, bound)? {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    fn receiver_type_is_external(
        &self,
        owner: CallBodyOwnerId,
        type_id: OrdinaryTypeUseId,
        type_relations: &[TypeRelation],
    ) -> Result<bool, SynParserError> {
        if self.type_use_is_external(owner, type_id)? {
            return Ok(true);
        }

        let Ok(source) = OrdinaryTypeSourceId::try_from(type_id) else {
            return Ok(false);
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

        for target in targets {
            for receiver in self.ordinary_receiver_targets(target, type_relations)? {
                let Ok(alias_id) = TypeAliasNodeId::try_from(receiver) else {
                    continue;
                };
                let alias_node = self.graph.get_type_alias_checked(alias_id)?;
                if self.type_use_is_external(owner, alias_node.type_id)? {
                    return Ok(true);
                }
            }
        }

        if self.workspace_type_use_alias_is_external(owner, type_id)? {
            return Ok(true);
        }

        Ok(false)
    }

    fn workspace_type_use_alias_is_external(
        &self,
        owner: CallBodyOwnerId,
        type_id: OrdinaryTypeUseId,
    ) -> Result<bool, SynParserError> {
        let resolution = match self.type_node(type_id)? {
            TypeNode::Named(node) => match node.path.as_slice() {
                [] => WorkspaceTypeResolution::Unresolved,
                [segment] => self.resolve_workspace_type_import(owner, segment)?,
                path => self.resolve_workspace_type_path(owner, path)?,
            },
            TypeNode::Reference(node) => {
                return self.workspace_type_use_alias_is_external(owner, node.referenced);
            }
            TypeNode::Paren(node) => {
                return self.workspace_type_use_alias_is_external(owner, node.inner);
            }
            _ => return Ok(false),
        };

        match resolution {
            WorkspaceTypeResolution::Resolved(candidate) => {
                Self::workspace_type_target_alias_is_external(candidate)
            }
            WorkspaceTypeResolution::Unresolved | WorkspaceTypeResolution::Ambiguous => Ok(false),
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

    fn is_external_param_method_call(
        &self,
        call: &MethodCallNode,
        name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<bool, SynParserError> {
        let Some(parameters) = self.owner_parameters(call.owner)? else {
            return Ok(false);
        };

        for param in parameters
            .iter()
            .filter(|param| !param.is_self && param.name.as_deref() == Some(name))
        {
            let direct_external = self.is_external_type_method(
                call.owner,
                param.type_id,
                &call.method_name,
                type_relations,
            )?;
            let dereferenced_external =
                if let Some(type_id) = self.dereferenced_type_use(param.type_id)? {
                    self.is_external_type_method(
                        call.owner,
                        type_id,
                        &call.method_name,
                        type_relations,
                    )?
                } else {
                    false
                };
            if direct_external || dereferenced_external {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn is_external_method_result_method(
        &self,
        owner: CallBodyOwnerId,
        result_method: &str,
        method_name: &str,
    ) -> Result<bool, SynParserError> {
        if result_method != "clone" || method_name != "oneshot" {
            return Ok(false);
        }

        self.has_external_service_ext_import(owner)
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
            if !matches!(resolution, AssocPathResolution::Unsupported) {
                return Ok(resolution);
            }
            if let Some(external_resolution) = self.resolve_external_trait_method_from_types(
                call.owner,
                &type_ids,
                &call.method_name,
                type_relations,
            )? {
                return Ok(external_resolution);
            }
            return Ok(resolution);
        }

        if let Some(external_resolution) = self.resolve_external_trait_method_from_types(
            call.owner,
            &type_ids,
            &call.method_name,
            type_relations,
        )? {
            return Ok(external_resolution);
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
            if !matches!(resolution, AssocPathResolution::Unsupported) {
                return Ok(resolution);
            }
            if let Some(external_resolution) = self.resolve_external_trait_method_from_types(
                call.owner,
                &dereferenced_ids,
                &call.method_name,
                type_relations,
            )? {
                return Ok(external_resolution);
            }
            return Ok(resolution);
        }

        if let Some(external_resolution) = self.resolve_external_trait_method_from_types(
            call.owner,
            &dereferenced_ids,
            &call.method_name,
            type_relations,
        )? {
            return Ok(external_resolution);
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

    fn resolve_external_trait_method_from_types(
        &self,
        owner: CallBodyOwnerId,
        type_ids: &[OrdinaryTypeUseId],
        method_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<Option<AssocPathResolution>, SynParserError> {
        let Some(receiver_path) = self.external_receiver_path(owner, type_ids)? else {
            return Ok(None);
        };

        let mut candidates = Vec::new();
        let mut matched = false;
        for impl_node in self
            .graph
            .impls()
            .iter()
            .filter(|impl_node| impl_node.trait_type.is_some())
        {
            let Some(module_id) = self.module_for_node(impl_node.id.as_any()) else {
                continue;
            };
            let Some(self_path) = self.external_type_path(module_id, impl_node.self_type)? else {
                continue;
            };
            if self_path != receiver_path {
                continue;
            }

            let Some(trait_node) = self.local_trait_node_for_impl(impl_node, type_relations)?
            else {
                continue;
            };
            if !trait_declares_instance_method(trait_node, method_name) {
                continue;
            }
            if !self.trait_is_visible_from_owner(owner, trait_node.id)? {
                continue;
            }

            matched = true;
            candidates.extend(
                impl_node
                    .methods
                    .iter()
                    .filter(|method| {
                        method.name == method_name
                            && method.parameters.iter().any(|param| param.is_self)
                    })
                    .map(|method| method.id),
            );
        }

        Ok(matched.then(|| Self::method_resolution(candidates)))
    }

    fn external_receiver_path(
        &self,
        owner: CallBodyOwnerId,
        type_ids: &[OrdinaryTypeUseId],
    ) -> Result<Option<Vec<String>>, SynParserError> {
        let Some(module_id) = self.containing_module_for_owner(owner) else {
            return Ok(None);
        };

        let mut paths = Vec::new();
        for type_id in type_ids {
            if let Some(path) = self.external_type_path(module_id, *type_id)? {
                paths.push(path);
            }
        }
        paths.sort();
        paths.dedup();

        Ok(match paths.as_slice() {
            [path] => Some(path.clone()),
            [] | [_, ..] => None,
        })
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

    fn resolve_try_method_result_method_call(
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
            AssocPathResolution::Resolved(method_id) => self
                .resolve_method_result_ok_return_type_method(
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
            MethodCallReceiver::SelfField { field_path } => {
                self.resolve_self_field_method_call(call, field_path, type_relations)
            }
            MethodCallReceiver::LocalBinding { name } => {
                self.resolve_param_method_call(call, name, type_relations)
            }
            MethodCallReceiver::BorrowedLocalBinding { name } => {
                self.resolve_param_method_call(call, name, type_relations)
            }
            MethodCallReceiver::TypedLocalBinding { type_path, .. } => {
                self.resolve_typed_local_method_call(call, type_path, type_relations)
            }
            MethodCallReceiver::InitializedLocalBinding { init_path, .. } => {
                self.resolve_typed_local_method_call(call, init_path, type_relations)
            }
            MethodCallReceiver::TupleReturnBinding { path, index, .. } => {
                self.resolve_tuple_return_method_call(call, path, *index, type_relations)
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
            MethodCallReceiver::TryMethodCallResult { method_name } => {
                self.resolve_try_method_result_method_call(call, method_name, type_relations)
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
            MethodCallReceiver::FieldLocalBinding { .. }
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

    fn resolve_tuple_return_method_call(
        &self,
        call: &MethodCallNode,
        path: &[String],
        index: usize,
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
                .resolve_tuple_return_type_method(
                    call.owner,
                    function_id,
                    index,
                    &call.method_name,
                    type_relations,
                ),
            LocalFunctionPathResolution::Unresolved => Ok(AssocPathResolution::Unresolved),
            LocalFunctionPathResolution::Ambiguous => Ok(AssocPathResolution::Ambiguous),
            LocalFunctionPathResolution::Unsupported => Ok(AssocPathResolution::Unsupported),
        }
    }

    fn resolve_tuple_return_type_method(
        &self,
        owner: CallBodyOwnerId,
        function_id: FunctionNodeId,
        index: usize,
        method_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        let Some(return_type) = self.function_return_type(function_id)? else {
            return Ok(AssocPathResolution::Unsupported);
        };
        let Some(element_type) = self.tuple_element_type(return_type, index)? else {
            return Ok(AssocPathResolution::Unsupported);
        };

        self.resolve_type_use_method(owner, &[element_type], method_name, type_relations)?
            .map_or(Ok(AssocPathResolution::Unsupported), Ok)
    }

    fn tuple_element_type(
        &self,
        type_id: OrdinaryTypeUseId,
        index: usize,
    ) -> Result<Option<OrdinaryTypeUseId>, SynParserError> {
        match self.type_node(type_id)? {
            TypeNode::Tuple(node) => Ok(node.elements.get(index).copied()),
            TypeNode::Paren(node) => self.tuple_element_type(node.inner, index),
            _ => Ok(None),
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

        Ok(Self::struct_field_type(
            &struct_node.fields,
            field_name,
            &struct_node.name,
        ))
    }
}
