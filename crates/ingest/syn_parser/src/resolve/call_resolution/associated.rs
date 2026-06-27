use crate::{
    error::SynParserError,
    parser::{
        graph::GraphAccess,
        nodes::{CallBodyOwnerId, OrdinaryTypeTargetId},
        relations::TypeRelation,
    },
};

use super::{AssocPathResolution, CallRelationResolver, LocalTraitResolution, LocalTypeResolution};

impl CallRelationResolver<'_> {
    pub(super) fn resolve_associated_function_path(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
        type_relations: &[TypeRelation],
    ) -> Result<Option<AssocPathResolution>, SynParserError> {
        let [type_segment, method_name] = path else {
            return Ok(None);
        };

        if type_segment == "Self" {
            return self
                .resolve_self_associated_function(owner, method_name)
                .map(Some);
        }

        if matches!(type_segment.as_str(), "crate" | "self" | "super") {
            return Ok(None);
        }

        if let Some(resolution) =
            self.resolve_trait_associated_function_path(owner, type_segment, method_name)?
        {
            return Ok(Some(resolution));
        }

        match self.resolve_local_type_segment(owner, type_segment)? {
            LocalTypeResolution::Resolved(target) => {
                self.resolve_type_associated_function(target, method_name, type_relations)
            }
            LocalTypeResolution::Unresolved => Ok(None),
            LocalTypeResolution::Ambiguous => Ok(Some(AssocPathResolution::Ambiguous)),
        }
    }

    fn resolve_trait_associated_function_path(
        &self,
        owner: CallBodyOwnerId,
        trait_segment: &str,
        method_name: &str,
    ) -> Result<Option<AssocPathResolution>, SynParserError> {
        match self.resolve_local_trait_segment(owner, trait_segment)? {
            LocalTraitResolution::Resolved(trait_id) => {
                let trait_node = self.graph.get_trait_checked(trait_id)?;
                Ok(Some(self.resolve_associated_function_in_trait(
                    trait_node,
                    method_name,
                )))
            }
            LocalTraitResolution::Unresolved => Ok(None),
            LocalTraitResolution::Ambiguous => Ok(Some(AssocPathResolution::Ambiguous)),
        }
    }

    fn resolve_self_associated_function(
        &self,
        owner: CallBodyOwnerId,
        method_name: &str,
    ) -> Result<AssocPathResolution, SynParserError> {
        let CallBodyOwnerId::Method(owner_method_id) = owner else {
            return Ok(AssocPathResolution::Unsupported);
        };

        if let Some(impl_id) = self.impl_for_owner_method(owner_method_id)? {
            let Some(impl_node) = self.maybe_impl_node(impl_id) else {
                return Ok(AssocPathResolution::Unsupported);
            };
            if impl_node.trait_type.is_some() {
                return Ok(AssocPathResolution::Unsupported);
            }
            return Ok(self.resolve_method_in_impl(impl_node, method_name));
        }

        let Some(trait_id) = self.trait_for_owner_method(owner_method_id)? else {
            return Ok(AssocPathResolution::Unsupported);
        };
        let trait_node = self.graph.get_trait_checked(trait_id)?;
        Ok(self.resolve_associated_function_in_trait(trait_node, method_name))
    }

    fn resolve_type_associated_function(
        &self,
        target: OrdinaryTypeTargetId,
        method_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<Option<AssocPathResolution>, SynParserError> {
        let receiver_targets = self.ordinary_receiver_targets(target, type_relations)?;
        let mut candidates = Vec::new();
        let mut matched_inherent_impl = false;
        for impl_node in self
            .graph
            .impls()
            .iter()
            .filter(|impl_node| impl_node.trait_type.is_none())
        {
            let Some(self_target) = self.impl_self_target(impl_node, type_relations)? else {
                continue;
            };
            if !receiver_targets.contains(&self_target) {
                continue;
            }
            matched_inherent_impl = true;
            candidates.extend(
                impl_node
                    .methods
                    .iter()
                    .filter(|method| method.name == method_name)
                    .map(|method| method.id),
            );
        }

        if matched_inherent_impl {
            Ok(Some(Self::method_resolution(candidates)))
        } else {
            Ok(None)
        }
    }
}
