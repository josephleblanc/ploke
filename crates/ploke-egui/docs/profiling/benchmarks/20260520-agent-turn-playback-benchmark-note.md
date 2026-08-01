cargo test benchmark filter verified the `ploke-egui` benchmark test surface; native interactive window behavior was not tested.

## Change Summary

This change updates `RunRecordSet` constructors touched by `ploke-egui` to include the new `agent_turn_records` field required by `ploke-tree` event-level playback drilldown.

## Verification

- `cargo test -p ploke-egui benchmark 2>&1 | tail -n 120`

## Baseline

No baseline report comparison was valid for this change. The edit is a record-constructor compatibility update, not a measured rendering or allocation change.

## Performance

- Allocation measurements: not measured; not requested.
- Native benchmark suite: not run; not requested.
- Measured improvement/regression: none claimed.

## Risk

The main unmeasured risk is record import size from carrying canonical agent-turn records through `ploke-tree`. The next useful check is a focused import/startup comparison only if this becomes visible in run-picker or graph-load measurements.
