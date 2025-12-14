# Todos for performance profiling

Created: 2025-12-14

Referenced (with date) in `ploke/docs/active/TECH_DEBT.md`

## Todo items

- [ ] UI frame-related items during different activity levels
  - [ ] idle
  - [ ] heavy conversation message load
  - [ ] simulated or actual resizing
  - [ ] during background activity (to verify non-UI events non-blocking)
- [ ] User-facing sources of potential latency for interactions
  - [ ] profiling of time taken for model picker to load (e.g. what
  percentage of time is the network request, which we cannot control, vs
  latency introduced by our systems, which we can)
  - [ ] profiling of any expensive processes to identify inefficiencies, e.g.
    - [ ] local embedding
    - [ ] database startup
- [ ] Database performance: need clear picture of performance of longer
queries executed within `cozo::Db` vs. retrieving longer sets of nodes and
sorting through them within the project, e.g. filtering through an iterator
or similar.
  - [ ] ideally profile each query we are adding to our `ploke_db::Database`
  methods for various numbers of nodes/edges in code graph

## Review Ad hoc performance tests

- [ ] `ploke/crates/ploke-tui/src/tests/ui_performance_comprehensive.rs`

- [ ] `ploke/crates/ploke-tui/benches/`
  - [ ] `common.rs`
  - [ ] `model_id.rs`
  - [ ] `model_id_bench_notes.md`
  - [ ] `model_id_tokio.rs`
  - [ ] `ui_measure.rs`

- [ ] `ploke/crates/ploke-db/benches/resolver_bench.rs`
