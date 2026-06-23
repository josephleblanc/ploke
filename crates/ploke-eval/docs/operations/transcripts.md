# Transcripts and Inspection

Use these commands after a run or batch has produced artifacts.

## Assistant transcript

Print assistant messages from the most recent completed run:

```bash
cargo run -p ploke-eval -- transcript
```

The default run is resolved through:

```text
~/.ploke-eval/last-run.json
```

## Conversations and inspect

```bash
cargo run -p ploke-eval -- conversations --help
cargo run -p ploke-eval -- inspect --help
```

Use these for turn lists, failures, tool calls, and stored snapshots.

## History-shaped projections

```bash
cargo run -p ploke-eval -- history --help
```

History commands are read-only projections over persisted evidence. In Prototype
1, projections are useful for debugging but do not replace sealed History or
explicit typed transition evidence.
