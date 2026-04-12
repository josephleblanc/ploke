# P0C0 Query Builder Survey Report

## implemented

- Surveyed the current `ploke-db` query-construction surface, historical query entrypoints, and representative `ploke-eval` / `ploke-tui` call sites.
- Confirmed that `QueryBuilder` is a fragment-assembly surface rather than a live execution path, and that the timestamp-aware helper currently in active use is `Database::raw_query_at_timestamp(...)`.
- Compared that surface to the current `ploke-eval` replay helpers and nearby raw-script callers to determine where `P0C` should attach.

## claims

- `P0C` should deliberately bypass `QueryBuilder` and use the narrow `raw_query_at_timestamp()` / `DbState` path instead.
- `P0D` and `P0E` should continue to ride the same timestamp-aware helper path rather than trying to force a builder expansion into this sprint.
- A future type-state or more typed historical-query builder direction is a separate redesign packet, not an incremental `P0C` change.

## evidence

- `QueryBuilder` has no live execution surface and no timestamp parameter; its `execute()` remains commented out in [builder.rs](</home/brasides/code/ploke/crates/ploke-db/src/query/builder.rs:496>).
- The only timestamp-shaped builder helper still hardcodes `@ 'NOW'` in [builder.rs](</home/brasides/code/ploke/crates/ploke-db/src/query/builder.rs:217>).
- `ploke-db` already exposes a timestamp-aware query entrypoint in [database.rs](</home/brasides/code/ploke/crates/ploke-db/src/database.rs:1758>) and [database.rs](</home/brasides/code/ploke/crates/ploke-db/src/database.rs:1786>).
- `ploke-eval` history helpers already route through that path via [record.rs](</home/brasides/code/ploke/crates/ploke-eval/src/record.rs:923>), [record.rs](</home/brasides/code/ploke/crates/ploke-eval/src/record.rs:985>), and [record.rs](</home/brasides/code/ploke/crates/ploke-eval/src/record.rs:1089>).
- Representative active `ploke-tui` DB call sites still use raw `@ 'NOW'` scripts rather than `QueryBuilder` in [app_state/database.rs](</home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs:149>), [app_state/database.rs](</home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs:192>), and [app_state/database.rs](</home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs:2684>).
- Historical query behavior already has a unit-test anchor at [database.rs](</home/brasides/code/ploke/crates/ploke-db/src/database.rs:4874>).

## unsupported_claims

- This report does not prove there are zero production `QueryBuilder` consumers outside the sampled files.
- This report does not claim the current `raw_query_at_timestamp()` implementation is the final long-term API.
- This report does not include test execution.

## not_checked

- Feature-gated or macro-generated `ploke-db` query call sites outside the sampled surface.
- Whether any other crates consume `QueryBuilder` indirectly.
- The exact pending code path for `P0C` implementation, since this was a pre-implementation survey.

## risks

- `raw_query_at_timestamp()` currently relies on string replacement of `@ 'NOW'`, so malformed or unusual scripts can still be surprising.
- If we standardize on the narrow helper path without one small wrapper, raw script variation can proliferate.
- A future typed historical-query composition layer may still be warranted, but that should not be smuggled into `P0C`.

## next_step

- Treat `P0C0` as accepted and implement `P0C` against `Database::raw_query_at_timestamp()` / `DbState`, not `QueryBuilder`.
- Keep `P0D` and `P0E` scoped to the same helper path so the replay/introspection story stays coherent.
- If builder redesign remains desirable, seed a separate follow-up packet after the primary P0 lane is no longer blocked.
