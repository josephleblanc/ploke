# Comprehensive UI Test Strategy for Ploke Proposal System

## Root Cause Analysis - UI Crash

**Issue**: Application crashes when pressing 'e' to open approvals overlay
**Root Cause**: Async lock deadlock in UI rendering context

### Technical Details:
1. `run_with()` method calls `draw()` with async guard held: `history_guard = app_state.chat.0.read().await`
2. `render_approvals_overlay()` calls `state.proposals.blocking_read()` during UI rendering
3. **Deadlock occurs**: Cannot acquire blocking read lock while async read lock is held in same executor context

### Fix Required:
The `render_approvals_overlay()` function must be refactored to avoid blocking calls during rendering. The proposals data should be prepared before the render phase.

## UI Test Strategy Overview

### 1. Test Architecture

**Test Levels:**
- **Unit Tests**: Individual component rendering with mocked data
- **Integration Tests**: Full UI workflow with state management
- **Snapshot Tests**: Visual regression detection using `insta`
- **Crash Reproduction Tests**: Systematic error condition testing

**Test Infrastructure:**
```rust
// Base test setup using available dependencies
use tokio_test;
use test_context::test_context;
use insta::{assert_snapshot, with_settings};
use fake::{Fake, Faker};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
```

### 2. Snapshot Testing Strategy

**Using `insta` crate** (already in Cargo.toml):

```rust
#[test]
fn test_approvals_overlay_empty_state() {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    let state = create_test_app_state_empty();
    let ui_state = ApprovalsState::default();
    
    terminal.draw(|frame| {
        render_approvals_overlay_safe(frame, frame.area(), &state, &ui_state);
    }).unwrap();
    
    let buffer = terminal.backend().buffer();
    assert_snapshot!("approvals_overlay_empty", buffer.content);
}
```

**Snapshot Categories:**
- Empty state (no proposals)
- Single proposal (pending/approved/denied/failed states)
- Multiple proposals with mixed states
- Error states (malformed data, missing files)
- Edge cases (very long file names, large diffs)

### 3. Crash Reproduction Test Suite

**Priority 1 - Deadlock Issues:**
```rust
#[tokio::test]
async fn test_approvals_overlay_deadlock_reproduction() {
    // Reproduce the exact deadlock scenario
    let app_state = create_test_app_state().await;
    let mut app = create_test_app(app_state).await;
    
    // Simulate the problematic sequence:
    // 1. Acquire async read lock (as in draw() method)
    let _guard = app.state.chat.0.read().await;
    
    // 2. Try to render approvals (should not deadlock)
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    let result = terminal.draw(|frame| {
        if let Some(approvals) = &app.approvals {
            // This should NOT call blocking_read()
            render_approvals_overlay_safe(frame, frame.area(), &app.state, approvals);
        }
    });
    
    assert!(result.is_ok(), "UI rendering should not deadlock");
}
```

**Priority 2 - State Corruption:**
```rust
#[tokio::test] 
async fn test_concurrent_proposal_access() {
    let state = create_test_app_state().await;
    
    // Simulate concurrent read/write to proposals
    let read_handle = tokio::spawn({
        let state = state.clone();
        async move {
            for _ in 0..100 {
                let _guard = state.proposals.read().await;
                tokio::task::yield_now().await;
            }
        }
    });
    
    let write_handle = tokio::spawn({
        let state = state.clone();
        async move {
            for i in 0..10 {
                let mut guard = state.proposals.write().await;
                let proposal = create_fake_proposal(i);
                guard.insert(proposal.request_id, proposal);
                tokio::task::yield_now().await;
            }
        }
    });
    
    let result = tokio::try_join!(read_handle, write_handle);
    assert!(result.is_ok(), "Concurrent access should not cause data races");
}
```

### 4. Component Test Coverage Matrix

| Component | Unit Tests | Integration Tests | Snapshot Tests | Error Tests |
|-----------|------------|-------------------|----------------|-------------|
| **ApprovalsState** | ✅ | ✅ | ✅ | ✅ |
| **render_approvals_overlay** | ❌ (Broken) | ❌ | ❌ | ❌ |
| **handle_overlay_key** | ✅ | ✅ | ✅ | ✅ |
| **OpenApprovals action** | ✅ | ❌ | ❌ | ❌ |
| **State synchronization** | ❌ | ❌ | N/A | ❌ |

### 5. Test Implementation Plan

**Phase 1: Fix Critical Issues**
```rust
// Fixed version of render_approvals_overlay
pub fn render_approvals_overlay_safe(
    frame: &mut Frame,
    area: Rect, 
    proposals_data: &[(Uuid, EditProposal)], // Pre-fetched data
    ui: &ApprovalsState,
) -> Option<uuid::Uuid> {
    // No blocking calls during render
    // Use pre-fetched data instead of state.proposals.blocking_read()
}
```

**Phase 2: Comprehensive Test Suite**
```rust
#[cfg(test)]
mod ui_tests {
    use super::*;
    use test_context::test_context;
    
    #[test_context(TestAppState)]
    #[tokio::test]
    async fn test_approvals_workflow_end_to_end(ctx: &TestAppState) {
        // 1. Create proposal
        let proposal = create_test_proposal();
        ctx.state.proposals.write().await.insert(proposal.request_id, proposal);
        
        // 2. Open approvals overlay
        let mut app = create_test_app(ctx.state.clone()).await;
        app.handle_action(Action::OpenApprovals);
        assert!(app.approvals.is_some());
        
        // 3. Test keyboard navigation
        app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        
        // 4. Verify proposal was approved
        tokio::time::sleep(Duration::from_millis(10)).await; // Allow async processing
        let proposals = ctx.state.proposals.read().await;
        let proposal = proposals.values().next().unwrap();
        assert_eq!(proposal.status, EditProposalStatus::Approved);
    }
    
    #[test]
    fn test_approvals_ui_visual_regression() {
        let proposals = vec![
            create_fake_proposal_pending(),
            create_fake_proposal_applied(), 
            create_fake_proposal_failed("IO Error: Permission denied"),
        ];
        
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        terminal.draw(|frame| {
            render_approvals_overlay_safe(frame, frame.area(), &proposals, &ApprovalsState::default());
        }).unwrap();
        
        assert_snapshot!("approvals_mixed_states", terminal.backend().buffer());
    }
}
```

**Phase 3: Performance & Load Testing**
```rust
#[tokio::test]
async fn test_large_proposal_list_performance() {
    let proposals: Vec<_> = (0..1000).map(|i| create_fake_proposal(i)).collect();
    
    let start = std::time::Instant::now();
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal.draw(|frame| {
        render_approvals_overlay_safe(frame, frame.area(), &proposals, &ApprovalsState::default());
    }).unwrap();
    let duration = start.elapsed();
    
    assert!(duration < Duration::from_millis(16), "UI should render within 16ms for 60fps");
}
```

### 6. Test Data Generation

**Using `fake` crate** (already available):
```rust
use fake::{Dummy, Fake, Faker};
use uuid::Uuid;

#[derive(Dummy)]
struct TestProposal {
    #[dummy(faker = "Uuid::new_v4()")]
    request_id: Uuid,
    
    #[dummy(faker = "Faker")]
    files: Vec<PathBuf>,
    
    #[dummy(faker = "Faker")] 
    status: EditProposalStatus,
    
    #[dummy(faker = "generate_fake_diff()")]
    preview: DiffPreview,
}

fn generate_fake_diff() -> DiffPreview {
    let diff_text = format!(
        "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,3 +1,4 @@\n fn main() {{\n+    println!(\"Hello!\");\n     // existing code\n }}",
    );
    DiffPreview::UnifiedDiff { text: diff_text }
}
```

### 7. CI/CD Integration

**Snapshot Test Management:**
```bash
# Update snapshots when UI changes are intentional
cargo insta review

# Run snapshot tests in CI
cargo test --test ui_snapshots

# Check for snapshot drift
cargo insta test --check
```

**Test Categories for CI:**
- **Fast Tests** (< 1s): Unit tests, snapshot tests
- **Integration Tests** (< 10s): End-to-end workflows
- **Performance Tests** (< 30s): Load and stress testing

### 8. Error Handling Test Matrix

| Error Condition | Test Method | Expected Behavior |
|-----------------|-------------|------------------|
| **Deadlock in render** | Reproduction test | Graceful fallback, no crash |
| **Empty proposals list** | Snapshot test | Show "No proposals" message |
| **Malformed proposal data** | Unit test | Skip invalid entries, log error |
| **Network timeout** | Mock test | Show loading state, timeout message |
| **Concurrent state changes** | Race condition test | Data consistency maintained |
| **Memory exhaustion** | Load test | Graceful degradation, pagination |

### 9. Test Execution Strategy

**Development Workflow:**
1. **Pre-commit**: Fast unit tests + snapshot checks
2. **PR Validation**: Full test suite + performance benchmarks  
3. **Release**: Comprehensive regression testing + manual validation

**Test File Organization:**
```
crates/ploke-tui/src/tests/
├── ui/
│   ├── components/
│   │   ├── approvals_tests.rs
│   │   ├── approvals_snapshots.rs
│   │   └── approvals_integration.rs
│   ├── workflows/
│   │   ├── approval_workflow_tests.rs
│   │   └── error_handling_tests.rs
│   └── performance/
│       └── ui_performance_tests.rs
├── fixtures/
│   ├── test_proposals.rs
│   └── mock_app_state.rs
└── snapshots/
    └── ui__approvals__*.snap
```

This comprehensive test strategy addresses the critical deadlock issue while establishing robust testing infrastructure for ongoing development and preventing future regressions.