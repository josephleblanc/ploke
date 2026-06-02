# Test Quality Review

## Findings
1. `crates/ploke-tui/src/tools/request_code_context.rs:534-556` - The payload check is substring-based, so it can pass on the wrong context part. Matching `canon_path` or `snippet` against a short label like `T`, `Item`, or `Map` is not unique enough to prove the target row was found.
2. `crates/ploke-tui/src/tools/request_code_context.rs:483-528` - The ignored live test does not tie `ToolCallCompleted` back to the specific request it emitted. Any parsable completion on the bus can satisfy the wait loop, which leaves room for stale or concurrent tool traffic to produce a false positive.
3. `crates/ploke-rag/src/core/unit_tests.rs:1204-1249` - The shared-matrix RAG test stops at `expand_hits_with_type_context` and a mock embedder path for TUI rows. It never exercises `RagService::get_context`, so it proves helper expansion, not the public retrieval assembly path described by the module docs.
4. `crates/ploke-rag/src/core/unit_tests.rs:1317-1318,1373-1376,1434-1437` and `crates/ploke-tui/src/tools/request_code_context.rs:292-295,427-430` - Several matrix consumers are overfit to `top_k = 1` and exact search phrases. That makes them sensitive to BM25 tokenization/ranking details instead of the production contracts they are supposed to protect.

## Open Questions
- Should the live tool test assert the request/completion identifiers as well as the payload body?
- Do you want one matrix row to go through `get_context` with a broader retrieval budget so the public assembly path is covered directly?

## Focused Reruns
- `cargo test -p ploke-db --features typed_type_graph corpus_matrix -- --nocapture`
- `cargo test -p ploke-rag --features typed_type_graph corpus_type_shape_matrix -- --nocapture`
- `cargo test -p ploke-tui --features typed_type_graph,test_harness request_code_context_tool_emits_matrix_type_context -- --nocapture`
- `cargo test -p ploke-tui --features typed_type_graph,test_harness live_request_code_context_matrix_uses_production_tool_payload -- --ignored --nocapture`

<oai-mem-citation>
<citation_entries>
MEMORY.md:935-939|note=[shared matrix rollout context and live tool payload validation]
MEMORY.md:974-978|note=[matrix authority and verified command set for reruns]
</citation_entries>
<rollout_ids>
019e34c3-a241-7e93-9236-ee3b9877b2db
019e3461-3a38-7f52-97f2-ab2faccad5ae
</rollout_ids>
</oai-mem-citation>
