<!-- Provided by Claude 3.5 -->

# TUI Component Design Overview

Jun 9 2025

This document outlines the design for the TUI component of the ploke project. The TUI provides a terminal-based interface for interacting with ploke's existing infrastructure.

1. SCOPE AND INTEGRATION

A. Role in ploke Architecture
- Terminal-based interface using `ratatui` for user interaction
- Coordinates with existing ploke components:
  - Uses `ploke-db` for RAG operations and code graph queries
  - Relies on `syn_parser` for code analysis
  - Integrates with `ploke-transform` for code modifications
  - Leverages `cozo` through existing infrastructure

B. Component Responsibilities
1. User Interface Rendering
2. Input Processing and Command Handling
3. Result Display and Formatting
4. Session Management
5. Configuration Interface

2. DETAILED COMPONENT DESIGN

A. TUI Component
- Main chat interface similar to aider
- Multiple modes (chat, code, review)
- Status bar showing current mode, git status, etc
- Split panes for:
  - Chat history
  - Code context
  - Proposed changes
  - Metadata (similarity scores, validation results)

B. Intent Processing
- Small LLM classifies user intent and expands query:
  - Code generation
  - Code modification
  - Question/explanation
  - Refactoring
- Query expansion to enhance context retrieval
- Combined intent + expanded query feeds into query generation
- Note: Query expansion strategy requires testing to determine optimal approach

C. Query Generation
- Convert intent into Datalog queries
- Multiple query strategies based on intent
- Query templating system for common patterns
- Query optimization layer

D. RAG Pipeline
- Vector embedding storage
- Multiple retrieval strategies
- Reranking system
- Context window optimization
- Chunking strategies

3. EXTENSION POINTS & FUTURE FEATURES

A. Static Analysis Integration
- Future development: rust-analyzer integration (not part of MVP)
- Integration with existing code graph
- Custom lints for AI-generated code
- Real-time validation
- Consider integration with user's preferred editor (nvim, VSCode, etc) for code preview
  - Trade-off: External editor provides familiar environment but fragments user experience
  - Recommendation: Start with built-in preview in TUI, add editor integration as optional feature

B. Version Control
- Git integration from day one
- Atomic commits for AI changes
- Branch management for experimental changes
- Change history separate from git
- Potential integration with cozo's timetravel feature
  - Could complement git by tracking fine-grained LLM changes
  - Consider using timetravel for undo/redo stack
  - Git remains source of truth for committed changes

C. Multi-Agent Review
- Pluggable agent system
- Role-based agents with user-defined prompts in config file
  - Default roles: architect, security, performance
  - Custom role definition format in config
- Agent communication protocol
- Consensus mechanisms

4. MVP ROADMAP

Development will follow an iterative framework-first approach:
1. Initial LLM interaction framework
2. RAG integration on tested framework

Phase 1: Core Chat Framework
- Basic TUI chat interface
- LLM communication pipeline
- Basic code editing
- Placeholder context management
- Error handling foundation

Phase 2: RAG Integration
- Simple RAG pipeline
- Query generation
- Intent processing
- Basic validation
- Schema versioning placeholder

Phase 3: Advanced Features
- Multi-agent review
- Enhanced static analysis
- UI refinements
- Persistent storage integration

5. IMPLEMENTATION APPROACH

A. Crate Structure
```
ploke-tui/           # New TUI crate
├── src/
│   ├── ui/         # TUI components
│   ├── intent/     # Intent processing
│   ├── query/      # Query generation
│   ├── rag/        # RAG pipeline
│   ├── agents/     # Multi-agent system
│   └── analysis/   # Code analysis
```

B. Key Traits & Interfaces
```rust
trait IntentProcessor {
    fn process(&self, input: &str) -> Intent;
}

trait QueryGenerator {
    fn generate(&self, intent: Intent) -> Vec<DatalogQuery>;
}

trait RagPipeline {
    fn retrieve(&self, queries: &[DatalogQuery]) -> Context;
    fn rerank(&self, context: &Context) -> RankedContext;
}

trait CodeGenerator {
    fn generate(&self, context: RankedContext) -> CodeChanges;
}
```

6. QUALITY OF LIFE FEATURES

A. Change Management
- Undo/redo stack for all changes
- Change preview before application
- Partial accept/reject UI
- Change refinement interface
- Toggleable recent changes panel
  - Timeline view of modifications
  - Diff visualization
  - Change metadata (timestamp, intent, etc)

B. Context Visualization
- Embedding similarity visualization
- Code relationship graphs
- Change impact analysis
- Performance metrics

7. RECOMMENDED PATH FORWARD

1. Start with new `ploke-tui` crate
2. Implement basic TUI chat interface
3. Create simple RAG pipeline using existing code graph
4. Add basic code editing capabilities
5. Implement intent processing
6. Add query generation
7. Enhance with validation and analysis
8. Add multi-agent features

8. KEY CONSIDERATIONS

A. Performance
- Async processing for LLM calls
- Efficient context management
- Smart caching of embeddings
- Incremental updates

B. User Experience
- Fast response times
- Clear feedback
- Intuitive commands
- Progressive disclosure of features
- Transparency of LLM interaction:
  - Minimal default UI showing essential info
  - Toggleable detailed views for:
    - Intent classification results
    - Query expansion details
    - RAG pipeline metrics
    - Embedding similarities
    - Agent reasoning chains
  - Status indicators for ongoing processes
  - Subtle hints for improving LLM interaction

C. Extensibility
- Plugin system for new features
- Custom agent definitions
- Additional analysis tools
- Alternative LLM backends

This design provides a solid foundation while maintaining flexibility for future enhancements. The modular approach allows for incremental development and easy addition of new features.

Would you like me to elaborate on any particular aspect of this design?
