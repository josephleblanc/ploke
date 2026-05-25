Findings:
- Provider attempt/retry/timeout and full-response sidecars are not covered by
  the agent-turn record family.
- Current live types are in `ploke-llm`; the run-local full-response sidecar is
  written by `ploke-tui` and copied by eval, while eval has private reader
  shapes. Do not reuse eval CLI-local projection as graph authority.
- `ploke-records` needs a separate passive owner for provider/full-response
  evidence before `ploke-tree` can ingest it.

Recommended next family:
- Add passive `llm_attempt` or `provider_observation` records in
  `ploke-records`.
- Load run-local `llm-full-responses.jsonl` through typed JSONL in `ploke-tree`.
- Treat global `prototype1_observation_*.jsonl` as explicit/log-root evidence,
  not something discovered by broad global scans.
