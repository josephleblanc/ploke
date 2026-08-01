# Evalnomicon

Note to LLMs: check llm.txt

`evalnomicon` is the living handbook for eval work in this repository.

It exists to tie together:

- conceptual definitions and distinctions
- introspection and review methodology
- experiment and meta-experiment lines of thought
- links outward to the live workflow record

## What This Is

- a synthesis surface for eval method and conceptual framing
- a place to evolve protocols before or alongside skill distillation
- a human-authored home for connecting polished ideas with semi-structured
  experiment writing

## Relationship To Other Workflow Areas

- [docs/active/workflow](../../../active/workflow)
  Live operational truth: control-plane state, registries, ledgers, handoffs,
  and recent activity.
- [docs/workflow/skills](../../skills)
  Repo-local workflow skills and skill candidates. Skills are executable
  procedure artifacts; this book explains the surrounding method and rationale.
- [eval-design.md](../../../active/plans/evals/eval-design.md)
  Central design and rationale document for the evaluation programme.

## Relationship To Eval Data

- `~/.ploke-eval/`
  As determined by our `ploke-eval` crate's eval output directory structure, the experiments in this book will primarily reference raw data referenced in these directories.

- TBD - TODO:update-me
  An output directory for structured output of `NOM` (Non-Obvious Metrics) derived from llm-driven protocols.

## Internal Epistemic Split

- `core`
  More stable concepts, definitions, and conceptual frameworks.
- `protocols`
  Current operational methods and review procedures.
- `experiments`
  Semi-structured writing about product-facing eval interventions and observed
  outcomes.
- `meta-experiments`
  Semi-structured writing about metrics, process, introspection design, and
  methodology experiments.

Durable conclusions should migrate upward into `core` or `protocols` when they
stop behaving like active working notes.
