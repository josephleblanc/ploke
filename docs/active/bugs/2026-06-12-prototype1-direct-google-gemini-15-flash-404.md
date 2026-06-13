# Prototype 1 Direct-Google Gemini 1.5 Flash Preflight 404

Status: blocked

## Broken Contract

`prototype1-doctor --live-protocol-preflight` must reject an admitted
Prototype 1 campaign before paid loop work when the configured protocol model
cannot be routed through the selected provider.

## Evidence

- Campaign:
  `p1-g25f-direct-protocol-2target-g0g2-1x3-state18-20260612-223232`
- Worktree:
  `/home/brasides/.ploke-eval/worktrees/p1-g25f-direct-protocol-2target-g0g2-1x3-state18-20260612-223232`
- Setup command:
  `/home/brasides/code/ploke/target/debug/ploke-eval loop prototype1-setup --campaign p1-g25f-direct-protocol-2target-g0g2-1x3-state18-20260612-223232 --profile /home/brasides/.ploke-eval/profiles/prototype1/p1-g25f-direct-protocol-2target-g0g2-1x3-template.toml --format json`
- Setup result:
  parent identity `node-ebe0710749ebcfc6`, branch
  `prototype1-parent-p1-g25f-direct-protocol-2target-g0g2-1x3-state18-20260612-223232-gen0`,
  admitted profile digest
  `953ce9c62d5e00b7b9d23bd5ea39d1c4da54aaa0497d7327909d3250b04ddc60`.
- Doctor command:
  `/home/brasides/code/ploke/target/debug/ploke-eval loop prototype1-doctor --repo-root /home/brasides/.ploke-eval/worktrees/p1-g25f-direct-protocol-2target-g0g2-1x3-state18-20260612-223232 --live-protocol-preflight --headless-tui-setup-preflight --format json`
- Doctor result:
  `headless_tui_setup_preflight.outcome = "passed"`,
  `protocol_preflight.outcome = "failed"`, `phase = "blocked"`, and
  `allowed_actions = ["doctor"]`.
- Provider response:
  direct Google returned HTTP 404 for
  `publishers/google/models/gemini-1.5-flash`:
  "Publisher Model ... was not found or your project does not have access to
  it."
- ADC-backed Vertex OpenAI-compatible probe:
  `google/gemini-1.5-flash` and `google/gemini-1.5-flash-002` returned HTTP
  404; bare `gemini-1.5-flash` and `gemini-1.5-flash-002` returned HTTP 400
  because the OpenAI-compatible endpoint requires `<publisher>/<model>`;
  `google/gemini-2.5-flash` returned HTTP 200.
- ADC-backed native Vertex `publishers/google/models/...:generateContent`
  probe:
  `gemini-1.5-flash` and `gemini-1.5-flash-002` returned HTTP 404 in both
  `us-central1` and `global`; `gemini-2.5-flash` returned HTTP 200 in both
  locations.
- `gcloud ai model-garden models list` could not be used as the authority
  surface in this shell because the interactive gcloud account needs
  non-interactive reauthentication. `gcloud auth application-default
  print-access-token` succeeded, and that ADC token was used for the Vertex
  probes above.

## Source Trace

`crates/ploke-eval/src/cli/prototype1_state/run/core.rs` owns the doctor live
protocol preflight. It first checks that the admitted protocol policy can make a
small JSON request through the selected route, then runs the larger
protocol-shaped budget canary. This campaign failed at the small routeability
check, before the 4096-token budget canary.

`crates/ploke-llm/src/router_only/google/mod.rs` owns the static direct-Google
Vertex catalog and currently lists routable chat slugs for the direct Google
route. `gemini-1.5-flash` is not in that catalog, and the live Vertex
OpenAI-compatible plus native Vertex endpoints rejected both alias and explicit
`-002` ids.

`crates/ploke-eval/src/cli/prototype1_state/profile.rs` currently validates
profile model fields syntactically: the id must parse, the provider key must be
valid, and `route_source = "direct-google"` may only use the `google` provider
sentinel. It does not require a profile model id to exist in the direct-Google
catalog before campaign admission. Doctor is the first live routeability gate.

## Docs/Policy Expectation

The admitted profile requested:

```toml
[protocol]
max_tokens = 4096

[protocol.model]
id = "google/gemini-1.5-flash"
route_source = "direct-google"
provider = "google"
```

The live preflight was added specifically to stop this class of configuration
issue before a full Prototype 1 loop spends eval/protocol work. Current Google
Gemini docs list current Gemini 3.x and 2.5 model families, while this direct
Google route returned 404 for the older 1.5 Flash alias.

## Current Repro Coverage

The live doctor preflight is the reproduction:

```bash
/home/brasides/code/ploke/target/debug/ploke-eval loop prototype1-doctor \
  --repo-root /home/brasides/.ploke-eval/worktrees/p1-g25f-direct-protocol-2target-g0g2-1x3-state18-20260612-223232 \
  --live-protocol-preflight \
  --headless-tui-setup-preflight \
  --format json
```

This proves the setup-time guard catches the inaccessible protocol model before
`prototype1-state` advances.

## Missing Repro / Validation

No repo regression is indicated yet. The replacement chosen for new Prototype 1
direct-Google runs is `google/gemini-2.5-flash-lite`. Remaining validation is a
fresh `prototype1-setup` plus doctor live preflight against a profile using that
model for both eval and protocol.

## Fix Direction

Do not resume this campaign for loop progress with the current admitted profile.
Create a fresh campaign from the updated profile template, which now uses
`google/gemini-2.5-flash-lite` for direct-Google eval and protocol, then rerun
doctor before `prototype1-state`.

Do not weaken doctor to treat model 404 as non-blocking. The provider route is a
required setup-time invariant for protocol evidence.

Before spending a full run, inspect the live Service Usage RPM rows with:

```bash
cargo xtask google-direct-rpm-limits --project "$GOOGLE_PROJECT_ID"
```

Rows marked `no_fixed_rpm_row` mean the text endpoint has no raiseable Service
Usage RPM bucket for that base model; Vertex Standard PayGo may still throttle
through Dynamic Shared Quota.

## Related Bugs

- `docs/active/bugs/2026-06-10-direct-google-malformed-function-call-finish-reason.md`
- `docs/active/bugs/2026-06-10-vertex-gemini-35-flash-dsq-shadow-quota-429.md`
