# Cozo queries for live-loop functionality questions

Store read-only Cozo scripts here, one query/probe per live-loop functionality question where practical.

Naming convention:

```text
<section-id>-<number>__<short-slug>.cozo
```

Examples:

```text
authority-01__parent-runtime.cozo
policy-04__tool-loop-gated.cozo
handoff-03__history-seal.cozo
```

Rules:

- Run queries only through `ploke-eval loop walk db_query --script <cozo>`.
- Do not inspect the SQLite database directly.
- Keep queries read-only.
- Prefer normalized eval-store relations.
- Do not treat `eval_record_ref` as a substitute for missing normalized facts; use it only as provenance evidence.
- If a question cannot be answered from DB rows, keep the closest probe here and explain the gap in the matching response file.
