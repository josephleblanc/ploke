# model_id URL Bench Notes

Purpose
- Track what changed, expected outcomes, and how to run/interpret the URL-building benchmarks for model IDs.

Scope of Changes (done)
- Added alternative URL builders across all types (`String`, `SmolStr`, `ArrayString`, `Intern(lasso)`):
  - `endpoints_url` (baseline): `format!` + `percent_encoding`.
  - `endpoints_url_concat_known`: direct concat, only encode `:` as `%3A`, no `format!`.
  - `endpoints_url_prealloc_write`: preallocate output and `write!` percent-encoded ID (no `format!`).
  - `endpoints_url_replace`: build ID then `replace(":", "%3A")` and concat prefix/suffix.
- Files touched: `benches/model_id.rs`, `benches/common.rs`, `benches/model_id_tokio.rs`.
- Bench groups expanded to include the three new variants for sync and tokio interning benches.

Assumptions
- Only `:` requires encoding in the `{author}/{slug}[:variant]` path for OpenRouter endpoints.
- Repeated inputs simulate realistic hot caches where interning should shine; unique inputs stress cold paths.

How to Run (when build is green)
- Sync benches: `CARGO_TARGET_DIR=./local-target cargo bench -p ploke-tui --bench model_id`
- Tokio benches: `CARGO_TARGET_DIR=./local-target cargo bench -p ploke-tui --bench model_id_tokio`
- Focus on `url_build/repeated/*` and `end_to_end/repeated/*` groups for interning benefits.

What We Expect to Observe
- URL builders:
  - concat_known fastest; prealloc_write a close second; replace moderate; baseline slowest.
  - Gap wider on longer IDs or larger batches; smaller on short strings.
- Interning:
  - Repeated datasets: interning wins on parse + hashmap/end-to-end due to smaller keys and deduped storage.
  - Unique datasets: interning may be neutral/slightly slower (extra `resolve()`/lookups outweigh wins).
  - In `url_build/*`, interning overhead can come from `resolve()`; caching can help if reused.

Interpretation Guide (if results differ)
- If concat_known is not fastest: check allocations (capacity math), or verify that other chars are not being encoded; try prealloc_write variant.
- If interning loses even on repeated datasets: investigate contention on global interner, lack of pre-seeding, or hot `resolve()` cost; consider caching encoded ID or full URL.
- If replace beats prealloc_write: percent-encoding overhead is minimal under our constraints; replacing `:` may be “good enough”.

Next Steps
- Optionally add a cached encoded-id or full-URL field for interned type and benchmark reuse patterns.
- Add allocation counters (criterion profiler or custom counters) to confirm fewer allocs in concat_known/prealloc_write.
- Document stable results in `docs/reports/` once we can run benches.

