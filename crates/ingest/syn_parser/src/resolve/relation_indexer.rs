use super::*;

pub trait RelationIndexer {
    fn relations_by_source(&self) -> &HashMap<AnyNodeId, Vec<usize>>;
    fn relations_by_source_mut(&mut self) -> &mut HashMap<AnyNodeId, Vec<usize>>;
    fn relations_by_target(&self) -> &HashMap<AnyNodeId, Vec<usize>>;
    fn relations_by_target_mut(&mut self) -> &mut HashMap<AnyNodeId, Vec<usize>>;
    fn tree_relations(&self) -> &Vec<TreeRelation>;
    fn tree_relations_mut(&mut self) -> &mut Vec<TreeRelation>;

    /// Finds relations originating from `source_id` that satisfy the `relation_filter` closure.
    ///
    /// The closure receives a reference to each candidate `TreeRelation` and should return `true`
    /// if the relation should be included in the results.
    ///
    /// # Arguments
    /// * `source_id`: The NodeId of the source node.
    /// * `relation_filter`: A closure `Fn(&Relation) -> bool` used to filter relations.
    ///
    /// # Returns
    /// A `Vec` containing references to the matching `Relation`s.
    ///
    /// # Complexity
    /// O(1) average lookup for the source ID + O(k) filter application, where k is the
    /// number of relations originating from `source_id`.
    #[allow(dead_code, reason = "useful later, probably.")]
    fn get_relations_from<F>(
        &self,
        source_id: &AnyNodeId, // Changed: Parameter is AnyNodeId
        relation_filter: F,    // Closure parameter
    ) -> Option<Vec<&TreeRelation>>
    where
        F: Fn(&TreeRelation) -> bool, // Closure takes &TreeRelation, returns bool
    {
        self.relations_by_source().get(source_id).map(|indices| {
            // Changed: Use AnyNodeId key
            // If source_id not in map, return empty
            indices
                .iter()
                .filter_map(|&index| {
                    self.tree_relations()
                        .get(index)
                        // filter() on Option returns Some only if the closure is true.
                        .filter(|&relation| relation_filter(relation))
                })
                .collect()
        })
    }
    fn get_iter_relations_from<'a>(
        &'a self,
        source_id: &AnyNodeId, // Changed: Parameter is AnyNodeId
    ) -> Option<impl Iterator<Item = &'a TreeRelation>> {
        self.relations_by_source().get(source_id).map(|indices| {
            // Changed: Use AnyNodeId key
            // If source_id not in map, return empty
            indices
                .iter()
                .filter_map(|&index| self.tree_relations().get(index))
        })
    }

    fn get_all_relations_from(&self, source_id: &AnyNodeId) -> Option<Vec<&TreeRelation>> {
        // Changed: Parameter is AnyNodeId
        self.relations_by_source().get(source_id).map(|indices| {
            // Changed: Use AnyNodeId key
            // If source_id not in map, return empty
            indices
                .iter()
                .filter_map(|&index| self.tree_relations().get(index))
                .collect()
        })
    }

    /// Finds relations pointing to `target_id` that satisfy the `relation_filter` closure.
    ///
    /// (Doc comments similar to get_relations_from)
    #[allow(dead_code, reason = "useful later, probably.")]
    fn get_relations_to<F>(
        &self,
        target_id: &AnyNodeId, // Changed: Parameter is AnyNodeId
        relation_filter: F,    // Closure parameter
    ) -> Option<Vec<&TreeRelation>>
    where
        F: Fn(&TreeRelation) -> bool, // Closure takes &TreeRelation, returns bool
    {
        self.relations_by_target().get(target_id).map(|indices| {
            // Changed: Use AnyNodeId key
            indices
                .iter()
                .filter_map(|&index| {
                    self.tree_relations()
                        .get(index)
                        .filter(|&tr| relation_filter(tr))
                })
                .collect()
        })
    }
    fn get_all_relations_to(&self, target_id: &AnyNodeId) -> Option<Vec<&TreeRelation>> {
        // Changed: Parameter is AnyNodeId
        self.relations_by_target().get(target_id).map(|indices| {
            // Changed: Use AnyNodeId key
            // If source_id not in map, return empty
            indices
                .iter()
                .filter_map(|&index| self.tree_relations().get(index))
                .collect()
        })
    }

    /// Returns an iterator over all `TreeRelation`s whose `target` is `target_id`.
    ///
    /// If `target_id` has no incoming relations, the iterator will simply be empty.
    /// The method never panics and does not allocate.
    ///
    /// # Examples
    /// ```ignore
    /// // Uses fixture graphs available only in the test harness.
    /// let parsed_graphs = (*PARSED_FIXTURE_CRATE_NODES).clone();
    /// let merged: ParsedCodeGraph = ParsedCodeGraph::merge_new(parsed_graphs).expect("Error parsing static graph");
    /// let tree = merged
    ///     .build_module_tree()
    ///     .expect("Error building module tree from static graph");
    /// let root_id = tree.root().as_any();
    /// // root_id should return a None value here, which is acceptable
    /// let mut should_be_none = tree.get_iter_relations_to(&root_id);
    /// assert!(should_be_none.next().is_none());
    /// for rel in tree.get_iter_relations_to(&root_id.as_any()) {
    ///     eprintln!("{rel:?}");
    /// }
    /// ```
    /// Reviewed by JL 25-07-2025, guided doc by kimi-k2
    fn get_iter_relations_to<'a>(
        &'a self,
        target_id: &AnyNodeId,
    ) -> impl Iterator<Item = &'a TreeRelation> {
        self.relations_by_target()
            .get(target_id)
            .map(|indices| {
                // Use AnyNodeId key
                // Map indices directly to relation references
                indices
                    .iter()
                    .filter_map(|&index| self.tree_relations().get(index))
            })
            .into_iter()
            .flatten()
    }

    /// Returns an iterator over all `TreeRelation`s whose `target` is `target_id`,
    /// treating the absence of *any* entry for that id as a logic error.
    ///
    /// # Success
    /// On success the iterator yields zero or more references to the incoming
    /// relations.  The iterator does not allocate and is obtained in O(1) time.
    ///
    /// # Failure
    /// Returns `Err(ModuleTreeError::NoRelationsFoundForId)` if no relations for
    /// `target_id` were ever recorded.  This is considered an invariant violation
    /// and a corresponding error is logged.
    ///
    /// Reviewed by JL 2025-07-25, guided doc by kimi-k2
    fn try_get_iter_relations_to<'a>(
        &'a self,
        target_id: &AnyNodeId,
    ) -> Result<impl Iterator<Item = &'a TreeRelation>, ModuleTreeError> {
        self.relations_by_target()
            .get(target_id)
            .map(|indices| {
                // Use AnyNodeId key
                // Map indices directly to relation references
                indices
                    .iter()
                    .filter_map(|&index| self.tree_relations().get(index))
            })
            .ok_or_else(|| {
                log::error!(target: LOG_TARGET_MOD_TREE_BUILD,
                    "{}: {} | {} ({}) {} {:?}",
                    "Invariant Violated".log_error(),
                    "All nodes must have a relation.",
                    "No relations found with target_id:",
                    target_id,
                    "Full ID:",
                    target_id
                );
                ModuleTreeError::NoRelationsFoundForId(*target_id)
            })
    }

    /// Adds a relation to the tree without checking if the source/target nodes exist.
    fn add_rel(&mut self, tr: TreeRelation) {
        let new_index = self.tree_relations().len();
        let relation = tr.rel(); // Get the inner Relation
        let source_id = relation.source(); // Get AnyNodeId
        let target_id = relation.target(); // Get AnyNodeId

        self.tree_relations_mut().push(tr);

        // Update indices using AnyNodeId keys
        self.relations_by_source_mut()
            .entry(source_id) // Use AnyNodeId directly
            .or_default()
            .push(new_index);
        self.relations_by_target_mut()
            .entry(target_id) // Use AnyNodeId directly
            .or_default()
            .push(new_index);
    }
}
#[cfg(test)]
mod tests {
    use crate::{
        parser::nodes::AsAnyNodeId, utils::test_setup::PARSED_FIXTURE_CRATE_NODES, ParsedCodeGraph,
    };

    use super::RelationIndexer;

    #[test]
    fn some_test() {
        let parsed_graphs = (*PARSED_FIXTURE_CRATE_NODES).clone();
        let mut merged =
            ParsedCodeGraph::merge_new(parsed_graphs).expect("Error parsing static graph");
        let tree = merged
            .build_tree_and_prune()
            .expect("Error building module tree from static graph");
        let root_id = tree.root().as_any();
        // root_id should return a None value here, which is acceptable
        let mut should_be_none = tree.get_iter_relations_to(&root_id);
        assert!(should_be_none.next().is_none());
        for rel in tree.get_iter_relations_to(&root_id.as_any()) {
            eprintln!("{rel:?}");
        }
    }
}
