# 2026-04-17 Failure Audit Packet

- task_id: `AUDIT-F1`
- title: failed eval run inventory and documentation reconciliation
- date: 2026-04-17
- owner_role: worker / verifier
- layer_workstream: `A2`
- related_hypothesis: parser/indexing failure families should be clustered and
  reconciled against existing limitation surfaces before new implementation work
- design_intent: separate true known limitations from undocumented failure
  families so follow-up implementation can target the real blockers
- scope:
  - inspect the `18` eval failures recorded in
    `~/.ploke-eval/campaigns/rust-baseline-grok4-xai/closure-state.json`
  - cluster them into concrete failure families
  - check whether each family already matches:
    - `docs/design/syn_parser_known_limitations.md`
    - `docs/design/known_limitations/`
    - existing bug docs under `docs/active/bugs/`
    - target/run-policy registry entries when applicable
  - identify which families are already documented and which are missing
- non_goals:
  - do not fix parser or runtime code
  - do not run new eval or protocol jobs
  - do not weaken correctness or reinterpret failures optimistically
- owned_files:
  - read-only analysis over campaign artifacts and existing docs
- dependencies:
  - `~/.ploke-eval/campaigns/rust-baseline-grok4-xai/closure-state.json`
  - run-local `parse-failure.json` and `indexing-status.json` artifacts
- acceptance_criteria:
  1. all `18` failed runs are accounted for in concrete clusters
  2. each cluster states its defended primary failure category and whether it
     matches an existing limitation / bug
  3. undocumented clusters are called out explicitly with the narrowest
     defendable description
- required_evidence:
  - exact instance ids per cluster
  - cited artifact paths and diagnostic summaries
  - cited matching limitation or bug docs when present
  - explicit statement when no matching limitation/bug doc was found
- report_back_location:
  - [failure-inventory.md](./failure-inventory.md)
- status: `in_progress`

## Known current cluster candidates

- `clap-rs__clap`: partial parse failures
- `nushell__nushell`: duplicate `crate::commands` module-tree path
- `nushell__nushell` and `serde-rs__serde`: `generic_lifetime` relation failure
- `nushell__nushell`: `indexing_completed` timeout
