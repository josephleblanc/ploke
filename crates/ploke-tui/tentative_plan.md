1. **Ratatui Fundamentals**:
- Immediate Mode Rendering: Every frame completely redraws the UI based on current state
- Widget-based Architecture: Build UI from composable components (blocks, paragraphs, lists)
- Terminal Backend Abstraction: Works with multiple backends (Crossterm in your case)

2. **Core Architectural Patterns**:
- **State Management**: Central `AppState` struct holding:
  ```rust
  struct AppState {
      chat_history: Vec<Message>, // LLM interaction history
      input_buffer: String,        // User's current input
      rag_context: RagContext,     // From your backend
      message_queue: flume::Receiver<BackendMessage>,
      // ... other state fields
  }
  ```
- **Event Loop Pattern**:
  ```rust
  async fn main_loop() {
      loop {
          // 1. Handle input
          // 2. Process backend messages
          // 3. Render UI
          // 4. Maintain 60 FPS cap
      }
  }
  ```

3. **Chat Interface Components**:
- Message List (Scrollable history)
- Input Field (With multiline support)
- Status Bar (Connection/Model status)
- Context Panel (RAG insights)

4. **Async Integration Strategy**:
- Dedicated Tokio tasks for:
  - LLM API communication
  - RAG graph queries
  - File system watchers
- Channel-based communication:
  ```rust
  #[derive(Clone)]
  struct AppChannels {
      ui_to_backend: flume::Sender<BackendRequest>,
      backend_to_ui: flume::Sender<UiUpdate>,
  }
  ```

5. **Key Ratatui Widgets to Use**:
- `Paragraph` for text input/display
- `List` for chat history
- `Block` for panel borders
- `Scrollbar` for history navigation
- `Tabs` for context switching

6. **Recommended Project Structure**:
```bash
src/
├── app.rs        # AppState and core logic
├── backend.rs    # RAG/LLM integration
├── ui/
│   ├── mod.rs    # Main layout composition
│   ├── chat.rs   # Chat interface components
│   └── context.rs# RAG context display
├── events.rs     # Input handling
├── config.rs     # TOML config parsing
└── lib.rs        # Public API
```

7. **Critical Implementation Details**:

a) **Non-blocking UI**:
```rust
// In main loop:
select! {
    _ = event_handler.next() => { /* Handle input */ },
    msg = backend_rx.recv_async() => { /* Update state */ },
    _ = sleep(Duration::from_millis(16)) => { /* Frame rate limit */ }
}
```

b) **Message Rendering**:
```rust
fn render_messages(messages: &[Message]) -> Vec<Line<'static>> {
    messages.iter().map(|msg| {
        Line::from(vec![
            Span::styled("> ", Style::new().fg(Color::Green)),
            Span::raw(msg.content)
        ])
    }).collect()
}
```

c) **RAG Context Integration**:
- Store current relevant code graph nodes in AppState
- Show context-aware suggestions as ghost text
- Highlight RAG-derived content differently

8. **First Steps Implementation Plan**:

1. Set up basic terminal initialization/cleanup
2. Implement frame rate limiter
3. Create core input handling system
4. Build basic chat message rendering
5. Connect to backend message channels
6. Add RAG context display panel
7. Implement scrolling history
8. Add configurable keybindings

Would you like me to dive deeper into any particular aspect of this architecture or demonstrate a specific component implementation?
