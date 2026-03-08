# Conversation UI Filtering

- connected with todo `../todo/2026-03-06_general.md`

## Description of Issue

We want to check our previous work on filtering the conversation view in the
UI. By default right now there is a lot of data presented to the user that
clutters up the UI. While this information is helpful in debugging, and we want
to make it possible to expose feedback from the different systems producing
these messages, we also want to keep the UI tight and focused on the most
relevant information for the user.

One example is in the feedback from the tools called by the LLM. Right now we
might see the number of items returned in the search result and similar info,
but instead we should present the user with minimal tool info, instead just
letting the user know the tool is being called, whether through the message
returned by the LLM during the tool call, or through a "calling tool <tool
name>" or similar.

## Survey of Verbosity Handling

### Matrix of Message Verbosity

Current behavior in `ploke-tui`:

| Message type | Source in conversation | Minimal | Normal (default) | Verbose | Notes |
|---|---|---|---|---|---|
| `User` | `Message.content` | Same as normal | Full content | Same as normal | Not controlled by tool verbosity. |
| `Assistant` | `Message.content` | Same as normal | Full content | Same as normal | Not controlled by tool verbosity. |
| `System` | `Message.content` | Same as normal | Full content | Same as normal | Not controlled by tool verbosity. |
| `SysInfo` | `Message.content` | Same as normal | Full content | Same as normal | Not controlled by tool verbosity; often used for diagnostics/status text. |
| `Tool` with `tool_payload` | `ToolUiPayload::render(...)` | `Tool: <name> - <summary>` | `Tool`, `Summary`, and all `Fields` | Normal + `Details` block | This is the only message type currently affected by verbosity. |
| `Tool` without `tool_payload` | `Message.content` | Same as normal | Full content | Same as normal | Fallback path if payload not attached. |
| User-facing annotations | `MessageAnnotation` with `Audience::User` | Same as normal | Rendered under message | Same as normal | Annotation display is independent of tool verbosity. |

Important current limitation: the conversation renderer still includes the full message path (no per-kind filtering mode yet), so lowering verbosity reduces tool payload detail but does not hide message classes.

### Verbosity Controls

Primary control points and dataflow:

| Layer | File(s) | Function(s) / Path | Responsibility |
|---|---|---|---|
| Tool payload model | `crates/ploke-tui/src/tools/ui.rs` | `ToolVerbosity`, `ToolUiPayload::render` | Defines verbosity levels and rendering format for tool payloads. |
| Conversation render path | `crates/ploke-tui/src/app/message_item.rs` | `render_message_content` | Applies `tool_verbosity` when rendering tool messages; non-tool messages bypass verbosity. |
| App-level runtime state | `crates/ploke-tui/src/app/mod.rs` | `App::tool_verbosity`, `apply_tool_verbosity`, `cycle_tool_verbosity` | Stores active runtime verbosity and updates config state. |
| Command parsing | `crates/ploke-tui/src/app/commands/parser.rs` | parse `tool verbosity` subcommands | Converts user command text to verbosity actions. |
| Command execution | `crates/ploke-tui/src/app/commands/exec.rs` | `ToolVerbositySet`, `ToolVerbosityToggle`, `ToolVerbosityShow` | Applies/queries verbosity through app runtime methods. |
| Keybinding entrypoint | `crates/ploke-tui/src/app/input/keymap.rs` | `Action::ToggleToolVerbosity` on `v` | Fast toggle in Normal mode. |
| Settings overlay | `crates/ploke-tui/src/app/view/components/config_overlay.rs` | `"Tool Verbosity"` enum item + `selected_tool_verbosity` + apply | UI control for changing verbosity without command line text. |
| Persistence model | `crates/ploke-tui/src/user_config.rs`, `crates/ploke-tui/src/app_state/core.rs` | `UserConfig.tool_verbosity` <-> `RuntimeConfig.tool_verbosity` | Persists verbosity across sessions. |
| Tool result producers | `crates/ploke-tui/src/tools/*.rs` | `ToolUiPayload::new(...).with_field(...).with_details(...)` | Determines how much structured information exists to be displayed at each verbosity. |

### User-facing Verbosity Controls

| Control surface | Values | Scope | Persistence | Current behavior |
|---|---|---|---|---|
| Normal mode keybind `v` | Cycles `minimal -> normal -> verbose` | Runtime app session | Yes (written into runtime config state) | Quick toggle; emits SysInfo confirmation message. |
| Command: `tool verbosity <minimal\|normal\|verbose\|toggle>` | Explicit set/toggle | Runtime app session | Yes | Main textual control for power users. |
| Command: `tool verbosity` | N/A (read-only) | Runtime app session | N/A | Prints current verbosity in SysInfo. |
| Config overlay: `UI -> Tool Verbosity` | `Minimal`, `Normal`, `Verbose` | Runtime app session | Yes | Interactive settings control. |
| Config file (`UserConfig.tool_verbosity`) | `minimal`, `normal`, `verbose` | Startup default + persisted preference | Yes | Seeds runtime default; currently defaults to `normal`. |

Behavioral note: these controls currently change only tool message formatting, not conversation message filtering (User/Assistant/System/SysInfo visibility).

### Verbosity State Locations

| Location type | File | Data structure / field | Mutated by | Read by | Notes |
|---|---|---|---|---|---|
| Enum definition | `crates/ploke-tui/src/tools/ui.rs` | `ToolVerbosity::{Minimal, Normal, Verbose}` | N/A (type definition) | All verbosity call sites | Canonical verbosity domain. |
| Persisted user config | `crates/ploke-tui/src/user_config.rs` | `UserConfig.tool_verbosity` | Config load/save pipeline | Runtime config construction | Serialized preference in user config. |
| Runtime config (state) | `crates/ploke-tui/src/app_state/core.rs` | `RuntimeConfig.tool_verbosity` | `App::apply_tool_verbosity`, config overlay apply | App startup/init and overlay hydration | Shared config state used by app runtime. |
| App runtime cache | `crates/ploke-tui/src/app/mod.rs` | `App.tool_verbosity` | `App::new`, `apply_tool_verbosity`, overlay sync path in `on_key_event` | Conversation rendering, copy selection, key toggle cycle | Immediate in-memory value used each frame. |
| Overlay UI state | `crates/ploke-tui/src/app/view/components/config_overlay.rs` | UI enum item `"Tool Verbosity"` + selected value | Overlay navigation/selection + `apply_to_runtime_config` | `selected_tool_verbosity` in app event loop | Acts as temporary editing buffer before commit. |
| Tool payload instance field | `crates/ploke-tui/src/tools/ui.rs` | `ToolUiPayload.verbosity` | Tool producers via `.with_verbosity(...)` | Currently not used by render path (render uses global argument) | Stored per payload, but global verbosity presently wins at render time. |
| Tool payload content shape | `crates/ploke-tui/src/tools/*.rs` | `ToolUiPayload.fields`, `details`, `summary` | Individual tool implementations | `ToolUiPayload::render` | Not the selected level itself, but defines what data each level can expose. |

Mutation paths (high-level):

| Entry path | Function chain | State transitions |
|---|---|---|
| Normal mode keybind `v` | `keymap -> Action::ToggleToolVerbosity -> App::cycle_tool_verbosity -> App::apply_tool_verbosity` | `App.tool_verbosity` updated, then `RuntimeConfig.tool_verbosity` updated asynchronously, with optional SysInfo confirmation. |
| Command `tool verbosity <...>` | `commands/parser -> commands/exec -> App::apply_tool_verbosity` | Same as above, but explicit value from command args. |
| Config overlay change | `ConfigOverlayState::apply_to_runtime_config` + `App::on_key_event` sync block | `RuntimeConfig.tool_verbosity` updated in config state; app copies selected value back into `App.tool_verbosity`. |
| Startup hydration | `UserConfig -> RuntimeConfig (From<UserConfig>) -> App::new(tool_verbosity)` | Persisted value becomes runtime default. |

## Survey of SysInfo and System Message Kinds

Survey scope: all `MessageKind::SysInfo` and `MessageKind::System` emit sites under `crates/` (primarily `crates/ploke-tui/src`).

Quick volume snapshot (emit-site count, rough): `app/commands/exec.rs` (~41), `app/events.rs` (~13), `rag/editing.rs` (~11), `app/mod.rs` (~10), `rag/search.rs` (~8), `app_state/database.rs` (~8).

### System Message Kinds

`System` is low-frequency and currently used for special/internal paths:

| Kind | Primary source(s) | Current intent | Suggested `VerbosityLevel` |
|---|---|---|---|
| Base system prompt message (`PROMPT_HEADER`) | `chat_history.rs` (`BASE_SYSTEM_PROMPT`) | Core LLM instruction context, not user-facing status | `Debug` (or hidden by default in conversation UI) |
| LLM loop overflow/secondary errors emitted into chat | `llm/manager/session.rs` (`emit_loop_error`) | Hard failure surfaced when assistant placeholder is already used | `Error` |
| Tool failed helper message (`Tool call failed: ...`) | `llm/manager/session.rs` (`add_tool_failed_message`) | Failure fallback as `System` (helper currently appears unused) | `Error` |
| Fallback/system notes inserted into request context (not chat message) | `rag/context.rs` (`RequestMessage::new_system(...)`) | Prompt construction metadata for LLM, not direct user feedback | `Debug` |

### SysInfo Message Kinds

`SysInfo` is the main operational/user-feedback channel, currently mixing user-facing updates with debug-heavy detail.

| Category | Primary source(s) | Examples | Suggested `VerbosityLevel` |
|---|---|---|---|
| UI/command acknowledgements | `app/mod.rs`, `app/commands/exec.rs`, `app_state/dispatcher.rs` | "Tool verbosity set to ...", "Context mode set to ...", "Copied selection.", help output, config save/load confirmations | `Info` |
| Model/provider/browser interaction warnings | `app/events.rs`, `app/commands/exec.rs` | Missing API key, no matching models/endpoints, failed endpoint fetch | `Warn` (hard failures as `Error`) |
| Indexing + DB lifecycle status | `app/events.rs`, `app_state/handlers/indexing.rs`, `app_state/handlers/db.rs`, `app_state/database.rs` | "Indexing...", "Indexing Succeeded", backup/load success, embedding restore notices | Success paths `Info`; degraded-but-recovered `Warn`; failures `Error` |
| RAG/search diagnostics and result dumps | `rag/search.rs` | BM25/dense/hybrid status, req_id-tagged result lists, score dumps | `Debug` (summary-only signal could be `Info`) |
| Tool proposal workflow summaries | `rag/tools.rs`, `tools/create_file.rs`, `rag/editing.rs` | Staged edit/create previews, approve/deny instructions, overlap/stale proposal notes, rescan summaries | Stage/preview blobs `Debug`; proposal state changes `Info`; overlap/retry guidance `Warn`; apply failures `Error` |
| General error funnel currently emitted as SysInfo | `app/events.rs` (`AppEvent::Error`), many command handlers | `"Error: ..."` messages routed through `SysInfo` kind | `Error` |

### Context Inclusion Note (Important for Future Filtering)

`SysInfo` context inclusion is currently mixed by insertion API, not by message intent:

| Path | Behavior |
|---|---|
| `chat::add_msg_immediate(...)` with `MessageKind::SysInfo` | Defaults to pinned context behavior (eligible for LLM context inclusion) |
| `chat::add_msg_immediate_sysinfo_unpinned(...)` | Explicitly UI-only, excluded from LLM context |

Current consequence: several noisy diagnostic `SysInfo` messages can be pinned unless callers opt into unpinned APIs. Message verbosity filtering should ideally separate:
1. visibility policy in conversation UI, and
2. context-inclusion policy for LLM prompt construction.

### Initial Classification Heuristic (for implementation planning)

| Signal pattern | Default classification |
|---|---|
| Message text starts with `Error:` / explicit failed operation / timeout / invalid path | `Error` |
| Missing configuration/data but app continues (missing key, no model match, no workspace selected, stale overlap) | `Warn` |
| Success/ack/state transition intended for normal user workflow | `Info` |
| Detailed dumps, previews, perf/score data, req_id-heavy diagnostics, long help/debug payloads | `Debug` |

## Actionable Next Steps

1. Define a new verbosity state enum that lives in `RuntimeConfig` with corresponding `UserConfig`

We will introduce a new enum, `MessageVerbosity` with variants for different message kinds such as User, Assistant, etc, which will provide more fine-grained control of message verbosity displayed in the conversation UI. 

Must also include a default_profile in `RuntimeConfig` and corresponding configs that contains hardcoded options for Minimal, Normal, and Verbose.

Render-only invariant (explicit):
- `MessageVerbosity` is a conversation presentation filter only.
- It MUST NOT alter LLM prompt assembly, request message construction, or chat loop behavior.
- It MUST NOT intersect with or mutate `ContextStatus` (`Pinned`/`Unpinned`), retention classes, TTL, or any context-management inclusion/exclusion workflow.
- Any truncation/hiding introduced by `MessageVerbosity` applies only at render time (and optional copy/view helpers), not storage or model-facing payloads.

```rust
pub enum MessageVerbosity {
    User { max_len: Option<u32>, syntax_highlighting: bool },
    Assistant { max_len: Option<u32>, syntax_highlighting: bool, 
        // Indicates whether or not to truncate messages before last message.
        truncate_prev_messages: bool,
        truncated_len: Option<u32>
    },
    SysInfo { max_len: Option<u32>, verbosity: VerbosityLevel },
    System { max_len: Option<u32>, verbosity: VerbosityLevel, 
        // Whether or not to display initial system message
        display_init: bool 
    }
}

pub enum VerbosityLevel {
    // Updates on app state with user-facing info that is not indicative of a
    // warning or error state, but not debug info either.
    Info,
    // Higher verbosity, granularity, or performance information useful in app
    // analysis or debugging. Not primarily user-facing.
    Debug,
    // Indicates a message that some recoverable fail state has been reached
    // and recovered from, or non-obvious misconfigurations or feedback.
    Warn,
    // Messages indicating some kind of fail state, such as failed file
    // resolution or networking timeout.
    Error
}
```

Implementation details (completed in this step):
- Added `VerbosityLevel`, `MessageVerbosity`, `MessageVerbosityProfile`, and `MessageVerbosityProfiles` in `crates/ploke-tui/src/user_config.rs`.
- Added persisted fields on `UserConfig`: `message_verbosity_profiles` and `message_verbosity_default_profile`.
- Added runtime fields on `RuntimeConfig` with full `From<UserConfig>` and `to_user_config()` mapping in `crates/ploke-tui/src/app_state/core.rs`.
- Added built-in defaults for `minimal`, `normal`, and `verbose` profile payloads (hardcoded constructors in `user_config.rs`).
- Added code comments documenting the render-only invariant so this state does not affect prompt assembly or context pinning workflows.
- No render/filter behavior was changed yet; this step only establishes state and persistence scaffolding.

2. Expand loaded user config

Add "Custom" message verbosity to loaded user config, and a "default_verbosity" which is either loaded from config and otherwise defaults to Minimal.

Implementation details (completed in this step):
- Added `MessageVerbosityProfile::Custom` in `crates/ploke-tui/src/user_config.rs`.
- Extended `MessageVerbosityProfiles` with a persisted `custom` profile vector and default constructor.
- Added persisted `UserConfig.default_verbosity` with default fallback to `Minimal`.
- Added a serde alias so older configs using `message_verbosity_default_profile` still deserialize into `default_verbosity`.
- Updated `RuntimeConfig` mapping (`From<UserConfig>` and `to_user_config()`) to use `default_verbosity`.

3. Expand interactive UI Config Overlay

Add config options for the different message verbosity levels, and a default verbosity profile.

Additionally, must include profile defaults for Minimal, Normal, Verbose, and Custom.

Implementation details (completed in this step):
- Added a new `UI` overlay option: `Default Message Verbosity` with `Minimal|Normal|Verbose|Custom`.
- Added a new `Message Verbosity` category in the config overlay with per-profile controls for:
  - `User` max length + syntax-highlighting
  - `Assistant` max length + syntax-highlighting + previous-message truncation controls
  - `SysInfo` verbosity level (`Info|Debug|Warn|Error`)
  - `System` verbosity level (`Info|Debug|Warn|Error`)
  - `System` initial-message visibility (`Show Init System`)
- Wired overlay apply logic to persist these settings into `RuntimeConfig.message_verbosity_profiles` and `RuntimeConfig.default_verbosity`.
- Wired conversation rendering (`ConversationView` / `message_item`) to resolve the active profile from `RuntimeConfig.default_verbosity` and apply render-time message filtering/truncation.
- Implemented render-time threshold filtering for `SysInfo` and `System` messages using configured `VerbosityLevel` and content-based classification heuristics.
- Enforced `System.display_init` in render path for initial `BASE_SYSTEM_PROMPT` visibility.
- Added focused integration tests for:
  - Presence of the new overlay controls
  - Applying `Custom` profile selections back into runtime config.

4. Expand commands for verbosity

Add a command `/verbosity profile <minimal|normal|verbose|custom>`

Custom verbosity level defined in user config or through UI config overlay

TODO (tests):
- Add a second approvals overlay test that explicitly switches diff view to non-minimal (`Expanded` or `Full`) and asserts unchanged-line rendering for normal behavior.
