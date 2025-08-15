// ... existing code ...
    /// Retrieves ordered embedding data for a list of target nodes.
    ///
    /// This method fetches the embedding data for a specific set of nodes identified by their UUIDs,
    /// returning the results in the same order as the input IDs. It includes file path, namespace,
    /// and other metadata needed for code understanding.
    ///
    /// # Arguments
    ///
    /// * `nodes` - A vector of UUIDs representing the nodes to retrieve
    ///
    /// # Returns
    ///
    /// A result containing a vector of `EmbeddingData` structs in the same order as the input UUIDs,
    /// or an error if the query fails.
    /// This is useful for retrieving the `EmbeddingData` required to retrieve code snippets from
    /// files after finding the Ids via a search method (dense embedding search, bm25 search)
    pub fn get_nodes_ordered(
        &self,
        nodes: Vec<Uuid>,
    ) -> Result<Vec<EmbeddingData>, ploke_error::Error> {
        tracing::debug!("get_nodes_ordered received {} node IDs: {:?}", nodes.len(), nodes);
        let ancestor_rules = Self::ANCESTOR_RULES;
        let has_embedding_rule = NodeType::primary_nodes().iter().map(|ty| {
            let rel = ty.relation_str();
            format!(r#"
            has_embedding[id, name, hash, span] := *{rel}{{id, name, tracking_hash: hash, span, embedding @ 'NOW' }}, !is_null(embedding)
            "#)
        }).join("\n");

        let script = format!(
            r#"
        target_ids[node_id, ordering] <- $data

        {ancestor_rules}

        {has_embedding_rule}

    batch[id, name, file_path, file_hash, hash, span, namespace, ordering] := 
        has_embedding[id, name, hash, span],
        ancestor[id, mod_id],
        *module{{id: mod_id, tracking_hash: file_hash}},
        *file_mod {{ owner_id: mod_id, file_path, namespace }},
        target_ids[id, ordering]

    ?[id, name, file_path, file_hash, hash, span, namespace, ordering] := 
        batch[id, name, file_path, file_hash, hash, span, namespace, ordering]
        :sort ordering
     "#
        );

        let ids_data: Vec<DataValue> = nodes
            .into_iter()
            .enumerate()
            .map(|(i, id)| {
                DataValue::List(vec![
                    DataValue::from(i as i64),
                    DataValue::Uuid(UuidWrapper(id)),
                ])
            })
            .collect();
        let limit = ids_data.len();

        // Create parameters map
        let mut params = BTreeMap::new();
        params.insert("data".into(), DataValue::List(ids_data));

        let query_result = self
            .db
            .run_script(&script, params, cozo::ScriptMutability::Immutable)
            .map_err(|e| DbError::Cozo(e.to_string()))?;
        let embedding_data = QueryResult::from(query_result).to_embedding_nodes()?;
        tracing::debug!("get_nodes_ordered returning {} nodes", embedding_data.len());
        Ok(embedding_data)
    }
// ... existing code ...
