# Module Map

This is a broad map for implementers. Use [Source Map](source-map.md) for the
current Prototype 1 typestate-specific anchors.

| Area | Source paths |
| --- | --- |
| CLI args | `src/cli/args/` |
| CLI handlers/dispatch | `src/cli/handlers/`, `src/cli/dispatch.rs` |
| Run manifests and execution | `src/runner/`, `src/run_history.rs` |
| Multi-SWE-bench support | `src/mbe/`, `src/msb.rs`, `src/runner/msb_*` |
| Campaigns and closure | `src/campaign.rs`, `src/closure.rs`, `src/cli/handlers/campaign.rs` |
| Model/provider state | `src/model_registry.rs`, `src/provider_prefs.rs`, `src/cli/provider.rs` |
| Protocol artifacts/reports | `src/protocol/`, `src/protocol_*.rs` |
| Intervention/scheduler | `src/intervention/` |
| Successor selection | `src/successor_selection/` |
| Prototype 1 state | `src/cli/prototype1_state/` |
| Prototype 1 process seam | `src/cli/prototype1_process.rs` |
| Replay/inspection | `src/replay/` |
| Metrics | `src/metric.rs`, `src/operational_metrics.rs`, `src/protocol/metrics.rs` |

## Change hygiene

When changing a command or record surface, update all three layers when needed:

1. source-owned schema/handler;
2. operator docs in this book;
3. tests or fixtures that assert the behavior.

Do not relax validation or silently accept schema drift unless that tradeoff is
explicitly approved.
