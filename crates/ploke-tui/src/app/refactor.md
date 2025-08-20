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
- [x] Phase 5: Iterator-based rendering
  - Replace Vec<RenderableMessage> with iterators; remove collect in App::run loop.
- [ ] Phase 6: Tests/benchmarks
  - Snapshot tests for rendering; criteria benches for measure/wrap.

Notes on current code (tech-debt to address)
- A nested display_file_info exists inside App::run and a duplicate in events.rs; unify in utils.rs.
- Avoid logging in tight render paths unless trace level is gated.
- Ensure measure and render use identical wrap width rules (currently subtracts 1 in some places).
- Reduce direct mutation of App fields from events/commands; funnel through components.

This plan aims to keep the learning curve low while moving toward clear, composable components and predictable data flow. Start small (Actions + ConversationView), validate with tests, then proceed to iterator-based rendering and command deblocking.


Phase 6: Tests and Benchmarks (detailed strategy)

Goals
- Confidence: prevent regressions in input mapping, scrolling, wrapping, selection sync, and command parsing.
- Speed: microbench the hot measurement path to detect performance cliffs.
- Stability: keep snapshot tests deterministic and low-flake.

Test layers (what to test)
- Unit tests (fast, pure)
  - input::keymap
    - KeyEvent -> Action mapping tables per Mode and CommandStyle.
    - Edge cases: pending sequences (e.g., gg), Ctrl modifiers, slash prefix switching to Command mode.
  - view logic (non-render)
    - ConversationView measurement math: item height accumulation, offset clamping, page up/down behavior.
    - InputView cursor/index arithmetic and scroll prev/next bounds.
  - commands::parser
    - Command strings -> Command enum, including malformed inputs.
- Component rendering tests (snapshot-like, deterministic)
  - ConversationView::prepare + ::render against a ratatui TestBackend buffer.
  - InputView::render with various content lengths, cursor positions, and widths.
  - Assertions: exact buffer text lines, no panics on narrow widths, selection is visible when free_scrolling=false.
- App-level behavior (targeted, not full event loop)
  - handle_action() effects:
    - For navigation Actions, assert StateCommand messages sent to the channel.
    - For free-scrolling Actions, assert ConversationView state flips and offset changes.
    - For Submit/ExecuteCommand, assert appropriate commands are emitted and buffers reset.
  - events::handle_event smoke tests: feed a couple of AppEvent variants and assert requested UI updates (e.g., needs_redraw, selection sync via ListNavigation).
- Property tests (optional but valuable)
  - Wrap invariants: for random widths and message lengths:
    - No panic in measure/render.
    - Rendered row count equals or exceeds sum(item_heights) within viewport clamp.
    - Selection line is visible when free_scrolling=false.
- Microbenchmarks
  - ConversationView::prepare for N messages, at various widths and message length distributions.
  - InputView wrapping/measurement functions (cursor-to-row, row-to-cursor transitions).

Suggested dependencies (dev-only)
- insta for golden text snapshots of buffers (kept small and human-readable).
- proptest for property tests (if desired).
- criterion for benches.
Add only the ones you use; keep minimal.

Folder layout
- src/app/... keep unit tests inline with #[cfg(test)] mod tests for tight cohesion where convenient.
- tests/ for integration-like tests that compose multiple modules without the terminal event loop.
- benches/ for criterion benches.

Concrete test plans and skeletons

1) input::keymap unit tests
- Location: src/app/input/keymap.rs (#[cfg(test)] mod tests)
- What:
  - Matrix of (Mode, CommandStyle, KeyEvent) -> Action?
  - Verify None for unmapped keys; verify multi-key sequences (g then g) toggles JumpTop/Bottom logic at Action layer.

Example skeleton:
#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn insert_char_maps_to_action() {
        let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        let act = to_action(Mode::Insert, key, CommandStyle::NeoVim);
        assert!(matches!(act, Some(Action::InsertChar('x'))));
    }

    #[test]
    fn slash_prefix_enters_command_mode() {
        let key = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
        let act = to_action(Mode::Insert, key, CommandStyle::Slash);
        // Still InsertChar('/') here; the switch to Mode::Command happens in handle_action
        assert!(matches!(act, Some(Action::InsertChar('/'))));
    }
}

2) ConversationView logic + render tests
- Location:
  - Unit tests in src/app/view/components/conversation.rs (#[cfg(test)] mod tests)
  - Optional snapshot tests in tests/conversation_render.rs if using insta.
- Strategy:
  - Use a tiny test message type that implements RenderMsg, or reuse a minimal real RenderMsg where available.
  - Prepare with a fixed width and messages, assert:
    - item_heights sum, offset clamping after page_up/page_down, bottom/top requests respected.
  - Render with ratatui::backend::TestBackend and verify buffer text exactly (no ANSI).

Example render skeleton:
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};
    use ratatui::layout::Rect;

    struct TestMsg { id: uuid::Uuid, text: &'static str }
    impl RenderMsg for TestMsg {
        fn id(&self) -> uuid::Uuid { self.id }
        fn user_prefix(&self) -> &str { "U" }
        fn content_str(&self) -> &str { self.text }
        // Add any minimal required methods used by ConversationView
    }

    #[test]
    fn renders_two_short_messages() -> color_eyre::Result<()> {
        let backend = TestBackend::new(30, 6);
        let mut term = Terminal::new(backend)?;
        let mut conv = ConversationView::default();
        let msgs = vec![
            TestMsg { id: uuid::Uuid::nil(), text: "hello" },
            TestMsg { id: uuid::Uuid::nil(), text: "world" },
        ];

        term.draw(|f| {
            let area = Rect::new(0,0,30,6);
            conv.prepare(&msgs, msgs.len(), 24, area.height, Some(1));
            conv.set_last_chat_area(area);
            conv.render(f, &msgs, 24, area, Some(1));
        })?;

        let buffer = term.backend().buffer();
        let text: Vec<String> = (0..buffer.area.height)
            .map(|row| {
                let mut s = String::new();
                for col in 0..buffer.area.width {
                    s.push(buffer.get(col, row).symbol.chars().next().unwrap_or(' '));
                }
                s
            })
            .collect();

        assert!(text.join("\n").contains("hello"));
        assert!(text.join("\n").contains("world"));
        Ok(())
    }
}

3) InputView tests
- Location: src/app/view/components/input_box.rs (#[cfg(test)] mod tests)
- What:
  - scroll_prev/scroll_next do not panic and clamp properly.
  - Render shows cursor line and respects width.
  - Backspace and multi-line input interaction (handled via App today) can be simulated by passing strings in.

4) App action routing tests (no terminal)
- Location: tests/app_actions.rs
- Strategy:
  - Construct a minimal App with:
    - Arc<AppState>::default() (or a focused helper from ploke-test-utils if available),
    - tokio::mpsc::channel<StateCommand>(N) to capture outgoing commands,
    - a local EventBus stub.
  - Call handle_action on navigation actions and assert commands received in the channel.
  - Verify UI-local changes: ConversationView free_scrolling toggled for scroll actions; mode switching; input_buffer changes.

Example harness snippet:
#[tokio::test]
async fn navigate_list_emits_state_command() {
    use tokio::sync::mpsc;
    let (tx, mut rx) = mpsc::channel(16);

    // Build a minimal EventBus and AppState
    let state = Arc::new(AppState::new_for_tests()); // if not available, provide a helper or simple constructor
    let event_bus = EventBus::new(); // or a small wrapper exposing subscribe()

    let mut app = App::new(CommandStyle::NeoVim, state, tx, &event_bus, "test/model".into());

    app.handle_action(Action::NavigateListDown);
    let cmd = rx.recv().await.expect("expected a command");
    assert!(matches!(cmd, StateCommand::NavigateList { direction: _ }));
}

5) Command parser/exec tests
- Location:
  - src/app/commands/parser.rs (#[cfg(test)] mod tests) for pure parsing.
  - tests/commands_exec.rs for exec -> StateCommand emission (using the same channel harness as above).
- What to cover:
  - :help, /help, /model list, /model set X, :hybrid with flags (if any), malformed variants return errors or help.
  - exec sends correct StateCommand(s) and adds SysInfo messages where expected.

6) Property tests (optional)
- If adopted, add dev-dependency: proptest
- Example properties:
  - For any width in 5..=120 and list of ascii lines in lengths 0..=500:
    - prepare + render never panic,
    - offset is within [0, total_height - viewport] after page_down/up sequences,
    - when free_scrolling=false and selected index S is provided, S is fully visible (or bottom-aligned) post-prepare.

7) Benchmarks
- Add benches/ui_measure.rs (criterion)
- Scenarios:
  - N messages in [10, 100, 1000], width in [40, 80, 120], average line len in [20, 120, 300].
  - Measure ConversationView::prepare time and allocations if using an allocator counter (optional).
- Gate: benches only run locally/CI nightly to keep PR checks fast.

Practical notes
- Determinism:
  - Fix width/height in tests and avoid using terminal size queries.
  - Use only ASCII in golden buffers to simplify diffing.
  - Ensure both measure and render use identical wrap widths to keep snapshots stable (see Notes in plan).
- No-std terminal and CI:
  - ratatui TestBackend does not need an actual terminal; tests run in CI.
- Keep snapshots small:
  - Prefer short, focused buffers over whole-screen dumps; assert key lines and ordering instead of entire frames when possible.

Minimal changes to code to enable testing (brief)
- Expose a tiny App constructor for tests or provide a helper in tests/ that builds App with a test EventBus and channels.
- Ensure RenderMsg is implementable in tests (keep it small and not sealed); if sealed, add a test-only adapter.
- Keep ConversationView/InputView state setters/getters used in tests pub(crate).

Milestones
- M1: Unit tests for keymap and parser; smoke tests for ConversationView::prepare (no render).
- M2: Component render snapshot tests for ConversationView and InputView.
- M3: Action routing tests for App.handle_action.
- M4: Optional proptests + criterion benches.
