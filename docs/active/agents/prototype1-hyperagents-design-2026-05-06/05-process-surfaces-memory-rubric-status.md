# Status

Prototype 1 already has one concrete low-risk self-improvement surface: tool-description text files. Those files are compiled into the TUI tool definitions through `include_str!`, are named by stable artifact-relative paths, and are the current `Mutated` partition in the History surface commitment.

The broader HyperAgents-like process layer exists mostly as documentation and projections: failure taxonomy, eval triage rubric, longitudinal metrics ledger, EDRs, postmortem conventions, and recent-activity summaries. Those files are useful process surfaces, but under the current ordinary descendant commitment model they are not admitted mutable surfaces.

There is no evidence that normal runs currently consume persistent memory summaries or performance summaries as input. Runs persist rich artifacts and summaries for inspection, batch control, and later human/CLI analysis, but the next run does not appear to load a learned memory/rubric/performance summary and condition behavior on it.

# Existing Pieces

## Tool-description surface

- `crates/ploke-core/src/tool_descriptions.rs:6` maps each `ToolName` to a markdown tool-description file through `include_str!`; the current files are under `crates/ploke-core/tool_text/*.md`. The same module exposes artifact-relative paths at `crates/ploke-core/src/tool_descriptions.rs:21`.
- `crates/ploke-tui/src/tools/mod.rs:566` builds the LLM-facing `ToolDefinition` from `Self::description()` and `Self::schema()`, so the included text is the actual tool-description prompt surface.
- `crates/ploke-core/src/tool_descriptions.rs:67` has an ignored regression test stating the important operational fact: runtime edits to these markdown files are invisible until rebuild because descriptions are baked in with `include_str!`.
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1339` derives the current mutated surface from `ToolName::ALL` and each tool's `description_artifact_relpath()`.
- Current tool text files found:
  - `crates/ploke-core/tool_text/apply_code_edit.md`
  - `crates/ploke-core/tool_text/cargo.md`
  - `crates/ploke-core/tool_text/code_item_edges.md`
  - `crates/ploke-core/tool_text/code_item_lookup.md`
  - `crates/ploke-core/tool_text/create_file.md`
  - `crates/ploke-core/tool_text/insert_rust_item.md`
  - `crates/ploke-core/tool_text/list_dir.md`
  - `crates/ploke-core/tool_text/non_semantic_patch.md`
  - `crates/ploke-core/tool_text/read_file.md`
  - `crates/ploke-core/tool_text/request_code_context.md`

## Prompt surfaces

- The TUI base system prompt is a static `PROMPT_HEADER` at `crates/ploke-tui/src/rag/context.rs:32`, and is pinned into chat history at `crates/ploke-tui/src/chat_history.rs:470` and `crates/ploke-tui/src/chat_history.rs:486`.
- The TUI also emits dynamic system messages: conversation-only fallback notes at `crates/ploke-tui/src/rag/context.rs:173` and `crates/ploke-tui/src/rag/context.rs:195`, and a focused-crate hint at `crates/ploke-tui/src/llm/manager/mod.rs:425` and `crates/ploke-tui/src/llm/manager/mod.rs:436`.
- `ChatPolicy.length_continue_prompt` is configurable at `crates/ploke-tui/src/user_config.rs:570` through `crates/ploke-tui/src/user_config.rs:581`, with the default prompt at `crates/ploke-tui/src/user_config.rs:903`.
- `ploke-eval` builds the benchmark issue prompt in code at `crates/ploke-eval/src/runner.rs:865` through `crates/ploke-eval/src/runner.rs:885`.
- Run records preserve prompt provenance slots but do not currently make prompt identity a required process surface: `AgentMetadata.system_prompt_version` and `tool_schema_version` are optional at `crates/ploke-eval/src/record.rs:657` through `crates/ploke-eval/src/record.rs:663`.

## Process/rubric/taxonomy docs

- `docs/active/workflow/README.md:1` defines `docs/active/workflow` as the live programme record, with key live entries listed at `docs/active/workflow/README.md:7` through `docs/active/workflow/README.md:19`.
- The canonical live failure taxonomy is `docs/active/workflow/failure-taxonomy.md`; its category table starts at `docs/active/workflow/failure-taxonomy.md:13` and classification rules are at `docs/active/workflow/failure-taxonomy.md:29`.
- The eval design document calls for automated triage classification at `docs/active/plans/evals/eval-design.md:138` through `docs/active/plans/evals/eval-design.md:143`, and defines the taxonomy expectation at `docs/active/plans/evals/eval-design.md:177` through `docs/active/plans/evals/eval-design.md:195`.
- A more operational eval triage rubric exists at `docs/workflow/evalnomicon/drafts/eval-triage-rubric.md:25` through `docs/workflow/evalnomicon/drafts/eval-triage-rubric.md:86`, with a compact decision procedure at `docs/workflow/evalnomicon/drafts/eval-triage-rubric.md:91` through `docs/workflow/evalnomicon/drafts/eval-triage-rubric.md:99`.
- EDR-0002 is an example of a prompt/rubric comparison process: it compares instruction-template variants against the same CLI evidence surface and shared rubric at `docs/active/workflow/edr/EDR-0002-cli-trace-review-skill-experiment.md:36` through `docs/active/workflow/edr/EDR-0002-cli-trace-review-skill-experiment.md:43`.

## Persistent run summaries and metrics

- Runs persist `record.json.gz` and related JSON artifacts. `ploke-eval` creates run-local paths such as `execution-log.json`, `repo-state.json`, and `record.json.gz` at `crates/ploke-eval/src/runner.rs:1665` through `crates/ploke-eval/src/runner.rs:1672`; agent mode also writes `agent-turn-trace.json`, `agent-turn-summary.json`, and `llm-full-responses.jsonl` at `crates/ploke-eval/src/runner.rs:2091` through `crates/ploke-eval/src/runner.rs:2101`.
- Agent-mode runs write the turn summary at `crates/ploke-eval/src/runner.rs:2379`.
- Batch runs write `batch-run-summary.json` at `crates/ploke-eval/src/runner.rs:2627` and persist instance-level results, record paths, turn summaries, and success/failure counts at `crates/ploke-eval/src/runner.rs:2665` through `crates/ploke-eval/src/runner.rs:2788`.
- `RunOutcomeSummary` captures status, outcome/verdict, turn count, token usage/cost, tool calls, and wall time at `crates/ploke-eval/src/record.rs:1437` through `crates/ploke-eval/src/record.rs:1464`.
- `record.json.gz` read/write helpers are implemented at `crates/ploke-eval/src/record.rs:1568` through `crates/ploke-eval/src/record.rs:1605`.
- Mechanized operational metrics are derivable from records. `OperationalRunMetrics` includes tool-call counts, patch apply state, submission artifact state, repair-loop indicators, convergence, and oracle eligibility at `crates/ploke-eval/src/operational_metrics.rs:36` through `crates/ploke-eval/src/operational_metrics.rs:69`.
- The longitudinal metrics ledger is designed as a central roll-up surface at `docs/active/workflow/longitudinal-metrics.md:1` through `docs/active/workflow/longitudinal-metrics.md:16`, but its proposed machine-readable companion and refresh path are still design/operations work at `docs/active/workflow/longitudinal-metrics.md:44` through `docs/active/workflow/longitudinal-metrics.md:66`.

## Implemented triage adjacent to evals

- General eval completeness is classified in code as `Complete`, `Partial`, `Failed`, or `Missing`, not as the failure-taxonomy categories. The classification logic is in `crates/ploke-eval/src/closure.rs:724` through `crates/ploke-eval/src/closure.rs:833`, with registration-aware handling at `crates/ploke-eval/src/closure.rs:836` through `crates/ploke-eval/src/closure.rs:895`.
- Protocol triage is implemented separately. `ProtocolCampaignTriageReport` carries campaign summary, issue kinds, problem families, exemplars, and next steps at `crates/ploke-eval/src/protocol_triage_report.rs:6` through `crates/ploke-eval/src/protocol_triage_report.rs:34`.
- Protocol call-review issue classification exists in `crates/ploke-eval/src/intervention/issue.rs:245` through `crates/ploke-eval/src/intervention/issue.rs:270`, using labels such as `search_thrash`, `partial_next_step`, and `no_clear_recovery`. This is useful for tool-description improvement, but it is not the documented general eval failure taxonomy.

## Current descendant mutation model

- The Prototype 1 state docs say the current concrete partition is `Immutable = crates/ploke-eval`, `Mutated = all tool-description text files`, and `Ambient = empty declared surface` at `crates/ploke-eval/src/cli/prototype1_state/mod.rs:101` through `crates/ploke-eval/src/cli/prototype1_state/mod.rs:109`.
- The same docs state ordinary self-improvement keeps the policy-bearing `ploke-eval` surface out of bounded edit scope until an explicit protocol-upgrade transition exists at `crates/ploke-eval/src/cli/prototype1_state/mod.rs:111` through `crates/ploke-eval/src/cli/prototype1_state/mod.rs:127`.
- The History docs repeat the rule: changing policy-bearing `ploke-eval` is a protocol upgrade/fork candidate, not an ordinary successor transition, at `crates/ploke-eval/src/cli/prototype1_state/history.rs:171` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:185`.
- `SurfaceCommitment` structurally models one immutable root and before/after mutated and ambient roots at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1455` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:1467`.
- The backend rejects ordinary succession if `crates/ploke-eval` changes, and only hashes tool-description files as the mutated surface at `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1217` through `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1257`.
- Tests pin the intended behavior: tool text mutation is allowed at `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1685` through `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1692`, while `ploke-eval` mutation is rejected at `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1695` through `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1704`.
- `ValidationPolicy::for_tool_description_target` allows exactly one known tool-description path and requires an existing, nonempty UTF-8 content change at `crates/ploke-eval/src/intervention/spec.rs:45` through `crates/ploke-eval/src/intervention/spec.rs:55`.
- The live materialization path resolves branch targets back to known tool-description relpaths at `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6144` through `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6154`.

# Gaps

- Prompt surfaces are mostly code/config, not ordinary descendant process surfaces. `PROMPT_HEADER`, benchmark issue prompt construction, focused-crate hints, and fallback notes are in `ploke-tui` or `ploke-eval` code. Editing most of them would either require changing policy-bearing code or adding a new admitted prompt-artifact surface.
- Tool-description markdown is editable, but because it is consumed through `include_str!`, the self-improvement loop must rebuild and run the successor binary before the changed text affects LLM calls. The ignored test at `crates/ploke-core/src/tool_descriptions.rs:67` is the clearest warning.
- Rubrics, taxonomy, validation checklists, EDRs, postmortems, and longitudinal metrics docs are not currently in the `Mutated` surface. They are process assets for humans/operators, not ordinary descendant-editable artifacts under History admission.
- There is no durable process-memory object that later runs consume. Current persisted artifacts are replay/inspection evidence (`record.json.gz`, turn summaries, batch summaries, protocol reports), not a learned memory injected into future prompt construction or selection.
- The general eval failure taxonomy is documented, and manual inventory files use it, but it is not implemented as a run-record field or automated classifier. Current code classifies eval closure completeness and protocol call-review issue families, which are adjacent but narrower.
- `PolicyConfigMutation` exists in the intervention spec at `crates/ploke-eval/src/intervention/spec.rs:70` through `crates/ploke-eval/src/intervention/spec.rs:77`, but I did not find a current ordinary descendant path that admits policy/config/rubric docs into the committed mutable surface.
- Performance summaries are derivable from records and batches, but the ledger automation path is still described rather than wired into normal run production/consumption.

# Recommended Next Slice

Smallest useful demo: a tool-description self-improvement loop over one protocol-reviewed failure family.

1. Choose one existing protocol issue family with strong evidence, such as repeated `non_semantic_patch` format/recovery failures or lookup/search repetition.
2. Use the existing intervention synthesis path to propose bounded rewrites for exactly one tool-description file. The synthesis prompt already asks for full replacement text and requires staying within one file at `crates/ploke-eval/src/intervention/synthesize.rs:113` through `crates/ploke-eval/src/intervention/synthesize.rs:156`; it fans out minimal/decision-rule/stronger strategies at `crates/ploke-eval/src/intervention/synthesize.rs:301` through `crates/ploke-eval/src/intervention/synthesize.rs:327`.
3. Materialize one selected candidate through the existing `ToolGuidanceMutation` path. The candidate construction uses `ValidationPolicy::for_tool_description_target` at `crates/ploke-eval/src/intervention/synthesize.rs:229` through `crates/ploke-eval/src/intervention/synthesize.rs:269`.
4. Rebuild the successor/runtime so the `include_str!` tool text actually changes.
5. Rerun a tiny fixed cohort and compare pre/post protocol issue counts plus `OperationalRunMetrics`. The result should be a process-surface win only if it reduces the targeted issue family without worsening tool-call failure count, patch apply state, or token/wall-time summary.

This slice fits the current authority model: ordinary descendants mutate only tool-description text, `ploke-eval` remains immutable, and the measurement comes from existing persisted records/protocol summaries. The next expansion after that should be an explicit `ProcessMemory` or `RubricArtifact` surface, not ad hoc docs mutation.
