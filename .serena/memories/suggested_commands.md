# Essential Development Commands

## Build & Check
```bash
cargo build              # Build entire workspace
cargo build --release    # Build with optimizations (recommended)
cargo check              # Check without building
cargo clippy             # Lint with Clippy
cargo fmt                # Format code
./scripts/no_gratuitous_collect.sh  # Check for gratuitous collect patterns
```

## Testing
```bash
cargo test                        # Run all tests
cargo test -- --nocapture        # Run tests with output
cargo test -p ploke-tui          # Run tests for specific crate
cargo test --features "live_api_tests"  # Run with live API tests
cargo test -p ploke-io --features watcher  # Run with specific features
```

## Running the Application
```bash
cargo run                # Start TUI application
cargo run --release      # Start with optimizations

# In TUI (vim-like bindings):
# Press 'i' for insert mode
/index start /absolute/path/to/crate    # Index crate for RAG
/query                                  # Test Datalog queries
```

## Development Scripts
```bash
./scripts/gen_project_context.sh           # Generate project overview
MODE=full ./scripts/gen_project_context.sh # Full uncapped overview
./scripts/gen_project_context.sh my_context.txt  # Custom output file
```

## Environment Setup
```bash
export OPENROUTER_API_KEY="your_key_here"  # Required for API integration
# Or use existing .env file
```

## Common Test Fixtures
```bash
/index start /path/to/ploke/tests/fixture_crates/fixture_tracking_hash
/index start /path/to/ploke/tests/fixture_crates/fixture_nodes
```