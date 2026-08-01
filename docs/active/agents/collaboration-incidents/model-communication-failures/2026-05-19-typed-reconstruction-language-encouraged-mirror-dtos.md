# 2026-05-19 Typed Reconstruction Language Encouraged Mirror DTOs

## Trigger

The user objected to the phrase "typed reconstruction" and noted that this
language often appears right before agents add one-time DTOs that are smaller
or less complete versions of existing types.

## User-Visible Failure

The agent framed remaining regression coverage as "typed reconstruction" work.
That made the next step sound like adding derived record carriers instead of
first using existing canonical types, borrowed accessors, or owner-schema
extensions.

## Touched Surface

- `AGENTS.md`
- `.codex/skills/typed-persistence-spine/SKILL.md`
- `.codex/skills/ploke-egui-benchmarking/SKILL.md`
- `.codex/skills/ploke-debugger-claim-workflow/SKILL.md`

No product Rust source was edited for this incident.

## What The Agent Did

- Used language derived from the local instruction stack: "typed projection",
  "record projection", and "UI-answer reconstruction".
- Treated that language as harmless shorthand in the final answer.
- Did not notice that the phrasing biased the next implementation toward
  subset DTOs and one-off derived carriers.

## Skipped Or Underweighted Instructions

- `AGENTS.md` already warns against ad hoc report/view/status carriers, but the
  same file also told agents to define a named typed projection struct when a
  reader needed only part of a record.
- The local `typed-persistence-spine` skill repeated that pattern by listing
  "typed projection structs" and "UI-answer reconstruction" as implementation
  outputs.

## Why This Was Risky

The wording made it too easy to satisfy "do not use `serde_json::Value`" by
creating narrower mirror DTOs. That preserves typed deserialization on paper
while weakening the actual domain model, duplicating existing schema, and
making later UI/replay/debug work depend on incomplete shapes.

## Prevention Rule

Do not use "typed reconstruction" as an implementation recommendation. When
persisted data already has an owner type, start from that type or add an
accessor/extension there. A new record type is acceptable only when it is a real
persisted or wire schema, or a reusable domain object with an owning module.
Never create a one-time subset DTO just to avoid reading the existing type.

## Memory Hypothesis

Memory emphasized typed persistence and projection boundaries from prior
sessions. That was useful against anonymous JSON walking, but it also amplified
the bad local instruction that partial readers should become named projection
types.
