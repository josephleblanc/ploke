# Type Diagram

> Imported from the generated Hermes code wiki at `~/.hermes/wikis/ploke` (generated `2026-06-03T03:55:43Z`, source commit `97b1a101b9363b88b8e41f1447cb36d76a3eb8a2`). Verify implementation details against current source before making changes.


## Core Types

```mermaid
classDiagram
    class AppState {
        +ChatState chat
        +ConfigState config
        +SystemState system
        +Database db
        +EmbeddingRuntime embedder
        +IoManagerHandle io_handle
    }
    class StateCommand {
        <<enum>>
        AddUserMessage
        GenerateLlmResponse
        Index
        ProcessWithRag
        ApproveEdits
    }
    class RagService {
        +search_bm25(query, top_k, scope)
        +search(query, top_k, scope)
        +hybrid_search(query, top_k, scope)
        +get_context(query, top_k, budget, strategy, scope)
    }
    class Database {
        +init_with_schema()
        +raw_query(query)
        +get_snippet_context_nodes_ordered(nodes)
        +active_embedding_set
    }
    class EmbeddingRuntime {
        +current_active_set()
        +activate(db, new_set, new_embedder)
        +generate_embeddings(snippets)
        +dimensions()
    }
    class IndexerTask {
        +new(db, io, runtime, cancellation, handle, batch_size)
        +with_bm25_tx(bm25_tx)
    }
    class IoManagerHandle {
        +new()
        +builder()
        +get_snippets_batch(requests)
        +scan_changes_batch(requests)
        +write_snippets_batch(requests)
    }
    class RequestMessage {
        +Role role
        +String content
        +Option tool_call_id
        +Option tool_calls
        +validate()
    }
    class ChatStepOutcome {
        <<enum>>
        Content
        ToolCalls
    }
    class Procedure {
        <<trait>>
        +name()
        +run(subject)
    }
    class RunForest {
        +CampaignRef campaign
        +Vec roots
        +Vec nodes
        +Lanes lanes
        +PassiveEvidence passive_evidence
    }
    class Graph {
        +Option forest
        +HistoryIndex history
        +AuthorityIndex authority
        +ArtifactIndex artifacts
        +RuntimeIndex runtimes
    }

    StateCommand --> AppState : mutates through dispatcher
    AppState --> Database : owns
    AppState --> EmbeddingRuntime : owns
    AppState --> IoManagerHandle : owns
    AppState --> RagService : optional service
    RagService --> Database : searches
    RagService --> EmbeddingRuntime : embeds query
    RagService --> IoManagerHandle : fetches snippets
    IndexerTask --> Database : writes embeddings
    IndexerTask --> EmbeddingRuntime : generates vectors
    IndexerTask --> IoManagerHandle : reads snippets
    RequestMessage --> ChatStepOutcome : parsed into
    Procedure --> Graph : produces evidence for projections
    RunForest --> Graph : included in read-side graph
```

## Notes

- This is a Rust type diagram, not an inheritance hierarchy. Arrows show ownership/use relationships visible in the source.
- `Procedure` is a trait in `ploke-protocol`; concrete `Step`, `Sequence`, `FanOut`, and `Merge` implement or compose it.
- `RunForest` and `Graph` consume passive records through `ploke-tree`; they do not grant authority to mutate eval/runtime state.
