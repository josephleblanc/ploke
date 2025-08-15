// ... existing code ...
    #[tokio::test]
    async fn test_search() -> Result<(), Error> {
        use tracing::{debug, info, instrument};

        // Initialize tracing for the test
        ploke_test_utils::init_test_tracing(Level::TRACE);

        let search_term = "use_all_const_static";
        let span = tracing::span!(Level::DEBUG, "test_search", search_term = %search_term);
        let _enter = span.enter();
        
        let db = TEST_DB_NODES
            .as_ref()
            .expect("Incorrect setup of TEST_DB_NODES")
            .clone();
        debug!("Loaded test database with {} embedded nodes", db.count_pending_embeddings()?);

        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = Arc::new(EmbeddingProcessor::new(source));
        let rag = RagService::new(db.clone(), embedding_processor)?;

        // Shouldn't need to upsert the nodes here since it will have been done alongside the dense
        // vector embedding process.
        // Tests for that are in `ploke-embed/src/indexer/tests.rs`

        debug!("Initializing RAG service...");
        let search_res: Vec<(Uuid, f32)> = rag.search(search_term, 15).await?;
        debug!("Search returned {} results", search_res.len());
        
        let ordered_node_ids: Vec<Uuid> = search_res.iter().map(|(id, _score)| *id).collect();
        debug!("Fetching nodes for IDs: {:?}", ordered_node_ids);
        
        // --- New Debug Verification ---
        if !ordered_node_ids.is_empty() {
            debug!("Verifying node IDs exist in database...");
            let mut verify_params = BTreeMap::new();
            let id_list: Vec<DataValue> = ordered_node_ids.iter().map(|id| DataValue::Uuid(UuidWrapper(*id))).collect();
            verify_params.insert("ids".to_string(), DataValue::List(id_list));
            
            let verify_script = r#"
                // Check if IDs exist in any primary node table at any time
                ?[id, name, ty, at] := 
                    input_id in $ids,
                    *function { id: input_id, name, at },
                    ty = "function"
                ?[id, name, ty, at] := 
                    input_id in $ids,
                    *struct { id: input_id, name, at },
                    ty = "struct"
                ?[id, name, ty, at] := 
                    input_id in $ids,
                    *enum { id: input_id, name, at },
                    ty = "enum"
                ?[id, name, ty, at] := 
                    input_id in $ids,
                    *trait_def { id: input_id, name, at },
                    ty = "trait_def"
                ?[id, name, ty, at] := 
                    input_id in $ids,
                    *impl_block { id: input_id, name, at },
                    ty = "impl_block"
                ?[id, name, ty, at] := 
                    input_id in $ids,
                    *module { id: input_id, name, at },
                    ty = "module"
                ?[id, name, ty, at] := 
                    input_id in $ids,
                    *const_static { id: input_id, name, at },
                    ty = "const_static"
                ?[id, name, ty, at] := 
                    input_id in $ids,
                    *type_alias { id: input_id, name, at },
                    ty = "type_alias"
                ?[id, name, ty, at] := 
                    input_id in $ids,
                    *macro_def { id: input_id, name, at },
                    ty = "macro_def"
            "#;
            
            match db.run_script(verify_script, verify_params, cozo::ScriptMutability::Immutable) {
                Ok(named_rows) => {
                    debug!("Verification query returned {} rows", named_rows.rows.len());
                    if named_rows.rows.is_empty() {
                        debug!("WARNING: None of the returned node IDs exist in any primary node table!");
                    } else {
                        for row in &named_rows.rows {
                            if row.len() >= 4 {
                                debug!("Found ID: {:?}, Name: {:?}, Type: {:?}, At: {:?}", 
                                    row.get(0), row.get(1), row.get(2), row.get(3));
                            }
                        }
                    }
                }
                Err(e) => {
                    debug!("Error running verification query: {:?}", e);
                }
            }
        }
        // --- End New Debug Verification ---
        
        // Add detailed tracing for the database query
        let span = tracing::span!(Level::DEBUG, "get_nodes_ordered", node_ids = ?ordered_node_ids);
        let _enter = span.enter();
        
        let node_info: Vec<EmbeddingData> = {
            debug!("Calling db.get_nodes_ordered with {} IDs", ordered_node_ids.len());
            let result = db.get_nodes_ordered(ordered_node_ids.clone());
            debug!("db.get_nodes_ordered returned: {:?}", result.as_ref().map(|v| v.len()));
            result?
        };
        
        debug!("Retrieved {} nodes from database", node_info.len());
        
        // Log each node's details
        for (i, node) in node_info.iter().enumerate() {
            debug!("Node {}: id={}, name={}, file_path={}", 
                   i, node.id, node.name, node.file_path.display());
        }

        let io_handle = IoManagerHandle::new();
        
        // Add tracing for snippet retrieval
        let span = tracing::span!(Level::DEBUG, "get_snippets_batch", node_count = node_info.len());
        let _enter = span.enter();
        
        let snippet_results: Vec<Result<String, Error>> = io_handle
            .get_snippets_batch(node_info)
            .await
            .expect("Problem receiving");
        
        debug!("Received {} snippet results", snippet_results.len());
        
        let mut snippets: Vec<String> = Vec::new();
        for (i, snip) in snippet_results.into_iter().enumerate() {
            let snip_ok = snip?;
            debug!("Snippet {}: {} chars, preview: '{}...'", 
                   i + 1, snip_ok.len(), &snip_ok.chars().take(50).collect::<String>());
            snippets.push(snip_ok);
        }
        
        debug!("Total snippets collected: {}", snippets.len());
        
        let snippet_match = snippets.iter().find(|s| s.contains(search_term));
        debug!("Found matching snippet: {}", snippet_match.is_some());
        
        if let Some(match_snippet) = snippet_match {
            debug!("Matching snippet preview: '{}...'", 
                   &match_snippet.chars().take(100).collect::<String>());
        } else {
            debug!("Available snippets:");
            for (i, snippet) in snippets.iter().enumerate() {
                debug!("Snippet {}: '{}...'", i, &snippet.chars().take(50).collect::<String>());
            }
        }
        
        debug!("Loaded test database with {} embedded nodes", db.count_pending_embeddings()?);
        assert!(snippet_match.is_some(), "No snippet found containing '{}'", search_term);
        info!("Test completed successfully - found matching snippet for '{}'", search_term);
        Ok(())
    }
    //
}
