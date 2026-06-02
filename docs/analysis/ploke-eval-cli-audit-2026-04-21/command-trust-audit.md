# Command Trust Audit

Date: 2026-04-21

This file answers five questions:
- which commands actually do roughly what they claim
- which ones are materially misleading
- what model/provider setup really does
- how usable the help is for a human
- how usable the help is for an agent

## Trust Bands

### Band A: Mostly trustworthy

These commands are close to their help text and are useful enough to rely on.

#### `list-msb-datasets`

What it claims:
- list built-in dataset registry entries

What it appears to do:
- print built-in dataset keys and URLs

Assessment:
- trustworthy
- simple and discoverable

#### `fetch-msb-repo`

What it claims:
- clone or refresh a benchmark repo into `~/.ploke-eval/repos`

Assessment:
- trustworthy as a setup command
- discoverability is fine

#### `prepare-msb-single`

What it claims:
- normalize one Multi-SWE-bench instance into a run manifest

What it really gives you:
- a stable `run.json` at `~/.ploke-eval/runs/<instance>/run.json`

Assessment:
- trustworthy
- one of the best commands in the surface

#### `run-msb-agent-single`

What it claims:
- execute one prepared run and one agentic benchmark turn

What it actually does, at the contract level:
- creates a unique nested run output directory under the instance root
- performs repo reset / indexing / snapshot path
- runs one agentic turn
- writes trace and summary artifacts

Assessment:
- mostly trustworthy
- this should be treated as the default execution primitive
- safer operationally than batch commands because outputs are not all collapsed into one shared batch artifact

#### `conversations`

What it claims:
- list all agent conversation turns from a run

Assessment:
- useful and reasonably clear
- good lightweight inspection surface

#### `inspect`

What it claims:
- inspect conversations, tool calls, DB snapshots, failures, config, turns, and protocol artifacts

Assessment:
- powerful and broadly honest
- still too broad, but it is a real inspection tool rather than a fake facade

#### `campaign export-submissions`

What it claims:
- export Multi-SWE-bench submission JSONL from completed runs in campaign closure state

What it actually does:
- loads completed runs from closure state
- prefers a treatment run that has a submission artifact
- reads per-run `multi-swe-bench-submission.jsonl`
- writes a campaign-scoped export

Assessment:
- more trustworthy than the batch aggregate export path
- this is a bright spot

### Band B: Basically real, but underexplained

These commands do real work, but their help leaves out critical operator semantics.

#### `model set`

What it claims:
- persist the active model selection

What it actually does:
- validates the model against the locally cached model registry
- writes the chosen model id to `~/.ploke-eval/models/active-model.json`

Assessment:
- trustworthy
- but it silently depends on a populated local model registry

#### `model provider set`

What it claims:
- persist the default provider for the current or specified model

What it actually does:
- resolves the model
- validates the requested provider against live OpenRouter endpoints for that model
- persists the result in `~/.ploke-eval/models/provider-preferences.json`

Assessment:
- trustworthy
- stronger than the help implies, because it validates rather than blindly storing the slug

#### `model providers`

What it claims:
- print provider endpoints returned for a model

What it actually does:
- makes a live OpenRouter endpoint fetch
- shows provider slug, name, tool support, selection marker, and context length

Assessment:
- basically real
- but the help undersells that this is network-backed, not just a local registry view

#### `model current`

What it claims:
- show the current active model selection

What it actually does:
- loads active-model state
- enriches with model name if the local registry is present

Assessment:
- trustworthy

#### `doctor`

What it claims:
- inspect the current eval setup and report likely configuration issues

What it actually does in practice:
- checks directories
- checks model registry and active model
- checks OpenRouter API key
- checks git availability

Assessment:
- useful but narrow
- it is a setup-health command, not a runtime-validity command
- it does not tell you whether batch semantics, artifact semantics, or repo isolation are safe

### Band C: Real commands with misleading operational contract

These commands are not imaginary, but their help text encourages unsafe assumptions.

#### `run-msb-batch`
#### `run-msb-agent-batch`

What they claim:
- execute many prepared runs from one batch manifest
- write `batch-run-summary.json`
- write `multi-swe-bench-submission.jsonl`

What materially matters in reality:
- they clear the batch aggregate submission file at batch start
- they only write the summary at the end
- aggregate append is implemented as read-modify-write, not durable append
- rerunning the same batch id reuses the same batch directory
- a partially completed or interrupted batch can leave ambiguous aggregate state

Assessment:
- the commands are real
- the help text is not strong enough for the operational risk
- they read like simple boring batch runners, but they are not robust enough to deserve that framing

Operational consequence:
- do not treat batch aggregate `multi-swe-bench-submission.jsonl` as the authoritative export artifact
- use per-run submission files or campaign export instead

## Model / Provider Setup: What It Actually Does

The CLI presents four overlapping concepts:

1. Active model
- set by `model set`
- persisted in `active-model.json`

2. Persisted default provider
- set by `model provider set`
- persisted in `provider-preferences.json`
- scoped by model id

3. Per-run provider override
- passed via `--provider`
- used for that invocation only

4. Embedding model/provider override
- only exposed on `run-msb-agent-single`
- separate from the primary chat model/provider

Actual resolution behavior:
- run commands resolve the selected model first
- then load the persisted provider preference for that model
- then override with an explicit `--provider` if present
- then validate provider availability/tool support against live OpenRouter endpoint data

What the help gets right:
- these knobs exist

What the help gets wrong by omission:
- it does not explain precedence in one place
- it does not clearly distinguish “active model” from “persisted provider for that model” from “one-off per-run override”
- it does not explain why embedding overrides only appear on one command

## Human-Facing Help Quality

### What is decent

- top-level help gives a visible command inventory
- the MSB commands are grouped around a recognizable prepare/run workflow
- default paths are surfaced reasonably well
- `inspect` subcommands are named sanely

### What is poor

- the surface is too wide for the average operator
- several overlapping commands are not differentiated well:
  - `prepare-single` vs `prepare-msb-single`
  - `run-msb-single` vs `run-msb-agent-single`
  - `conversations` vs `inspect conversations`
  - `transcript` vs `inspect turn --show messages`
- `campaign`, `registry`, and `closure` are powerful but underintroduced
- stale/awkward examples still leak through
- important semantics are implied instead of stated:
  - which artifact is authoritative
  - whether batch outputs are resumable or overwrite-prone
  - provider/model precedence

Overall human assessment:
- usable for someone already steeped in the system
- not a clean operator manual
- too much semantic load is pushed onto the user

## Agent-Facing Help Quality

### Good for agents

- command names are descriptive enough to cluster workflows
- defaults are often explicit
- many subcommands expose concrete file locations

### Bad for agents

- the command surface is broad and semantically overlapping
- several commands require knowing hidden distinctions:
  - when to use top-level `conversations` versus `inspect`
  - when to trust batch artifacts versus per-run artifacts
  - whether `campaign` is optional admin tooling or the real intended workflow
- some commands have many flags with weak prioritization:
  - `prepare-msb-batch`
  - `closure advance eval`
  - `inspect turn`
- the help is better at listing knobs than communicating decision rules

Overall agent assessment:
- better than nothing
- still not sufficient as a standalone operator contract
- good enough for inventory, not good enough for safe autonomous execution at the batch level

## Low-discoverability Commands / Flags

The highest-friction surfaces are:

#### `prepare-msb-batch`

Why:
- multiple selection modes
- optional `--batch-id`
- budget knobs
- path overrides

Problem:
- powerful, but the help does not say which flags are the normal ones for everyday use

#### `run-msb-agent-single`

Why:
- chat model/provider overrides plus embedding model/provider overrides

Problem:
- it is not obvious when embedding overrides matter or when they should be left alone

#### `inspect turn`

Why:
- many `--show` modes
- optional `--index`
- optional role filters

Problem:
- useful for deep inspection, but too dense for casual use without an example crib sheet

#### `closure advance eval`

Why:
- campaign id, dataset overrides, model overrides, provider overrides, required procedures, path overrides, dry-run, limit, stop-on-error, output format

Problem:
- clearly aimed at a power operator, but the help does not explain when this should replace ordinary prepare/run commands

## How Much We Have Fucked Ourselves

Medium-high, not catastrophic.

Why not catastrophic:
- there is a real single-run workflow
- there is a real inspection workflow
- model/provider state is real
- campaign export gives us a safer aggregation path

Why still bad:
- the CLI has grown multiple overlapping operator layers
- batch commands present a cleaner contract than they deserve
- the “right” high-level entrypoint is not obvious from help alone
- artifact authority is not communicated strongly enough

The core problem is not that there are zero real commands.
The core problem is that the CLI mixes:
- primitive execution
- inspection
- campaign control-plane logic
- protocol machinery

without clearly telling the operator which layer should be primary.

## How To Unfuck Ourselves

### Immediate

1. Standardize on one-target execution for interactive work.
2. Treat `run-msb-agent-single` as the default execution primitive.
3. Treat per-run `multi-swe-bench-submission.jsonl` and `campaign export-submissions` as authoritative.
4. Treat batch aggregate submission JSONL as non-authoritative until semantics are hardened.
5. Publish one short operator guide and stop making people reverse-engineer workflows from raw help output.

### Near-term CLI cleanup

1. Add a top-level “recommended workflows” section to `--help`.
2. Add explicit artifact-authority notes:
   - per-run submission artifact
   - campaign export
   - batch aggregate caveat
3. Add one clear model/provider precedence explanation.
4. Make `campaign` / `closure` read like the intended measured-eval workflow instead of mysterious side machinery.
5. Collapse or more clearly distinguish overlapping surfaces like `conversations` and `inspect conversations`.

### Structural

1. Decide whether batch execution is meant to be a trusted operator surface or just an internal convenience.
2. If trusted, harden it and document resumability/overwrite semantics explicitly.
3. If not trusted, demote it in help text and push operators toward campaign/closure plus per-run exports.
