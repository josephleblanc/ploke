//! Collect node UUIDs present in a [`ParsedCodeGraph`] for compilation-unit membership masks.

use std::collections::HashSet;

use ploke_core::IdTrait;
use uuid::Uuid;

use crate::parser::{
    graph::GraphAccess,
    nodes::{AnyNodeId, GraphNode, TypeDefNode},
    types::TypeNode,
    ParsedCodeGraph,
};

/// Node ids referenced from the pruned graph (primary, secondary where present, and type ids).
pub fn collect_all_node_uuids_in_graph(graph: &ParsedCodeGraph) -> HashSet<Uuid> {
    let mut out = HashSet::new();

    for n in graph.functions() {
        insert_any(&mut out, n.any_id());
        for p in &n.parameters {
            out.insert(p.type_id.uuid());
        }
        for gp in &n.generic_params {
            insert_any(&mut out, gp.id.into());
        }
    }

    for t in graph.traits() {
        insert_any(&mut out, t.any_id());
        for m in &t.methods {
            insert_any(&mut out, m.any_id());
            for p in &m.parameters {
                out.insert(p.type_id.uuid());
            }
            for gp in &m.generic_params {
                insert_any(&mut out, gp.id.into());
            }
        }
        for gp in &t.generic_params {
            insert_any(&mut out, gp.id.into());
        }
    }

    for i in graph.impls() {
        insert_any(&mut out, i.any_id());
        for m in &i.methods {
            insert_any(&mut out, m.any_id());
            for p in &m.parameters {
                out.insert(p.type_id.uuid());
            }
            for gp in &m.generic_params {
                insert_any(&mut out, gp.id.into());
            }
        }
        for gp in &i.generic_params {
            insert_any(&mut out, gp.id.into());
        }
    }

    for def in graph.defined_types() {
        match def {
            TypeDefNode::Struct(s) => {
                insert_any(&mut out, s.any_id());
                for f in &s.fields {
                    insert_any(&mut out, f.id.into());
                }
                for gp in &s.generic_params {
                    insert_any(&mut out, gp.id.into());
                }
            }
            TypeDefNode::Enum(e) => {
                insert_any(&mut out, e.any_id());
                for v in &e.variants {
                    insert_any(&mut out, v.id.into());
                    for f in &v.fields {
                        insert_any(&mut out, f.id.into());
                    }
                }
                for gp in &e.generic_params {
                    insert_any(&mut out, gp.id.into());
                }
            }
            TypeDefNode::TypeAlias(t) => {
                insert_any(&mut out, t.any_id());
                for gp in &t.generic_params {
                    insert_any(&mut out, gp.id.into());
                }
            }
            TypeDefNode::Union(u) => {
                insert_any(&mut out, u.any_id());
                for f in &u.fields {
                    insert_any(&mut out, f.id.into());
                }
                for gp in &u.generic_params {
                    insert_any(&mut out, gp.id.into());
                }
            }
        }
    }

    for n in graph.modules() {
        insert_any(&mut out, n.any_id());
    }

    for n in graph.consts() {
        insert_any(&mut out, n.any_id());
    }

    for n in graph.statics() {
        insert_any(&mut out, n.any_id());
    }

    for n in graph.macros() {
        insert_any(&mut out, n.any_id());
    }

    for n in graph.use_statements() {
        insert_any(&mut out, n.any_id());
    }

    for n in graph.unresolved_nodes() {
        insert_any(&mut out, n.any_id());
    }

    collect_type_nodes(graph.type_graph(), &mut out);

    out
}

fn insert_any(out: &mut HashSet<Uuid>, id: AnyNodeId) {
    out.insert(id.uuid());
}

fn collect_type_nodes(types: &[TypeNode], out: &mut HashSet<Uuid>) {
    let mut stack: Vec<&TypeNode> = types.iter().collect();
    while let Some(t) = stack.pop() {
        out.insert(t.id.uuid());
        for rt in &t.related_types {
            out.insert(rt.uuid());
        }
    }
}
