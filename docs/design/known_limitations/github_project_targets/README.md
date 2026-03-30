# GitHub Project Target Limitations

This directory tracks parser limitations discovered while running the curated
GitHub target corpus through:

```text
cargo xtask parse debug corpus ...
```

Use one document per limitation family, not one document per failed repository.
Group multiple repositories into the same document when they fail for the same
underlying reason.

## Workflow

1. Run the corpus harness and capture the run id.
2. Inspect the first failing target with `cargo xtask parse debug corpus-show`.
3. Reduce the failure to the smallest parser limitation we can explain.
4. Decide whether to:
   - fix the parser now, or
   - document the limitation and add an intermediate mitigation / extension
     point.
5. Record the result in a document created from
   [issue_template.md](/home/brasides/code/ploke/docs/design/known_limitations/github_project_targets/issue_template.md).

## Document scope

Each limitation document should capture:

- the affected repositories and corpus run ids
- the failing pipeline stage (`discovery`, `resolve`, or `merge`)
- the smallest known reproduction
- whether the issue is in scope for a near-term parser fix
- the interim handling we want before full support lands
- the extension points future work should build on

## Naming

Prefer filenames shaped like:

```text
KL-GHT-00x-short-slug.md
```

Examples:

- `KL-GHT-001-include-macro-dangling-paths.md`
- `KL-GHT-002-unnamed-const-id-collision.md`

