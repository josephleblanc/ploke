# Scope
Campaign slice: `ploke-eval inspect protocol-overview --campaign rust-baseline-grok4-xai --status error`.

# Claims
- `target_family`: artifact/schema failures in `tool_call_intent_segmentation`.
- `why_this_family`: 9/9 error runs in the campaign slice land in the same family, and the aggregate triage shows `0/177` call reviews usable, so protocol interpretation is blocked at the artifact load boundary.
- `observed_pattern`: `inspect protocol-overview --instance <run>` fails immediately with `failed to deserialize protocol artifact ... missing field \`label\``.
- `suspected_root_cause`: at least one persisted `segments[]` entry is missing `label`, while the consumer still requires it.
- `code_surface`: `crates/ploke-eval/src/protocol_aggregate.rs:668-707` defines `RawAnchorSegment` with required `label`; `crates/ploke-eval/src/cli.rs:3644-3671` and `3672-3700` are the generation/load paths; `crates/ploke-eval/src/protocol_artifacts.rs:69-91` is the artifact summary branch for this procedure.
- `small_slice`: patch or regenerate the affected intent-segmentation artifacts so every segment carries `label`, then rerun protocol overview on the impacted runs.
- `medium_slice`: add a schema check on write/load for `tool_call_intent_segmentation` artifacts and fail with a clearer field-level diagnostic before aggregate inspection.
- `long_term`: version the protocol artifact schema and add round-trip tests for stored intent-segmentation payloads against the aggregate loader.
- `recommended_next_step`: fix the producer/schema mismatch for `segments[].label`, then rerun the campaign slice; expected payoff is turning this slice from 9/9 errors into loadable protocol reports.
- `confidence`: high on the failure mode, medium on whether the missing field is emitted by the producer or introduced by a drifted stored artifact.
- `exemplar runs reviewed`: `clap-rs__clap-3775`, `clap-rs__clap-3968`, `rayon-rs__rayon-986`.

# Evidence
- Campaign triage: 9/221 tracked runs selected; all 9 are `error`; top family is `artifact/schema failures 9/9`; representative exemplar is `clap-rs__clap-3775`.
- Instance checks:
  - `protocol-overview --instance clap-rs__clap-3775` => `missing field \`label\`` in `...tool_call_intent_segmentation...json`.
  - `protocol-overview --instance clap-rs__clap-3968` => same error on a different `clap` run.
  - `protocol-overview --instance rayon-rs__rayon-986` => same error on a non-`clap` run.
- Artifact inspection:
  - `clap-rs__clap-3775` has 5 segments; 1 segment is missing `label`.
  - `rayon-rs__rayon-986` has 6 segments; 1 segment is missing `label`.
- Relevant code:
  - `RawAnchorSegment` requires `label: String` in `protocol_aggregate.rs:696-707`.
  - `execute_protocol_intent_segments_quiet` and `ProtocolToolCallIntentSegmentsCommand::run` persist `segmented.output` unchanged in `cli.rs:3255-3259` and `3644-3681`.
  - `protocol_artifact_summary` only summarizes segment counts for this procedure, so it does not catch field-level drift in `protocol_artifacts.rs:69-91`.

# Unsupported Claims
- I did not inspect the upstream `segment::ToolCallIntentSegmentation` implementation to prove exactly where `label` is dropped.
- I did not verify whether all 9 error runs have the same missing segment index or the same model response shape.
- I did not run a rebuild or fix to confirm the smallest repair path.

# Not Checked
- Full contents of the upstream segmentation module that produces `segmented.output`.
- Whether a schema migration already exists for older intent-segmentation artifacts.
- Whether any non-error campaign runs are adjacent to this failure mode but hidden by the `status=error` filter.

# Risks
- A loader-side relaxation would mask bad artifacts and weaken schema guarantees.
- A producer fix alone will not recover already persisted malformed artifacts.
- If the missing field is introduced by stale stored outputs rather than current generation, reruns may still fail until the bad artifacts are regenerated or migrated.
