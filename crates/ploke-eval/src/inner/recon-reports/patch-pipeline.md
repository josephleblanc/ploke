**Packet**
**Runtime Sites**
- Production setup-only:
  - [runner.rs:1125](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:1125) `RunMsbSingleRequest::run`
- Production patch-generation:
  - [runner.rs:1454](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:1454) `RunMsbAgentSingleRequest::run`
- Batch wrapper over those two:
  - [runner.rs:1892](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:1892) `run_batch`
- Replay/debug runtime:
  - [runner.rs:2166](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:2166) `setup_replay_runtime`
  - [runner.rs:2085](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:2085) `ReplayMsbBatchRequest::run`
- Tests-only direct construction:
  - [tests/replay.rs:434](/home/brasides/code/ploke/crates/ploke-eval/src/tests/replay.rs:434)
  - [runner.rs:3217](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:3217)

**Main Duplication**
- Shared between setup-only and agent path:
  - manifest load
  - repo checkout
  - DB init or cache restore
  - `XDG_CONFIG_HOME` sandbox
  - codestral embedder setup
  - `TestRuntime` construction
  - indexing or cached-workspace seeding
  - setup artifacts
  - snapshot writing
  - `record.json.gz`
  - `multi-swe-bench-submission.jsonl`
- Main divergence:
  - setup-only stops after setup artifacts
  - agent path adds `spawn_llm_manager`, `spawn_observability`, `run_benchmark_turn`, turn artifacts, full-response trace

**Core Patch-Generation Domain**
- Binary entry:
  - [main.rs:6](/home/brasides/code/ploke/crates/ploke-eval/src/main.rs:6)
- CLI dispatch into this domain:
  - [cli.rs:296](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:296)
  - [cli.rs:2028](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:2028) `run-msb-single`
  - [cli.rs:2078](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:2078) `run-msb-agent-single`
- Prepared input types:
  - [spec.rs:69](/home/brasides/code/ploke/crates/ploke-eval/src/spec.rs:69) `PrepareSingleRunRequest`
  - [spec.rs:98](/home/brasides/code/ploke/crates/ploke-eval/src/spec.rs:98) `PreparedSingleRun`
  - [spec.rs:80](/home/brasides/code/ploke/crates/ploke-eval/src/spec.rs:80) `RunSource`
- MSB mapping into prepared runs:
  - [msb.rs:71](/home/brasides/code/ploke/crates/ploke-eval/src/msb.rs:71)
  - [msb.rs:130](/home/brasides/code/ploke/crates/ploke-eval/src/msb.rs:130)
- Execution:
  - [runner.rs:1125](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:1125)
  - [runner.rs:1454](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:1454)
- Run record:
  - [record.rs:1518](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:1518)

**Where The Local Path Ends**
- Per-run local outputs under `runs/<instance>`:
  - setup-only:
    - `execution-log.json`
    - `repo-state.json`
    - `indexing-status.json`
    - `snapshot-status.json`
    - checkpoints
    - `record.json.gz`
    - optional `multi-swe-bench-submission.jsonl`
  - agent:
    - all of the above
    - `agent-turn-trace.json`
    - `agent-turn-summary.json`
    - `llm-full-responses.jsonl`
- Submission payload creation:
  - [runner.rs:821](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:821) `build_msb_submission_record`
  - [runner.rs:836](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:836) `collect_submission_fix_patch`

**Configuration Surfaces**
- Explicit setup structs:
  - [spec.rs:69](/home/brasides/code/ploke/crates/ploke-eval/src/spec.rs:69) `PrepareSingleRunRequest`
  - [spec.rs:98](/home/brasides/code/ploke/crates/ploke-eval/src/spec.rs:98) `PreparedSingleRun`
  - [msb.rs:15](/home/brasides/code/ploke/crates/ploke-eval/src/msb.rs:15) `PrepareMsbSingleRunRequest`
  - [msb.rs:25](/home/brasides/code/ploke/crates/ploke-eval/src/msb.rs:25) `PrepareMsbBatchRequest`
  - [campaign.rs:25](/home/brasides/code/ploke/crates/ploke-eval/src/campaign.rs:25) `CampaignManifest`
- Persisted preference/config files:
  - [layout.rs:50](/home/brasides/code/ploke/crates/ploke-eval/src/layout.rs:50) `models/registry.json`
  - [layout.rs:54](/home/brasides/code/ploke/crates/ploke-eval/src/layout.rs:54) `models/active-model.json`
  - [layout.rs:58](/home/brasides/code/ploke/crates/ploke-eval/src/layout.rs:58) `models/provider-preferences.json`
  - [layout.rs:70](/home/brasides/code/ploke/crates/ploke-eval/src/layout.rs:70) starting-db cache root
- Ambient root:
  - [layout.rs:7](/home/brasides/code/ploke/crates/ploke-eval/src/layout.rs:7) `PLOKE_EVAL_HOME`
- Model/provider resolution:
  - [model_registry.rs:194](/home/brasides/code/ploke/crates/ploke-eval/src/model_registry.rs:194) explicit model -> default model -> active model file
  - [provider_prefs.rs:66](/home/brasides/code/ploke/crates/ploke-eval/src/provider_prefs.rs:66) provider prefs by model id
  - [runner.rs:1135](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:1135) run-time provider selection
- Hardcoded run policy:
  - [spec.rs:22](/home/brasides/code/ploke/crates/ploke-eval/src/spec.rs:22) default budget
  - [runner.rs:61](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:61) benchmark chat policy
  - [runner.rs:144](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:144) run-arm labels
  - [runner.rs:57](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:57) hardcoded codestral embedding preset

**Most Important Dissonances**
- One CLI mixes:
  - prepare/run
  - replay/inspect
  - protocol
  - campaign
  - closure
- Source:
  - [cli.rs:206](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:206)
- One run directory mixes:
  - telemetry
  - sandbox/config state
  - run record
  - benchmark submission artifact
- Source:
  - [README.md:68](/home/brasides/code/ploke/crates/ploke-eval/README.md:68)
  - [record.rs:1](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:1)
- Setup-only and agent path both write submission artifacts:
  - [runner.rs:1386](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:1386)
  - [runner.rs:1786](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:1786)
- `record.json.gz` is described as the unified record, but the crate still relies on many parallel persisted surfaces.
- Campaign/closure/protocol are downstream consumers but share the same command surface and artifact neighborhood.

**Out Of Domain For Your New Edge, But Nearby**
- Campaign export:
  - [cli.rs:2427](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:2427)
- Closure:
  - [cli.rs:2258](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:2258)
  - [closure.rs:626](/home/brasides/code/ploke/crates/ploke-eval/src/closure.rs:626)
- Protocol:
  - [cli.rs:2226](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:2226)
  - [protocol_artifacts.rs:132](/home/brasides/code/ploke/crates/ploke-eval/src/protocol_artifacts.rs:132)
- Replay/inspect:
  - [cli.rs:2131](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:2131)
  - [cli.rs:2155](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:2155)

**Short Take**
- If you define the first new pipeline as:
  - prepared run input
  - runtime/app setup
  - indexing/setup
  - optional agent turn
  - local patch/run outputs
- then the highest-signal old modules to study are:
  - `spec.rs`
  - `msb.rs`
  - `runner.rs`
  - `record.rs`
  - thin parts of `cli.rs`
- and the biggest immediate contamination source is:
  - shared setup code plus shared artifact writing between setup-only and agent paths
