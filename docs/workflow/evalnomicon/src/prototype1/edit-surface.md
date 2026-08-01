# Edit Surface

Prototype 1 uses a bounded edit surface so the first self-improvement loop can be observed without letting ordinary children rewrite the authority core that decides admission and succession.

## Surface partition

Current code comments use this partition:

```text
ArtifactSurface = Immutable + Mutated + Ambient
```

Current concrete Prototype 1 partition:

- **Immutable:** `crates/ploke-eval`
- **Mutated:** tool-description text files
- **Ambient:** empty declared surface

The exact partition is an implementation policy, not a universal model. The important rule is that the policy-bearing authority surface must be explicitly identified and protected.

## Policy-bearing surface

The policy-bearing surface contains the code that defines parent creation, child/successor execution, Crown transitions, History admission, surface checks, and handoff. Ordinary Prototype 1 self-improvement must not mutate that surface until an explicit protocol-upgrade transition exists.

Changing that surface is not just another child patch. It is a protocol upgrade or fork candidate.

## Admission role

The edit surface should support answers to these questions:

- What files may a child propose edits to?
- What files are protected from ordinary mutation?
- What evidence proves the candidate artifact stayed within the grant?
- What commitments are sealed for verifier replay?
- Which transition admits a broader surface later?

## Canonical sources

- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
- Draft source: `docs/workflow/evalnomicon/drafts/edit-surface/model.md`
