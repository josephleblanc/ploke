# Prototype 1 proves embedding execution only after baseline evidence starts

Status: readiness gap repaired; external quota blocker; Stage 0 campaign abandoned

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

## Source Boundary

The eval runner builds an `EmbeddingRequest<OpenRouter>` and invokes the sole
`HasEmbeddings` implementation, the OpenRouter backend. The setup
`embedding_provider_slug` controls OpenRouter provider ordering; it does not
select a different embedding transport.

Direct Google/ADC is implemented for chat completions only. Current eval
embeddings have no direct-Google, local, Hugging Face, or OpenAI adapter exposed
through Prototype 1 setup. Google-named, Perplexity, Qwen, Nvidia, and free
embedding catalog entries all still use the same OpenRouter endpoint and key.

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

Remaining external recovery:

6. After an OpenRouter key limit increase/reset or replacement key is available,
   create a fresh `...-20260713-2` campaign with the same explicit profile and
   embedding selection; do not repair the failed campaign in place.

Adding a direct-Google or other non-OpenRouter eval embedding adapter is a
separate source change, not a current config remedy.

## Related Reports

- [`2026-06-07-prototype1-baseline-embedding-overrides-dropped.md`](2026-06-07-prototype1-baseline-embedding-overrides-dropped.md)
  covers lost setup overrides at earlier campaign boundaries.
- [`2026-04-18-openrouter-codestral-embed-404-fallback.md`](2026-04-18-openrouter-codestral-embed-404-fallback.md)
  covers a different OpenRouter embedding failure and fallback behavior.
