# Command Stress Testing Plan (Options 1–3)

## Goal
Stress the command pipeline (input -> parsed command -> actor dispatch) under heavy concurrency and randomized ordering to surface race conditions, deadlocks, or inconsistent state transitions.

## Scope
- Crate: `crates/ploke-tui`
- Focus: `app_state::state_manager` command processing and chat/system state integrity.
- Excludes: live API, networked LLM calls, filesystem operations requiring external data.

## Option 1: Tokio Multi-Thread Stress Test (Probabilistic)
**Purpose:** High-throughput concurrency pressure using real async runtime.

### Plan
- Start `state_manager` with a mock `AppState`, `EventBus`, and `RagEvent` channel.
- Spawn N worker tasks (e.g., 8–16) concurrently.
- Each worker sends randomized commands to the shared `StateCommand` channel.
- Use a fixed RNG seed per worker for reproducibility.
- Apply small randomized `yield_now()` or sleeps to diversify interleavings.

### Command Set (Safe + Local)
- `AddMessageImmediate`
- `AddUserMessage`
- `UpdateMessage`
- `DeleteMessage`
- `NavigateList`
- `SetEditingPreviewMode`
- `SetEditingMaxPreviewLines`
- `SetEditingAutoConfirm`

### Assertions
- Test completes without panic or deadlock.
- If `chat.messages` is non-empty, `chat.current` and `chat.tail` must exist in `messages`.

### Deliverable
- Integration test: `crates/ploke-tui/tests/command_stress_tokio.rs`

## Option 2: Proptest Schedule Fuzzing (Structured Randomization)
**Purpose:** Repeatable randomized schedules with shrinking and reproducibility.

### Plan
- Use proptest to generate:
  - A list of `TestCommand` items.
  - A worker assignment schedule.
- Execute the schedule in a tokio runtime (spawn per worker).
- Use deterministic seeds from proptest on failure.

### Command Set
Same as Option 1, with bounded content sizes.

### Assertions
- No panic during execution.
- Same post-run state integrity checks as Option 1.

### Deliverable
- Integration test: `crates/ploke-tui/tests/command_stress_proptest.rs`

## Option 3: Loom Model Concurrency Test (True Interleavings)
**Purpose:** Exhaustively explore interleavings via `loom`.

### Plan
- Add a `loom`-gated integration test that models producer/consumer concurrency.
- Use `loom::model` to spawn multiple producers and a single consumer.
- Use a simplified command queue and state model that mirrors core invariants.

### Limitation
- The real `state_manager` uses `tokio` channels and async tasks, which are not loom-compatible.
- The loom test validates concurrency invariants of the model, not the full async actor system.
- A true loom test of the real actor system would require a refactor to loom-compatible primitives.

### Assertions
- No lost commands in the modeled queue.
- State invariants remain valid after all interleavings.

### Deliverable
- Loom-gated test: `crates/ploke-tui/tests/command_stress_loom.rs`
- Cargo feature: `loom`

## Execution Order
1) Implement Option 1 (tokio stress).
2) Implement Option 2 (proptest schedule).
3) Implement Option 3 (loom model test).
