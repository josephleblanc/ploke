# Prototype 1 baseline eval drops embedding overrides

Status: fixed in source, fresh live loop validation pending

## Broken Contract

Prototype 1 setup must preserve operator-selected eval embedding model/provider
through campaign admission, closure advancement, batch execution, and the
single-agent eval run that performs baseline indexing.

## Evidence

- Campaign:
  `p1-handofffix-5g1x2-a2-20260607-190702`
- Worktree:
  `/home/brasides/.ploke-eval/worktrees/p1-handofffix-5g1x2-a2-20260607-190702`
- Command that exposed the blocker:
  `./target/debug/ploke-eval loop prototype1-state --repo-root /home/brasides/.ploke-eval/worktrees/p1-handofffix-5g1x2-a2-20260607-190702 --campaign p1-handofffix-5g1x2-a2-20260607-190702 --debug-tools`
- Closure state:
  `/home/brasides/.ploke-eval/campaigns/p1-handofffix-5g1x2-a2-20260607-190702/closure-state.json`
- Batch summary:
  `/home/brasides/.ploke-eval/batches/prototype1/p1-handofffix-5g1x2-a2-20260607-190702/ripgrep-burntsushi-ripgrep-2209-eval-slice-20260608021147/batch-run-summary.json`

Observed baseline failure:

```text
database setup failed during 'embedding_model_preflight':
embedding preflight failed for 'mistralai/codestral-embed-2505':
Var error: Error from env variable, original: environment variable not found
```

The loop then stopped before child planning with:

```text
Parent<node-2e58ea711dd2446e> cannot spawn children:
baseline instance 'BurntSushi__ripgrep-2209' is Failed
```

## Source Trace

`prototype1-setup` exposes `--embedding-model-id` and
`--embedding-provider`, but the admitted campaign eval policy had no embedding
fields. `advance_eval_closure` then constructed `RunMsbAgentBatchRequest`
without embedding values, and `runner/msb_batch.rs` constructed each
`RunMsbAgentSingleRequest` with `embedding_model_id: None` and
`embedding_provider: None`.

That forced the eval runner through the default
`mistralai/codestral-embed-2505` embedding preflight even when the operator
intended to use an OpenRouter embedding route backed by the available
`OPENROUTER_API_KEY`.

## Docs / Policy Expectation

The setup CLI help advertises:

- `--embedding-model-id <EMBEDDING_MODEL_ID>`
- `--embedding-provider <PROVIDER>`

Those flags must affect the baseline eval run for the admitted campaign.

## Current Repro Coverage

Added:

- `agent_batch_request_preserves_embedding_overrides`
  Verifies the batch-to-single request boundary preserves embedding model and
  provider overrides.
- `loop_prototype1_setup_command_parses`
  Verifies the operator-facing setup command parses the embedding flags.
- `prototype1_setup_campaign_manifest_preserves_embedding_overrides`
  Verifies `prototype1-setup` persists embedding overrides into the campaign
  manifest and resolved campaign config.
- `prototype1_eval_set_id_includes_embedding_overrides`
  Verifies eval-set identity changes when the eval embedding model/provider
  selection changes.

Verification passed:

```bash
RUSTFLAGS=-Awarnings cargo test -p ploke-eval agent_batch_request_preserves_embedding_overrides -- --nocapture
RUSTFLAGS=-Awarnings cargo test -p ploke-eval loop_prototype1_setup_command_parses -- --nocapture
RUSTFLAGS=-Awarnings cargo test -p ploke-eval prototype1_setup_campaign_manifest_preserves_embedding_overrides -- --nocapture
RUSTFLAGS=-Awarnings cargo test -p ploke-eval prototype1_eval_set_id_includes_embedding_overrides -- --nocapture
RUSTFLAGS=-Awarnings cargo check -p ploke-eval
```

## Missing Repro / Validation

- A fresh campaign must be admitted with an explicit embedding model/provider
  and reach past baseline eval before this blocker is fully cleared for the
  live loop.

## Fix Direction

Carry embedding selection through the existing production carriers:

- add optional embedding fields to `EvalCampaignPolicy`;
- populate them from `prototype1-setup`;
- forward them from closure eval into `RunMsbAgentBatchRequest`;
- forward them from batch execution into `RunMsbAgentSingleRequest`.

Do not silently change the default embedding model or make failed baseline eval
state acceptable to the same campaign.

Implemented source fix:

- `EvalCampaignPolicy` now carries optional embedding model/provider fields.
- `prototype1-setup` stores the embedding flags in the admitted campaign
  manifest.
- Closure baseline eval forwards those fields into `RunMsbAgentBatchRequest`.
- Agent batch execution forwards them into every `RunMsbAgentSingleRequest`.
- Prototype 1 eval-set identity includes the embedding model/provider selection.

## Disposition

`p1-handofffix-5g1x2-a2-20260607-190702` is abandoned for loop progress. It has
persisted required baseline eval failure evidence, so the next live validation
must use a fresh campaign/worktree after the source fix is committed.
