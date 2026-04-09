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

```text
┌────────────┬────────────────┬───────────┬──────────────┬──────────┬──────────────────────────────┐
│ Message    │ Source in      │ Minimal   │ Normal       │ Verbose  │ Notes                        │
│ type       │ conversation   │           │ (default)    │          │                              │
╞════════════╪════════════════╪═══════════╪══════════════╪══════════╪══════════════════════════════╡
│ `User`     │ `Message.      │ Same as   │ Full content │ Same as  │ Not controlled by tool       │
│            │ content`       │ normal    │              │ normal   │ verbosity.                   │
├────────────┼────────────────┼───────────┼──────────────┼──────────┼──────────────────────────────┤
│ `Assistan- │ `Message.      │ Same as   │ Full content │ Same as  │ Not controlled by tool       │
│ t`         │ content`       │ normal    │              │ normal   │ verbosity.                   │
├────────────┼────────────────┼───────────┼──────────────┼──────────┼──────────────────────────────┤
│ `System`   │ `Message.      │ Same as   │ Full content │ Same as  │ Not controlled by tool       │
│            │ content`       │ normal    │              │ normal   │ verbosity.                   │
├────────────┼────────────────┼───────────┼──────────────┼──────────┼──────────────────────────────┤
│ `SysInfo`  │ `Message.      │ Same as   │ Full content │ Same as  │ Not controlled by tool       │
│            │ content`       │ normal    │              │ normal   │ verbosity; often used for    │
│            │                │           │              │          │ diagnostics/status text.     │
├────────────┼────────────────┼───────────┼──────────────┼──────────┼──────────────────────────────┤
│ `Tool`     │ `ToolUiPayloa- │ `Tool:    │ `Tool`,      │ Normal + │ This is the only message     │
│ with       │ d::render(...) │ <name> -  │ `Summary`,   │ `Detail- │ type currently affected by   │
│ `tool_     │ `              │ <summary  │ and all      │ s` block │ verbosity.                   │
│ payload`   │                │ >`        │ `Fields`     │          │                              │
├────────────┼────────────────┼───────────┼──────────────┼──────────┼──────────────────────────────┤
│ `Tool`     │ `Message.      │ Same as   │ Full content │ Same as  │ Fallback path if payload not │
│ without    │ content`       │ normal    │              │ normal   │ attached.                    │
│ `tool_     │                │           │              │          │                              │
│ payload`   │                │           │              │          │                              │
├────────────┼────────────────┼───────────┼──────────────┼──────────┼──────────────────────────────┤
│ User-      │ `MessageAnnot- │ Same as   │ Rendered     │ Same as  │ Annotation display is        │
│ facing     │ ation` with    │ normal    │ under        │ normal   │ independent of tool          │
│ annotatio- │ `Audience::    │           │ message      │          │ verbosity.                   │
│ ns         │ User`          │           │              │          │                              │
└────────────┴────────────────┴───────────┴──────────────┴──────────┴──────────────────────────────┘
```

Important current limitation: the conversation renderer still includes the full message path (no per-kind filtering mode yet), so lowering verbosity reduces tool payload detail but does not hide message classes.

### Verbosity Controls

Primary control points and dataflow:

```text
┌──────────┬────────────────────────────┬────────────────────────┬─────────────────────────────────┐
│ Layer    │ File(s)                    │ Function(s) / Path     │ Responsibility                  │
╞══════════╪════════════════════════════╪════════════════════════╪═════════════════════════════════╡
│ Tool     │ `crates/ploke-tui/src/     │ `ToolVerbosity`,       │ Defines verbosity levels and    │
│ payload  │ tools/ui.rs`               │ `ToolUiPayload::       │ rendering format for tool       │
│ model    │                            │ render`                │ payloads.                       │
├──────────┼────────────────────────────┼────────────────────────┼─────────────────────────────────┤
│ Convers- │ `crates/ploke-tui/src/app/ │ `render_message_       │ Applies `tool_verbosity` when   │
│ ation    │ message_item.rs`           │ content`               │ rendering tool messages;        │
│ render   │                            │                        │ non-tool messages bypass        │
│ path     │                            │                        │ verbosity.                      │
├──────────┼────────────────────────────┼────────────────────────┼─────────────────────────────────┤
│ App-     │ `crates/ploke-tui/src/app/ │ `App::tool_verbosity`, │ Stores active runtime verbosity │
│ level    │ mod.rs`                    │ `apply_tool_           │ and updates config state.       │
│ runtime  │                            │ verbosity`,            │                                 │
│ state    │                            │ `cycle_tool_verbosity` │                                 │
├──────────┼────────────────────────────┼────────────────────────┼─────────────────────────────────┤
│ Command  │ `crates/ploke-tui/src/app/ │ parse `tool verbosity` │ Converts user command text to   │
│ parsing  │ commands/parser.rs`        │ subcommands            │ verbosity actions.              │
├──────────┼────────────────────────────┼────────────────────────┼─────────────────────────────────┤
│ Command  │ `crates/ploke-tui/src/app/ │ `ToolVerbositySet`,    │ Applies/queries verbosity       │
│ executi- │ commands/exec.rs`          │ `ToolVerbosityToggle`, │ through app runtime methods.    │
│ on       │                            │ `ToolVerbosityShow`    │                                 │
├──────────┼────────────────────────────┼────────────────────────┼─────────────────────────────────┤
│ Keybind- │ `crates/ploke-tui/src/app/ │ `Action::              │ Fast toggle in Normal mode.     │
│ ing      │ input/keymap.rs`           │ ToggleToolVerbosity`   │                                 │
│ entrypo- │                            │ on `v`                 │                                 │
│ int      │                            │                        │                                 │
├──────────┼────────────────────────────┼────────────────────────┼─────────────────────────────────┤
│ Settings │ `crates/ploke-tui/src/app/ │ `"Tool Verbosity"`     │ UI control for changing         │
│ overlay  │ view/components/config_    │ enum item +            │ verbosity without command line  │
│          │ overlay.rs`                │ `selected_tool_        │ text.                           │
│          │                            │ verbosity` + apply     │                                 │
├──────────┼────────────────────────────┼────────────────────────┼─────────────────────────────────┤
│ Persist- │ `crates/ploke-tui/src/     │ `UserConfig.tool_      │ Persists verbosity across       │
│ ence     │ user_config.rs`,           │ verbosity` <->         │ sessions.                       │
│ model    │ `crates/ploke-tui/src/app_ │ `RuntimeConfig.tool_   │                                 │
│          │ state/core.rs`             │ verbosity`             │                                 │
├──────────┼────────────────────────────┼────────────────────────┼─────────────────────────────────┤
│ Tool     │ `crates/ploke-tui/src/     │ `ToolUiPayload::new(.. │ Determines how much structured  │
│ result   │ tools/*.rs`                │ .).with_field(...).    │ information exists to be        │
│ produce- │                            │ with_details(...)`     │ displayed at each verbosity.    │
│ rs       │                            │                        │                                 │
└──────────┴────────────────────────────┴────────────────────────┴─────────────────────────────────┘
```

### User-facing Verbosity Controls

```text
┌─────────────────────────┬───────────────┬────────────────┬────────────────┬──────────────────────┐
│ Control surface         │ Values        │ Scope          │ Persistence    │ Current behavior     │
╞═════════════════════════╪═══════════════╪════════════════╪════════════════╪══════════════════════╡
│ Normal mode keybind `v` │ Cycles        │ Runtime app    │ Yes (written   │ Quick toggle; emits  │
│                         │ `minimal ->   │ session        │ into runtime   │ SysInfo confirmation │
│                         │ normal ->     │                │ config state)  │ message.             │
│                         │ verbose`      │                │                │                      │
├─────────────────────────┼───────────────┼────────────────┼────────────────┼──────────────────────┤
│ Command: `tool          │ Explicit      │ Runtime app    │ Yes            │ Main textual control │
│ verbosity               │ set/toggle    │ session        │                │ for power users.     │
│ <minimal|normal|verbos- │               │                │                │                      │
│ e|toggle>`              │               │                │                │                      │
├─────────────────────────┼───────────────┼────────────────┼────────────────┼──────────────────────┤
│ Command: `tool          │ N/A           │ Runtime app    │ N/A            │ Prints current       │
│ verbosity`              │ (read-only)   │ session        │                │ verbosity in         │
│                         │               │                │                │ SysInfo.             │
├─────────────────────────┼───────────────┼────────────────┼────────────────┼──────────────────────┤
│ Config overlay: `UI ->  │ `Minimal`,    │ Runtime app    │ Yes            │ Interactive settings │
│ Tool Verbosity`         │ `Normal`,     │ session        │                │ control.             │
│                         │ `Verbose`     │                │                │                      │
├─────────────────────────┼───────────────┼────────────────┼────────────────┼──────────────────────┤
│ Config file             │ `minimal`,    │ Startup        │ Yes            │ Seeds runtime        │
│ (`UserConfig.tool_      │ `normal`,     │ default +      │                │ default; currently   │
│ verbosity`)             │ `verbose`     │ persisted      │                │ defaults to          │
│                         │               │ preference     │                │ `normal`.            │
└─────────────────────────┴───────────────┴────────────────┴────────────────┴──────────────────────┘
```

Behavioral note: these controls currently change only tool message formatting, not conversation message filtering (User/Assistant/System/SysInfo visibility).

### Verbosity State Locations

```text
┌──────────┬────────────────┬─────────────┬──────────────────┬────────────────┬────────────────────┐
│ Location │ File           │ Data        │ Mutated by       │ Read by        │ Notes              │
│ type     │                │ structure / │                  │                │                    │
│          │                │ field       │                  │                │                    │
╞══════════╪════════════════╪═════════════╪══════════════════╪════════════════╪════════════════════╡
│ Enum     │ `crates/ploke- │ `ToolVerbo- │ N/A (type        │ All verbosity  │ Canonical          │
│ definit- │ tui/src/tools/ │ sity::      │ definition)      │ call sites     │ verbosity domain.  │
│ ion      │ ui.rs`         │ {Minimal,   │                  │                │                    │
│          │                │ Normal,     │                  │                │                    │
│          │                │ Verbose}`   │                  │                │                    │
├──────────┼────────────────┼─────────────┼──────────────────┼────────────────┼────────────────────┤
│ Persist- │ `crates/ploke- │ `UserConfi- │ Config load/save │ Runtime config │ Serialized         │
│ ed user  │ tui/src/user_  │ g.tool_     │ pipeline         │ construction   │ preference in user │
│ config   │ config.rs`     │ verbosity`  │                  │                │ config.            │
├──────────┼────────────────┼─────────────┼──────────────────┼────────────────┼────────────────────┤
│ Runtime  │ `crates/ploke- │ `RuntimeCo- │ `App::apply_     │ App            │ Shared config      │
│ config   │ tui/src/app_   │ nfig.tool_  │ tool_verbosity`, │ startup/init   │ state used by app  │
│ (state)  │ state/core.rs` │ verbosity`  │ config overlay   │ and overlay    │ runtime.           │
│          │                │             │ apply            │ hydration      │                    │
├──────────┼────────────────┼─────────────┼──────────────────┼────────────────┼────────────────────┤
│ App      │ `crates/ploke- │ `App.tool_  │ `App::new`,      │ Conversation   │ Immediate          │
│ runtime  │ tui/src/app/   │ verbosity`  │ `apply_tool_     │ rendering,     │ in-memory value    │
│ cache    │ mod.rs`        │             │ verbosity`,      │ copy           │ used each frame.   │
│          │                │             │ overlay sync     │ selection, key │                    │
│          │                │             │ path in          │ toggle cycle   │                    │
│          │                │             │ `on_key_event`   │                │                    │
├──────────┼────────────────┼─────────────┼──────────────────┼────────────────┼────────────────────┤
│ Overlay  │ `crates/ploke- │ UI enum     │ Overlay          │ `selected_     │ Acts as temporary  │
│ UI state │ tui/src/app/   │ item `"Tool │ navigation/      │ tool_          │ editing buffer     │
│          │ view/          │ Verbosity"` │ selection +      │ verbosity` in  │ before commit.     │
│          │ components/    │ + selected  │ `apply_to_       │ app event loop │                    │
│          │ config_        │ value       │ runtime_config`  │                │                    │
│          │ overlay.rs`    │             │                  │                │                    │
├──────────┼────────────────┼─────────────┼──────────────────┼────────────────┼────────────────────┤
│ Tool     │ `crates/ploke- │ `ToolUiPay- │ Tool producers   │ Currently not  │ Stored per         │
│ payload  │ tui/src/tools/ │ load.       │ via              │ used by render │ payload, but       │
│ instance │ ui.rs`         │ verbosity`  │ `.with_          │ path (render   │ global verbosity   │
│ field    │                │             │ verbosity(...)`  │ uses global    │ presently wins at  │
│          │                │             │                  │ argument)      │ render time.       │
├──────────┼────────────────┼─────────────┼──────────────────┼────────────────┼────────────────────┤
│ Tool     │ `crates/ploke- │ `ToolUiPay- │ Individual tool  │ `ToolUiPayloa- │ Not the selected   │
│ payload  │ tui/src/tools/ │ load.       │ implementations  │ d::render`     │ level itself, but  │
│ content  │ *.rs`          │ fields`,    │                  │                │ defines what data  │
│ shape    │                │ `details`,  │                  │                │ each level can     │
│          │                │ `summary`   │                  │                │ expose.            │
└──────────┴────────────────┴─────────────┴──────────────────┴────────────────┴────────────────────┘
```

Mutation paths (high-level):

```text
┌────────────┬──────────────────────────────────┬──────────────────────────────────────────────────┐
│ Entry path │ Function chain                   │ State transitions                                │
╞════════════╪══════════════════════════════════╪══════════════════════════════════════════════════╡
│ Normal     │ `keymap ->                       │ `App.tool_verbosity` updated, then               │
│ mode       │ Action::ToggleToolVerbosity ->   │ `RuntimeConfig.tool_verbosity` updated           │
│ keybind    │ App::cycle_tool_verbosity ->     │ asynchronously, with optional SysInfo            │
│ `v`        │ App::apply_tool_verbosity`       │ confirmation.                                    │
├────────────┼──────────────────────────────────┼──────────────────────────────────────────────────┤
│ Command    │ `commands/parser ->              │ Same as above, but explicit value from command   │
│ `tool      │ commands/exec ->                 │ args.                                            │
│ verbosity  │ App::apply_tool_verbosity`       │                                                  │
│ <...>`     │                                  │                                                  │
├────────────┼──────────────────────────────────┼──────────────────────────────────────────────────┤
│ Config     │ `ConfigOverlayState::apply_to_   │ `RuntimeConfig.tool_verbosity` updated in config │
│ overlay    │ runtime_config` +                │ state; app copies selected value back into       │
│ change     │ `App::on_key_event` sync block   │ `App.tool_verbosity`.                            │
├────────────┼──────────────────────────────────┼──────────────────────────────────────────────────┤
│ Startup    │ `UserConfig -> RuntimeConfig     │ Persisted value becomes runtime default.         │
│ hydration  │ (From<UserConfig>) ->            │                                                  │
│            │ App::new(tool_verbosity)`        │                                                  │
└────────────┴──────────────────────────────────┴──────────────────────────────────────────────────┘
```

## Survey of SysInfo and System Message Kinds

Survey scope: all `MessageKind::SysInfo` and `MessageKind::System` emit sites under `crates/` (primarily `crates/ploke-tui/src`).

Quick volume snapshot (emit-site count, rough): `app/commands/exec.rs` (~41), `app/events.rs` (~13), `rag/editing.rs` (~11), `app/mod.rs` (~10), `rag/search.rs` (~8), `app_state/database.rs` (~8).

### System Message Kinds

`System` is low-frequency and currently used for special/internal paths:

```text
┌─────────────────────────────┬─────────────────────┬─────────────────────────┬────────────────────┐
│ Kind                        │ Primary source(s)   │ Current intent          │ Suggested          │
│                             │                     │                         │ `VerbosityLevel`   │
╞═════════════════════════════╪═════════════════════╪═════════════════════════╪════════════════════╡
│ Base system prompt message  │ `chat_history.rs`   │ Core LLM instruction    │ `Debug` (or hidden │
│ (`PROMPT_HEADER`)           │ (`BASE_SYSTEM_      │ context, not            │ by default in      │
│                             │ PROMPT`)            │ user-facing status      │ conversation UI)   │
├─────────────────────────────┼─────────────────────┼─────────────────────────┼────────────────────┤
│ LLM loop overflow/secondary │ `llm/manager/       │ Hard failure surfaced   │ `Error`            │
│ errors emitted into chat    │ session.rs`         │ when assistant          │                    │
│                             │ (`emit_loop_error`) │ placeholder is already  │                    │
│                             │                     │ used                    │                    │
├─────────────────────────────┼─────────────────────┼─────────────────────────┼────────────────────┤
│ Tool failed helper message  │ `llm/manager/       │ Failure fallback as     │ `Error`            │
│ (`Tool call failed: ...`)   │ session.rs`         │ `System` (helper        │                    │
│                             │ (`add_tool_failed_  │ currently appears       │                    │
│                             │ message`)           │ unused)                 │                    │
├─────────────────────────────┼─────────────────────┼─────────────────────────┼────────────────────┤
│ Fallback/system notes       │ `rag/context.rs`    │ Prompt construction     │ `Debug`            │
│ inserted into request       │ (`RequestMessage::  │ metadata for LLM, not   │                    │
│ context (not chat message)  │ new_system(...)`)   │ direct user feedback    │                    │
└─────────────────────────────┴─────────────────────┴─────────────────────────┴────────────────────┘
```

### SysInfo Message Kinds

`SysInfo` is the main operational/user-feedback channel, currently mixing user-facing updates with debug-heavy detail.

```text
┌─────────────┬─────────────────────────┬──────────────────────────────┬───────────────────────────┐
│ Category    │ Primary source(s)       │ Examples                     │ Suggested                 │
│             │                         │                              │ `VerbosityLevel`          │
╞═════════════╪═════════════════════════╪══════════════════════════════╪═══════════════════════════╡
│ UI/command  │ `app/mod.rs`,           │ "Tool verbosity set to ...", │ `Info`                    │
│ acknowledg- │ `app/commands/exec.rs`, │ "Context mode set to ...",   │                           │
│ ements      │ `app_state/dispatcher.  │ "Copied selection.", help    │                           │
│             │ rs`                     │ output, config save/load     │                           │
│             │                         │ confirmations                │                           │
├─────────────┼─────────────────────────┼──────────────────────────────┼───────────────────────────┤
│ Model/      │ `app/events.rs`,        │ Missing API key, no matching │ `Warn` (hard failures as  │
│ provider/   │ `app/commands/exec.rs`  │ models/endpoints, failed     │ `Error`)                  │
│ browser     │                         │ endpoint fetch               │                           │
│ interaction │                         │                              │                           │
│ warnings    │                         │                              │                           │
├─────────────┼─────────────────────────┼──────────────────────────────┼───────────────────────────┤
│ Indexing +  │ `app/events.rs`,        │ "Indexing...", "Indexing     │ Success paths `Info`;     │
│ DB          │ `app_state/handlers/    │ Succeeded", backup/load      │ degraded-but-recovered    │
│ lifecycle   │ indexing.rs`,           │ success, embedding restore   │ `Warn`; failures `Error`  │
│ status      │ `app_state/handlers/db. │ notices                      │                           │
│             │ rs`,                    │                              │                           │
│             │ `app_state/database.rs` │                              │                           │
├─────────────┼─────────────────────────┼──────────────────────────────┼───────────────────────────┤
│ RAG/search  │ `rag/search.rs`         │ BM25/dense/hybrid status,    │ `Debug` (summary-only     │
│ diagnostics │                         │ req_id-tagged result lists,  │ signal could be `Info`)   │
│ and result  │                         │ score dumps                  │                           │
│ dumps       │                         │                              │                           │
├─────────────┼─────────────────────────┼──────────────────────────────┼───────────────────────────┤
│ Tool        │ `rag/tools.rs`,         │ Staged edit/create previews, │ Stage/preview blobs       │
│ proposal    │ `tools/create_file.rs`, │ approve/deny instructions,   │ `Debug`; proposal state   │
│ workflow    │ `rag/editing.rs`        │ overlap/stale proposal       │ changes `Info`;           │
│ summaries   │                         │ notes, rescan summaries      │ overlap/retry guidance    │
│             │                         │                              │ `Warn`; apply failures    │
│             │                         │                              │ `Error`                   │
├─────────────┼─────────────────────────┼──────────────────────────────┼───────────────────────────┤
│ General     │ `app/events.rs`         │ `"Error: ..."` messages      │ `Error`                   │
│ error       │ (`AppEvent::Error`),    │ routed through `SysInfo`     │                           │
│ funnel      │ many command handlers   │ kind                         │                           │
│ currently   │                         │                              │                           │
│ emitted as  │                         │                              │                           │
│ SysInfo     │                         │                              │                           │
└─────────────┴─────────────────────────┴──────────────────────────────┴───────────────────────────┘
```

### Context Inclusion Note (Important for Future Filtering)

`SysInfo` context inclusion is currently mixed by insertion API, not by message intent:

```text
┌───────────────────────────────────────────┬──────────────────────────────────────────────────────┐
│ Path                                      │ Behavior                                             │
╞═══════════════════════════════════════════╪══════════════════════════════════════════════════════╡
│ `chat::add_msg_immediate(...)` with       │ Defaults to pinned context behavior (eligible for    │
│ `MessageKind::SysInfo`                    │ LLM context inclusion)                               │
├───────────────────────────────────────────┼──────────────────────────────────────────────────────┤
│ `chat::add_msg_immediate_sysinfo_         │ Explicitly UI-only, excluded from LLM context        │
│ unpinned(...)`                            │                                                      │
└───────────────────────────────────────────┴──────────────────────────────────────────────────────┘
```

Current consequence: several noisy diagnostic `SysInfo` messages can be pinned unless callers opt into unpinned APIs. Message verbosity filtering should ideally separate:
1. visibility policy in conversation UI, and
2. context-inclusion policy for LLM prompt construction.

### Initial Classification Heuristic (for implementation planning)

```text
┌────────────────────────────────────────────────────────────────────────────────┬─────────────────┐
│ Signal pattern                                                                 │ Default         │
│                                                                                │ classification  │
╞════════════════════════════════════════════════════════════════════════════════╪═════════════════╡
│ Message text starts with `Error:` / explicit failed operation / timeout /      │ `Error`         │
│ invalid path                                                                   │                 │
├────────────────────────────────────────────────────────────────────────────────┼─────────────────┤
│ Missing configuration/data but app continues (missing key, no model match, no  │ `Warn`          │
│ workspace selected, stale overlap)                                             │                 │
├────────────────────────────────────────────────────────────────────────────────┼─────────────────┤
│ Success/ack/state transition intended for normal user workflow                 │ `Info`          │
├────────────────────────────────────────────────────────────────────────────────┼─────────────────┤
│ Detailed dumps, previews, perf/score data, req_id-heavy diagnostics, long      │ `Debug`         │
│ help/debug payloads                                                            │                 │
└────────────────────────────────────────────────────────────────────────────────┴─────────────────┘
```

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

Implementation details (completed in this step):
- Added parser commands in `crates/ploke-tui/src/app/commands/parser.rs`:
  - `verbosity profile <minimal|normal|verbose|custom>` -> `Command::VerbosityProfileSet(...)`
  - `verbosity profile` -> `Command::VerbosityProfileShow`
- Added executor handling in `crates/ploke-tui/src/app/commands/exec.rs` to:
  - persist selected profile into `RuntimeConfig.default_verbosity`, and
  - emit SysInfo confirmation/status messages for set/show.
- Added `App::apply_message_verbosity_profile(...)` in `crates/ploke-tui/src/app/mod.rs` for command-driven profile updates.
- Added `MessageVerbosityProfile::as_str()` in `crates/ploke-tui/src/user_config.rs` for consistent command/status text.
- Updated command help/completion tables in `crates/ploke-tui/src/app/commands/mod.rs` with the new `verbosity profile` command.
- Added parser tests in `crates/ploke-tui/tests/command_verbosity_profile.rs` for slash-style set and NeoVim-style show parsing.

Implementation details (tests completed in this step):
- Added non-minimal approvals render coverage in `crates/ploke-tui/tests/approvals_overlay_render.rs`:
  - new `approvals_overlay_renders_codeblocks_preview_expanded_includes_unchanged_lines` test sets `DiffViewMode::Expanded` and asserts unchanged context lines are present.
- Updated `crates/ploke-tui/tests/post_apply_rescan.rs` for profile-aware rescan message behavior:
  - split into `approve_emits_rescan_sysinfo_under_default_profile` and `approve_emits_rescan_sysinfo_under_verbose_profile`.
  - both variants explicitly set config profile (`Minimal` / `Verbose`) and assert rescan SysInfo emission in chat storage.

## Adjust and Expand Message for Feedback Policy

Some of the changes have over-tuned for less user-facing feedback and should be
fixed. As a result, we will formalize a policy that all user commands have some
kind of response.

These responses do not all need to occur within the conversation messages, but
if they are not in conversation messages, then they must appear elsewhere in
the TUI. For example, while we do not need a follow-up feedback message
informing the user that the selected model has changed upon changing models
because there is already a UI component that displays the currently selected
model (appears in top-right of the UI), we do need a conversation message when
the embedding model is selected (since this information does not appear in the
UI).

The overall intent of this policy is to always present the user with some
response when they take an action.

### 1) Survey commands (completed)

Survey result summary:

- Feedback exists for almost all commands, but many acknowledgements were `Info`
  `SysInfo` and were hidden under default `Minimal` profile (`SysInfo=Warn`).
- Some commands intentionally provide non-conversation feedback through UI state
  changes (overlays, app quit, preview toggle, active model indicator).
- Two direct gaps were found:
  - `index pause|resume|cancel` had no explicit acknowledgement message.
  - legacy malformed `query load ...` without both args was silent.

### 2) Adjust default verbosity (completed)

Changed default minimal profile in `crates/ploke-tui/src/user_config.rs`:

- `MessageVerbosity::SysInfo.verbosity` changed from `Warn` -> `Info`.

This preserves the compact minimal view while ensuring normal command
acknowledgements are visible by default.

### 3) Create feedback content (completed)

Added/expanded immediate feedback in `crates/ploke-tui/src/app/commands/exec.rs`:

- Added immediate ack for `model load` (`Loading configuration...`).
- Added immediate ack for `model save` (`Saving configuration...`).
- Added immediate ack for `update` (`Scanning workspace for updates...`).
- Added immediate ack for successful `index start` dispatch
  (`Indexing requested for '<workspace>'`).
- Added explicit acks for `index pause|resume|cancel`.
- Added usage feedback for malformed `query load`:
  `Usage: query load <query_name> <file_name>`.

### 4) Non-I/O test (completed)

Added `crates/ploke-tui/tests/command_feedback_policy.rs`:

- `non_io_commands_emit_user_feedback_within_500ms`
- Uses `AppHarness` and synthetic key input (same UI path as user command entry).
- Asserts `SysInfo` feedback appears within 500ms for:
  - `verbosity profile verbose`
  - `provider tools-only on`
  - `index pause`
  - `index resume`
  - `index cancel`

### 5) I/O tests for file + network paths (completed)

Added in the same test file:

- `io_commands_emit_user_feedback_within_500ms`
- File I/O command coverage:
  - `model load /definitely/missing/path/...`
  - expects immediate load/start or failure feedback within 500ms.
- Network-capable command coverage:
  - `model providers invalid_model_id`
  - expects immediate validation/auth/fetch feedback within 500ms.

### 6) Implement missing feedback (completed)

Missing feedback identified by tests/survey is implemented (see step 3
changes). New tests pass.

### 7) Run tests (completed)

Executed:

- `cargo test -q -p ploke-tui --test command_feedback_policy`
- `cargo test -q -p ploke-tui`

Result: passing (warnings only, no test failures).

### 8) Re-evaluate tests + implementation (completed)

Test helper was adjusted to use slash command entry from insert mode (`/cmd`),
matching real command input semantics and avoiding normal-mode `/` prefill
(`"/hybrid "`).

No placeholder/flicker behavior was required for the covered commands because
immediate acknowledgement messages now appear.

### 9) Documentation matrix (completed)

```text
┌───────────┬────────────────┬─────────────────┬─────────────────┬──────────┬──────────────────────┐
│ Command   │ Feedback       │ Data            │ Feedback        │ Placeho- │ Validation           │
│           │                │ structures/     │ definition      │ lder     │                      │
│           │                │ state touched   │ location        │          │                      │
╞═══════════╪════════════════╪═════════════════╪═════════════════╪══════════╪══════════════════════╡
│ `index    │ `SysInfo` ack  │ `StateCommand:: │ `app/commands/  │ No       │ Covered indirectly   │
│ start     │ + indexing     │ IndexWorkspace  │ exec.rs`,       │          │ by existing indexing │
│ [directo- │ status         │ `, indexing     │ `app_state/     │          │ tests; explicit      │
│ ry]`      │ messages       │ state           │ handlers/       │          │ feedback policy      │
│           │                │                 │ indexing.rs`,   │          │ matrix only          │
│           │                │                 │ `app/events.rs` │          │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `index    │ `SysInfo:      │ `StateCommand:: │ `app/commands/  │ No       │ `command_feedback_   │
│ pause`    │ Indexing pause │ PauseIndexing`, │ exec.rs`        │          │ policy::non_io_      │
│           │ requested.`    │ indexing        │                 │          │ commands_emit_user_  │
│           │                │ control channel │                 │          │ feedback_within_     │
│           │                │                 │                 │          │ 500ms`               │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `index    │ `SysInfo:      │ `StateCommand:: │ `app/commands/  │ No       │ `command_feedback_   │
│ resume`   │ Indexing       │ ResumeIndexing  │ exec.rs`        │          │ policy::non_io_      │
│           │ resume         │ `, indexing     │                 │          │ commands_emit_user_  │
│           │ requested.`    │ control channel │                 │          │ feedback_within_     │
│           │                │                 │                 │          │ 500ms`               │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `index    │ `SysInfo:      │ `StateCommand:: │ `app/commands/  │ No       │ `command_feedback_   │
│ cancel`   │ Indexing       │ CancelIndexing  │ exec.rs`        │          │ policy::non_io_      │
│           │ cancel         │ `, indexing     │                 │          │ commands_emit_user_  │
│           │ requested.`    │ control channel │                 │          │ feedback_within_     │
│           │                │                 │                 │          │ 500ms`               │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `check    │ API key        │ none (display   │ `app/commands/  │ No       │ Covered by           │
│ api`      │ guidance       │ only)           │ exec.rs`        │          │ survey/manual path   │
│           │ `SysInfo`      │                 │                 │          │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `copy`    │ `SysInfo` copy │ selected        │ `app/mod.rs`    │ No       │ Existing copy tests  │
│           │ success/       │ message state,  │                 │          │                      │
│           │ failure        │ clipboard       │                 │          │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `model    │ `SysInfo`      │ runtime config  │ `app/commands/  │ No       │ Existing model       │
│ list`     │ active model + │ model registry  │ exec.rs`        │          │ command tests        │
│           │ provider pins  │                 │                 │          │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `model    │ `SysInfo`      │ runtime config  │ `app/commands/  │ No       │ Existing model       │
│ info`     │ model + params │ model/params    │ exec.rs`        │          │ command tests        │
│           │ + provider     │                 │                 │          │                      │
│           │ pins           │                 │                 │          │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `model    │ model switch   │ `StateCommand:: │ `app_state/     │ No       │ Existing model/event │
│ use       │ feedback       │ SwitchModel`,   │ models.rs`,     │          │ tests                │
│ <name>`   │ (`SysInfo`) +  │ `SystemEvent::  │ `app/events.rs` │          │                      │
│           │ active model   │ ModelSwitched`, │                 │          │                      │
│           │ indicator UI   │ app indicator   │                 │          │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `model    │ immediate      │ config/registry │ `app/commands/  │ No       │ Survey/manual path   │
│ refresh   │ refresh/reload │ reload path     │ exec.rs`        │          │                      │
│ [--       │ `SysInfo`      │                 │                 │          │                      │
│ local]`   │                │                 │                 │          │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `model    │ immediate      │ `UserConfig::   │ `app/commands/  │ No       │ `command_feedback_   │
│ load      │ `Loading       │ load_from_      │ exec.rs`        │          │ policy::io_commands_ │
│ [path]`   │ configuration  │ path`, runtime  │                 │          │ emit_user_feedback_  │
│           │ ...` +         │ config replace  │                 │          │ within_500ms`        │
│           │ success/       │                 │                 │          │                      │
│           │ failure        │                 │                 │          │                      │
│           │ `SysInfo`      │                 │                 │          │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `model    │ immediate      │ `RuntimeConfig  │ `app/commands/  │ No       │ Survey/manual path   │
│ save      │ `Saving        │ ::to_user_      │ exec.rs`        │          │                      │
│ [path]    │ configuration  │ config`, file   │                 │          │                      │
│ [--with-  │ ...` +         │ write           │                 │          │                      │
│ keys]`    │ success/       │                 │                 │          │                      │
│           │ failure        │                 │                 │          │                      │
│           │ `SysInfo`      │                 │                 │          │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `model    │ immediate      │ model browser   │ `app/commands/  │ No       │ Existing UI/browser  │
│ search    │ model browser  │ overlay state + │ exec.rs`,       │          │ tests                │
│ <keyword  │ overlay, async │ llm event bus   │ `app/events.rs` │          │                      │
│ >`        │ results        │                 │                 │          │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `embeddi- │ immediate      │ embedding       │ `app/commands/  │ No       │ Existing UI/browser  │
│ ng search │ embedding      │ browser overlay │ exec.rs`,       │          │ tests                │
│ <keyword  │ browser        │ + llm event bus │ `app/events.rs` │          │                      │
│ >`        │ overlay, async │                 │                 │          │                      │
│           │ results        │                 │                 │          │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `model    │ validation/    │ network request │ `app/commands/  │ No       │ `command_feedback_   │
│ providers │ auth/fetch     │ + provider      │ exec.rs`        │          │ policy::io_commands_ │
│ <model_   │ `SysInfo`      │ parsing         │                 │          │ emit_user_feedback_  │
│ id>`      │                │                 │                 │          │ within_500ms`        │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `provider │ `SysInfo`      │ runtime config  │ `app/commands/  │ No       │ Survey/manual path   │
│ strictne- │ confirmation   │ model registry  │ exec.rs`        │          │                      │
│ ss <...>` │                │ strictness      │                 │          │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `provider │ off>`          │ `SysInfo`       │ runtime config  │ `app/    │ No                   │
│ tools-    │                │ confirmation    │ gating flag     │ command- │                      │
│ only <on  │                │                 │                 │ s/exec.  │                      │
│           │                │                 │                 │ rs`      │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `provider │ `SysInfo`      │ registry        │ `app_state/     │ No       │ Existing provider    │
│ select    │ model/provider │ selected        │ dispatcher.rs`  │          │ tests + survey       │
│ ...` /    │ selection      │ endpoints +     │                 │          │                      │
│ `provider │ feedback       │ active model    │                 │          │                      │
│ pin ...`  │                │                 │                 │          │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `bm25     │ request/       │ RAG BM25        │ `app/commands/  │ No       │ Existing BM25 tests  │
│ rebuild`  │ result/failure │ service         │ exec.rs`,       │          │                      │
│           │ `SysInfo`      │                 │ `rag/search.rs` │          │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `bm25     │ status/failure │ RAG BM25        │ `rag/search.rs` │ No       │ Existing BM25 tests  │
│ status`   │ `SysInfo`      │ service         │                 │          │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `bm25     │ immediate ack  │ RAG BM25 save   │ `app/commands/  │ No       │ Existing BM25 tests  │
│ save      │ +              │ path            │ exec.rs`,       │          │                      │
│ <path>`   │ result/failure │                 │ `rag/search.rs` │          │                      │
│           │ `SysInfo`      │                 │                 │          │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `bm25     │ immediate ack  │ RAG BM25 load   │ `app/commands/  │ No       │ Existing BM25 tests  │
│ load      │ +              │ path            │ exec.rs`,       │          │                      │
│ <path>`   │ result/failure │                 │ `rag/search.rs` │          │                      │
│           │ `SysInfo`      │                 │                 │          │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `bm25     │ immediate ack  │ RAG search      │ `app/commands/  │ No       │ Existing BM25 tests  │
│ search    │ +              │                 │ exec.rs`,       │          │                      │
│ <query>   │ results/       │                 │ `rag/search.rs` │          │                      │
│ [top_k]`  │ failure        │                 │                 │          │                      │
│           │ `SysInfo`      │                 │                 │          │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `hybrid   │ immediate ack  │ RAG hybrid      │ `app/commands/  │ No       │ Existing hybrid      │
│ <query>   │ +              │ search          │ exec.rs`,       │          │ tests                │
│ [top_k]`  │ results/       │                 │ `rag/search.rs` │          │                      │
│           │ failure        │                 │                 │          │                      │
│           │ `SysInfo`      │                 │                 │          │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `preview  │ off            │ toggle]`        │ immediate UI    │ `App.    │ `app/commands/exec.  │
│ [on       │                │                 │ state change    │ show_    │ rs`                  │
│           │                │                 │ (preview pane)  │ context  │                      │
│           │                │                 │                 │ _previe- │                      │
│           │                │                 │                 │ w`       │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `edit     │ diff>`         │ `SysInfo`       │ runtime editing │ `app_    │ No                   │
│ preview   │                │ confirmation    │ config          │ state/   │                      │
│ mode      │                │                 │                 │ dispatc- │                      │
│ <code     │                │                 │                 │ her.rs`  │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `edit     │ `SysInfo`      │ runtime editing │ `app_state/     │ No       │ Existing             │
│ preview   │ confirmation   │ config          │ dispatcher.rs`  │          │ parser/dispatcher    │
│ lines     │                │                 │                 │          │ tests                │
│ <N>`      │                │                 │                 │          │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `edit     │ off>`          │ `SysInfo`       │ runtime editing │ `app_    │ No                   │
│ auto <on  │                │ confirmation    │ config          │ state/   │                      │
│           │                │                 │                 │ dispatc- │                      │
│           │                │                 │                 │ her.rs`  │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `edit     │ approval/      │ proposals map + │ `rag/editing.   │ No       │ `post_apply_rescan.  │
│ approve   │ rescan/failure │ apply/edit      │ rs`             │          │ rs` + editing tests  │
│ <request  │ `SysInfo`      │ pipeline        │                 │          │                      │
│ _id>`     │                │                 │                 │          │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `edit     │ deny           │ proposals map   │ `rag/editing.   │ No       │ editing tests        │
│ deny      │ confirmation/  │                 │ rs`             │          │                      │
│ <request  │ failure        │                 │                 │          │                      │
│ _id>`     │ `SysInfo`      │                 │                 │          │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `create   │ approval/      │ create          │ `rag/editing.   │ No       │ editing tests        │
│ approve   │ rescan/failure │ proposals map + │ rs`             │          │                      │
│ <request  │ `SysInfo`      │ apply pipeline  │                 │          │                      │
│ _id>`     │                │                 │                 │          │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `create   │ deny           │ create          │ `rag/editing.   │ No       │ editing tests        │
│ deny      │ confirmation/  │ proposals map   │ rs`             │          │                      │
│ <request  │ failure        │                 │                 │          │                      │
│ _id>`     │ `SysInfo`      │                 │                 │          │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `tool     │ toggle>`       │ `SysInfo`       │ app/runtime     │ `app/    │ No                   │
│ verbosity │                │ confirmation/   │ tool verbosity  │ mod.rs`, │                      │
│ <...      │                │ show            │ fields          │ `app/    │                      │
│           │                │                 │                 │ command- │                      │
│           │                │                 │                 │ s/exec.  │                      │
│           │                │                 │                 │ rs`      │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `verbosi- │ show>`         │ `SysInfo`       │ runtime default │ `app/    │ No                   │
│ ty        │                │ confirmation/   │ profile +       │ mod.rs`, │                      │
│ profile   │                │ show            │ render policy   │ `app/    │                      │
│ <...      │                │                 │                 │ command- │                      │
│           │                │                 │                 │ s/exec.  │                      │
│           │                │                 │                 │ rs`      │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `help     │ help text      │ none            │ `app/commands/  │ No       │ existing help render │
│ [topic]`  │ `SysInfo`      │                 │ exec.rs`        │          │ tests                │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `search   │ immediate      │ context browser │ `app/commands/  │ No       │ existing context     │
│ <query>`  │ context-       │ overlay +       │ exec.rs`,       │          │ browser tests        │
│           │ browser        │ search dispatch │ `app/events.rs` │          │                      │
│           │ overlay +      │                 │                 │          │                      │
│           │ async results  │                 │                 │          │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `update`  │ immediate scan │ `ScanForChange` │ `app/commands/  │ No       │ survey/manual path   │
│           │ ack + summary  │ + db update     │ exec.rs`        │          │                      │
│           │ `SysInfo`      │ dispatch        │                 │          │                      │
├───────────┼────────────────┼─────────────────┼─────────────────┼──────────┼──────────────────────┤
│ `quit`    │ app exit       │ app lifecycle   │ `app/commands/  │ No       │ `command_quit.rs`    │
│           │                │                 │ exec.rs`        │          │                      │
└───────────┴────────────────┴─────────────────┴─────────────────┴──────────┴──────────────────────┘
```

### Commands not fully mocked/tested under step 5

```text
┌─────────────────────┬────────────────────────────────────┬───────────────────────────────────────┐
│ Command(s)          │ Why not fully covered by step-5    │ Needed setup                          │
│                     │ harness test                       │                                       │
╞═════════════════════╪════════════════════════════════════╪═══════════════════════════════════════╡
│ `model search`,     │ Existing policy test covers        │ Router/network mock for OpenRouter    │
│ `embedding search`  │ immediate UI feedback pattern but  │ model list endpoints to validate      │
│ (successful remote  │ not deterministic remote success   │ successful async result rendering     │
│ fetch path)         │ payloads                           │ deterministically                     │
├─────────────────────┼────────────────────────────────────┼───────────────────────────────────────┤
│ `model providers`   │ Policy test validates immediate    │ Endpoint mock returning `endpoints`   │
│ (successful remote  │ error/validation feedback; not     │ payload to assert formatted provider  │
│ fetch path)         │ deterministic success path under   │ listing                               │
│                     │ network variance                   │                                       │
├─────────────────────┼────────────────────────────────────┼───────────────────────────────────────┤
│ `index start` full  │ Existing indexing tests validate   │ Stable fixture workspace + explicit   │
│ run with            │ responsiveness and completion, but │ ack timing assertion in app-harness   │
│ parse/index work    │ not strict 500ms command-ack for   │ command-input test                    │
│                     │ all workspaces                     │                                       │
├─────────────────────┼────────────────────────────────────┼───────────────────────────────────────┤
│ `update` full scan  │ Policy coverage is immediate ack   │ File-system fixture or IO mock with   │
│ result timing under │ only; summary timing depends on    │ controllable scan delay and           │
│ heavy workspace     │ filesystem and scan workload       │ deterministic completion signal       │
└─────────────────────┴────────────────────────────────────┴───────────────────────────────────────┘
```

### 10) Add more setup (next)

Next setup items:

- Introduce OpenRouter HTTP mock fixtures for command-level network success tests.
- Add deterministic scan/index fixtures with controllable delay hooks for strict
  ack + completion timing assertions.

### 11) Final review

Final outcome:

- Default minimal conversation profile now preserves command feedback (`SysInfo`
  at `Info` threshold).
- Commands with previously missing explicit feedback now provide it.
- Added harness-level 500ms policy tests for non-I/O and I/O command paths.
- `cargo test -q -p ploke-tui` passes at the end of implementation.
