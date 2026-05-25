# Parent Patcher Direct-Google Provider Preference

Status: fixed in source; current blocked campaign should not be treated as a clean child-plan run.

## Summary

Prototype 1 direct-Google campaigns can pass eval and protocol routing, then
fail at `child_plan` because the broad headless-TUI parent-patcher path reads
the persisted parent-patcher model and persisted provider preference instead of
respecting the direct-Google registry route.

The direct-Google model id is intentionally the same logical id used through
OpenRouter:

```text
google/gemini-3.5-flash
```

That means a stale OpenRouter provider preference like `google-ai-studio` is a
valid preference for OpenRouter routing, but must not be treated as an explicit
provider pin when the registry row or admitted profile says the route source is
direct Google.

## Evidence

Campaign:

```text
p1-gemini35-flash-direct-fresh-20260524-190632
```

Command:

```text
/home/brasides/code/ploke/target/debug/ploke-eval loop prototype1-step \
  --repo-root /home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-fresh-20260524-190632 \
  --format json
```

The previous phases were healthy:

- eval complete
- protocol complete
- `call_review_count = 56`
- `segment_review_count = 4`
- doctor phase before the step: `child_plan`

The child-plan step then failed all nine broad harness slots before any
admissible edit:

```text
database setup failed during 'headless_model_route': direct Google model 'google/gemini-3.5-flash' does not accept OpenRouter provider 'google-ai-studio'
```

The final step error was:

```text
batch selection is invalid: broad harness admitted 0 child transaction(s), fewer than required minimum 3
```

Post-failure doctor reports `phase = blocked` because the published broad
harness request prompts now reference candidate workspaces that were never
materialized.

## Broken Contract

Persisted provider preferences are route preferences, not explicit operator
pins. The parent-patcher default path must ignore OpenRouter provider
preferences when the selected model's registry row has
`route_source = direct_google`.

Explicit CLI provider pins should still validate and reject an OpenRouter
provider for a direct-Google route.

## Fix

`load_parent_patcher_model_selection()` now resolves persisted provider
preferences through a preference-aware helper. Direct-Google registry rows
produce a direct-Google `ModelSelection` with no provider pin; explicit
`headless_model_selection(model, Some(provider))` still rejects incompatible
OpenRouter provider pins.

## Regression Tests

```text
cargo test -p ploke-eval parent_patcher_selection_ignores_openrouter_preference_for_direct_google_registry_row -- --nocapture
cargo test -p ploke-eval headless_model_selection_explicit_direct_google_rejects_openrouter_provider_pin -- --nocapture
```

Both pass.

## Disposition

Repair-and-resume is source-valid for the route-selection bug, but the concrete
campaign already has failed published request slots and prompt-preflight
blockers. Treat this campaign as a stop-use evidence source unless a later
operator explicitly decides to clean or reinterpret the request-slot artifacts.

The next trustworthy loop evidence should come from a fresh campaign using the
fixed source.
