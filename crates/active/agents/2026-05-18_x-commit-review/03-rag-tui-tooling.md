# x commit review: RAG/TUI tooling

## Findings

1. `request_code_context` now hard-caps retrieval by `cfg.rag.top_k`, which can starve explicit tool requests and drop `type_context` even when the token budget would allow more context. The new clamp is at `crates/ploke-tui/src/tools/request_code_context.rs:139-155`, and the new tests reinforce the low-cap path by setting `cfg.rag.top_k = 1` at `:292-296` and `:427-431`. I reproduced the regression with `cargo test -p ploke-tui --features typed_type_graph,test_harness request_code_context_tool_emits_matrix_type_context -- --nocapture`: it failed on `named_generic_argument_chrono_weekday_set_single_day` with a single returned snippet and no `type_context`.

## Open questions / test gaps

- The ignored live matrix test still matches `ToolCallCompleted` by parseable payload rather than by `call_id`, so it can theoretically latch onto the wrong completion if another tool fires in the same turn.
- `tool_io_roundtrip.rs` exercises `RequestCodeContextResult` only with `type_context: None`, so the optional provenance carrier itself is not round-tripped there.

## Summary

The rest of the touched scope looked structurally consistent. The main risk is the new config coupling on `request_code_context`; it changes production behavior, and the new matrix tests currently hide that by forcing the low-top-k path.
