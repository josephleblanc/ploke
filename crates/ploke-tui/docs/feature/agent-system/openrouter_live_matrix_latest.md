OpenRouter Live Matrix Summary (latest)

Artifacts directory (matrix):
- crates/ploke-tui/ai_temp_data/openrouter_matrix/

Artifacts directory (roundtrip):
- crates/ploke-tui/ai_temp_data/openrouter_roundtrip/

Notes
- Use metrics.jsonl and summary.json in the latest run-* folder under the matrix directory for per-combination results.
- Roundtrip runs will include request.json, error.txt or final.txt, and duration_ms.txt/not_validated.txt as applicable.

How to re-run
- Matrix: `cargo test -p ploke-tui --test live_openrouter_matrix --features live_api_tests -- --nocapture`
- Roundtrip: `cargo test -p ploke-tui --test live_openrouter_roundtrip --features live_api_tests -- --nocapture`

Environment overrides
- PLOKE_LIVE_MODELS: comma-separated model IDs
- PLOKE_LIVE_PROMPTS: "id::prompt||id2::prompt2"
- PLOKE_LIVE_MAX_COMBOS: maximum combinations per run

