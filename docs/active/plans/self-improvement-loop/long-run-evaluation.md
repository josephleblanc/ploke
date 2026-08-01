# Long Run Evaluation Route

Updated: 2026-05-10

This route is for evaluating a recent long Prototype 1 loop run. Do not use `scheduler.json` as authority for this track.

## Trustworthy Surfaces

- Sealed History blocks:
  `prototype1/history/blocks/segment-*.jsonl`
- Transition journal:
  `prototype1/transition-journal.jsonl`
- Successor ready/completion artifacts:
  `prototype1/nodes/<node-id>/successor-ready/*` and `prototype1/nodes/<node-id>/successor-completion/*`
- Evaluation artifacts:
  `prototype1/evaluations/*.json`
- Protocol artifacts joined from each branch evaluation's treatment run path.

## Current Playback Command

The sealed-record route is:

`HistoryCommand -> HistorySubcommand::Playback -> run_history_playback -> history_playback::run -> FsRunStore::load_history_blocks`

Smallest operator command:

```bash
cargo run -p ploke-eval -- history playback --campaign <campaign> --granularity coarse --format table
```

Use `--granularity fine` for sealed step detail. This path is sealed-History only; it will be empty until sealed History blocks exist.

## Live Status Caveat

The monitor terminal-state path currently combines:

- successor completion artifacts;
- transition-journal materialization state;
- parent PID liveness;
- a `scheduler.json` branch.

For this track, ignore the `scheduler.json` branch. A follow-up implementation should make the non-scheduler live-status route explicit instead of relying on caller discipline.

## Safe Raw Inspection

Before opening any raw journal or JSONL segment:

```bash
ls -lh <path>
wc -l <path>
```

Then use width-capped reads only:

```bash
tail -n 3 <path> | cut -c 1-400
rg -n '<pattern>' <path> | head -n 5 | cut -c 1-400
```

Do not read `transition-journal.jsonl` or sealed History JSONL segments wholesale.

## Missing Evaluation Pieces

- A typed, scheduler-free stop-reason projection suitable for the frontend.
- A joined transition/playback view that places runtime phases beside sealed History without letting journal order override sealed History authority.
- Aggregate benchmark trajectory across generations.
- A clear distinction between missing benchmark evidence, failed benchmark evidence, and not-applicable benchmark evidence.
