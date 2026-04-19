# OpenRouter Codestral Embedding 404 Forces Conversation-Only Fallback

## Summary

Live eval runs currently select the OpenRouter embedding model `mistralai/codestral-embed-2505`. In the `clap-rs__clap-4032` live run, the embedding request to `https://openrouter.ai/api/v1/embeddings` failed with HTTP `404` and `No successful provider responses`, which caused `request_code_context` to fail and the run to fall back to conversation-only prompting.

This is a real runtime regression, not just a reporting issue.

## Concrete evidence

Live run directory:

`~/.ploke-eval/runs/clap-rs__clap-4032/runs/run-1776554061157-structured-current-policy-62dc477c/`

Observed artifact evidence:

- In `agent-turn-summary.json`, the `request_code_context` tool failure records:
  - `dense search failed during BM25 fallback`
  - `status: 404`
  - `model=mistralai/codestral-embed-2505`
  - `url: "https://openrouter.ai/api/v1/embeddings"`
  - `No successful provider responses.`
- In the same run summary and trace, the constructed prompt begins with:
  - `No workspace context loaded; proceeding without code context. Index or load a workspace to enable RAG.`

Concrete snippets:

- `~/.ploke-eval/runs/clap-rs__clap-4032/runs/run-1776554061157-structured-current-policy-62dc477c/agent-turn-summary.json:591`
- `~/.ploke-eval/runs/clap-rs__clap-4032/runs/run-1776554061157-structured-current-policy-62dc477c/agent-turn-summary.json:596`
- `~/.ploke-eval/runs/clap-rs__clap-4032/runs/run-1776554061157-structured-current-policy-62dc477c/agent-turn-summary.json:30`

## Code references

`ploke-eval` explicitly selects the Codestral OpenRouter embedding set:

- `crates/ploke-eval/src/runner.rs:58`
  - `const OPENROUTER_CODESTRAL_MODEL: &str = "mistralai/codestral-embed-2505";`
- `crates/ploke-eval/src/runner.rs:930`
  - `starting_db_cache_metadata()` records the embedding model from `codestral_embedding_set()`
- `crates/ploke-eval/src/runner.rs:2339`
  - `codestral_embedding_set()` constructs `EmbeddingSet::new(... "openrouter", "mistralai/codestral-embed-2505", 1536 ...)`
- `crates/ploke-eval/src/runner.rs:2347`
  - `activate_codestral_runtime()` activates that embedding set in the app state

`ploke-tui` then builds a special OpenRouter embedding config for this exact model:

- `crates/ploke-tui/src/app_state/dispatcher.rs:36`
  - `openrouter_embedding_config(model, dims)` has a dedicated branch for `mistralai/codestral-embed-2505`
- `crates/ploke-tui/src/app_state/dispatcher.rs:491`
  - OpenRouter embedder construction uses that config when the provider contains `openrouter`

The conversation-only fallback is intentional once RAG context fetch fails:

- `crates/ploke-tui/src/rag/context.rs:164`
  - logs `RAG get_context failed; falling back to conversation-only prompt`
- `crates/ploke-tui/src/rag/context.rs:193`
  - uses the system fallback note `No workspace context loaded; proceeding without code context. Index or load a workspace to enable RAG.`

## Impact on eval behavior

- `request_code_context` becomes unavailable for the run path that depends on embeddings.
- The agent loses loaded-workspace RAG context and proceeds with conversation-only prompting.
- Retrieval quality drops sharply at the start of the run, before tool use can converge.
- This can distort downstream eval conclusions by turning a retrieval/runtime regression into apparent model/tool-search weakness.

## Minimal repro

```bash
cargo run -p ploke-eval -- prepare-msb-single --dataset ~/.ploke-eval/datasets/clap-rs__clap_dataset.jsonl --instance clap-rs__clap-4032
cargo run -p ploke-eval -- run-msb-agent-single --instance clap-rs__clap-4032
```

Then inspect the live run artifacts:

```bash
rg -n "codestral-embed-2505|No successful provider responses|No workspace context loaded" \
  ~/.ploke-eval/runs/clap-rs__clap-4032/runs/run-1776554061157-structured-current-policy-62dc477c/agent-turn-summary.json
```

## Notes

This report does not claim root cause beyond the observed regression. The concrete failure is that the eval path currently selects `mistralai/codestral-embed-2505` for OpenRouter embeddings, and that live request returned `404` during the reproduced run.
