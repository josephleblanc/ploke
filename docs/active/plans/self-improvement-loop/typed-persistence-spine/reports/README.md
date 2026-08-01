# Raw Survey Reports

Sub-agents write bounded JSONL reports here.

Naming:

```text
YYYY-MM-DD-<family-slug>.<agent-slug>.jsonl
```

Each report must include one `summary` object and bounded `surface` objects as defined in [`../../typed-persistence-survey-orchestration.md`](../../typed-persistence-survey-orchestration.md).
