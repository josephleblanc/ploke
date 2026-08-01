# Retrieval and LLM Crates

## Retrieval flow

```text
query text
  -> sparse BM25 search
  -> dense vector search
  -> fusion / ranking
  -> snippet assembly
  -> LLM prompt or tool response
```

## `ploke-rag`

`ploke-rag` owns retrieval strategies and context assembly. It should describe what context is useful without taking over low-level database or filesystem responsibilities.

## `ploke-db`

`ploke-db` provides the query/search substrate used by RAG.

## `ploke-io`

`ploke-io` reads source snippets for assembled context while preserving path and hash checks.

## `ploke-llm`

`ploke-llm` routes chat requests to configured providers and parses responses, including tool-call outcomes where applicable.
