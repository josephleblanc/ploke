use super::*;

pub(super) trait RelationIndexer {
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

    /// Returns an iterator over all `TreeRelation`s pointing to the given `target_id`.
    ///
    /// This provides efficient access to incoming relations without collecting them into a `Vec`.
    /// Filtering by relation kind should be done by the caller on the resulting iterator,
    /// or by using `get_relations_to` with a filter closure.
    ///
    /// # Arguments
    /// * `target_id`: The ID of the target node.
    ///
    /// # Returns
    /// An `Option` containing an iterator yielding `&TreeRelation` if the target ID exists
    /// in the index, otherwise `None`.
    fn get_iter_relations_to<'a>(
        &'a self,
        target_id: &AnyNodeId,
    ) -> Option<impl Iterator<Item = &'a TreeRelation>> {
        self.relations_by_target().get(target_id).map(|indices| {
            // Use AnyNodeId key
            // Map indices directly to relation references
            indices
                .iter()
                .filter_map(|&index| self.tree_relations().get(index))
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
