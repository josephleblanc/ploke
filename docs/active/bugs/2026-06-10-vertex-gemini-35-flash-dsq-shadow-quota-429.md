# Vertex Gemini 3.5 Flash DSQ Shadow Quota 429

Status: open

## Broken Contract

Operators cannot see or enforce capacity limits for `google/gemini-3.5-flash` on
the Vertex `direct_google` route, so live `RESOURCE_EXHAUSTED` (HTTP 429)
responses terminate Prototype 1 treatment and broad child turns without a
provider-capacity classification distinct from patch-merit failure.

## Chat API Quota vs Vertex Path (embedded answer)

The Console quota row **"Gemini for Google Cloud API / Chat API requests per day
per user / 1,500"** is **not** the API surface Ploke uses for `direct_google`.

| Surface | What it governs | Ploke usage |
| --- | --- | --- |
| Chat API (per-user daily) | Gemini API / AI Studio style Chat API | **Not used** by `direct_google` |
| Vertex `aiplatform.googleapis.com` | `PredictionService.ChatCompletions` via OpenAI-compatible endpoint + ADC | **Used** by `direct_google` |

Our 429s are on **`PredictionService.ChatCompletions`** through Vertex
OpenAI-compat with ADC (Standard PayGo / **dynamic shared quota** for Flash
models). IAM Quotas `base_model` filter lists `gemini-3.0-flash` and
`gemini-2.5-flash` but **not** `gemini-3.5-flash`, so operators cannot inspect
or raise a per-model limit for the model we routed.

## Evidence

| Item | Observation |
| --- | --- |
| Campaign | `p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020` |
| Campaign root | `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020` |
| Profile model | `[model]` and `[protocol.model]` both `google/gemini-3.5-flash`, `route_source = direct-google` |
| Endpoint | `aiplatform.googleapis.com` / `direct_google` (not Chat API) |
| Gen-2 parallel 2209 failures | Two of three gen-2 treatment branches on `BurntSushi__ripgrep-2209` aborted with `HTTP_429` / `RESOURCE_EXHAUSTED`; parent runner surfaced `treatment_failed` without provider-vs-merit separation |
| Terminal synthesis | [`2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-terminal-outcome.md`](../agents/run-reviews/2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-terminal-outcome.md) |
| IAM `consumerQuotaMetrics` | Project `cs-poc-gtxw7jmtfuwfsiauziui9yx`: **0 rows** with `base_model` containing `gemini-3.5-flash`; rows exist for `gemini-3.0-flash` and `gemini-2.5-flash` |
| Metrics Explorer | `api/request_count` shows 429 for generate-content/chat paths; no matching `quota/exceeded` or per-model quota metric explaining the throttle |
| Request model string | Ploke sends `google/gemini-3.5-flash` (no alias to 3.0/2.5); see `crates/ploke-llm/src/router_only/google/mod.rs` catalog and completion URL builder |

Example gen-2 failure chain (2209):

```text
direct_google treatment request (google/gemini-3.5-flash)
  -> Vertex PredictionService.ChatCompletions
  -> HTTP 429 RESOURCE_EXHAUSTED
  -> headless terminal aborted
  -> parent runner treatment_failed / batch invalidity
  -> branch evaluation treats provider abort like merit batch failure
```

## Source Trace

Upstream cause is **external GCP dynamic shared quota / PayGo capacity** for a
model without IAM Quotas visibility, not a Ploke model-id prefix bug (contrast
[`google-api.md`](../plans/self-improvement-loop/google-api.md) `models/` prefix
fix).

Downstream repo gap: Prototype 1 branch evaluation and runner batch validity do
not classify direct-Google `RESOURCE_EXHAUSTED` as a resumable provider-capacity
blocker separate from patch-merit `treatment_failed`.

Trace:

`Vertex 429 -> headless/TUI adapter abort -> persisted terminal failure ->
closure/runner batch invalidity -> successor selection / campaign evidence
tainted for merit interpretation`.

Authority layers:

- **External:** GCP DSQ / org-level Flash TPM tiers (not Console-visible for 3.5).
- **Operator:** profile/model selection (`ploke-eval` persisted defaults and
  prototype1 run profiles).
- **Repo (separate follow-up):** provider-capacity disposition in eval/runner
  (see related bugs).

## Docs/Policy Expectation

- [`google-api.md`](../plans/self-improvement-loop/google-api.md): `direct_google`
  uses Vertex ADC, not Chat API per-user limits.
- Operator policy: provider/auth/capacity failures are external blockers; do not
  reinterpret aborted campaigns as loop-progress evidence.
- IAM Quotas `base_model` filter is the operator's expected place to see
  per-model Vertex limits; absence of `gemini-3.5-flash` violates that
  expectation while the model remains routable in Ploke.

## Current Repro Coverage

Live campaign evidence only:

- [`2026-06-08-prototype1-direct-google-g35flash-quota-empty-baseline.md`](./2026-06-08-prototype1-direct-google-g35flash-quota-empty-baseline.md)
  — baseline turn 429 + empty patch (closure guard since fixed).
- [`2026-06-09-prototype1-broad-child-google-429-zero-admission.md`](./2026-06-09-prototype1-broad-child-google-429-zero-admission.md)
  — broad child zero admission on 429.
- `state3-20260609-203020` — gen-2 parallel treatment 429 on 2209 with partial
  2295 success; campaign otherwise reached configured `max_generations` stop.

No deterministic replay covers DSQ 429; historical replays must not be stretched
to prove quota visibility.

## Missing Repro / Validation

- Fresh live run on `google/gemini-3.0-flash` (visible IAM quota rows) under the
  same profile shape to confirm capacity recovery.
- Optional: `GOOGLE_REGION=global` vs `us-central1` comparison if 429 persists
  on 3.0.
- Repo-side: doctor/preflight or runner classification test that stops before
  merit batch invalidity when direct-Google returns `RESOURCE_EXHAUSTED` (not
  justified until 3.0 route also fails under low concurrency).

## Fix Direction

**Operator (immediate):**

- Downgrade persisted defaults and new prototype1 profiles from
  `google/gemini-3.5-flash` to **`google/gemini-3.0-flash`** (IAM Quotas
  `base_model` rows present).
- New profile template:
  `~/.ploke-eval/profiles/prototype1/p1-g30f-direct-protocol-2target-g0g2-1x3-template.toml`.
- Consider `GOOGLE_REGION=global` if regional DSQ pressure continues.
- Private preview / PT entitlement for 3.5 if that model is required later.

**Do not:**

- Treat 429-aborted treatment branches as patch-quality evidence.
- Patch closure or selection to salvage zero-useful provider failures.
- Conflate the 1,500/day Chat API per-user quota with Vertex ADC usage.

**Repo (separate):**

- Provider-capacity disposition for `RESOURCE_EXHAUSTED` on `direct_google`
  (distinct from merit `treatment_failed`).

## Related Bugs

- [`2026-06-08-prototype1-direct-google-g35flash-quota-empty-baseline.md`](./2026-06-08-prototype1-direct-google-g35flash-quota-empty-baseline.md)
- [`2026-06-09-prototype1-broad-child-google-429-zero-admission.md`](./2026-06-09-prototype1-broad-child-google-429-zero-admission.md)
- [`2026-05-22-prototype1-continue-protocol-quota-no-progress-loop.md`](./2026-05-22-prototype1-continue-protocol-quota-no-progress-loop.md)
- [`google-api.md`](../plans/self-improvement-loop/google-api.md) — `models/`
  prefix and direct-Google catalog parity (different issue from DSQ visibility)
- [Google Cloud dynamic shared quota documentation](https://cloud.google.com/vertex-ai/generative-ai/docs/dynamic-shared-quota)
