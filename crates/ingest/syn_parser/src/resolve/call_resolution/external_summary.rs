use crate::{
    error::SynParserError,
    parser::{
        graph::GraphAccess,
        nodes::{AsAnyNodeId, CallBodyOwnerId, MethodCallNode, MethodCallReceiver},
        relations::TypeRelation,
    },
};

use super::{AssocPathResolution, CallRelationResolver, trait_declares_instance_method};

#[derive(Debug, Clone, Copy)]
struct ExternalTupleMethodReturnSummary {
    source_type_path: &'static [&'static str],
    method_name: &'static str,
    tuple_index: usize,
    return_type_path: &'static [&'static str],
}

// Audited exact summaries for external methods whose tuple return element is
// needed only to resolve a local extension-trait receiver. These entries do not
// create edges to external methods.
const EXTERNAL_TUPLE_METHOD_RETURN_SUMMARIES: &[ExternalTupleMethodReturnSummary] =
    &[ExternalTupleMethodReturnSummary {
        source_type_path: &["http", "Request"],
        method_name: "into_parts",
        tuple_index: 0,
        return_type_path: &["http", "request", "Parts"],
    }];

impl CallRelationResolver<'_> {
    pub(super) fn resolve_external_tuple_method_return_method_call(
        &self,
        owner: CallBodyOwnerId,
        inner_call: &MethodCallNode,
        tuple_index: usize,
        method_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<Option<AssocPathResolution>, SynParserError> {
        let Some(summary) =
            self.external_tuple_method_return_summary(owner, inner_call, tuple_index)?
        else {
            return Ok(None);
        };

        self.resolve_external_return_type_method(
            owner,
            summary.return_type_path,
            method_name,
            type_relations,
        )
        .map(Some)
    }

    fn external_tuple_method_return_summary(
        &self,
        owner: CallBodyOwnerId,
        inner_call: &MethodCallNode,
        tuple_index: usize,
    ) -> Result<Option<&'static ExternalTupleMethodReturnSummary>, SynParserError> {
        let Some(source_type_path) = self.external_method_receiver_type_path(owner, inner_call)?
        else {
            return Ok(None);
        };

        Ok(EXTERNAL_TUPLE_METHOD_RETURN_SUMMARIES
            .iter()
            .find(|summary| {
                summary.method_name == inner_call.method_name
                    && summary.tuple_index == tuple_index
                    && path_matches(&source_type_path, summary.source_type_path)
            }))
    }

    fn external_method_receiver_type_path(
        &self,
        owner: CallBodyOwnerId,
        inner_call: &MethodCallNode,
    ) -> Result<Option<Vec<String>>, SynParserError> {
        let MethodCallReceiver::PathCallResult { path } = &inner_call.receiver else {
            return Ok(None);
        };
        let Some((_call_name, receiver_path)) = path.split_last() else {
            return Ok(None);
        };
        self.external_source_type_path(owner, receiver_path)
    }

    fn external_source_type_path(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
    ) -> Result<Option<Vec<String>>, SynParserError> {
        if path.is_empty() {
            return Ok(None);
        }
        if self.is_external_path(path) {
            return Ok(Some(path.to_vec()));
        }

        let [segment] = path else {
            return Ok(None);
        };
        let Some(module_id) = self.containing_module_for_owner(owner) else {
            return Ok(None);
        };
        let module_id = self.import_scope_module(module_id)?;
        let mut paths = self.segment_external_paths(module_id, segment, 0)?;
        paths.sort();
        paths.dedup();

        Ok(match paths.as_slice() {
            [path] => Some(path.clone()),
            [] | [_, ..] => None,
        })
    }

    fn resolve_external_return_type_method(
        &self,
        owner: CallBodyOwnerId,
        return_type_path: &[&str],
        method_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
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
            if !path_matches(&self_path, return_type_path) {
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

        Ok(if matched {
            Self::method_resolution(candidates)
        } else {
            AssocPathResolution::Unsupported
        })
    }
}

fn path_matches(path: &[String], expected: &[&str]) -> bool {
    path.len() == expected.len()
        && path
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual == expected)
}
