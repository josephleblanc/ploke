Verified surface: focused code/doc/git inspection plus targeted tests, not live replay or egui.

Current status: the **first graph-backed runtime playback slice has landed**, mostly in commit `4892aba7`. The implementation handoff is stale where it says `RuntimePlaybackRef` does not exist; it now does.

What is implemented:

- `ploke-records` has the older generic playback vocabulary: `EvidenceStrength`, `PlaybackGranularity`, `RunPlayback<G>`, and `RunPlaybackRef<'a, G>` in [playback.rs](/home/brasides/code/ploke/crates/ploke-records/src/playback.rs:5).
- `ploke-tree` now has graph-backed runtime playback: `PlaybackScope`, `PlaybackCursor`, `RuntimePlaybackIndex`, `RuntimePlaybackRef`, frames, deltas, and warnings in [runtime.rs](/home/brasides/code/ploke/crates/ploke-tree/src/playback/runtime.rs:12).
- Implemented scopes are `Lineage` and `ArtifactAncestry` in [runtime.rs](/home/brasides/code/ploke/crates/ploke-tree/src/playback/runtime.rs:15).
- Runtime steps borrow sealed History blocks from `Graph` and carry `EvidenceStrength::SealedHistory` in [runtime.rs](/home/brasides/code/ploke/crates/ploke-tree/src/playback/runtime.rs:158).
- Agent-turn event playback exists as borrowed `TurnCursor` / `TurnEventStepRef` / `TurnEventPlaybackRefSteps` in [turn.rs](/home/brasides/code/ploke/crates/ploke-tree/src/playback/turn.rs:41).
- `RuntimePlaybackFrameRef` currently attaches agent-turn steps, but the join is still unscoped and warning-bearing, not a real selected-step timeline join.
- `ploke-eval run replay inspect`, `turn-live`, and `self-edit-live` exist. `turn-live` supports selected prefixes, `--tail stop`, `--tail live`, `--tail live-step`, and branch tape support around typed `TurnCursor` anchors.

What is still missing:

- No owned `RuntimePlayback<G>` for graph-backed runtime playback yet.
- No single shared playback cursor state with `seek`, singular `frame()`, or singular `delta()` as described in the design doc.
- No typed `AgentTurns` or `CostEvents` runtime playback iterator family yet.
- No scoped join from one runtime playback step to the relevant agent-turn timeline; current `agent_turn_steps()` returns all loaded turn events.
- No records-owned request snapshot / provider exchange schema yet.
- Replay request tap still does not capture the full `ChatCompRequest` boundary.
- No egui consumer of `RuntimePlaybackRef`; search found usage only in `ploke-tree` exports/tests, not `ploke-egui`.

Recent commits that matter:

- `958e1a9d` added agent-turn playback cursors.
- `4c4f3c76` added the historical replay probe workflow and the `inspect` / `turn-live` replay modules.
- `c4920d1b` added live-step replay and branch tape support.
- `5c881735` refactored replay probe table rendering.
- `44095f2e` added runtime playback observability design docs.
- `16535496` mapped the runtime playback record inventory.
- `4892aba7` added `crates/ploke-tree/src/playback/runtime.rs` and exported the graph-backed runtime playback slice.

Tests run:

- `cargo test -p ploke-tree runtime_playback 2>&1 | tail -n 80`: passed 4 tests. These prove lineage scope ordering from graph lineage order, missing block warnings instead of fabricated steps, borrowed step refs pointing into the graph, and artifact ancestry matching History artifact refs.
- `cargo test -p ploke-eval replay::turn 2>&1 | tail -n 80`: passed 4 tests. These prove turn anchor resolution, response-index prefix slicing, event-to-response mapping, and prefix-then-live installation by assistant message id.
- `cargo test -p ploke-eval replay::inspect 2>&1 | tail -n 80`: passed 1 test. This proves inspect can list typed agent-turn steps without scheduler, detect a gapped tape, and surface stale prompt/workspace path signals.

Worktree note: there are unrelated/local dirty changes, including a small test-helper update in `crates/ploke-tree/src/graph/build/passive.rs`; it does not change the runtime playback implementation shape.
