use crate::{
    error::SynParserError,
    parser::{
        graph::GraphAccess,
        nodes::{
            AnyCallSiteId, AsAnyNodeId, CallArgument, CallBodyOwnerId, CallNode, FieldNode,
            FunctionNodeId, MethodCallNode, MethodCallReceiver, MethodNodeId, OrdinaryTypeSourceId,
            OrdinaryTypeTargetId, OrdinaryTypeUseId, PathCallNode, StructNodeId, TraitTypeSourceId,
            TypeAliasNodeId, TypeGenericParamNodeId,
        },
        relations::{CallRelation, CallResolutionKind, CallResolutionStatus, TypeRelation},
        types::TypeNode,
    },
};

use super::{
    AssocPathResolution, CallRelationResolver, LocalFunctionPathResolution, LocalTraitResolution,
    LocalTypeResolution, WorkspaceTypeResolution,
    path::{ParameterCallResolution, ParameterCallTarget},
    trait_declares_instance_method,
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
        if let MethodCallReceiver::MethodResultLocalBinding {
            method_name,
            method_span,
            ..
        } = &call.receiver
            && self.is_external_local_result_method(
                call,
                method_name,
                *method_span,
                type_relations,
            )?
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
                self.resolve_initialized_local_method_call(call, init_path, type_relations)?
            }
            MethodCallReceiver::AliasedLocalBinding { source_path, .. } => {
                self.resolve_alias_method_call(call, source_path, type_relations)?
            }
            MethodCallReceiver::TupleReturnBinding { path, index, .. } => {
                self.resolve_tuple_return_method_call(call, path, *index, type_relations)?
            }
            MethodCallReceiver::TupleMethodReturn {
                method_name,
                method_span,
                index,
                ..
            } => self.resolve_tuple_method_return_method_call(
                call,
                method_name,
                *method_span,
                *index,
                type_relations,
            )?,
            MethodCallReceiver::BorrowedInitializedLocalBinding { init_path, .. } => {
                self.resolve_initialized_local_method_call(call, init_path, type_relations)?
            }
            MethodCallReceiver::BorrowedTypedLocalBinding { type_path, .. } => {
                self.resolve_typed_local_method_call(call, type_path, type_relations)?
            }
            MethodCallReceiver::DereferencedInitializedLocalBinding { init_path, .. } => {
                self.resolve_initialized_local_method_call(call, init_path, type_relations)?
            }
            MethodCallReceiver::PathCallResult { path } => {
                self.resolve_path_result_method_call(call, path, type_relations)?
            }
            MethodCallReceiver::AwaitPathCallResult { path } => {
                self.resolve_path_result_method_call(call, path, type_relations)?
            }
            MethodCallReceiver::AwaitMethodCallResult { method_name } => {
                self.resolve_await_method_result_method_call(call, method_name, type_relations)?
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
            MethodCallReceiver::MethodResultLocalBinding {
                method_name,
                method_span,
                ..
            } => self.resolve_local_result_method_call(
                call,
                method_name,
                *method_span,
                type_relations,
            )?,
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
            MethodCallReceiver::FieldLocalBinding { name, field_path } => {
                self.resolve_param_field_method_call(call, name, field_path, type_relations)?
            }
            MethodCallReceiver::AwaitResult
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
                if let Some(resolution) = self.resolve_method_callback_argument_call(call)? {
                    statuses.push(push_method_callback_resolution(call, resolution, relations));
                } else {
                    statuses.push(CallResolutionStatus::Unsupported { source });
                }
            }
        }

        Ok(())
    }

    fn resolve_method_callback_argument_call(
        &self,
        call: &MethodCallNode,
    ) -> Result<Option<ParameterCallResolution>, SynParserError> {
        if call.method_name != "and_then" || call.arg_count != 1 {
            return Ok(None);
        }
        let Some(CallArgument::Path { path }) = call.arguments.first() else {
            return Ok(None);
        };
        self.resolve_parameter_value_call(call.owner, path)
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
        let mut candidates = vec![field_type.declared];
        if let Some(impl_arg) = field_type.impl_arg {
            candidates.push(impl_arg);
        }

        for field_type in &candidates {
            if self.is_external_type_method(
                call.owner,
                *field_type,
                &call.method_name,
                type_relations,
            )? {
                return Ok(true);
            }
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
        let Some(field_type) = self.self_field_path_type(call.owner, field_path, type_relations)?
        else {
            return Ok(AssocPathResolution::Unsupported);
        };
        self.resolve_type_use_method(call.owner, &[field_type], &call.method_name, type_relations)?
            .map_or(Ok(AssocPathResolution::Unsupported), Ok)
    }

    fn self_field_path_type(
        &self,
        owner: CallBodyOwnerId,
        field_path: &[String],
        type_relations: &[TypeRelation],
    ) -> Result<Option<OrdinaryTypeUseId>, SynParserError> {
        let Some((first, rest)) = field_path.split_first() else {
            return Ok(None);
        };
        let Some(mut field_type) = self.self_field_type(owner, first, type_relations)? else {
            return Ok(None);
        };

        for field_name in rest {
            let Some(nested) =
                self.local_struct_field_type(field_type, field_name, type_relations)?
            else {
                return Ok(None);
            };
            field_type = nested;
        }

        Ok(Some(field_type))
    }

    fn local_struct_field_type(
        &self,
        type_id: OrdinaryTypeUseId,
        field_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<Option<OrdinaryTypeUseId>, SynParserError> {
        let Some(target) = self.single_ordinary_target(type_id, type_relations)? else {
            return Ok(None);
        };
        let Ok(struct_id) = StructNodeId::try_from(target) else {
            return Ok(None);
        };
        let struct_node = self.graph.get_struct_checked(struct_id)?;

        Ok(Self::struct_field_type(
            &struct_node.fields,
            field_name,
            &struct_node.name,
        ))
    }

    pub(super) fn self_field_type(
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
        if method_name == "len" && self.type_use_is_slice(type_id)? {
            return Ok(true);
        }

        let TypeNode::Named(type_node) = self.type_node(type_id)? else {
            if method_name == "len"
                && let Some(inner) = self.dereferenced_type_use(type_id)?
            {
                return self.is_external_type_method(owner, inner, method_name, type_relations);
            }
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
            [segment] if matches!(segment.as_str(), "String" | "Vec" | "str") => {
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
        let expected_trait = match method_name {
            "into" => "Into",
            "poll_ready" => "Service",
            _ => return Ok(false),
        };

        let Some(target) = self.single_ordinary_target(field_type, type_relations)? else {
            return Ok(false);
        };
        let Ok(param_id) = TypeGenericParamNodeId::try_from(target) else {
            return Ok(false);
        };

        let sources = self.generic_bound_sources(owner, target, Some(param_id), type_relations)?;
        for source in sources {
            if self.is_external_trait_bound(owner, source, expected_trait)? {
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

    pub(super) fn is_external_trait_bound(
        &self,
        owner: CallBodyOwnerId,
        source: TraitTypeSourceId,
        expected_trait: &str,
    ) -> Result<bool, SynParserError> {
        let path = match self.type_node(source)? {
            TypeNode::Named(node) => &node.path,
            TypeNode::TraitBound(node) => &node.path,
            _ => return Ok(false),
        };

        self.is_external_trait_path(owner, path, expected_trait)
    }

    fn is_external_service_path(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
    ) -> Result<bool, SynParserError> {
        self.is_external_trait_path(owner, path, "Service")
    }

    fn is_external_trait_path(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
        expected_trait: &str,
    ) -> Result<bool, SynParserError> {
        if !path.last().is_some_and(|segment| segment == expected_trait) {
            return Ok(false);
        }
        if self.is_external_path(path) {
            return Ok(true);
        }
        if matches!(expected_trait, "Default" | "Into" | "IntoIterator")
            && matches!(path, [segment] if segment == expected_trait)
        {
            return Ok(!self.has_direct_local_trait_named(owner, expected_trait)?);
        }
        if self.is_external_import_path(owner, path)? {
            return Ok(!self.trait_path_is_local(owner, path)?);
        }

        let [segment] = path else {
            return Ok(false);
        };
        let Some(module_id) = self.containing_module_for_owner(owner) else {
            return Ok(false);
        };
        let module_id = self.import_scope_module(module_id)?;
        let paths = self.segment_external_paths(module_id, segment, 0)?;
        if paths.is_empty() || self.trait_path_is_local(owner, path)? {
            return Ok(false);
        }

        Ok(paths
            .iter()
            .all(|path| path.last().is_some_and(|segment| segment == expected_trait)))
    }

    fn trait_path_is_local(
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

    fn has_direct_local_trait_named(
        &self,
        owner: CallBodyOwnerId,
        trait_name: &str,
    ) -> Result<bool, SynParserError> {
        let Some(owner_module) = self.containing_module_for_owner(owner) else {
            return Ok(false);
        };
        let owner_module = self.import_scope_module(owner_module)?;

        for trait_node in self
            .graph
            .traits()
            .iter()
            .filter(|node| node.name == trait_name)
        {
            let Some(trait_module) = self.module_for_node(trait_node.id.as_any()) else {
                continue;
            };
            if self.import_scope_module(trait_module)? == owner_module {
                return Ok(true);
            }
        }

        Ok(false)
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
            let impl_trait_bound_external = self.is_external_impl_trait_bound_method(
                call.owner,
                param.type_id,
                &call.method_name,
            )?;
            if direct_external || dereferenced_external || impl_trait_bound_external {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn is_external_impl_trait_bound_method(
        &self,
        owner: CallBodyOwnerId,
        type_id: OrdinaryTypeUseId,
        method_name: &str,
    ) -> Result<bool, SynParserError> {
        let expected_trait = match method_name {
            "into" => "Into",
            _ => return Ok(false),
        };

        match self.type_node(type_id)? {
            TypeNode::ImplTrait(node) => {
                for source in &node.bounds {
                    if self.is_external_trait_bound(owner, *source, expected_trait)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            TypeNode::Reference(node) => {
                self.is_external_impl_trait_bound_method(owner, node.referenced, method_name)
            }
            TypeNode::Paren(node) => {
                self.is_external_impl_trait_bound_method(owner, node.inner, method_name)
            }
            _ => Ok(false),
        }
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

    fn is_external_local_result_method(
        &self,
        call: &MethodCallNode,
        result_method: &str,
        result_span: (usize, usize),
        type_relations: &[TypeRelation],
    ) -> Result<bool, SynParserError> {
        if result_method != "into_iter" || call.method_name != "size_hint" {
            return Ok(false);
        }

        let Some(init_call) = self.method_call_by_span(call.owner, result_method, result_span)
        else {
            return Ok(false);
        };
        let MethodCallReceiver::LocalBinding { name } = &init_call.receiver else {
            return Ok(false);
        };

        self.param_has_external_bound(call.owner, name, "IntoIterator", type_relations)
    }

    fn param_has_external_bound(
        &self,
        owner: CallBodyOwnerId,
        name: &str,
        expected_trait: &str,
        type_relations: &[TypeRelation],
    ) -> Result<bool, SynParserError> {
        let Some(parameters) = self.owner_parameters(owner)? else {
            return Ok(false);
        };

        for param in parameters
            .iter()
            .filter(|param| !param.is_self && param.name.as_deref() == Some(name))
        {
            if let Some(target) = self.single_ordinary_target(param.type_id, type_relations)?
                && let Ok(param_id) = TypeGenericParamNodeId::try_from(target)
            {
                let sources =
                    self.generic_bound_sources(owner, target, Some(param_id), type_relations)?;
                for source in sources {
                    if self.is_external_trait_bound(owner, source, expected_trait)? {
                        return Ok(true);
                    }
                }
            }

            let Some(type_segment) = self.type_use_single_segment(param.type_id)? else {
                continue;
            };
            for scope in self.generic_bound_scopes(owner)? {
                for generic_param in scope.params {
                    if generic_param.kind.name() != Some(type_segment.as_str()) {
                        continue;
                    }
                    let Some(bounds) = generic_param.kind.bounds() else {
                        continue;
                    };
                    for bound in bounds {
                        if self.is_external_trait_bound(owner, *bound, expected_trait)? {
                            return Ok(true);
                        }
                    }
                }

                for predicate in scope.predicates {
                    if !self.type_path_matches_segment(predicate.subject, &type_segment)? {
                        continue;
                    }
                    for bound in &predicate.bounds {
                        if self.is_external_trait_bound(owner, *bound, expected_trait)? {
                            return Ok(true);
                        }
                    }
                }
            }
        }

        Ok(false)
    }

    fn type_use_single_segment(
        &self,
        type_id: OrdinaryTypeUseId,
    ) -> Result<Option<String>, SynParserError> {
        match self.type_node(type_id)? {
            TypeNode::Named(node) => Ok(match node.path.as_slice() {
                [segment] => Some(segment.clone()),
                _ => None,
            }),
            TypeNode::Reference(node) => self.type_use_single_segment(node.referenced),
            TypeNode::Paren(node) => self.type_use_single_segment(node.inner),
            _ => Ok(None),
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

    fn resolve_initialized_local_method_call(
        &self,
        call: &MethodCallNode,
        init_path: &[String],
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        if let Some(init_call) = self.local_binding_initializer_path_call(call, init_path) {
            if let Some(resolution) = self.resolve_associated_function_path(
                call.owner,
                init_path,
                init_call.arg_count,
                type_relations,
            )? {
                return match resolution {
                    AssocPathResolution::Resolved(method_id) => self
                        .resolve_method_return_type_method(
                            call.owner,
                            method_id,
                            &call.method_name,
                            type_relations,
                        ),
                    AssocPathResolution::Unresolved => Ok(AssocPathResolution::Unresolved),
                    AssocPathResolution::Ambiguous => Ok(AssocPathResolution::Ambiguous),
                    AssocPathResolution::Unsupported => Ok(AssocPathResolution::Unsupported),
                };
            }
        }

        self.resolve_typed_local_method_call(call, init_path, type_relations)
    }

    fn local_binding_initializer_path_call(
        &self,
        call: &MethodCallNode,
        init_path: &[String],
    ) -> Option<&PathCallNode> {
        let mut candidates = self
            .graph
            .call_sites()
            .iter()
            .filter_map(|candidate| match candidate {
                CallNode::PathCall(inner)
                    if inner.owner == call.owner
                        && inner.path == init_path
                        && inner.span.1 <= call.span.0 =>
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

    fn resolve_await_method_result_method_call(
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
            AssocPathResolution::Resolved(method_id) if self.method_is_async(method_id)? => self
                .resolve_method_return_type_method(
                    call.owner,
                    method_id,
                    &call.method_name,
                    type_relations,
                ),
            AssocPathResolution::Resolved(_) => Ok(AssocPathResolution::Unsupported),
            AssocPathResolution::Unresolved => Ok(AssocPathResolution::Unresolved),
            AssocPathResolution::Ambiguous => Ok(AssocPathResolution::Ambiguous),
            AssocPathResolution::Unsupported => Ok(AssocPathResolution::Unsupported),
        }
    }

    fn resolve_local_result_method_call(
        &self,
        call: &MethodCallNode,
        result_method: &str,
        result_span: (usize, usize),
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        let Some(init_call) = self.method_call_by_span(call.owner, result_method, result_span)
        else {
            return Ok(AssocPathResolution::Unsupported);
        };

        let inner_resolution = self.resolve_method_call_target(init_call, type_relations)?;
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
            AssocPathResolution::Unresolved if inner_method_name == "ok_or" => {
                self.resolve_option_ok_or_result_method_call(call, inner_call, type_relations)
            }
            AssocPathResolution::Unresolved => Ok(AssocPathResolution::Unresolved),
            AssocPathResolution::Ambiguous => Ok(AssocPathResolution::Ambiguous),
            AssocPathResolution::Unsupported if inner_method_name == "ok_or" => {
                self.resolve_option_ok_or_result_method_call(call, inner_call, type_relations)
            }
            AssocPathResolution::Unsupported => Ok(AssocPathResolution::Unsupported),
        }
    }

    fn resolve_option_ok_or_result_method_call(
        &self,
        call: &MethodCallNode,
        ok_or_call: &MethodCallNode,
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        let MethodCallReceiver::PathCallResult { path } = &ok_or_call.receiver else {
            return Ok(AssocPathResolution::Unsupported);
        };
        let Some(path_call) = self.direct_receiver_path_call(ok_or_call, path) else {
            return Ok(AssocPathResolution::Unsupported);
        };

        match self.resolve_associated_function_path(
            call.owner,
            path,
            path_call.arg_count,
            type_relations,
        )? {
            Some(AssocPathResolution::Resolved(method_id)) => {
                let associated_target = self.associated_path_type_target(call.owner, path)?;
                self.resolve_method_option_some_return_type_method(
                    call.owner,
                    method_id,
                    associated_target,
                    &call.method_name,
                    type_relations,
                )
            }
            Some(AssocPathResolution::Unresolved) => Ok(AssocPathResolution::Unresolved),
            Some(AssocPathResolution::Ambiguous) => Ok(AssocPathResolution::Ambiguous),
            Some(AssocPathResolution::Unsupported) | None => Ok(AssocPathResolution::Unsupported),
        }
    }

    fn associated_path_type_target(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
    ) -> Result<Option<OrdinaryTypeTargetId>, SynParserError> {
        let Some((_method_name, type_path)) = path.split_last() else {
            return Ok(None);
        };
        if type_path.is_empty() {
            return Ok(None);
        }
        if type_path.len() != 1 && !self.is_explicit_local_path(type_path) {
            return Ok(None);
        }

        match self.resolve_local_type_path(owner, type_path)? {
            LocalTypeResolution::Resolved(target) => Ok(Some(target)),
            LocalTypeResolution::Unresolved | LocalTypeResolution::Ambiguous => Ok(None),
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

    fn method_call_by_span(
        &self,
        owner: CallBodyOwnerId,
        method_name: &str,
        span: (usize, usize),
    ) -> Option<&MethodCallNode> {
        let candidates = self
            .graph
            .call_sites()
            .iter()
            .filter_map(|candidate| match candidate {
                CallNode::MethodCall(call)
                    if call.owner == owner
                        && call.method_name == method_name
                        && call.span == span =>
                {
                    Some(call)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        match candidates.as_slice() {
            [call] => Some(*call),
            _ => None,
        }
    }

    fn direct_receiver_path_call(
        &self,
        call: &MethodCallNode,
        path: &[String],
    ) -> Option<&PathCallNode> {
        let mut candidates = self
            .graph
            .call_sites()
            .iter()
            .filter_map(|candidate| match candidate {
                CallNode::PathCall(inner)
                    if inner.owner == call.owner
                        && inner.path.as_slice() == path
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
                self.resolve_initialized_local_method_call(call, init_path, type_relations)
            }
            MethodCallReceiver::AliasedLocalBinding { source_path, .. } => {
                self.resolve_alias_method_call(call, source_path, type_relations)
            }
            MethodCallReceiver::TupleReturnBinding { path, index, .. } => {
                self.resolve_tuple_return_method_call(call, path, *index, type_relations)
            }
            MethodCallReceiver::TupleMethodReturn {
                method_name,
                method_span,
                index,
                ..
            } => self.resolve_tuple_method_return_method_call(
                call,
                method_name,
                *method_span,
                *index,
                type_relations,
            ),
            MethodCallReceiver::BorrowedInitializedLocalBinding { init_path, .. } => {
                self.resolve_initialized_local_method_call(call, init_path, type_relations)
            }
            MethodCallReceiver::BorrowedTypedLocalBinding { type_path, .. } => {
                self.resolve_typed_local_method_call(call, type_path, type_relations)
            }
            MethodCallReceiver::DereferencedInitializedLocalBinding { init_path, .. } => {
                self.resolve_initialized_local_method_call(call, init_path, type_relations)
            }
            MethodCallReceiver::PathCallResult { path } => {
                self.resolve_path_result_method_call(call, path, type_relations)
            }
            MethodCallReceiver::AwaitPathCallResult { path } => {
                self.resolve_path_result_method_call(call, path, type_relations)
            }
            MethodCallReceiver::AwaitMethodCallResult { method_name } => {
                self.resolve_await_method_result_method_call(call, method_name, type_relations)
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
            MethodCallReceiver::MethodResultLocalBinding {
                method_name,
                method_span,
                ..
            } => self.resolve_local_result_method_call(
                call,
                method_name,
                *method_span,
                type_relations,
            ),
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
            MethodCallReceiver::FieldLocalBinding { name, field_path } => {
                self.resolve_param_field_method_call(call, name, field_path, type_relations)
            }
            MethodCallReceiver::AwaitResult
            | MethodCallReceiver::TryResult
            | MethodCallReceiver::Literal
            | MethodCallReceiver::Unsupported => Ok(AssocPathResolution::Unsupported),
        }
    }

    fn resolve_param_field_method_call(
        &self,
        call: &MethodCallNode,
        name: &str,
        field_path: &[String],
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

        let mut targets = Vec::new();
        let mut unsupported = false;
        for param in params {
            let Some(root_target) = self.single_ordinary_target(param.type_id, type_relations)?
            else {
                unsupported = true;
                continue;
            };

            match self.resolve_field_type_method(
                call.owner,
                root_target,
                field_path,
                &call.method_name,
                type_relations,
            )? {
                AssocPathResolution::Resolved(target) => targets.push(target),
                AssocPathResolution::Unresolved => return Ok(AssocPathResolution::Unresolved),
                AssocPathResolution::Ambiguous => return Ok(AssocPathResolution::Ambiguous),
                AssocPathResolution::Unsupported => unsupported = true,
            }
        }

        targets.sort_unstable();
        targets.dedup();

        Ok(match (targets.as_slice(), unsupported) {
            ([target], false) => AssocPathResolution::Resolved(*target),
            ([], _) => AssocPathResolution::Unsupported,
            _ => AssocPathResolution::Ambiguous,
        })
    }

    fn resolve_alias_method_call(
        &self,
        call: &MethodCallNode,
        source_path: &[String],
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        let [name] = source_path else {
            return Ok(AssocPathResolution::Unsupported);
        };

        self.resolve_param_method_call(call, name, type_relations)
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

    fn resolve_tuple_method_return_method_call(
        &self,
        call: &MethodCallNode,
        method_name: &str,
        method_span: (usize, usize),
        index: usize,
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        let Some(inner_call) = self.method_call_at(call.owner, method_name, method_span) else {
            return Ok(AssocPathResolution::Unsupported);
        };

        match self.resolve_method_call_target(inner_call, type_relations)? {
            AssocPathResolution::Resolved(method_id) => self
                .resolve_tuple_method_return_type_method(
                    call.owner,
                    method_id,
                    index,
                    &call.method_name,
                    type_relations,
                ),
            AssocPathResolution::Unresolved => Ok(AssocPathResolution::Unresolved),
            AssocPathResolution::Ambiguous => Ok(AssocPathResolution::Ambiguous),
            AssocPathResolution::Unsupported => Ok(self
                .resolve_external_tuple_method_return_method_call(
                    call.owner,
                    inner_call,
                    index,
                    &call.method_name,
                    type_relations,
                )?
                .unwrap_or(AssocPathResolution::Unsupported)),
        }
    }

    fn method_call_at(
        &self,
        owner: CallBodyOwnerId,
        method_name: &str,
        span: (usize, usize),
    ) -> Option<&MethodCallNode> {
        self.graph
            .call_sites()
            .iter()
            .find_map(|candidate| match candidate {
                CallNode::MethodCall(inner)
                    if inner.owner == owner
                        && inner.method_name == method_name
                        && inner.span == span =>
                {
                    Some(inner)
                }
                _ => None,
            })
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

    fn resolve_tuple_method_return_type_method(
        &self,
        owner: CallBodyOwnerId,
        method_id: MethodNodeId,
        index: usize,
        method_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        let Some(return_type) = self.method_return_type(method_id)? else {
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

fn push_method_callback_resolution(
    call: &MethodCallNode,
    resolution: ParameterCallResolution,
    relations: &mut Vec<CallRelation>,
) -> CallResolutionStatus {
    match resolution {
        ParameterCallResolution::Exact(target) => {
            push_method_callback_target(call, target, relations);
            CallResolutionStatus::Resolved {
                source: AnyCallSiteId::Method(call.id),
                kind: CallResolutionKind::LocalExact,
            }
        }
        ParameterCallResolution::Ambiguous(targets) => {
            for target in targets {
                push_method_callback_target(call, target, relations);
            }
            CallResolutionStatus::Ambiguous {
                source: AnyCallSiteId::Method(call.id),
            }
        }
    }
}

fn push_method_callback_target(
    call: &MethodCallNode,
    target: ParameterCallTarget,
    relations: &mut Vec<CallRelation>,
) {
    match target {
        ParameterCallTarget::Function(target) => {
            relations.push(CallRelation::MethodCallbackFunction {
                source: call.id,
                target,
            });
        }
        ParameterCallTarget::Closure(target) => {
            relations.push(CallRelation::MethodCallbackClosure {
                source: call.id,
                target,
            });
        }
    }
}
