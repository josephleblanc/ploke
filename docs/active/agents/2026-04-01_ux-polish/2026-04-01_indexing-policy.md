# Task: Refactor `ploke-tui` commands
2026-04-01

status: in_progress

We will refactor the commands in `ploke-tui` to align with a new design that
better expresses the intent of the application to provide vector embedding on
the user's code base that is consistent with cargo semantics and is robust to
user error while attempting to guide recovery or achieve user intent.

## 1. Fill out partially defined decision tree
status: complete

If uncertain regarding the desired functionality implied by filled out
sections, just put TODO and the human will review it and provide guidance or
fill in sections until desired functionality is clear.

Identify any sections that you have low confidence in completing.

Decision tree for `/index`, `/load`, `/save`, `/update` commands
- pwd is workspace root:
  - no db loaded:
    - `/index` indexes workspace
    - `/index workspace` indexes workspace
    - `/index workspace .` indexes workspace
    - `/index path/to/crate` indexes target crate (if valid, see "Valid Crate Path" definition in 1b)
    - `/index crate path/to/crate` indexes target crate (if valid, see "Valid Crate Path" definition in 1b)
    - `/index crate <crate-name>` if member of workspace, index crate at
    workspace-defined path (validates via 1b)
    - `/index crate` add chat message with list of crate members of
    workspace + suggested command to index a crate, `/index crate <crate-name>`
    - `/load crate <crate-name>` loads crate if it exists in registry, otherwise 
      - if crate is member of workspace suggest command to index that crate
      `/index crate <crate-name>` with name filled in.
      - otherwise add a chat message with list of crates + suggested command
    - `/load workspace <crate-name>` if `crate-name` is not a workspace in
    registry and a crate with that name does exist in registry, prompt user
    with suggested command in chat: "It looks like you want to load a
    crate, use command: `/load crate <name-filled-in>`"
    - `/load workspace` load workspace that matches pwd if it exists,
    otherwise add informative error to chat with suggested command for
    `/index`
    - `/save db`
      - informative error message added to chat (e.g. "No crate/workspace in db...")
    - `/update` 
      - informative error message added to chat (e.g. "No crate/workspace in db...")
  - db already loaded and is:
    - single crate and workspace (focused crate is not workspace member)
      - debug_assert: invariant violated, loaded crate must be workspace member
    - single crate and workspace (focused crate is a workspace member)
      - `/index` re-indexes the focused crate (not entire workspace)
      - `/index workspace` re-indexes entire workspace (all members)
      - `/index crate <name>` 
        - if name matches focused crate: re-index focused crate
        - if name is different workspace member: switch focus to that member, then index
        - if name is not a workspace member: informative error "Crate '{name}'
          is not a member of the loaded workspace. Add it to your workspace's
          Cargo.toml members, then use `/index crate <path>` to index it."
      - `/load crate <crate-name>` 
        - if crate-name matches a workspace member: suggest using `/index crate <crate-name>` instead
        - if not a member but in registry: informative error "Crate
          '{crate-name}' is not a workspace member. Either add it to your
          workspace's Cargo.toml and index it, or switch workspaces with `/load
          workspace <name>`."
        - if not in registry: suggest indexing first with `/index crate <path>`
      - `/save db` saves the workspace snapshot
      - `/update` scans the focused crate for changes and re-indexes if stale
      - Transition cases (loading different crate/workspace):
        - if not same crate as in db 
          - is workspace member
            - in crate registry: load to db
            - not in crate registry: informative error message added to chat
            with suggested command to index the crate
          - not workspace member
            - in crate registry: informative error message "Attempting to
            load crate that is not member of workspace: either add to
            workspace members or select a workspace member"
            - not in registry: informative error message "Attempting to load
            crate that does exist in registry and is not a workspace member.
            You can index a new member with `<command>`"
    - single crate and not workspace
      - `/index` re-indexes the loaded standalone crate
      - `/index crate <name>` 
        - if crate name matches loaded crate: re-index
        - if different name: informative error "Already have a standalone crate loaded. 
          Use `/load crate <name>` to switch to a different crate."
      - `/index workspace` informative error "Current directory is not a workspace 
        root. Use `/index` to index the current crate or navigate to a workspace root"
      - `/load crate <crate-name>` 
        - check for unsaved changes; if unsaved, prompt to save or use `--force`
        - if crate exists in registry: unload current crate and load new crate
        - else: informative error with suggestion to index first
      - `/load workspace <name>` check for unsaved changes; if unsaved, prompt user 
        to save with `/save db` or use `--force` to discard; then load workspace 
        (replaces current standalone crate)
      - `/save db` saves the standalone crate snapshot (TODO: consider renaming to `/save` for brevity)
      - `/update` scans loaded crate for changes and re-indexes if stale
      - `/index path/to/other/crate` informative error "Cannot index a different 
        crate while one is loaded. Use `/load crate <name>` to switch to a different crate."
    - multiple crates and workspace
      - `/index` re-indexes all workspace members (full workspace)
      - `/index crate <name>` 
        - if member of workspace: indexes that specific crate
        - else: informative error "Crate '{name}' is not a member of the loaded workspace. 
          Use `/load crate <name>` to switch workspaces."
      - `/index workspace` re-indexes all workspace members (same as `/index`)
      - `/index path/to/crate` 
        - if path is within workspace: indexes that crate (path must be a valid workspace member; validates via 1b)
        - if outside workspace: informative error "Cannot index crate outside loaded 
          workspace. Add it to your workspace's Cargo.toml members first, then index."
      - `/load crate <crate-name>`
        - if name matches a workspace member: suggest using `/index crate <crate-name>` instead
        - if not a member but in registry: check for unsaved changes, then prompt to save or 
          use `--force`, then unload workspace and load the single crate
        - if not in registry: suggest indexing with `/index crate <path>`
      - `/load workspace <name>` if different workspace, check for unsaved changes; 
        if unsaved, prompt user to save with `/save db` or use `--force` to discard; 
        then load new workspace
      - `/save db` saves the workspace snapshot with all members
      - `/update` scans all workspace members for changes and re-indexes stale ones
    - multiple crates and not workspace
      -> debug_assert, invariant violated: multiple crates must be members of
         same workspace
    - multiple crates and multiple workspaces
      -> debug_assert, invariant violated: cannot have multiple workspaces loaded
    - single crate and multiple workspaces
      -> debug_assert, invariant violated: cannot have multiple workspaces loaded
  - db already loaded and is single crate and is workspace
    - Same as "single crate and workspace" above
- pwd is crate:
  - no db loaded:
    - `/index` indexes the current crate at pwd (detects if standalone or workspace member; validates via 1b)
    - `/index crate` indexes the current crate at pwd (validates via 1b)
    - `/index crate .` indexes the current crate at pwd (validates via 1b)
    - `/index workspace` if current crate is a workspace member, indexes entire 
      workspace; else informative error "Current directory is not a workspace member"
    - `/index path/to/other/crate` indexes that crate if valid (see "Valid Crate Path" definition below)
    - `/load crate <name>` loads crate from registry if entry exists and points to valid crate (see 1b), else informative error with suggestion to index
    - `/load workspace <name>` loads workspace from registry if exists, else 
      informative error with suggestion to index workspace root
    - `/save db` informative error "No crate/workspace in db to save"
    - `/update` informative error "No crate/workspace in db to update"
  - db already loaded:
    - `/index` re-indexes the crate at pwd (if it's a loaded crate); 
      if pwd is not a loaded crate, informative error "Current directory is not 
      a loaded crate. Use `/index crate <path>` to index a specific crate."
    - `/index workspace` re-indexes entire workspace (if pwd is a workspace member);
      if pwd is standalone crate, informative error "Current directory is not a workspace member"
    - `/index crate <name>` 
      - if name matches crate at pwd: re-index that crate
      - if name is different loaded crate: switch focus to that crate, then index
      - if name is not loaded: follow "pwd is workspace root" rules for loading new crates
    - `/load crate <name>` 
      - if name matches crate at pwd: informative error "Crate '{name}' is already loaded. Use `/index` to re-index."
      - else: check for unsaved changes, then proceed with load per "pwd is workspace root" rules
    - `/load workspace <name>` check for unsaved changes per "pwd is workspace root" rules
    - `/save db`, `/update` follow "pwd is workspace root" rules (pwd context does not affect these)

### 1a. Pre-Task Behavior Analysis: Standalone Crate ↔ Workspace Transitions

**Current Implementation Details (from `core.rs`, `database.rs`):**

1. **Both modes use `LoadedWorkspaceState` internally** (`core.rs:412-438`):
   - Standalone crate = single-member workspace (member_roots = vec![crate_root])
   - Full workspace = multi-member workspace
   - The distinction is conceptual, not structural

2. **State transitions are always destructive replacements**:
   - `load_standalone_crate()` calls `loaded_crates.clear()` and replaces `loaded_workspace` (`core.rs:580-595`)
   - `set_loaded_workspace()` calls `loaded_crates.clear()` and replaces `loaded_workspace` (`core.rs:529-561`)
   - No merge behavior exists - previous state is fully discarded

3. **`/load workspace <name>` does NOT check for unsaved changes** (`database.rs:917-1173`):
   - Immediately clears DB indices (`clear_hnsw_idx`, `clear_relations`)
   - Imports new backup and replaces system state
   - No prompt to save unsaved changes

4. **No `/workspace clear` command exists**:
   - `workspace rm` prevents removing the last crate (`database.rs:1866-1869`)
   - Users must load another crate/workspace to switch

**Decision Tree Updates for Transition Behavior:**

- `/load workspace <name>` when standalone crate loaded:
  - Check if current standalone crate has unsaved changes (not in registry or registry entry is stale)
  - If unsaved: prompt user with chat message: "Current crate has unsaved changes. Use `/save db` to save before loading workspace, or use `/load workspace <name> --force` to discard changes."
  - If saved or `--force`: proceed with load (current behavior)

- `/load crate <name>` when workspace loaded:
  - Same unsaved changes check as above
  - If unsaved: prompt user to save or use `--force`
  - If saved or `--force`: unload entire workspace and load single crate
  - Note: This is a destructive operation - entire workspace is replaced

- `/load crate <name>` when different crate loaded (standalone → standalone):
  - Same unsaved changes check
  - Prompt to save before switching if needed
  - Replace current crate with new crate

- `/index` when db already loaded (any mode):
  - **Destructive operation**: Re-parses source files and replaces DB content
  - Does not require `--force` - this is expected behavior (source files are source of truth)
  - Does not trigger state transitions (workspace/crate membership unchanged)
  - Note: `/update` is the incremental alternative that only processes changed files

### 1b. Valid Crate Path Definition

A crate path is **valid** for `/index` operations when all of the following are true:

1. **Path exists** - The directory exists on the filesystem (`syn_parser::discovery` checks this)
2. **Contains valid Cargo.toml** - The directory contains a `Cargo.toml` with a valid `[package]` section (`syn_parser::discovery::Manifest::from_path`)
3. **Not already loaded** - No crate with the same root path is already in the loaded state (check against `SystemStatus.loaded_crates`)
4. **Discoverable** - `syn_parser::discovery::run_discovery_phase_with_target` succeeds (validates crate structure, src/ directory, etc.)

**Implementation:** Use `syn_parser::discovery::run_discovery_phase_with_target` for validation. It returns specific `DiscoveryError` variants for each failure case.

**Validation applies to:**
- `/index path/to/crate` (explicit path)
- `/index crate <name>` when resolving name to path via workspace membership
- `/index` (implicit path from pwd)

**Error handling:** Map `DiscoveryError` variants to user-friendly chat messages:
- `CratePathNotFound`: "Path '{path}' does not exist. Check the path and try again."
- `MissingPackageName`/`InvalidCargoToml`: "Path '{path}' does not contain a valid Cargo.toml. Ensure this is a crate root directory."
- Already loaded (custom check): "Crate at '{path}' is already loaded. Use `/index` to re-index or `/load crate <name>` to switch."
- Other `DiscoveryError` variants: "Cannot index crate at '{path}': {error_details}"

**Consistency Principles Applied:**
1. All load operations that replace state (`/load crate`, `/load workspace`) check for unsaved changes
2. `/index` never triggers state transitions - only re-indexes existing state
3. Context (pwd) determines default scope: workspace root → full workspace, crate root → single crate
4. Context (pwd) determines the default target for commands that accept `<path>` or `<name>` arguments
5. Error messages always include suggested recovery command

**Sections with lower confidence:**
- [RESOLVED] ~~The exact behavior when transitioning between standalone crate and workspace modes~~
- Whether `/workspace clear` or similar command exists (marked as TODO) - **DECISION**: Do not implement for this task; use save-before-load prompt pattern instead
- The precise error message wording preferences
- **NEW**: Definition of "unsaved changes" - needs clarification:
  - Option A: Any `WorkspaceFreshness::Stale` crate counts as unsaved
  - Option B: Only check if workspace exists in registry at all
  - Option C: Track explicit `last_saved_ms` timestamp vs `last_modified_ms`
  - Option D: Always warn when replacing loaded state (safest, simplest for MVP)

## 2. Add a smoke test in TDD style for new decision tree behavior
status: in progress

Test file: `crates/ploke-tui/tests/command_decision_tree_smoke.rs`

The smoke test follows TDD principles - it should fail until the new decision tree is implemented. Test run results:

```
Summary: 8/13 tests passed, 5 failed

Failed tests:
1. save_db_no_db_loaded - Expected error about no crate/workspace, got "Success: Cozo data saved..."
2. update_no_db_loaded - Expected error about no crate/workspace, got actual behavior
3. (and 3 others)
```

This confirms the test correctly captures the expected new behavior. Once the decision tree is implemented (Step 3), these tests should pass.

Before writing test, verify that no `ploke-tui` tests are red. If any red, stop
and ask for guidance.

Test design:
- cover all cases in the decision tree
- use tracing subscriber with test-specific target and optional file append
- create mocked app
  - for actor that manages input, use or mirror actual default construction
  - for incoming/outgoing events, use mocked senders/receivers
- use expected input events sent with mocked sender
- validate expected output events with mocked receivers
- instead of panicking at first failure to match expected output, collect events and use tracing::error to print errors with event for context and tracing::debug to print successes with event for context
- keep a vec of tuple (successes, failures) for each success/failure
- after all inputs tested, enumerate on output, panic at first failure
- test should have a timeout

Test should fail until we implement new decision tree, confirm by running new test

## 2.5 Add event-based smoke test (CORRECT implementation)
status: not started

**Reference**: See `command_flow_analysis.md` for detailed architecture research.

### What happened in Step 2 (Initial Attempt)

The initial implementation in `crates/ploke-tui/tests/command_decision_tree_smoke.rs` was comprehensive but diverged from the specific instructions. It:

1. **Tested state, not events**: Polls `harness.state.chat` to check for `MessageKind::SysInfo` messages rather than validating output events from mocked receivers
2. **Used string matching**: Brittle predicates like `s.contains("indexing")` on UI text rather than checking event types
3. **Required expensive actor operations**: State setup involved actual indexing/parsing, making tests slow (~44 seconds for 72 cases)

**IMPORTANT**: The current comprehensive test is NOT a source of truth. Due to string matching brittleness, it may have false positives/negatives. It will need significant refactoring or may be scrapped entirely. **Do not rely on it for correctness.**

### Correct Step 2.5 Implementation

Create a NEW test file: `crates/ploke-tui/tests/command_decision_tree_smoke.rs` (replace the current one)

**Focus**: Test the **command parser + dispatcher** component directly via its event interface.

**Architecture Overview**:

Current: TODO

Desired: TODO

**Implementation Steps**:

1. Sort tests by DB access required (group all with same access in single test function)

TODO

**Test Design Requirements**:
TODO: Verify 65 correct number for all cases
- **Fast**: Each test <10ms (65 cases × 10ms = 650ms total)
- **Complete coverage**: All ~65 decision tree scenarios

**Key Files to Reference**:
- `app/commands/parser.rs` - Command parsing
- `app/commands/exec.rs` - Current (legacy) execution
- `app_state/commands.rs` - StateCommand definitions
- `app_state/dispatcher.rs` - Command dispatch
- `event_bus/mod.rs` - Event broadcasting
- `app_state/events.rs` - SystemEvent definitions

**Why this is better**:
1. **Fast**: No expensive actor operations; only validation logic runs
2. **Precise**: Tests the dispatcher contract other actors depend on
3. **Stable**: Event types are API; UI strings are implementation detail
4. **Focused**: Validates the component being refactored

**The comprehensive test**: Keep as `command_decision_tree_comprehensive.rs` but do not rely on it. Serves as scenario reference only. May be deleted after Step 2.5 is complete.

## 3. Implement new decision tree
status: not started

Follow design of new decision tree. Depending on implementation details, may or may not follow same design, but base conditions should be the same.

Condition for task completion: the TDD-style smoke test is green

#### re: implementation style

Where possible, prefer, in order:
1. lazy i/o (e.g. reading files)
2. easy performance wins where readability is near-equivalent (e.g. avoid
   unnecessary allocations)
3. match statements on tuples over if/else chains

Avoid:
- deeply nested if/else (none more than three chained if statements)

## 4. Review any new test failures
status: not started

Run the full `ploke-tui` test suite, verified in step 2 to be all green before implementing new decision tree.

Review the failing tests, sorting them into buckets using your best judgment.
Do not edit the tests in this step. The buckets sort where the failing test is failing due to:
1. Previous behavior and possibly side-effects that are neutral or beneficial to the desired functionality of the `ploke-tui` application
2. Previous behavior only, not unintended side effects. Indicates test should be updated in step 5.
3. Previous command behavior and catching negative unintended side-effect of new decision tree design that indicates underlying issues with new design. This would indicate that the test is still correct and should not be updated in step 5.
4. Any other failures

Create a report with the tests sorted as described, with a short description of
what each test is doing and referencing the test name + file, and your
reasoning on why it goes in that bucket.

Stop and wait for human review before moving on.

## 5. Update tests and/or revisit implementation
status: not started

Action here depend on (4), but generally will:
1. First address any underlying issues with the new design.
2. update tests that indicate healthy application behavior under new design to
   align with new design.

## 6. Update docs
status: not started

- Update documentation related to command behavior to align with new design,
including README.md
