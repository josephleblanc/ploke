# Test Infrastructure Upgrade for Command Decision Tree Tests

**Date:** 2026-04-01  
**Status:** Phase 1 Complete - Infrastructure Ready

## Overview

We've enhanced the decision tree test framework to support the new command grouping architecture and `UiError` pattern. The infrastructure now captures error events from the event bus and allows tests to assert on user-facing error messages and recovery suggestions.

## What Was Built

### 1. Enhanced Test Case Structure

Added new fields to `TestCase`:

```rust
pub enum ValidationExpectation {
    None,           // No validation required
    Success,        // Validation should succeed
    Failure { reason: Option<String> }, // Validation should fail
}

pub struct ExpectedUiError {
    pub message_contains: Option<String>,
    pub recovery_suggestion: Option<String>,
}

struct TestCase {
    // ... existing fields ...
    expected_validation: ValidationExpectation,
    expected_error: ExpectedUiError,
}
```

### 2. Builder Pattern

Created `TestCase::new()` builder to simplify test case construction and provide sensible defaults:

```rust
TestCase::new(
    "4.1 /index re-indexes loaded crate",
    DbSetup::StandaloneCrate,
    TestPwd::Crate("tests/fixture_crates/fixture_nodes"),
    "/index",
    "TestTodo",
    None,
    Some("test_standalone_index_reindexes"),
)
```

### 3. Enhanced Validation Probe with Error Capture

Extended `ValidationRelayStateCmd` in `harness.rs` to:
- Subscribe to the event bus for error events
- Capture `AppEvent::Error` emissions after command execution
- Extract error messages from `ErrorEvent`
- Include error information in `ValidationProbeEvent`

```rust
// In ValidationRelayStateCmd::run_relay():
let mut error_rx = event_bus.subscribe(EventPriority::Realtime);

// After forwarding command:
let (error_message, recovery_suggestion) =
    match timeout(Duration::from_millis(10), error_rx.recv()).await {
        Ok(Ok(AppEvent::Error(error_event))) => {
            (Some(error_event.message.clone()), None)
        }
        _ => (None, None),
    };
```

### 4. Error Assertion Logic

Updated test runner to assert on expected error messages and recovery suggestions:

```rust
if let Some(expected_msg) = &case.expected_error.message_contains {
    if let Some(actual_msg) = validation.error_message() {
        assert!(
            actual_msg.contains(expected_msg),
            "Error message should contain '{}', got '{}'",
            expected_msg, actual_msg
        );
    } else {
        panic!("Expected error message containing '{}', but no error was emitted", expected_msg);
    }
}
```

### 5. Flexible Validation Logic

Updated test runner to handle three validation scenarios:
- **No validation** (backward compatible with existing tests)
- **Success validation** (asserts validation passes)
- **Failure validation** (asserts validation fails with optional reason)

## Current State

✅ **All 7 decision tree tests pass** (3 ignored as expected)  
✅ **Backward compatible** - all existing test cases work without modification  
✅ **Error capture implemented** - validation probe captures `AppEvent::Error`  
✅ **Error assertions ready** - test runner can assert on error messages and recovery  
✅ **Builder pattern** reduces boilerplate for new test cases

## Next Steps (Phase 2)

1. **Implement UiError pattern** in command executor to emit structured errors with recovery suggestions
2. **Update test cases** to use `expected_validation` and `expected_error` fields according to the indexing policy
3. **Replace TestTodo** with proper UiError emissions for error cases
4. **Add new test cases** for previously untested error paths from the policy
5. **Enhance error extraction** to parse recovery suggestions from UiError when implemented

## Design Principles

- **TDD-friendly**: Tests define expected behavior before implementation
- **Policy-driven**: Test cases serve as executable specification of the indexing policy
- **Fast**: Tests remain fast by not executing expensive effects (10ms timeout for error capture)
- **Clear**: Error messages and recovery suggestions are explicit in test cases
- **Gradual migration**: Infrastructure supports both old and new patterns during transition