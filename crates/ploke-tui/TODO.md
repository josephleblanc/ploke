 # Ploke TUI Development Roadmap

 ## Immediate Priorities
 - [x] Add quit confirmation dialog (handle_key_event)
 - [ ] Implement configurable history limits (App::new)
 - [ ] Replace "LLM:" prefix with actual model name (AppEvent::BackendResponse)
 - [ ] Add scrollable message history
 - [ ] Implement real LLM API integration (replace backend.rs simulation)

 ## Core Features
 - [ ] Implement command system (`/help`, `/save`, `/config`)
 - [ ] Add actual LLM API integration (backend.rs)
 - [ ] Input validation/sanitization

 ## UI Enhancements
 - [ ] Message timestamps
 - [ ] Syntax highlighting for code blocks
 - [ ] Status bar with mode/connection info
 - [ ] Interactive elements (button-like controls)

 ## Architectural Improvements
 - [ ] Configuration file support
 - [ ] Message persistence (file/database)
 - [ ] Proper error handling for channel comms
 - [ ] Unit test framework implementation

 ## Future Considerations
 - [ ] Plugin system for LLM providers
 - [ ] Multi-chat tab support
 - [ ] Search message history
 - [ ] User preferences system
