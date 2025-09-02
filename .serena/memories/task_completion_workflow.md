# Task Completion Workflow

## When Completing Tasks

### Required Steps
1. **Run tests** - Always run relevant tests for your changes
2. **Check formatting** - Run `cargo fmt` 
3. **Run linting** - Run `cargo clippy`
4. **Custom linting** - Run `./scripts/no_gratuitous_collect.sh`
5. **Build verification** - Ensure `cargo build` succeeds

### Testing Strategy
```bash
# For specific crate changes:
cargo test -p crate-name

# For broader changes:
cargo test

# With live API tests (when applicable):
cargo test --features "live_api_tests"

# Always with nocapture for debugging:
cargo test -- --nocapture
```

### Evidence-Based Changes
- **Run targeted and full test suites**
- **Update design/reflection docs for trade-offs**
- **Document performance impacts when relevant**
- **Follow live gates discipline**: When live gates are ON, tests must exercise the live path

### Safety-First Editing
- **Stage edits with verified file hashes**
- **Apply atomically via IoManager**
- **Use git for versioning and rollback capability**
- **Preview diffs before application**

### Documentation Updates
- Update relevant docs in `crates/ploke-tui/docs/` when making architectural changes
- Keep implementation logs in `crates/ploke-tui/docs/agent-system/impl-log/`
- Update decision documents in `crates/ploke-tui/docs/decisions_required.md` if needed

### Performance Considerations
- Benchmark before/after for performance-sensitive changes
- Ensure optimizations don't break async boundaries or event semantics
- Make incremental changes to isolate impact
- Verify optimizations work across the entire pipeline