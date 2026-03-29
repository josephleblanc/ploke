# KL-GHT-XXX Short Title

## Metadata

- Status: Proposed | Active | Mitigated | Fixed | Documented
- First observed: YYYY-MM-DD
- Last updated: YYYY-MM-DD
- Component:
- Pipeline stage: discovery | resolve | merge
- Severity: low | medium | high
- In scope for near-term fix: yes | no | undecided

## Corpus Evidence

- Run id:
- Primary failing target:
- Additional affected targets:
- Command:
  - `cargo xtask parse debug corpus --limit ...`
- Follow-up commands:
  - `cargo xtask parse debug corpus-show <run-id> --target <owner/repo>`
  - `cargo xtask parse debug logical-paths <crate-path>`
  - `cargo xtask parse debug modules-premerge <crate-path>`
  - `cargo xtask parse debug path-collisions <crate-path>`
- Artifact paths:
  - run summary:
  - target summary:
  - stage artifact:
  - failure artifact:

## Symptom

Describe the user-visible parser failure precisely.

## Current Root Cause

State the narrowest root cause we can defend from the artifacts. If still
uncertain, label this section as a hypothesis and list the remaining unknowns.

## Minimal Reproduction

### Real target

- Repository:
- Commit:
- Entry crate/path:

### Reduced fixture

- Existing fixture or test:
- New fixture needed:
- Smallest code shape that reproduces the issue:

```rust
// minimal repro here
```

## Impact

- What graph information is missing, wrong, or unstable?
- Does the failure panic, return an error, or silently produce degraded data?
- Which downstream features are blocked or made unreliable?

## Decision

- Chosen path: fix now | document for now
- Rationale:

## Intermediate Mitigation

If full support is out of scope, describe the smallest safe improvement we
should implement now.

- Detection to add:
- Data to preserve for later handling:
- Operator-facing diagnostics:
- Safety/correctness constraints:

## Extension Points

Describe how later work should plug into the intermediate mitigation.

- Expected future capability:
- Proposed API / data model seam:
- Files or modules likely to change:

## Implementation Notes

- Candidate code locations:
- Risks:
- Alternatives considered:

## Validation

- Tests added or planned:
- Corpus target(s) to rerun:
- Success criteria:

## References

- Related issue/docs:
- Related commits/PRs:
