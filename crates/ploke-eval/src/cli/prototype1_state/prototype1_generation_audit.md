# Prototype 1 Parent Identity / Generation Audit

Date: 2026-04-26

Status: pre-flight note. The first artifact-carried identity path is now wired,
but a bounded canary still needs to prove the checkout, handoff, and resume
behavior end to end before long runs.

## Core Issue

The parent must not learn who it is primarily from command-line/process
context:

```text
loop prototype1-state --campaign C --node-id N --repo-root R
```

That is the wrong semantic carrier. A Parent is a Runtime hydrated from an
Artifact checkout, so the checkout itself must contain the Parent identity.

The durable identity is a fixed-path file committed into every parent Artifact
branch:

```text
.ploke/prototype1/parent_identity.json
```

The important part is:

```text
same path in every parent checkout
committed in the artifact branch
loaded by the parent at startup
```

Then a fresh checkout of generation 8 can know it is generation 8 without the
operator passing `--node-id` or `--generation` on the command line. This also
allows a stopped run to be resumed later by checking out the last parent
Artifact and increasing scheduler bounds.

## What The Identity File Should Carry

Minimum useful fields:

```text
campaign_id
parent_id or node_id
generation
lineage / parent identity
branch_id or artifact id
runtime policy/version fields as needed
```

The scheduler can still mirror this information, but it should not be the only
place the active Parent discovers its identity. The active checkout is the
dehydrated Runtime; it needs the identity record inside it.

## Remaining Risk

The typed-state handoff now has the right process shape and an
artifact-carried identity file. The remaining risk is live behavior: the first
canary still needs to prove that every selected successor branch really carries
the expected identity, that startup refuses mismatches, and that resume from a
checked-out parent artifact works without passing `--node-id`.

The old unsafe shape was:

```text
Parent process starts with --node-id as source of truth
Parent loads node generation from scheduler records
Parent decides whether continuation is allowed
Parent spawns successor with another command-line identity
```

That shape is being retired. `--node-id` remains only as bootstrap/legacy input
and is validated against the artifact identity when the file exists.

## Relevant Current Generation References

- `Prototype1SearchPolicy::max_generations`
  `crates/ploke-eval/src/intervention/scheduler.rs:16`

  Campaign/global bound. Default is `1`.

- `Prototype1ContinuationDecision::next_generation`
  `crates/ploke-eval/src/intervention/scheduler.rs:47`

  Proposed next generation from a continuation decision. This is not itself
  Parent identity.

- `Prototype1NodeRecord::generation`
  `crates/ploke-eval/src/intervention/scheduler.rs:76`

  Scheduler/node mirror of generation.

- `prototype1_node_id(branch_id, generation)`
  `crates/ploke-eval/src/intervention/scheduler.rs:223`

  Current node-id derivation. Useful scheduler identity, but too weak as the
  only active Parent identity.

- `decide_continuation(...)`
  `crates/ploke-eval/src/intervention/scheduler.rs:526`

  Computes `next_generation = current_generation + 1` and stops when
  `next_generation > max_generations`.

- `register_treatment_evaluation_node(...)`
  `crates/ploke-eval/src/intervention/scheduler.rs:587`

  Writes scheduler/node/request generation records. This should probably also
  participate in producing or validating the artifact-carried identity record.

- `Prototype1StateCommand::run`
  `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1008`

  Loads `.ploke/prototype1/parent_identity.json` when present. `--node-id` is
  now fallback/bootstrap input and is checked against the artifact identity.

## Desired Shape

Base case:

```text
operator creates/checks out initial parent Artifact
artifact contains parent_identity.json
operator runs prototype1-state over repo root
parent reads parent_identity.json
```

Inductive step:

```text
Parent(k) creates/selects successor Artifact
successor Artifact contains parent_identity.json for Parent(k+1)
Parent(k) switches stable checkout to successor Artifact branch
Parent(k) spawns prototype1-state over the same repo root with only handoff token if needed
Parent(k+1) reads parent_identity.json, acknowledges handoff, continues
```

Resume case:

```text
operator checks out last parent Artifact branch
operator increases max_generations in scheduler policy
operator runs prototype1-state over repo root
parent reads parent_identity.json and knows it is generation 8
```

## Hard Pre-Flight Invariants

Before any long run:

```text
active checkout contains parent identity file
identity file campaign matches selected campaign / scheduler
identity file generation is the generation used for continuation decisions
workspace backend admits the active checkout for that identity before parent run
gen0 branch has exactly the parent identity initialization commit as HEAD
handoff installs a branch whose identity file is the successor identity
successor startup refuses to run if identity file is missing or mismatched
```

The command line may still carry campaign, repo root, and handoff token, but it
should not be the source of truth for parent identity/generation.
