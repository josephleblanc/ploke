use std::collections::BTreeMap;

use syn_parser::parser::{
    graph::CodeGraph,
    nodes::{
        CallArgument, CallBodyOwnerId, CallNode, FunctionNodeId, LocalBindingId, LocalBindingKind,
        LocalBindingSource,
    },
    relations::{CallRelation, LocalBindingRelation},
};
use syn_parser::resolve::call_resolution::CallResolutionReport;

pub(super) fn derive_argument_parameter_relations(
    graph: &CodeGraph,
    report: &CallResolutionReport,
) -> Vec<LocalBindingRelation> {
    let parameter_bindings = parameter_binding_ids(graph);
    let calls = path_calls(graph);
    let mut relations = Vec::new();

    for relation in &report.relations {
        let CallRelation::Function { source, target } = relation else {
            continue;
        };
        let Some(call) = calls.get(source) else {
            continue;
        };
        let Some(parameter_names) = function_parameter_names(graph, *target) else {
            continue;
        };

        for (idx, argument) in call.arguments.iter().enumerate() {
            if !argument_has_exact_callable_source(argument) {
                continue;
            }
            let Some(name) = parameter_names.get(idx) else {
                continue;
            };
            let Some(binding_id) = parameter_bindings.get(&(*target, *name)) else {
                continue;
            };
            relations.push(LocalBindingRelation::ArgumentSuppliesParameter {
                source: (*source).into(),
                target: *binding_id,
            });
        }
    }

    relations.sort_unstable();
    relations.dedup();
    relations
}

pub(super) fn derive_value_alias_relations(graph: &CodeGraph) -> Vec<LocalBindingRelation> {
    let bindings_by_name = local_bindings_by_owner_name(graph);
    let mut relations = Vec::new();

    for binding in &graph.local_bindings {
        let LocalBindingSource::ValueAlias { source_path } = &binding.source else {
            continue;
        };
        let [source_name] = source_path.as_slice() else {
            continue;
        };
        let Some(targets) = bindings_by_name.get(&(binding.owner, source_name.as_str())) else {
            continue;
        };
        let [target] = targets.as_slice() else {
            continue;
        };
        if binding.id == *target {
            continue;
        }
        relations.push(LocalBindingRelation::BindingAliasesBinding {
            source: binding.id,
            target: *target,
        });
    }

    relations.sort_unstable();
    relations.dedup();
    relations
}

fn local_bindings_by_owner_name(
    graph: &CodeGraph,
) -> BTreeMap<(CallBodyOwnerId, &str), Vec<LocalBindingId>> {
    let mut bindings = BTreeMap::<(CallBodyOwnerId, &str), Vec<LocalBindingId>>::new();
    for binding in &graph.local_bindings {
        bindings
            .entry((binding.owner, binding.name.as_str()))
            .or_default()
            .push(binding.id);
    }
    bindings
}

fn parameter_binding_ids(
    graph: &CodeGraph,
) -> BTreeMap<(FunctionNodeId, &str), syn_parser::parser::nodes::LocalBindingId> {
    graph
        .local_bindings
        .iter()
        .filter_map(|binding| {
            let CallBodyOwnerId::Function(function_id) = binding.owner else {
                return None;
            };
            (binding.kind == LocalBindingKind::ParameterBinding)
                .then_some(((function_id, binding.name.as_str()), binding.id))
        })
        .collect()
}

fn path_calls(
    graph: &CodeGraph,
) -> BTreeMap<syn_parser::parser::nodes::PathCallSiteId, &syn_parser::parser::nodes::PathCallNode> {
    graph
        .call_sites
        .iter()
        .filter_map(|site| match site {
            CallNode::PathCall(call) => Some((call.id, call)),
            _ => None,
        })
        .collect()
}

fn function_parameter_names(graph: &CodeGraph, id: FunctionNodeId) -> Option<Vec<&str>> {
    graph
        .functions
        .iter()
        .find(|function| function.id == id)
        .map(|function| {
            function
                .parameters
                .iter()
                .filter(|param| !param.is_self)
                .filter_map(|param| param.name.as_deref())
                .collect()
        })
}

fn argument_has_exact_callable_source(argument: &CallArgument) -> bool {
    match argument {
        CallArgument::Path { .. }
        | CallArgument::ReferencedPath { .. }
        | CallArgument::BoxedPath { .. }
        | CallArgument::Closure { .. }
        | CallArgument::ClosureBinding { .. } => true,
        CallArgument::Constructed { fields, .. } => fields
            .iter()
            .any(|field| !field.field_path.is_empty() && !field.init_path.is_empty()),
        CallArgument::Array { .. } | CallArgument::Other => false,
    }
}
