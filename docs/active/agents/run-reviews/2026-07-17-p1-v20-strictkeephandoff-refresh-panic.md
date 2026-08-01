# P1 v20 Post-Edit Refresh Panic Review

Campaign:
`p1-v20-strictkeephandoff-mbe-g35f-direct-3g1x3-p3-20260717-214004`

Status: preserved failed live run; controller attempt abandoned at the
generation-zero baseline boundary; no successor handoff.

## Verdict

V20 did not test strict successor handoff. Its baseline model turn mutated the
benchmark workspace, then a strict post-edit parser invariant panicked without
producing a terminal tool event. The enclosing run waited until its
1,800-second wall-clock deadline and the R5-to-R6 edge did not commit.

The strict parser check was correct: the model introduced a duplicate
`replacement_multi_line_look_around` relation. The operational defect was the
missing failure terminalization around that check. V20 is preserved and
abandoned; changing its source, proposal file, profile, transition journal, or
hash evidence would invalidate it as handoff history.

## Admitted Shape

The committed profile used:

```toml
[storage.eval]
backend = "dual-strict"

[search]
max_generations = 3
max_total_nodes = 10
require_keep_for_continuation = true

[search.children]
min = 1
max = 3
parallel_targets = 3

[selection]
strategy = "generation-local"

[selection.oracle]
mode = "record-only"
require_evidence = true
gate = "all-resolved"

[execution.broad_tui]
max_attempts = 3
fresh_slots_per_child = 2
timeout_secs = 1800

[execution.mbe]
enabled = true

[control]
mode = "step"
parallel_cap = 3
```

Eval and protocol used direct Google `google/gemini-3.5-flash`. Eval indexing
was explicitly pinned to OpenRouter
`mistralai/codestral-embed-2505`. The baseline budget was 40 turns, 200 tool
calls, 32,768 completion tokens, and 1,800 seconds.

## Incident Evidence

- Run root:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-v20-strictkeephandoff-mbe-g35f-direct-3g1x3-p3-20260717-214004/BurntSushi__ripgrep-2209/runs/run-1784324895421-structured-current-policy-f98eca48`
- Trace: `agent-turn-trace.json`
- Admitted base: `4dc6c73c5a9203c5a8a89ce2161feca542329812`
- First applied request:
  `function-call-719232cf-a3bc-4957-9e6f-459ecc38ef14`
- Stranded duplicate request:
  `function-call-5704a665-4e76-4f86-b03b-8a5eacb11651`

The duplicate request wrote a second function with the same semantic identity
to `crates/printer/src/standard.rs`. Strict validation reported
`Expected unique relations`, but the detached auto-confirm task exited before
the tool loop observed either completion or failure.

No selection, History block, successor checkout, or successor-runtime handoff
occurred.

## Repair And Replay

Commit `8ae465ebf` contains the parser unwind at the in-memory refresh boundary
and settles write-plus-refresh failures as durable `PartiallyApplied` tool
failures. It does not weaken parser, History, digest, epoch, checkout, or
handoff verification.

The historical replay uses the real run manifest and typed trace carriers,
replays both exact requests through production code, and verifies disk
mutation, the unchanged strict invariant, structured applied-file context,
exact write/hash UI details, save-before-terminal ordering, and the absence of
a false applied completion.

The exact replay, full serial `ploke-tui` library suite, and full serial
non-default `ploke-eval` library suite pass. The complete replay-feature suite
has one unrelated unavailable fd-1121 trace fixture; 1,395 other tests,
including v20, pass.

## Disposition And Next Proof

Keep v20 unchanged. The next proof must:

1. admit a fresh campaign from `8ae465ebf` or later;
2. retain the same strict dual-store, oracle, MBE, provider, embedding, and
   3-generation 1x3 profile;
3. complete generation zero with a resolved `keep`;
4. seal and verify the existing History/digest chain;
5. hand the same walk endpoint to the verified successor runtime; and
6. let that successor advance at least one additional generation without
   rewriting predecessor evidence.

If a future model edit violates the parser invariant, the lane may fail, but it
must now fail terminally and remain inspectable instead of hanging.
