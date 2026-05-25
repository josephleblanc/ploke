---
name: type-reuse-discipline
description: Use this skill before adding a new Rust struct, enum, DTO, fixture schema, request/response carrier, replay record type, or serialized data shape, especially in ploke-eval, ploke-tui, ploke-records, or tests. It prevents duplicate local types when existing persisted or typed carriers already express the same data.
---

# Type Reuse Discipline

Use this before introducing a new data shape. The default is to reuse an
existing type, persisted record, or fixture format unless there is a concrete
reason it cannot represent the contract under test.

## Hard Gate

Do not add a new struct, enum, DTO, fixture wrapper, or ad hoc JSON schema until
you can answer these in one or two sentences:

- What exact contract needs a named type?
- Which existing types or persisted formats did you check?
- Why is each existing type insufficient?
- Is the new type private test glue, or is it becoming a public or persisted
  contract?

If the answer is "I need a convenient shape for this test," first check whether
the test can use the real persisted shape instead.

## Search First

Search by role, not just by field names:

```bash
rg -n "struct .*Request|struct .*Record|enum .*Event|RawFullResponseRecord|ToolRequestRecord|Published.*Request|Headless.*Summary|fixture|replay" crates/ploke-eval/src crates/ploke-tui/src crates/ploke-records/src
```

For ploke-eval replay or Prototype 1 work, check these surfaces first:

- `crates/ploke-eval/src/replay/`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/`
- `crates/ploke-records/src/`
- existing crate-local fixtures under `src/tests/fixtures/`

## Preference Order

1. Use the real persisted format the production path writes or reads.
2. Use an existing typed carrier from the owning crate.
3. Add a small helper function that builds the existing type.
4. Add a private test-local wrapper only if it removes duplication without
   becoming a second source of truth.
5. Add a new shared type only when it names a real domain concept and has an
   owning module.

Avoid mirror types that just rename fields from an existing record. They drift,
hide behavior, and make tests prove the wrapper instead of the production
contract.

## Fixture Rule

Checked-in fixtures should usually be examples of existing persisted formats,
not new test-only schemas. For replay tests, prefer checked-in
`RawFullResponseRecord`, `ToolRequestRecord`, `PublishedBroadHarnessRequest`, or
headless evidence summaries over a bespoke replay-fixture wrapper.

If a compact wrapper is unavoidable, keep it private to the test module and
include a comment naming the production type it deliberately does not replace.

## Report Back

When this skill changes your implementation choice, say which existing carrier
you reused and which new type you avoided adding.
