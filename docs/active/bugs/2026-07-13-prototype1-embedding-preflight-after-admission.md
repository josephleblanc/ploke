# Prototype 1 proves embedding execution only after baseline evidence starts

Status: readiness gap repaired; external quota blocker still active; original
Stage 0 campaign abandoned; 2026-07-16 preflight-only campaign resumable

Discovered: 2026-07-13

## Broken Contract

An operator can statically resolve and admit an explicit embedding model and
provider, pass ordinary doctor, pass the no-model-call headless setup preflight,
and pass the live protocol preflight without exercising the embedding backend.
The first actual embedding request currently occurs inside baseline eval, where
a provider-readiness failure is persisted as terminal failed evidence.

Prototype 1 needs a narrow, non-mutating embedding execution preflight that uses
the same registry resolution, backend, credential, request, and dimension
validation as baseline eval. It must run before a fresh campaign spends or
persists baseline work.

## Affected Run

```text
campaign: p1-stage0-g35f-pplxembed-3g1x3-p3-20260713-1
parent:   node-bcd4165ec5eda984
instance: BurntSushi__ripgrep-2209
phase before request: baseline_eval
```

The setup-only selection was explicit:

```text
embedding_model_id = perplexity/pplx-embed-v1-4b
embedding_provider_slug = perplexity
```

The admitted profile commitment remained unchanged at:

```text
b156cde303ac74949805943cccc478a52d89f177589085cf6ebe3cd9169a12ac
```

## Evidence Chain

```text
prototype1-state
-> generation-0 baseline eval
-> embedding_model_preflight
-> OpenRouter POST /api/v1/embeddings
-> HTTP 403 Key limit exceeded (monthly limit)
-> closure instance eval_status = failed
-> prototype1-doctor phase = blocked
-> allowed_actions = [doctor]
```

The persisted batch summary records one attempted instance, zero successes, one
failure, direct-Google Gemini 3.5 Flash as the chat route, and the embedding
preflight error before an execution log or run record was created.

A read-only OpenRouter key metadata request confirmed the external state:

```text
usage:          5.026779451
limit:          5
limit_remaining: 0
```

Account credits do not override this key-specific monthly limit.

## Protocol-v10 Live Proof, 2026-07-16

The fresh multi-generation proof campaign
`p1-v10-multigen-g35f-oropenai-3g1x3-p3-20260716-011330` was admitted from
source checkpoint `e1bc583f9` with parent `node-c2f27ef5530dedd6`. Ordinary
doctor, the headless-TUI setup preflight, and the direct-Google Gemini 3.5
Flash protocol preflight all passed. No walk server or typestate edge was
started.

The production embedding preflight failed against each exported OpenRouter
credential without exposing credential values:

```text
OPENROUTER_API_KEY:           HTTP 403, monthly key limit exceeded
AGENT_EVO_OPENROUTER_API_KEY: HTTP 401, user not found
CODEX_OPENROUTER_API_KEY:     HTTP 402, insufficient credits
```

The default key's provider response identifies the same exhausted key-specific
limit recorded above. This isolates the OpenRouter embedding credential as the
only live readiness blocker for the admitted campaign.

## Source Boundary

For the current setup schema, the eval runner supports OpenRouter and direct
OpenAI embedding transports. The setup `embedding_provider_slug` controls
provider ordering within the selected transport; choosing a different
OpenRouter model or provider slug still uses the same OpenRouter endpoint and
key.

Direct Google/ADC is implemented for chat completions only. Prototype 1 does
not expose a direct-Google, local, or Hugging Face eval embedding route.
Direct OpenAI is now exposed, but it is not a valid recovery for the current
`BurntSushi__ripgrep-2209` target: production indexing contains a 66,807-byte
node that exceeds OpenAI's input limit, as recorded in
[`2026-07-14-direct-openai-embedding-overlong-snippet.md`](2026-07-14-direct-openai-embedding-overlong-snippet.md).

Therefore changing only the embedding model or provider slug cannot bypass this
key-level limit.

## Run Disposition

The failed closure evidence is valid and must not be rewritten. Doctor refuses
to rerun over it, so this campaign is preserved as an abandoned quota-failure
run. It produced no agent trace, tool lifecycle, patch, or protocol review;
those surfaces are not applicable rather than missing.

This note is not indexed as a full run review because the provider failure
occurred before an agent trace existed.

## Required Repair

Implemented in the Stage 1 readiness slice:

1. `prototype1-doctor --live-embedding-preflight` reuses the production
   embedding selection and request path.
2. Its typed report contains the selected model on success, requested model and
   provider preference, actual backend, registry path, observed dimensions, and
   classified failure detail.
3. Regressions verify the doctor adapter forwards production `None`/`None`
   auto-selection unchanged, reports typed success and failure shapes, preserves
   an already-failed closure, and does not write campaign, instance, batch, or
   parent-checkout evidence.
4. A live invocation against the exhausted key returned
   `class = provider_account` in under one second. That invocation exercised the
   exact production resolver; the campaign closure-state digest remained
   byte-identical before and after the probe.
5. Console diagnostics now use stderr, so provider warnings no longer corrupt
   `--format json` stdout during a failed live preflight.

Remaining external recovery for the original Stage 0 campaign:

6. After an OpenRouter key limit increase/reset or replacement key is available,
   create a fresh `...-20260713-2` campaign with the same explicit profile and
   embedding selection; do not repair the failed campaign in place.

For the 2026-07-16 campaign, the failure occurred only in the idempotent doctor
preflight. It did not write closure, eval, protocol, or typestate evidence, so
the blocker disposition is repair-and-resume. After raising or removing the
monthly limit on `OPENROUTER_API_KEY`, rerun:

```text
ploke-eval loop prototype1-doctor \
  --repo-root ~/.ploke-eval/worktrees/p1-v10-multigen-g35f-oropenai-3g1x3-p3-20260716-011330 \
  --live-embedding-preflight --format json
```

Start the walk server only if that report has
`embedding_preflight.outcome = "passed"` and no blockers.

Adding a direct-Google or other non-OpenRouter eval embedding adapter is a
separate source change, not a current config remedy.

## Related Reports

- [`2026-06-07-prototype1-baseline-embedding-overrides-dropped.md`](2026-06-07-prototype1-baseline-embedding-overrides-dropped.md)
  covers lost setup overrides at earlier campaign boundaries.
- [`2026-04-18-openrouter-codestral-embed-404-fallback.md`](2026-04-18-openrouter-codestral-embed-404-fallback.md)
  covers a different OpenRouter embedding failure and fallback behavior.
