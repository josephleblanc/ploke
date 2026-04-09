# Eval Postmortem Template Review

- date: 2026-04-08
- task title: Ripgrep Batch Postmortem Template Review
- task description: Review the batch postmortem template and current artifact structure after reading the completed `ripgrep-all` instance reports.
- related planning files:
  - [2026-04-08_postmortem-template.md](/home/brasides/code/ploke/docs/active/agents/2026-04-08_eval-postmortems/2026-04-08_postmortem-template.md)
  - [2026-04-08_batch-postmortem-index.md](/home/brasides/code/ploke/docs/active/agents/2026-04-08_eval-postmortems/2026-04-08_ripgrep-all-batch/2026-04-08_batch-postmortem-index.md)
  - [2026-04-08_batch-postmortem-meta-notes.md](/home/brasides/code/ploke/docs/active/agents/2026-04-08_eval-postmortems/2026-04-08_ripgrep-all-batch/2026-04-08_batch-postmortem-meta-notes.md)

## Review

The current template is close, but it still lets authors mix authoritative and non-authoritative artifacts, blur local run success with official benchmark success, and reconstruct the timeline from noisy logs by hand. The reports show the same failure modes in different forms:

- [BurntSushi__ripgrep-2209](./BurntSushi__ripgrep-2209/2026-04-08_BurntSushi__ripgrep-2209_minimax-m2.5_postmortem.md) had a good diagnosis, but the report still had to infer truth from a mix of trace, summary, and submission artifacts.
- [BurntSushi__ripgrep-2610](./BurntSushi__ripgrep-2610/2026-04-08_BurntSushi__ripgrep-2610_postmortem.md) explicitly warned that the trace looked stale or partial, which means the template should force that judgment into a dedicated trust section.
- [BurntSushi__ripgrep-2626](./BurntSushi__ripgrep-2626/2026-04-08_BurntSushi__ripgrep-2626.md) shows how easy it is to treat `success` and `applied: true` as a complete verdict even when `final_assistant_message` is missing and `expected_file_changes` is inconsistent.
- [BurntSushi__ripgrep-454](./BurntSushi__ripgrep-454/2026-04-08_BurntSushi__ripgrep-454_qwen-qwen3.6-plus_alibaba_postmortem.md) shows a third state the template should name explicitly: the run can abort cleanly with an empty submission artifact and no patch.
- [BurntSushi__ripgrep-1980](./BurntSushi__ripgrep-1980/2026-04-08_BurntSushi__ripgrep-1980_qwen-qwen3.6-plus_alibaba_postmortem.md) shows that lookup churn and embedding-provider failure can dominate a run before any patch is emitted, so the template should capture that as distinct from plain model drift.
- [BurntSushi__ripgrep-2626](./BurntSushi__ripgrep-2626/2026-04-08_BurntSushi__ripgrep-2626_qwen-qwen3.6-plus_alibaba_postmortem.md) adds a useful contract-mismatch case: an unsupported tool name can abort the session before any patch exists, so invalid-tool attempts deserve their own report field.
- [BurntSushi__ripgrep-1642](./BurntSushi__ripgrep-1642/2026-04-08_BurntSushi__ripgrep-1642_qwen3.6plus-alibaba_indexing-timeout_postmortem.md) and the other indexing-timeout runs show a pre-turn failure mode that the template does not name explicitly.

## Top 3 Improvements

1. Add an `artifact trust` subsection that names the authoritative artifact, lists stale or partial artifacts, and states why they are trusted or not trusted.
2. Add explicit fields for `first tool failure`, `first recovery step`, `first validation failure`, and `official benchmark verdict`, so authors do not reconstruct the chronology from log search.
3. Require `run.json`-sourced identity fields in the header, especially `base sha`, `instance id`, and `expected_patch_files`, because memory or log-envelope values are easy to misstate.
4. Add a dedicated `invalid tool attempt` field so runs that fail on unsupported tool names are not conflated with normal code or lookup errors.

## What Is Missing Or Easy To Misread

- `agent-turn-summary.json` still does not preserve the final assistant message content reliably, so reports must fall back to the trace or raw eval log.
- `agent-turn-trace.json` can show `patch_artifact.expected_file_changes` as empty even when the submission JSONL clearly changed the target file.
- The template does not currently force a distinction between local run completion and official Multi-SWE-bench evaluation status.
- The stable eval log is useful as an evidence source, but it is too noisy to serve as the only timeline source.
- Reports need a standard place to say whether the trace is complete, stale, or partial; otherwise readers have to infer that from narrative prose.
- There is no dedicated slot for a pre-turn indexing timeout, so the author has to explain that no assistant turn ever happened.
- `run.json.selected_provider` can be null even when the batch summary has the actual provider, so the template should explicitly tell authors where to recover provider provenance.

## Structural Notes

- The batch index is doing the right thing by assigning one report per instance, but the template should better support cross-report comparison by standardizing the same evidence fields in every report.
- The review sections work, but they should explicitly call out whether the report is based on a single tight event slice or on broad log search.
- The current template asks for evidence, but it should more strongly discourage using source-code links as evidence unless they are tied to a concrete artifact or log event.

## Recommended Template Edits

- Add a required `artifact trust` block under `Outcome Snapshot`.
- Add a required `report confidence limits` line under `Failure Classification`.
- Add a required `first failure` block with separate tool, recovery, and validation entries.
- Add a required `official benchmark follow-through` block that distinguishes local completion from downstream evaluator output.
- Add a note in the header to pull identity fields from `run.json`, not from the stable log banner.
- Add a `pre-turn failure` block for runs that never reach an assistant turn because indexing timed out.
- Add a required `invalid tool attempt` field when the run hits a schema violation like an unsupported tool name.
- Add a note that batch-level summary data may be the authoritative source for provider selection when `run.json` omits it.

## Missing Or Low-Salience Information

- There is no single field that reports `final_assistant_message` across the batch, so the reports have to use trace inspection for that detail.
- There is no one-line indicator for whether the patch actually touched the expected target file.
- There is no standard summary of how many tool retries were needed before recovery.
- There is no standard place for the exact evaluator result, if a downstream benchmark pass exists later.
- There is no standard place to record that a submission artifact exists but carries an empty `fix_patch`, which is materially different from a missing artifact.
- There is no dedicated slot for an upstream embedding or context-assembly failure that blocks recovery even when the model is still trying to reason about the right problem.

## Bottom Line

The template should move from "write a good narrative" toward "capture a small set of authoritative facts plus a narrative." That would make the reports easier to compare, easier to trust, and less dependent on manual inference from noisy artifacts.
