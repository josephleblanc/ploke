use crate::{
    error::SynParserError,
    parser::{
        graph::GraphAccess,
        nodes::{
            CallBodyOwnerId, EnumNodeId, ModuleNodeId, OrdinaryTypeTargetId, PathCallNode,
            StructNodeId,
        },
        relations::TypeRelation,
    },
};

use super::{
    CallRelationResolver, ConstructorPathResolution, LocalModulePathResolution, LocalTypeResolution,
};

impl CallRelationResolver<'_> {
    pub(super) fn resolve_constructor_path(
        &self,
        call: &PathCallNode,
        type_relations: &[TypeRelation],
    ) -> Result<Option<ConstructorPathResolution>, SynParserError> {
        match call.path.as_slice() {
            [name] if name == "Self" => {
                let Some(target) =
                    self.resolve_self_constructor_target(call.owner, type_relations)?
                else {
                    return Ok(None);
                };
                Ok(Some(self.resolve_tuple_struct_constructor(
                    target,
                    type_relations,
                    call.arg_count,
                )?))
            }
            [struct_name] => {
                let resolution = match self.resolve_local_type_segment(call.owner, struct_name)? {
                    LocalTypeResolution::Resolved(target) => self
                        .resolve_tuple_struct_constructor(target, type_relations, call.arg_count)?,
                    LocalTypeResolution::Unresolved => return Ok(None),
                    LocalTypeResolution::Ambiguous => ConstructorPathResolution::Ambiguous,
                };
                Ok(Some(resolution))
            }
            [type_name, item_name] => {
                self.resolve_two_segment_constructor(call, type_relations, type_name, item_name)
            }
            [_, _, ..] => self.resolve_module_qualified_tuple_constructor(call, type_relations),
            _ => Ok(None),
        }
    }

    fn resolve_two_segment_constructor(
        &self,
        call: &PathCallNode,
        type_relations: &[TypeRelation],
        type_name: &str,
        item_name: &str,
    ) -> Result<Option<ConstructorPathResolution>, SynParserError> {
        let enum_resolution = match self.resolve_local_type_segment(call.owner, type_name)? {
            LocalTypeResolution::Resolved(target) => self.resolve_enum_variant_constructor(
                target,
                type_relations,
                item_name,
                call.arg_count,
            )?,
            LocalTypeResolution::Unresolved => ConstructorPathResolution::Unresolved,
            LocalTypeResolution::Ambiguous => ConstructorPathResolution::Ambiguous,
        };
        if !matches!(enum_resolution, ConstructorPathResolution::Unresolved) {
            return Ok(Some(enum_resolution));
        }

        self.resolve_module_qualified_tuple_constructor(call, type_relations)
    }

    fn resolve_module_qualified_tuple_constructor(
        &self,
        call: &PathCallNode,
        type_relations: &[TypeRelation],
    ) -> Result<Option<ConstructorPathResolution>, SynParserError> {
        let Some((type_name, module_path)) = call.path.split_last() else {
            return Ok(None);
        };
        if module_path.is_empty() {
            return Ok(None);
        }

        let module_id = match self.resolve_direct_module_prefix(call.owner, module_path)? {
            LocalModulePathResolution::Resolved(module_id) => module_id,
            LocalModulePathResolution::Ambiguous => {
                return Ok(Some(ConstructorPathResolution::Ambiguous));
            }
            LocalModulePathResolution::Unresolved => return Ok(None),
        };
        let target = match self.resolve_terminal_type(module_id, type_name)? {
            LocalTypeResolution::Resolved(target) => target,
            LocalTypeResolution::Unresolved => return Ok(None),
            LocalTypeResolution::Ambiguous => {
                return Ok(Some(ConstructorPathResolution::Ambiguous));
            }
        };

        Ok(Some(self.resolve_tuple_struct_constructor(
            target,
            type_relations,
            call.arg_count,
        )?))
    }

    fn resolve_direct_module_prefix(
        &self,
        owner: CallBodyOwnerId,
        prefix: &[String],
    ) -> Result<LocalModulePathResolution, SynParserError> {
        if prefix.is_empty() {
            return Ok(LocalModulePathResolution::Unresolved);
        }

        let Some(mut current_module) = self.containing_module_for_owner(owner) else {
            return Ok(LocalModulePathResolution::Unresolved);
        };
        current_module = self.import_scope_module(current_module)?;

        for segment in prefix {
            match segment.as_str() {
                "crate" => {
                    current_module = self.tree.root();
                }
                "self" => {}
                "super" => {
                    current_module =
                        self.tree.get_parent_module_id(current_module).ok_or_else(|| {
                            SynParserError::InternalState(format!(
                                "call resolution could not find parent module for {} while resolving constructor module prefix {}",
                                current_module,
                                prefix.join("::")
                            ))
                        })?;
                }
                _ => {
                    current_module =
                        match self.resolve_direct_child_module(current_module, segment)? {
                            LocalModulePathResolution::Resolved(module_id) => module_id,
                            LocalModulePathResolution::Unresolved => {
                                return Ok(LocalModulePathResolution::Unresolved);
                            }
                            LocalModulePathResolution::Ambiguous => {
                                return Ok(LocalModulePathResolution::Ambiguous);
                            }
                        };
                }
            }
        }

        Ok(LocalModulePathResolution::Resolved(current_module))
    }

    fn resolve_direct_child_module(
        &self,
        module_id: ModuleNodeId,
        segment: &str,
    ) -> Result<LocalModulePathResolution, SynParserError> {
        let Some(parent) = self
            .graph
            .modules()
            .iter()
            .find(|module| module.id == module_id)
        else {
            return Ok(LocalModulePathResolution::Unresolved);
        };
        let mut child_path = parent.path.clone();
        child_path.push(segment.to_string());

        let matching = self
            .graph
            .modules()
            .iter()
            .filter(|module| module.path == child_path)
            .map(|module| self.import_scope_module(module.id))
            .collect::<Result<Vec<_>, _>>()?;
        let mut matching = matching;
        matching.sort_unstable();
        matching.dedup();

        Ok(match matching.as_slice() {
            [module_id] => LocalModulePathResolution::Resolved(*module_id),
            [] => LocalModulePathResolution::Unresolved,
            _ => LocalModulePathResolution::Ambiguous,
        })
    }

    fn resolve_self_constructor_target(
        &self,
        owner: CallBodyOwnerId,
        type_relations: &[TypeRelation],
    ) -> Result<Option<OrdinaryTypeTargetId>, SynParserError> {
        let CallBodyOwnerId::Method(owner_method_id) = owner else {
            return Ok(None);
        };
        let Some(impl_id) = self.impl_for_owner_method(owner_method_id)? else {
            return Ok(None);
        };
        let Some(impl_node) = self.maybe_impl_node(impl_id) else {
            return Ok(None);
        };
        self.impl_self_target(impl_node, type_relations)
    }

    fn resolve_tuple_struct_constructor(
        &self,
        target: OrdinaryTypeTargetId,
        type_relations: &[TypeRelation],
        arg_count: usize,
    ) -> Result<ConstructorPathResolution, SynParserError> {
        let mut candidates = Vec::new();
        for target in self.ordinary_receiver_targets(target, type_relations)? {
            let Ok(struct_id) = StructNodeId::try_from(target) else {
                continue;
            };
            let struct_node = self.graph.get_struct_checked(struct_id)?;
            if struct_node.fields.is_empty()
                || !struct_node.fields.iter().all(|field| {
                    field.name.as_deref().is_some_and(|name| {
                        name.starts_with(&format!("unnamed_field{}", struct_node.name))
                    })
                })
                || struct_node.fields.len() != arg_count
            {
                continue;
            }
            candidates.push(struct_id);
        }
        candidates.sort_unstable();
        candidates.dedup();

        Ok(match candidates.as_slice() {
            [target] => ConstructorPathResolution::TupleStruct(*target),
            [] => ConstructorPathResolution::Unresolved,
            _ => ConstructorPathResolution::Ambiguous,
        })
    }

    fn resolve_enum_variant_constructor(
        &self,
        target: OrdinaryTypeTargetId,
        type_relations: &[TypeRelation],
        variant_name: &str,
        arg_count: usize,
    ) -> Result<ConstructorPathResolution, SynParserError> {
        let mut candidates = Vec::new();
        for target in self.ordinary_receiver_targets(target, type_relations)? {
            let Ok(enum_id) = EnumNodeId::try_from(target) else {
                continue;
            };
            let enum_node = self.graph.get_enum_checked(enum_id)?;
            candidates.extend(
                enum_node
                    .variants
                    .iter()
                    .filter(|variant| {
                        variant.name == variant_name
                            && !variant.fields.is_empty()
                            && variant.fields.iter().all(|field| field.name.is_none())
                            && variant.fields.len() == arg_count
                    })
                    .map(|variant| variant.id),
            );
        }
        candidates.sort_unstable();
        candidates.dedup();

        Ok(match candidates.as_slice() {
            [target] => ConstructorPathResolution::EnumVariant(*target),
            [] => ConstructorPathResolution::Unresolved,
            _ => ConstructorPathResolution::Ambiguous,
        })
    }
}
