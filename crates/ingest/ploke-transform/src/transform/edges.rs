// ------------------------------------------------------------
// ----------------- To Implement -----------------------------
// ------------------------------------------------------------
//
// These relations are needed to ensure we do not have any orphaned nodes:
// Ensure the following edges are added explicitly to the graph:
//  Contains:
//  Imports:
//  Attribute:
//      - Currently being handled by creating a node that has a reference to their node. See end
//      note (1)
//  GenericParams (basic)
//  Reexports:
//  HasMethod:
//  HasField:
//  HasVariant:
//
// Requires Type Resolution:
//  Self Type
//  GenericParams (advanced, needs planning)
//  TraitImpl (Trait->Impl)
//  StructImpl (Struct->Impl)
//
// Abstracted Relations (Not strictly syntactic, composed of other relations)
//  ImplementsTrait (struct->method)
//      - Struct->Impl
//      - Impl->Trait
//  ImplementsMethod (struct->method)
//      - Struct->Impl
//      - Impl->Method
//  ImplementsTraitMethod (struct->method)
//      - Struct->Impl
//      - Impl->Trait   // These two previous are ImplementsTrait
//      - Trait->Method
//
//
//
// On cozo relations:
//  I'm not yet sure how to best represent things that don't seem inherent (i.e. unremovable
//  without fundementally altering the node's core identity) to a given node. An example of this is
//  the node's attributes.
//  Attributes are currently handled by forming an implicit edge with the node they modify by
//  holding the ID of that node. This seems a reasonable way to represent them in the graph, but I
//  don't yet have enough experience with graph operations to say whether this would be difficult
//  to deal with under any circumstances.
//  The one possible tripping point I anticipate is when updating or removing a node from the graph
//  I will need to be careful to find ALL nodes that are connected to it. If we have a central
//  storage of all edges, this would be fairly simple, but if we also have some things which
//  essentially only hold pointers to the nodes themselves, then it might be easy to lose track and
//  have what amounts to a memory leak in the form of orphaned nodes.
//  I'll re-evaluate as I go, but it might be worth keeping all relations two-way, at least for
//  now. Soon I should just implement a simple way to create IDs for the Attributes by using
//  something highly specific, like a combination of the standard namespace used for all nodes,
//  plus their defining node's hash, and then a hash of all their contents or something. That way
//  their parent at least has a direct connection to them. Or I can make a specific relation/edge
//  type for this, and leave the core node type the attribute refers to unpolluted.
//
// On type safety in the database:
//  Type safety in the database could end up being a hassle. `cozo` itself is 'strongly' typed, but
//  it definitely doesn't have the kind of explicit typing I would want, e.g. in the form of enums.
//  I'll need to consider how to handle this more concretely.
//  While it wouldn't be very human-readible, I think I'd like to add some fields that would help
//  with type-safety, such as discriminiant fields for enum wrappers of the AnyNodeId and other
//  typed ids. These would require fields for, e.g.
//      - AnyNodeId (discriminant for each variant such as FunctionNode, MethodNode, StructNode,
//      etc.)
//      - PrimaryNodeId: discriminant for each again, but this field would only exist on primary
//      nodes.
//      - etc for all categories a node is connected to.
//  Having these descriminants would really help make at least the typed IDs type-safe on
//  transforming into and out of the database, and make them less reliant on strings. Keeping
//  dedicated traits to handle the transformation that could be shared across crates in the
//  workspace would also help, possibly defined in `ploke-core`.
//  This pattern may be worth repeating for other fields as well. The challenge will be defining
//  these things in such a way that they don't clutter up the fields. Perhaps I can keep a
//  dedicated set of relations that hold the mappings from node-id to their types/discriminants?
//  I think that could work, but I'll want to consider more how to design this as I get more
//  experience with queries.
//

use cozo::{Db, MemStorage};
use syn_parser::parser::relations::SyntacticRelation;

use crate::schema::edges::SyntacticRelationSchema;
use super::*;

pub(super) fn transform_relations(
    db: &Db<MemStorage>,
    relations: Vec<SyntacticRelation>,
) -> Result<(), TransformError> {
    let schema = &SyntacticRelationSchema::SCHEMA;
    for relation in relations {
        schema.insert_relation(db, &relation)?;
    }
    Ok(())
}
// do stuff

#[cfg(test)]
mod tests {

    use cozo::{Db, MemStorage};
    use ploke_test_utils::test_run_phases_and_collect;
    use syn_parser::parser::ParsedCodeGraph;

    use crate::{error::TransformError, schema::edges::SyntacticRelationSchema};

    use super::transform_relations;

    #[test]
    fn test_transform_edges() -> Result<(), Box<TransformError>> {
        let _ = env_logger::builder()
            .is_test(true)
            .format_timestamp(None) // Disable timestamps
            .try_init();

        // Setup printable nodes
        let successful_graphs = test_run_phases_and_collect("fixture_nodes");
        let merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");

        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");

        let rel_schema = &SyntacticRelationSchema::SCHEMA;
        rel_schema.create_and_insert(&db)?;

        // transform and insert impls into cozo
        transform_relations(&db, merged.graph.relations)?;

        Ok(())
    }
}
