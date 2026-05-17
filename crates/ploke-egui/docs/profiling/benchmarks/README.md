# Native Benchmarks

This directory holds small tracked benchmark summaries produced by the native
`ploke-egui` benchmark suite. Each benchmark run writes:

- `README.md`
- `report.json`

Short Markdown `*-benchmark-note.md` files are allowed for docs-only, test-only,
or small edits where the native benchmark is not run. Each note must name the
exact verification surface, baseline comparison status, performance increases,
regressions, and residual risk.

Large Puffin captures stay local under
`crates/ploke-egui/data/profiling/puffin/benchmarks/` and are referenced from
`report.json` by path, byte size, and SHA-256 hash.

Standard v1 command:

```sh
cargo run -p ploke-egui --features "dev native-benchmark" -- \
  --run-root /home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1 \
  --benchmark-suite standard
```
