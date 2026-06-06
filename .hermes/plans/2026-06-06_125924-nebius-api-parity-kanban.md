# Nebius API Parity Implementation Plan

> **For Hermes:** Use `kanban-orchestrator` to implement this plan through durable Kanban cards. Worker cards must load/follow `test-driven-development`; reviewer cards must load/follow `requesting-code-review`, `github-code-review`, and the Rust review guidance from `systematic-debugging` (especially rust warning cleanup). Do not use a broad `#[allow(...)]`, visibility widening, or `_name` churn to silence Rust warnings.

**Goal:** Bring `crates/ploke-llm` Nebius Token Factory support from the current chat-completion skeleton to practical parity with existing OpenRouter/Google support, plus typed/safe coverage of the rest of the Nebius API surface where it belongs in `ploke-llm`.

**Architecture:** Treat Nebius as an OpenAI-compatible direct provider, closest to Google’s direct-provider shape and OpenRouter’s OpenAI request/response shape. Implement shared typed request/response surfaces in small modules under `router_only/nebius/`, then wire only proven provider behavior into registry/routing/manager paths. Use live API smoke tests behind `live_api_tests` and `PLOKE_RUN_LIVE_TESTS` gates.

**Tech Stack:** Rust 2024 workspace; `reqwest`, `serde`, `tokio`, `color-eyre`; Hermes Kanban profiles and cron; Nebius Token Factory API at `https://api.tokenfactory.nebius.com/v1` with bearer-token auth via `NEBIUS_API_KEY`.

---

## Current context / assumptions

- Workspace: `/home/team_ploke_dev/code/ploke-nebius`.
- Branch: `feature/nebius-endpoint`.
- Current dirty files are the intended Nebius skeleton:
  - `crates/ploke-llm/src/router_only/nebius/mod.rs`
  - `crates/ploke-llm/src/request/tests/nebius_chat_completion.rs`
  - plus module/export wiring in `lib.rs`, `request/tests/mod.rs`, `router_only/mod.rs`, `router_only/openrouter/mod.rs`.
- Current focused verification already passed before this plan:
  - `cargo test -p ploke-llm request::tests::nebius_chat_completion -- --nocapture`
  - `cargo check -p ploke-llm`
- Discovered profiles on this machine: `default`, `implementer`, `reviewer`. The requested `implementor-high`, `reviewer-high`, `debugger-high`, and `spark-mini` profiles do not currently appear in `hermes profile list`; implementation must verify/create/configure them before real Kanban work.
- Current Kanban board is `three-particles-rust-wasm`, not Ploke. Implementation must create/switch to a dedicated `ploke-nebius` board before creating cards.
- No installed skill named `rust-style-review` was found. Use `requesting-code-review`, `github-code-review`, `test-driven-development`, and `systematic-debugging` Rust guidance as the reviewer skill bundle unless the user later supplies a specific Rust review skill name.
- Nebius docs observed from `https://docs.tokenfactory.nebius.com/api-reference/introduction` and navigation:
  - Introduction example uses `base_url="https://api.tokenfactory.nebius.com/v1/"`, `NEBIUS_API_KEY`, and chat model `meta-llama/Meta-Llama-3.1-70B-Instruct`.
  - Auth uses bearer-token authorization.
  - API reference includes inference endpoints: completions, chat completions, embeddings, rerank, responses, generate; and resource groups: models, files, fine-tuning, dedicated-endpoints, datasets.

## Definition of parity

### Must-have parity with existing OpenRouter/Google surfaces

1. `Nebius` implements the core `Router` contract cleanly.
2. Chat completion request serialization/deserialization covers Nebius-documented OpenAI-compatible fields without breaking OpenRouter/Google.
3. `HasModels` works against `GET /v1/models`, normalizes model rows into `request::models::ResponseItem`, and tags route source as direct Nebius.
4. Route registry supports direct Nebius routes like direct Google routes.
5. Calibration keys/timing support direct Nebius model keys.
6. Manager chat send path can run `ChatCompRequest<Nebius>` through `chat_step`/`chat_step_with_attempts` with request/response diagnostics and retries.
7. Live tests include at least one real Nebius models call and one real low-token chat completion call when `NEBIUS_API_KEY` and `PLOKE_RUN_LIVE_TESTS=1` are set.
8. Embeddings reach parity with OpenRouter embedding support where Nebius docs and live model support permit.

### Extended API-surface coverage

Implement typed client modules and tests for Nebius docs endpoints that are safe/useful in `ploke-llm`:

- Inference: `POST /v1/completions`, `/v1/chat/completions`, `/v1/embeddings`, `/v1/rerank`, `/v1/responses`, `/v1/images/generations` or actual docs path for Generate after doc audit.
- Models: `GET /v1/models`.
- Files, fine-tuning, datasets, dedicated endpoints: typed request/response skeletons and safe list/get/delete surfaces only after doc audit; do not add mutating upload/delete/fine-tune operations without explicit tests and clear safety gates.

## Kanban setup plan

### Operator preflight card: profile/model verification

**Assignee:** `debugger-high` once created; otherwise initial setup done by operator under this plan.

**Commands to run during implementation (not in plan mode):**

```bash
cd /home/team_ploke_dev/code/ploke-nebius
hermes profile list
hermes kanban assignees
hermes kanban boards list
```

Create/configure these profiles only after exact model probes pass:

```bash
# High-reasoning workers; provider should prefer openai-codex/OpenAI Pro, not OpenRouter.
hermes profile create implementor-high --clone-from implementer --clone --description 'High-reasoning Ploke implementation worker'
hermes profile create reviewer-high --clone-from reviewer --clone --description 'High-reasoning Ploke reviewer worker'
hermes profile create debugger-high --clone-from implementer --clone --description 'High-reasoning Ploke debugger/root-cause worker'

hermes -p implementor-high config set model.provider openai-codex
hermes -p implementor-high config set model.default gpt-5.5
hermes -p implementor-high config set agent.reasoning_effort high
hermes -p reviewer-high config set model.provider openai-codex
hermes -p reviewer-high config set model.default gpt-5.5
hermes -p reviewer-high config set agent.reasoning_effort high
hermes -p debugger-high config set model.provider openai-codex
hermes -p debugger-high config set model.default gpt-5.5
hermes -p debugger-high config set agent.reasoning_effort high
```

For the requested mini spark, first probe the exact model name. Do not silently substitute if unavailable:

```bash
hermes chat -Q --provider openai-codex -m gpt-5.3-mini -t safe -q 'Reply exactly: ok'
```

If that exact probe passes, create/configure:

```bash
hermes profile create spark-mini --clone-from implementer --clone --description 'Cheap Nebius Kanban spark: obvious retries/unblocks only'
hermes -p spark-mini config set model.provider openai-codex
hermes -p spark-mini config set model.default gpt-5.3-mini
hermes -p spark-mini config set agent.reasoning_effort low
```

If `gpt-5.3-mini` fails, block and ask the user which exact mini model/provider to use. Do not substitute `gpt-5-mini`, OpenRouter, or another provider without approval.

Verify every profile with direct execution:

```bash
hermes profile show implementor-high
hermes profile show reviewer-high
hermes profile show debugger-high
hermes profile show spark-mini
hermes -p implementor-high chat -Q -t safe -q 'Reply exactly: implementor-high-ok'
hermes -p reviewer-high chat -Q -t safe -q 'Reply exactly: reviewer-high-ok'
hermes -p debugger-high chat -Q -t safe -q 'Reply exactly: debugger-high-ok'
hermes -p spark-mini chat -Q -t safe -q 'Reply exactly: spark-mini-ok'
```

Create/switch board:

```bash
hermes kanban boards create ploke-nebius --name 'Ploke Nebius API parity' --default-workdir /home/team_ploke_dev/code/ploke-nebius --switch
```

### Spark cron / watchdog plan

Create a 5-minute job under `spark-mini` only after profile verification. Its job is stewardship, not implementation.

**Schedule:** every 5 minutes.

**Model/profile:** `spark-mini` with exact probed `gpt-5.3-mini`.

**Allowed actions:**
- inspect `hermes kanban stats --board ploke-nebius` and `hermes kanban list`;
- run `hermes kanban dispatch --board ploke-nebius --max 3` if ready work exists and no worker is running;
- reclaim tasks only when claim is stale or dispatcher diagnostics explicitly say stalled;
- retry one obvious transient failure once if the failed card already has a clear retry command and no code decision is needed;
- create or promote a `debugger-high` card for ambiguous failures, repeated failures, missing credentials, model mismatch, live API ambiguity, schema disagreement, or anything that would require code judgment.

**Must not:** edit source files, commit, create recursive cron jobs, silently switch models, or mark implementation/review tasks done.

**Ambiguous escalation pattern:** create a `debugger-high` Kanban card with the failing task id, logs, exact command, and why the spark could not decide.

**Cron creation shape for implementation phase:**

```bash
hermes cron create 'every 5m' \
  --profile spark-mini \
  --workdir /home/team_ploke_dev/code/ploke-nebius \
  --name 'ploke-nebius-spark' \
  --prompt-file /home/team_ploke_dev/code/ploke-nebius/.hermes/plans/ploke-nebius-spark-prompt.md
```

The prompt file should say explicitly: do not schedule cron jobs; perform only obvious stewardship; ambiguous issues become `debugger-high` cards.

### Overseer cadence

The operator session should run a 15-minute stewardship loop until completion:

1. Every 15 minutes inspect board stats, running cards, blocked cards, and new comments.
2. If a task is blocked by missing context, answer by comment or create a narrow debugger card.
3. If a reviewer blocks work, create a new implementor-high remediation card with the reviewer findings as parent/context; do not rerun the same task with a vague instruction.
4. If all cards are done, run final local verification and report.

Implementation may use a bounded cron/steward job for this cadence, but the plan itself does not create it.

## Task graph

### T0: Board/profile/bootstrap and current skeleton review

**Assignee:** `debugger-high` for setup verification, then `reviewer-high` for current skeleton review.

**Objective:** Establish known-good high/mini profiles, Ploke board, and decide whether the current Nebius skeleton is acceptable as the base.

**Files:** no source edits expected except possible generated prompt under `.hermes/plans/` for spark cron.

**Steps:**
1. Verify/create profiles and board as above.
2. Run focused baseline commands and save logs under `target/test-output/nebius/`:
   - `cargo test -p ploke-llm request::tests::nebius_chat_completion -- --nocapture`
   - `cargo check -p ploke-llm`
3. Reviewer inspects current skeleton diff against OpenRouter/Google examples.
4. If accepted, commit the current skeleton as a coherent base commit, e.g. `feat(llm): add Nebius router skeleton`.
5. If rejected, create a remediation card before any broad API work.

**Acceptance:** clean focused tests/check, reviewer approval, and either a commit SHA or a blocked card with exact remediation.

### T1: Nebius docs audit and endpoint contract map

**Assignee:** `implementor-high` or `debugger-high` (read-only research card).

**Objective:** Convert Nebius API docs into a local contract map for implementation, without adding project docs unless needed by cards.

**Files likely to change:** none, unless the card stores a compact artifact in `target/test-output/nebius/docs-contract.md` (not committed).

**Steps:**
1. Read Nebius docs for every endpoint group: inference, models, files, fine-tuning, dedicated-endpoints, datasets.
2. Capture exact HTTP method/path, auth, query params, request body fields, response shape examples, and whether the endpoint is safe for live smoke tests.
3. Identify which endpoints are OpenAI-compatible and which require Nebius-specific structs.
4. Record live-test prerequisites per endpoint: env vars, model ids, project id/query params, destructive risk.

**Acceptance:** a compact endpoint matrix with paths and safe/unsafe classification. Reviewer-high verifies the matrix against docs before implementation cards rely on it.

### T2: Models endpoint and model normalization

**Assignee:** `implementor-high`; review by `reviewer-high`.

**Objective:** Implement `HasModels` for Nebius and normalize `GET /v1/models` into existing model registry rows.

**Files:**
- Modify: `crates/ploke-llm/src/router_only/nebius/mod.rs`
- Possibly create: `crates/ploke-llm/src/router_only/nebius/models.rs`
- Modify: `crates/ploke-llm/src/request/models.rs`
- Tests: `crates/ploke-llm/src/router_only/nebius/tests.rs` or `crates/ploke-llm/src/request/tests/nebius_models.rs`

**TDD steps:**
1. Add a unit test deserializing Nebius docs example:
   ```json
   {"object":"list","data":[{"id":"meta-llama/Llama-3.3-70B-Instruct","created":1717511223,"object":"model","owned_by":"system"}]}
   ```
2. Verify RED: `cargo test -p ploke-llm nebius_models -- --nocapture` fails because types/impl are absent.
3. Implement `NebiusModelsResponse`, `NebiusModel`, `HasModelId`, `From<NebiusModel> for models::ResponseItem`.
4. Add `ModelRouteSource::DirectNebius` plus `is_direct_nebius()`.
5. Verify GREEN and run `cargo check -p ploke-llm`.

**Acceptance:** model id stays `meta-llama/Llama-3.3-70B-Instruct`; route source is direct Nebius; no OpenRouter/Google regression.

### T3: Direct Nebius route registry and calibration

**Assignee:** `implementor-high`; review by `reviewer-high`.

**Objective:** Make Nebius usable as a first-class direct route like Google.

**Files:**
- Modify: `crates/ploke-llm/src/registry/route.rs`
- Modify: `crates/ploke-llm/src/registry/calibration.rs`
- Modify: `crates/ploke-llm/src/lib.rs`
- Tests in the same modules.

**TDD steps:**
1. Add tests for `LlmRoute::nebius`, `direct_nebius_model`, `selected_provider_slug() == "nebius"`, and `router() == RouterVariants::Nebius(Nebius)`.
2. Add tests for `NebiusCalibrationKey` producing `nebius:<model>`.
3. Verify RED.
4. Implement minimal route and calibration structs.
5. Verify GREEN.

**Acceptance:** route/calibration APIs mirror direct Google without provider endpoint requirements.

### T4: Chat completion live API and response-shape hardening

**Assignee:** `implementor-high`; review by `reviewer-high`; ambiguous failures to `debugger-high`.

**Objective:** Prove the chat path works against real Nebius with `NEBIUS_API_KEY`.

**Files:**
- Modify: `crates/ploke-llm/src/router_only/nebius/mod.rs`
- Modify/create tests: `crates/ploke-llm/src/router_only/nebius/live_tests.rs` or module tests under `nebius/mod.rs`
- Possibly modify: `crates/ploke-llm/src/manager/session.rs` only if Nebius response shape exposes a real incompatibility.

**TDD/live steps:**
1. Add non-live serialization tests for all documented chat fields currently in `NebiusChatCompFields`.
2. Add live test behind `#[cfg(feature = "live_api_tests")]` and runtime gate `PLOKE_RUN_LIVE_TESTS=1`:
   ```bash
   PLOKE_RUN_LIVE_TESTS=1 NEBIUS_API_KEY=... \
   cargo test -p ploke-llm --features live_api_tests nebius_live_chat -- --nocapture
   ```
3. Live model comes from `PLOKE_LIVE_NEBIUS_CHAT_MODEL`, defaulting to the docs example only if affordable/acceptable; otherwise block and ask.
4. Request must use tiny max tokens and deterministic prompt: “Reply exactly: ok”.
5. Save full stdout/stderr to `target/test-output/nebius/live-chat.log`.

**Acceptance:** real live response returns content or a clearly classified provider/API error. If auth/model quota missing and `PLOKE_RUN_LIVE_TESTS` is not set, test skips with explicit message; if strict live is requested, missing config fails.

### T5: Embeddings parity

**Assignee:** `implementor-high`; review by `reviewer-high`.

**Objective:** Implement Nebius embeddings using existing `embeddings` traits where possible.

**Files:**
- Create: `crates/ploke-llm/src/router_only/nebius/embed.rs`
- Modify: `crates/ploke-llm/src/router_only/nebius/mod.rs`
- Tests: `crates/ploke-llm/src/embeddings/mod.rs` tests or Nebius module tests.

**TDD steps:**
1. Add serialization test for `EmbeddingRequest<Nebius>` and docs-compatible `POST /v1/embeddings` URL.
2. Add response deserialization tests from docs or live captured minimal response.
3. Implement `HasEmbeddings` for Nebius with `EMBEDDINGS_URL = "https://api.tokenfactory.nebius.com/v1/embeddings"`.
4. Add live test gated by `PLOKE_LIVE_NEBIUS_EMBED_MODEL`; skip unless model env is set or docs audit identifies a reliable default.

**Acceptance:** unit tests pass; live embedding test either validates dims or skips/fails according to strict live gate.

### T6: Text completions, responses, rerank, and generate typed clients

**Assignee:** split into independent `implementor-high` cards after T1 docs audit; review each by `reviewer-high`.

**Objective:** Add small typed request/response clients for Nebius inference endpoints beyond chat/embeddings, without forcing them into existing chat abstractions prematurely.

**Files likely to create:**
- `crates/ploke-llm/src/router_only/nebius/completions.rs`
- `crates/ploke-llm/src/router_only/nebius/responses.rs`
- `crates/ploke-llm/src/router_only/nebius/rerank.rs`
- `crates/ploke-llm/src/router_only/nebius/generate.rs`
- Module tests alongside each file.

**Per-endpoint TDD template:**
1. Add request serialization test from docs example.
2. Add response deserialization test from docs example.
3. Implement minimal structs/builders and URL constants.
4. Add `fetch_*` method only if the endpoint is safe and request body is understood.
5. Add live smoke only if safe and model/env prerequisites are explicit.

**Acceptance:** typed surfaces compile, docs examples roundtrip/deserialise, and no endpoint is wired into higher-level app routing until a user-facing need exists.

### T7: Files, datasets, fine-tuning, dedicated endpoints safe surfaces

**Assignee:** `implementor-high`; review by `reviewer-high`; destructive ambiguity to `debugger-high`.

**Objective:** Provide safe typed skeletons for non-chat resource endpoints while avoiding accidental destructive operations.

**Files likely to create:**
- `crates/ploke-llm/src/router_only/nebius/files.rs`
- `crates/ploke-llm/src/router_only/nebius/datasets.rs`
- `crates/ploke-llm/src/router_only/nebius/fine_tuning.rs`
- `crates/ploke-llm/src/router_only/nebius/dedicated_endpoints.rs`

**Rules:**
- Implement list/get/status structs first.
- Mutating operations (`upload`, `delete`, `cancel`, `create fine tune`, dedicated endpoint create/delete`) require explicit tests and should be feature-gated or omitted unless docs audit makes them clearly safe.
- Live tests for list-only endpoints are allowed if auth/project requirements are known.

**Acceptance:** safe list/get operations are typed and tested; destructive operations are either absent or explicitly gated with reviewer approval.

### T8: Integration into higher-level manager/session flows

**Assignee:** `implementor-high`; review by `reviewer-high`.

**Objective:** Ensure Nebius direct routes can be selected/executed where `ploke-tui` or registry code expects `LlmRoute`/`RouterVariants`.

**Files likely to inspect/modify:**
- `crates/ploke-llm/src/manager/session.rs`
- `crates/ploke-llm/src/manager/commands.rs`
- `crates/ploke-llm/src/manager/builders/*`
- `crates/ploke-llm/src/registry/cache.rs`
- `crates/ploke-tui` only if existing route selection code requires it.

**TDD steps:**
1. Add tests showing a direct Nebius model route creates a `ChatCompRequest<Nebius>` and uses Nebius URL/auth.
2. Verify RED.
3. Implement the narrowest route plumbing.
4. Verify focused tests and `cargo check -p ploke-llm`.

**Acceptance:** no broad refactor; direct Google/OpenRouter behavior unchanged.

### T9: Final live validation and regression gates

**Assignee:** `debugger-high` for root-cause handling; `reviewer-high` final review.

**Objective:** Prove the implementation works locally and against live Nebius where credentials permit.

**Commands:**

```bash
cd /home/team_ploke_dev/code/ploke-nebius
cargo fmt --all
cargo test -p ploke-llm request::tests::nebius_chat_completion -- --nocapture
cargo test -p ploke-llm nebius -- --nocapture
cargo check -p ploke-llm
cargo clippy -p ploke-llm --all-targets -- -D warnings
```

Live, strict when credentials are intentionally present:

```bash
PLOKE_RUN_LIVE_TESTS=1 cargo test -p ploke-llm --features live_api_tests nebius_live -- --nocapture 2>&1 | tee target/test-output/nebius/live-suite.log
```

If full workspace verification is feasible:

```bash
cargo test --workspace 2>&1 | tee target/test-output/nebius/workspace-test.log
```

**Acceptance:** focused and crate-level checks pass; live tests either pass or fail with a concrete provider/account/documented unsupported reason. Reviewer-high approves final diff and verifies no unrelated worktree drift.

## Review policy

Every implementation card must:

1. Follow TDD: write failing test, run and capture RED, implement, run GREEN.
2. Commit a coherent task-owned diff before handing off to review, unless explicitly unsafe.
3. Include exact commands, exit codes, and log paths in the Kanban handoff.
4. Avoid broad `allow`, visibility widening, or unrelated formatting/refactors.
5. Keep build artifacts under normal Cargo target behavior; do not set persistent `CARGO_TARGET_DIR`.

Every reviewer card must:

1. Load/use `requesting-code-review`, `github-code-review`, and `test-driven-development`.
2. Verify the task’s commit SHA exists and the worktree is not dirty-only.
3. Inspect diff and relevant files, not just test output.
4. Run or delegate focused verification.
5. Block with precise remediation if tests are missing, live evidence is absent, a model/profile constraint was not verified, or code weakens invariants.

## Risks / tradeoffs / open questions

- Exact `gpt-5.3-mini` availability is unverified. Implementation must probe and ask if unavailable.
- Nebius endpoint docs may include fields not currently represented in shared OpenAI-compatible structs (`reasoning`, `reasoning_content`, `stream_options`, `service_tier`, etc.). Add only documented/tested fields and keep provider-specific extras under Nebius structs.
- Some Nebius endpoints are stateful or destructive. Start with safe typed/list/get surfaces; require explicit gated tasks for upload/delete/fine-tune/dedicated endpoint creation.
- Live API tests depend on `NEBIUS_API_KEY`, account/project access, model availability, and costs. Tests must be strict only when `PLOKE_RUN_LIVE_TESTS=1` is set.
- Response examples in docs may have inconsistencies (e.g. chat completion example resembling text completion). Live tests should record actual shapes and debugger-high should resolve mismatches systematically before changing core response parsing.
- The current board is not Ploke; forgetting to switch boards would route work into the wrong project.
- The current skeleton is uncommitted; first implementation action should review/commit or remediate it before parallel workers edit the same files.

## Final done criteria

- `Nebius` is a first-class direct provider for chat, models, route registry, and calibration.
- Embeddings are implemented to the extent Nebius supports them.
- Remaining Nebius endpoints have typed, tested safe skeletons or documented deferred reasons.
- At least one real Nebius models call and one real Nebius chat call have been executed in live tests, or the board is blocked with exact missing credential/account evidence.
- `cargo fmt --all`, focused Nebius tests, `cargo check -p ploke-llm`, and final reviewer approval are complete.
- Spark watchdog and 15-minute overseer loop have either completed all cards or left only explicitly deferred/destructive API work with user-facing rationale.
