use std::iter::successors;

use crate::{
    error::SynParserError,
    parser::{graph::GraphAccess, nodes::{AsAnyNodeId, PrimaryNodeId}, relations::SyntacticRelation, ParsedCodeGraph}, resolve::module_tree::TreeRelation,
};

use super::module_tree::{ModuleTree, ModuleTreeError, ResolvedItemInfo};

pub fn traverse_tree(
    tree: &ModuleTree,
    item_pid: PrimaryNodeId,  // typed id, e.g. PrimaryNodeId(FunctionNode(Uuid))
    parsed: &ParsedCodeGraph, // contains all relations, see below
) -> IntoIterator<Item = TreeRelation> {
    let first_rel = tree.get_relations_to(item_pid.as_any())
        .map(|rels| rels.iter().find(|&tr| tr.rel().is_contains())
        .expect("todo: error Orphaned Node, invalid state"));

    let mut arbitrary_traversal = parsed
        .relations()
        .into_iter()
        .copied()
        // Now we wrap the base iterator values in Some()
        .map(|r| Some(r.clone()))
        .cycle() // Cycling makes this cycle O(n!) times in worst case, but that shouldn't be
        // possible with tree. For a tree it would be O(n*p) where p is number of complete paths
        // explored, though `take` could mess with that.
        // `take` could be interesting... the problem is branching paths. We could use some number
        // of values instead of just one `r` above, like `(r_1, r_2, ..., r_y)`, where y is the
        // number of possible branches. If we traverse up from the leaf that might work, since this
        // is a module tree of a code graph of rust source code. We would have at most one
        // containing module, one re-export, one child of the current module using that re-export..
        // hmmm... could be tricky.
        // I wonder if this might be one of those cases where even inefficient algorithms outerform
        // efficient algorithms by relying on hardware, e.g. by performing all computations in the
        // cache or bus or something, never going to RAM (though this still uses `get` from the
        // hashmap, it might be worth testing with a copied `Vec` so nothing lives in RAM after
        // initial retrieval. Even if it is 100x less efficient, could
        // end up being faster in the majority of cases, or for all values of n < x. Worth testing anyway.
        .scan(first_rel.copied(), |now_rel, next_rel| {
            *now_rel = now_rel.and_then(|now_r| match now_r.rel() {
                SyntacticRelation::Contains { 
                    source, 
                    target
                    // Only process if Some
                } if next_rel.is_some_and(|r| now_r.rel().src_eq_trg(r)) => {
                    // safe unwrap, then take. Underlying iterator now holds none in that place?
                    // Not sure how this will interact with `cycle`, or if this would even compile
                    // (since I'm supposed to be working on fixing rustacean in nvim)
                    // this is kind of fun though.
                    let taken_rel = next_rel.take().unwrap(); // safe b/c is_some_and
                    // use the underlying, immutable tree's hashmap of ID->Vec<usize> to get
                    // indicies of stored Vec<TreeRelation>, basically O(1)
                    tree.get_relations_to_primary(&taken_rel.target())
                },
                SyntacticRelation::ResolvesToDefinition { source, target } => todo!(),
                SyntacticRelation::CustomPath { source, target } => todo!(),
                SyntacticRelation::ReExports { source, target } => todo!(),
                SyntacticRelation::Sibling { source, target } => todo!(),
                // etc, others as needed for type of search
                _ => None
            }
            )
        });
    arbitrary_traversal
}

// inside ModuleTree impl
// ..
    pub fn get_relations_to_primary(
        &self,
        target_id: &PrimaryNodeId,
    ) -> Option<&TreeRelation>
    {
        self.relations_by_target.get(target_id.as_any()).map(|indices| {
            // Changed: Use AnyNodeId key
            indices
                .iter()
                .find_map(|&index| {
                    self.tree_relations.get(index)
                })
            })
    }
// .. other methods
