# ploke-tui app module refactor plan

Goal
- Make the app module modular and testable, with clear boundaries between: input handling, command parsing/execution, rendering, and app-state interactions.
- Reduce UI jank by avoiding unnecessary clones/collects and isolating blocking work.
- Ease future feature additions (e.g., new panes, popups, search UI) via reusable components.

High-level options

1) Componentized View + Action-based Input (recommended)
- Introduce UI components with a small trait that separates measurement and rendering.
  - trait UiComponent {
      fn measure(&mut self, constraints: Constraints, ctx: &ViewCtx) -> Size;
      fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &ViewCtx);
    }
  - Keep state local to each component (e.g., selection, scroll offset).
  - App composes components (ConversationView, InputView, StatusBar, PreviewPane).
- Input is mapped to high-level Actions in input::keymap; App routes Actions to components or dispatches StateCommands.
- Rendering and measurement are pure w.r.t the shared AppState: they read snapshots via ViewCtx.

Pros:
- Clear separation of concerns, easy unit testing for each component.
- Removes big methods from App and scales to more panes.
- Minimizes redraw logic in App.

Cons:
- Slightly more boilerplate to define component traits.

2) Elm-style Update loop (Msg + update + view)
- Model an Elm-ish architecture:
  - enum Msg { Ui(UiMsg), Event(AppEvent), Timer(...), ... }
  - fn update(model: &mut Model, msg: Msg) -> Cmd<Msg>
  - fn view(model: &Model) -> View
- The App becomes a thin runtime: read events -> to Msg -> update -> view(render).
- Commands/state changes are queued as typed commands that spawn async tasks.

Pros:
- Explicit unidirectional data flow, easy to reason about and test.
- Natural place to batch state changes and control side effects.

Cons:
- Bigger switch if you don’t already prefer Elm/MVU; may feel verbose in Rust.
- Might be overkill for a TUI unless standardized across the project.

3) Minimalist free functions + plain modules
- Move large methods off App into plain free functions grouped by module (view, input, commands).
- Keep App as a simple coordinator with minimal state.
- Prefer free functions over traits when there’s only a single implementation.

Pros:
- Lowest friction to adopt.
- Embraces dtolnay’s bias toward simple, concrete APIs.

Cons:
- Less reusable if you later add alternative renderers or layouts.
- Harder to stub/mutate when components grow.

Traits and generics

- View traits:
  - UiComponent (above) with a ViewCtx providing read-only snapshots (Arc<...> read locks resolved before render).
  - Sealed trait pattern if we don’t want external impls (pub(crate) trait, hidden Sealed).
- Input mapping:
  - keymap maps KeyEvent -> Action (enum). Components implement fn handle(&mut self, action: &Action, app: &mut App) if needed.
- Rendering helpers:
  - Generic measurement to accept iterators to avoid collect:
    - fn measure_messages<'a, I>(iter: I, width: u16) -> (u16, Vec<u16>)
      where I: IntoIterator<Item = &'a RenderableMessage>, 'a: 'a;
  - Prefer impl Iterator over Vec to reduce allocations.
- Data borrowing:
  - Prefer Cow<'a, str> or &str views in rendering paths to reduce clones.
- Typed events:
  - Keep AppEvent variants granular; add sub-mod traits if helpful (UiEvent, RagEvent, LlmEvent).
  - Consider a small EventSubscriber trait for components that react to app events:
    - trait EventSubscriber { fn on_event(&mut self, event: &AppEvent); }

Modular organization

- src/app/
  - mod.rs (small reexports + App runtime wiring)
  - app_core.rs (App struct, event loop, run, minimal state)
  - input/
    - mod.rs
    - keymap.rs (KeyEvent -> Action)
    - handler.rs (Action dispatcher; per-mode handlers)
  - commands/
    - mod.rs
    - parser.rs (string -> Command enum)
    - exec.rs (Command -> StateCommand; UI-local side effects)
  - view/
    - mod.rs (ViewCtx, utilities)
    - components/
      - conversation.rs (measure/render path & scrolling)
      - input_box.rs (input rendering + cursor/layout)
      - status.rs
      - preview.rs
  - events.rs (routing stays thin; mostly forwards to components/App)
  - types.rs (RenderableMessage, Mode, Action, Command)
  - utils.rs (truncate_uuid, formatters)

Concrete near-term steps

1) Extract types
- Move RenderableMessage, Mode, and utils like truncate_uuid into types.rs and utils.rs.
- Move text wrapping helpers from message_item.rs into view/components/conversation.rs.

2) Introduce Action enum and input::keymap
- Define Action covering current behaviors (navigation, scroll, mode switches, command prompt openers).
- Convert on_key_event to: KeyEvent -> Action via keymap; then route action to either component or App/commands.

3) Componentize the view
- Create ConversationView with:
  - Internal state: offset_y, item_heights, auto_follow, free_scrolling, last_viewport_height.
  - measure() reads message list iterator; render() draws with final offset.
- Create InputView with:
  - Internal state: vscroll, scrollstate, cursor col/row, trailing whitespace flag.
  - expose fn set_mode(Mode) and content ref.

4) Stop collecting messages for render
- Add render path that reads an iterator from state:
  - let history = state.chat.0.read().await; let msgs = history.iter_current_path();
  - View consumes iterator; use generics to avoid materialization.

5) Cull blocking calls from UI loop
- Replace block_in_place calls with async tasks that send StateCommand and push SysInfo messages on completion.
- Commands::list_models should schedule retrieval and then print; UI remains responsive.

6) Event routing
- events::handle_event forwards to components implementing EventSubscriber.
- App only updates global bits (model indicator), and requests a redraw.

7) Testing and benchmarking
- Snapshot rendering: Feed fixed ViewModel into components; use a “testing backend” for ratatui or text snapshot.
- Property tests for wrapping invariants (no panic on narrow widths; selected line is visible when auto-follow false).
- Microbench measurement functions (criterion) to catch regressions from wrap/measure logic.

8) UX polish
- Stabilize height calculations: single wrap strategy shared by measure/render.
- Cursor/clamp logic centralized in InputView.

Guidelines from dtolnay and burntsushi applied

- Prefer concrete types and free functions until polymorphism is necessary (dtolnay).
- Avoid unnecessary trait objects; use generics for zero-cost abstraction in hot paths (dtolnay).
- Keep public API small; use pub(crate) and sealed traits to constrain impl surface (dtolnay).
- Optimize last: first make the design simple and tests solid; measure with criterion before tuning (burntsushi).
- Minimal dependencies; isolate third-party crates behind module boundaries for easier swaps (burntsushi).
- Clear module boundaries and targeted tests per module (burntsushi).
- Error handling:
  - UI loop should not panic; use structured errors and add context where side effects occur.
- Data ownership:
  - Make cloning explicit and local; pass &str/&[T] where feasible.

Migration plan (incremental, low-risk)

- [x] Phase 1: Types + Input
  - Add types.rs, utils.rs, input/keymap.rs with Action.
  - Convert on_key_event to action routing.
- [x] Phase 2: View Components
  - Implement ConversationView and InputView; move height/scroll logic out of App.
  - App::draw becomes orchestration of components.
- [x] Phase 3: Commands split
  - commands/{parser,exec}.rs; wire help/list/update as examples; de-block list_models.
- [x] Phase 4: Events
  - EventSubscriber for components; keep events.rs as router.
- [/] Phase 5: Iterator-based rendering
  - Replace Vec<RenderableMessage> with iterators; remove collect in App::run loop.
- [ ] Phase 6: Tests/benchmarks
  - Snapshot tests for rendering; criteria benches for measure/wrap.

crates/ploke-tui/src/app/mod.rs
crates/ploke-tui/src/app/view/components/conversation.rs
crates/ploke-tui/src/app/message_item.rs

Notes on current code (tech-debt to address)
- A nested display_file_info exists inside App::run and a duplicate in events.rs; unify in utils.rs.
- Avoid logging in tight render paths unless trace level is gated.
- Ensure measure and render use identical wrap width rules (currently subtracts 1 in some places).
- Reduce direct mutation of App fields from events/commands; funnel through components.

This plan aims to keep the learning curve low while moving toward clear, composable components and predictable data flow. Start small (Actions + ConversationView), validate with tests, then proceed to iterator-based rendering and command deblocking.
