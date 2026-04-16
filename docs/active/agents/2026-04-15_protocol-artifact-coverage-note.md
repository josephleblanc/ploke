# Protocol Artifact Coverage Note

- date: 2026-04-15
- task title: protocol artifact coverage note
- task description: track current persisted protocol-artifact coverage across finished eval runs and record sampled sanity-check results
- related planning files: `docs/active/agents/2026-04-15_orchestration-hygiene-and-artifact-monitor.md`, `docs/active/CURRENT_FOCUS.md`

## Coverage Snapshot

Current on-disk inventory during the coverage pass:

- finished runs with `record.json.gz`: `39`
- runs with `protocol-artifacts/`: `39`
- remaining missing coverage: `0`

Remaining missing instances at this checkpoint:

- none

Operational note:

- coverage rose from `1` run with protocol artifacts at the start of the pass to full `39/39` coverage through the unattended CLI-driven generation loop plus the follow-up completion push
- the initial generation objective is now complete for the current finished-run set

## Generation Method

No-code CLI generation path used:

- build `ploke-eval`
- inspect existing protocol artifacts with `inspect protocol-artifacts`
- run protocol commands directly per instance, starting with `protocol tool-call-intent-segments`

This is intentionally using the supported CLI surface rather than ad hoc scripts
inside the codebase.

## First Sanity-Check Result

Sampled runs:

- `BurntSushi__ripgrep-1367`
- `tokio-rs__tokio-4789`

Observed result:

- both sampled intent-segmentation artifacts broadly align with the qualitative
  structure seen through `ploke-eval inspect` commands
- `ripgrep-1367` matched a 3-segment exploratory pattern:
  - `locate_target`
  - `refine_search`
  - `validate_hypothesis`
- `tokio-4789` matched a 3-segment search/inspect/edit pattern:
  - `refine_search`
  - `inspect_candidate`
  - `edit_attempt`

Limitations:

- this is a partial sanity-check only
- it validates broad trace-shape alignment, not semantic correctness of patching
  or final task success
- stronger validation should include additional persisted protocol artifact types
  when available
