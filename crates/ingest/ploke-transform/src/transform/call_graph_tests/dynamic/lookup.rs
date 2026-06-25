use super::*;

pub(super) fn find_dynamic_relation(
    call_report: &CallResolutionReport,
    graph: &ParsedCodeGraph,
    predicate: impl Fn(&DynamicCallNode) -> bool,
    message: &str,
) -> (DynamicCallSiteId, FunctionNodeId) {
    call_report
        .relations
        .iter()
        .copied()
        .find_map(|relation| match relation {
            CallRelation::DynamicFunction { source, target } => {
                let dynamic_call = dynamic_call_by_id(graph, source)?;
                predicate(dynamic_call).then_some((source, target))
            }
            CallRelation::Function { .. }
            | CallRelation::Method { .. }
            | CallRelation::AssociatedFunction { .. }
            | CallRelation::TupleStructConstructor { .. }
            | CallRelation::EnumVariantConstructor { .. } => None,
        })
        .expect(message)
}

pub(super) fn find_dynamic_site(
    graph: &ParsedCodeGraph,
    predicate: impl Fn(&DynamicCallNode) -> bool,
    message: &str,
) -> DynamicCallSiteId {
    graph
        .call_sites()
        .iter()
        .find_map(|call| match call {
            CallNode::DynamicCall(dynamic_call) if predicate(dynamic_call) => Some(dynamic_call.id),
            _ => None,
        })
        .expect(message)
}

fn dynamic_call_by_id(
    graph: &ParsedCodeGraph,
    site_id: DynamicCallSiteId,
) -> Option<&DynamicCallNode> {
    let source_any = AnyCallSiteId::Dynamic(site_id);
    graph
        .call_sites()
        .iter()
        .find(|call| call.id() == source_any)
        .and_then(|call| match call {
            CallNode::DynamicCall(dynamic_call) => Some(dynamic_call),
            _ => None,
        })
}
