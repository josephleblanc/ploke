# Comprehensive End-to-End Testing Plan for Tool Calls

## Overview

This document outlines a comprehensive implementation plan for testing the complete tool-calling pipeline in ploke-tui, based on the requirements in `tool-e2e-usr-rqst.md`. The goal is to validate every aspect of the tool system from API requests through local execution and back to API responses.

## Primary Goals

1. **Event Flow Validation**: Ensure all events are sent and received correctly throughout the pipeline
2. **API Shape Validation**: Verify request/response shapes match OpenRouter specification exactly
3. **Deserialization Validation**: Confirm all fields are correctly deserialized without type errors
4. **GAT System Validation**: Test the Generic Associated Types (GAT) deserialization system
5. **Tool Loop Validation**: End-to-end messaging loop verification
6. **Error Path Validation**: Comprehensive testing of both happy paths and failure scenarios
7. **Performance Benchmarking**: Rigorous performance profiling using criterion

## Architectural Quality Principles

### Code Quality and Maintainability
- **Trait-Based Tool System**: Maintain and validate the trait-based approach for tool definition and execution
- **Static Dispatch**: Ensure all tool dispatch uses static compilation over dynamic dispatch for performance
- **Zero-Copy Deserialization**: Leverage GAT (Generic Associated Types) for zero-copy deserialization wherever possible
- **Strong Typing**: All API interactions must use strongly typed data structures (no stringly-typed interfaces)

### API Type Safety
- **Outbound Type Safety**: All messages to OpenRouter API must be strongly typed with proper serialization validation
- **Inbound Type Safety**: All responses from OpenRouter API must be deserialized into strongly typed structures (building on existing work in `ploke-tui/src/llm/openrouter/`)
- **Schema Validation**: Comprehensive validation that our types match OpenRouter API specification exactly

### Development Quality Gates
- **Continuous Validation**: Run `cargo check` frequently during development to catch regressions early
- **Test Suite Integrity**: Periodically run existing test suite to prevent unexpected regressions
- **Regression Justification**: Any temporary regressions must be thoroughly documented with clear reasoning and mitigation plans
- **Type System Verification**: Ensure all new code maintains compile-time type safety guarantees

## Testing Architecture

### Phase 1: Foundation and Test Infrastructure

#### 1.1 Enhanced Test Harness
- **Location**: `crates/ploke-tui/tests/harness.rs`
- **Building On**: Existing infrastructure and maintaining architectural principles
- **Enhancements Needed**:
  - **Typed Event Capture**: Event validation system using strongly typed event structures
  - **API Request/Response Recording**: Leverage existing OpenRouter types in `src/llm/openrouter/`
  - **Trait-Based Tool Testing**: Integration with existing trait-based tool system
  - **Static Dispatch Validation**: Ensure test harness uses static dispatch throughout
  - **GAT-Based Deserialization Testing**: Zero-copy testing infrastructure
  - **Timing and Performance Metrics**: Collection with focus on static vs dynamic dispatch performance
  - **Resource Usage Monitoring**: Memory and CPU usage validation
  - **Parallel Test Execution**: Support with proper type safety isolation

#### 1.2 Test Utilities Rebuild
- **Location**: `crates/ploke-tui/tests/test_utils/`
- **Components**:
  - `event_tracker.rs` - Comprehensive event flow tracking
  - `api_validator.rs` - OpenRouter API specification validation
  - `serialization_tester.rs` - Type serialization/deserialization validation
  - `performance_collector.rs` - Performance metrics and benchmarking
  - `artifact_manager.rs` - Test artifact persistence and analysis

#### 1.3 Live API Test Framework
- **Features**:
  - Feature-gated live tests (`#[cfg(all(feature = "live_api_tests", feature = "test_harness"))]`)
  - Multiple provider/model matrix testing
  - Rate limiting and API key management
  - Snapshot testing for API responses
  - Response shape validation against OpenRouter spec
- **API Key Access**: Full permission to run live tests against OpenRouter endpoints
  - API key available via `.env` file in project root or `OPENROUTER_API_KEY` environment variable
  - Tests should fail early with clear error message if no API key is available
  - Dedicated testing budget allocated for comprehensive validation

### Phase 2: Event Flow Validation

#### 2.1 Event Bus Testing
- **File**: `tests/e2e_event_flow.rs`
- **Coverage**:
  - `AppEvent::System(SystemEvent::ToolCallRequested)` emission
  - `AppEvent::System(SystemEvent::ToolCallCompleted)` reception
  - `AppEvent::System(SystemEvent::ToolCallFailed)` error handling
  - `AppEvent::LlmTool(ToolEvent::*)` lifecycle events
  - Event correlation and timing validation
  - Event ordering and sequence verification

#### 2.2 Event Correlation Testing
- **Validation**:
  - Request ID propagation throughout the pipeline
  - Parent-child message relationships
  - Tool call ID consistency
  - Event sequence integrity

### Phase 3: API Shape and Deserialization Validation

#### 3.1 OpenRouter API Compliance Testing
- **File**: `tests/e2e_api_compliance.rs`
- **Based on**: `crates/ploke-tui/docs/openrouter/request_structure.md`
- **Test Coverage**:
  - Request format validation:
    - `messages` array structure
    - `tools` array with `ToolDefinition` schema
    - `tool_choice` parameter validation
    - Model-specific parameter validation
  - Response format validation:
    - `choices[0].message.tool_calls` structure
    - Tool call `id`, `type`, `function` fields
    - Error response structures
    - Streaming response formats

#### 3.2 Type System Validation
- **File**: `tests/e2e_type_validation.rs`
- **Coverage**:
  - `RequestMessage` serialization with all `Role` variants
  - `ToolDefinition` JSON schema generation
  - `ToolCall` deserialization from API responses
  - `Role::Tool` message construction and validation
  - Error type serialization/deserialization

#### 3.3 GAT System Testing
- **File**: `tests/e2e_gat_validation.rs`
- **Focus**:
  - **Trait-Based Tool Validation**: Test all tool implementations (`GetFileMetadata`, `RequestCodeContextGat`, `ApplyCodeEdit`) through trait interfaces
  - **Static Dispatch Verification**: Ensure tool dispatch uses static compilation (no `dyn Trait` objects)
  - **Zero-Copy Deserialization**: Validate GAT-based zero-copy parameter parsing
  - **Tool Parameter Deserialization**: Type-safe argument parsing with comprehensive error handling
  - **Tool Result Serialization**: Strongly typed result structures
  - **Compile-Time Type Safety**: Verification that invalid tool usage fails at compile time
  - **Runtime Type Validation**: Proper error handling for runtime type mismatches

#### 3.4 OpenRouter Type System Validation
- **File**: `tests/e2e_openrouter_types.rs`
- **Building On**: Existing work in `crates/ploke-tui/src/llm/openrouter/`
- **Focus**:
  - **Request Type Validation**: All outbound API calls use strongly typed structures
  - **Response Type Validation**: All API responses deserialized into typed structures
  - **Schema Compliance**: Verify our types exactly match OpenRouter specification
  - **Serialization Roundtrip**: Ensure no data loss in serialize/deserialize cycles
  - **Error Response Handling**: Typed error structures for all API error scenarios

### Phase 4: Complete Tool Loop Validation

#### 4.1 Single Tool Execution
- **File**: `tests/e2e_single_tool_execution.rs`
- **Test Cases**:
  - `get_file_metadata` with valid file paths
  - `request_code_context` with various search terms
  - `apply_code_edit` with valid edit operations
  - Tool parameter validation and sanitization
  - Tool result format verification

#### 4.2 Multi-Tool Conversations
- **File**: `tests/e2e_multi_tool_conversations.rs`
- **Scenarios**:
  - Sequential tool calls in single conversation
  - Tool results incorporated into follow-up requests
  - Tool call chains and dependencies
  - Conversation state management across tool calls

#### 4.3 RequestSession Integration
- **File**: `tests/e2e_request_session_integration.rs`
- **Focus**:
  - `RequestSession::run()` with tool-enabled models
  - Tool call dispatch and result awaiting
  - Multi-turn conversation handling
  - Session timeout and retry behavior
  - Tool result incorporation into message history

### Phase 5: Error Scenario Validation

#### 5.1 API Error Handling
- **File**: `tests/e2e_api_error_scenarios.rs`
- **Coverage**:
  - Invalid API keys and authentication failures
  - Rate limiting and quota exceeded responses
  - Model unavailability and fallback behavior
  - Malformed API responses
  - Network timeouts and connection failures

#### 5.2 Tool Execution Errors
- **File**: `tests/e2e_tool_error_scenarios.rs` (enhanced)
- **Scenarios**:
  - Invalid tool parameters and JSON parsing errors
  - File system errors (permissions, missing files)
  - Hash mismatches and concurrent modification
  - Tool timeout and resource exhaustion
  - Tool-specific error conditions

#### 5.3 Event System Error Handling
- **File**: `tests/e2e_event_error_scenarios.rs`
- **Coverage**:
  - Event bus disconnection and reconnection
  - Event ordering violations
  - Missing correlation IDs
  - Event channel overflow and backpressure

### Phase 6: Performance and Load Testing

#### 6.1 Performance Benchmarks
- **File**: `benches/tool_performance.rs`
- **Metrics**:
  - Tool execution latency (p50, p95, p99)
  - API request/response times
  - Event propagation latency
  - Memory usage during tool execution
  - Concurrent tool execution scaling

#### 6.2 Load Testing
- **File**: `tests/e2e_load_testing.rs`
- **Scenarios**:
  - High-frequency tool execution
  - Concurrent multi-user simulation
  - Resource exhaustion testing
  - Memory leak detection
  - Thread pool saturation

#### 6.3 Criterion Integration
- **Setup**:
  - Cargo.toml benchmark configuration
  - Baseline performance recording
  - Regression detection
  - Performance reporting and visualization

### Phase 7: Live API Matrix Testing

#### 7.1 Provider/Model Matrix
- **File**: `tests/live_api_matrix.rs`
- **Coverage**:
  - OpenAI models via OpenRouter
  - Anthropic models via OpenRouter
  - Other tool-capable models
  - Provider-specific behavior differences
  - Model capability detection and adaptation

#### 7.2 Tool Capability Testing
- **Validation**:
  - Model tool-calling support detection
  - Tool definition format compatibility
  - Tool response format consistency
  - Provider-specific tool limitations

### Phase 8: Test Documentation and Validation Report

#### 8.1 Test Coverage Documentation
- **File**: `docs/testing/e2e-coverage-report.md`
- **Content**:
  - Complete test matrix and coverage
  - Validated behaviors and properties
  - Known limitations and gaps
  - Untested execution paths and edge cases

#### 8.2 Performance Baseline Documentation
- **File**: `docs/testing/performance-baseline.md`
- **Content**:
  - Benchmark results and baselines
  - Performance regression thresholds
  - Bottleneck identification and optimization targets
  - Resource usage profiles

#### 8.3 API Compliance Report
- **File**: `docs/testing/api-compliance-report.md`
- **Content**:
  - OpenRouter specification adherence
  - Provider compatibility matrix
  - Type system validation results
  - Serialization/deserialization verification

## Current Implementation Status (September 2, 2025)

### Completed Tests
All 8 tests in `e2e_complete_tool_conversations.rs` are passing:
- `e2e_basic_message_addition` - Basic message addition to chat history
- `e2e_complete_get_metadata_conversation` - Complete conversation with metadata tool
- `e2e_tool_execution_event_flow` - Tool execution event flow validation
- `e2e_multi_step_tool_conversation` - Multi-step tool conversation (with tracing)
- `e2e_conversation_with_tool_errors` - Tool error handling
- `e2e_conversation_state_persistence` - State persistence across messages (with tracing)
- `e2e_tool_result_conversation_integration` - Tool result integration
- `e2e_conversation_context_for_tools` - Context building for tool calls

### Priority Implementation Tasks (Sept 2)

#### Enhanced Testing Requirements
The following tests need to be added to verify the complete tool-calling pipeline:

1. **Message Receipt and Deserialization Test**
   - Verify messages are correctly received from API
   - Validate deserialization into strongly-typed structures
   - Check all fields are properly parsed without type errors
   - Ensure no data loss in deserialization process

2. **Tool Call and Return Test**
   - Verify tools are called with correct parameters
   - Validate tool execution and result generation
   - Ensure tool results are properly formatted
   - Check error handling in tool execution

3. **Tool Output to API Test**
   - Verify tool outputs are sent to API correctly
   - Validate the shape of tool result messages
   - Ensure proper formatting of Role::Tool messages
   - Check that tool results are incorporated into conversation

4. **Workflow Compliance Test**
   - Verify message handling matches documented workflow
   - Check event flow matches `message_and_tool_call_lifecycle.md`
   - Validate StateCommand processing order
   - Ensure proper event bus message routing

### Testing Infrastructure Updates
- Enhanced tracing with `init_tracing_to_file_ai()` for detailed logging
- Log output written to `crates/ploke-tui/ai_temp_data/e2e_tool_logs/`
- Live API testing enabled via `.env` file with OPENROUTER_API_KEY
- Test harness supports multi-turn conversations with tool usage

## Development Workflow and Quality Gates

### Quality Assurance Process
1. **Pre-Development Validation**:
   - Run `cargo check` to ensure clean starting state
   - Execute existing test suite to establish baseline
   - Document any existing issues or technical debt

2. **During Development**:
   - Run `cargo check` after each significant change
   - Execute relevant subset of existing tests frequently
   - Validate type safety at each compilation boundary
   - Ensure no `dyn Trait` usage creeps into tool system

3. **Change Validation**:
   - Full existing test suite execution before committing changes
   - Performance regression detection
   - Type system integrity verification
   - Documentation of any temporary regressions with mitigation plans

4. **Regression Handling**:
   - **Acceptable Temporary Regressions**: Only when clearly documented with:
     - Root cause analysis
     - Clear timeline for resolution
     - Mitigation strategy
     - Impact assessment
   - **Unacceptable Regressions**: Any that compromise type safety, introduce dynamic dispatch, or break existing functionality without clear justification

### Static Analysis Integration
- **Compile-Time Verification**: Leverage Rust's type system to catch issues early
- **Tool Trait Validation**: Ensure all tools implement required traits with proper GAT usage
- **Zero-Copy Validation**: Verify deserialization paths avoid unnecessary allocations
- **API Type Validation**: Confirm all OpenRouter interactions use strongly typed interfaces

## Implementation Timeline

### Week 1: Foundation and Type System Validation
- Enhanced test harness development with strong typing
- Validation of existing OpenRouter type infrastructure
- Basic event tracking system with typed event structures
- Live API test framework setup with typed request/response validation

### Week 2: API and GAT Validation
- OpenRouter specification compliance testing using existing typed structures
- GAT system validation and zero-copy deserialization testing
- Trait-based tool system validation
- Static dispatch verification

### Week 3: Tool Loop Testing
- Single tool execution validation through trait interfaces
- Multi-tool conversation testing with typed message flows
- RequestSession integration with strong type safety

### Week 4: Error Scenarios and Type Safety
- Comprehensive error path testing with typed error structures
- Edge case validation maintaining type safety
- Failure mode analysis with proper error type propagation

### Week 5: Performance Testing
- Benchmark development focusing on zero-copy paths
- Load testing implementation with static dispatch validation
- Performance baseline establishment for trait-based system

### Week 6: Live API Testing
- Provider/model matrix testing with typed API interactions
- Tool capability validation through trait system
- Real-world scenario testing maintaining type safety

### Week 7: Documentation and Validation
- Type system architecture documentation
- Performance reporting with static vs dynamic dispatch analysis
- API compliance verification and type safety validation report

## Success Criteria

### Functional Validation
1. **100% Event Flow Coverage**: All events tracked and validated through typed interfaces
2. **Full API Compliance**: Complete adherence to OpenRouter specification with strongly typed requests/responses
3. **Zero Type Errors**: All serialization/deserialization validated with no runtime type failures
4. **Comprehensive Error Handling**: All failure modes tested with typed error structures
5. **Live API Validation**: Real-world testing across multiple providers with typed interactions

### Architectural Quality Validation
6. **Trait-Based Tool System Integrity**: All tools implement and execute through trait interfaces
7. **Static Dispatch Verification**: No dynamic dispatch (`dyn Trait`) in critical tool execution paths
8. **Zero-Copy Deserialization**: GAT-based deserialization paths validated for performance
9. **Strong Type Safety**: All API interactions use compile-time verified types
10. **Performance Baselines**: Benchmarks established with focus on static dispatch performance

### Quality Assurance
11. **Regression-Free Development**: No unacceptable regressions introduced during implementation
12. **Complete Documentation**: Full test coverage, type system validation, and architectural compliance reports
13. **Continuous Validation**: `cargo check` and existing test suite pass throughout development

## Testing Quality Gates

### Functional Quality Gates
1. **Unit Test Gate**: All individual components must pass isolated testing with type safety validation
2. **Integration Gate**: Cross-component interactions must be validated through trait interfaces
3. **Performance Gate**: All operations must meet latency and throughput requirements with static dispatch
4. **Live API Gate**: Real-world scenarios must pass with multiple providers using typed interactions

### Architectural Quality Gates
5. **Type Safety Gate**: All code must compile with `cargo check` and maintain strong typing
6. **Trait System Gate**: All tools must implement proper trait interfaces with GAT usage
7. **Static Dispatch Gate**: No dynamic dispatch allowed in critical tool execution paths
8. **Zero-Copy Gate**: Deserialization paths must demonstrate zero-copy performance where applicable

### Development Quality Gates  
9. **Regression Gate**: Existing test suite must pass, with any regressions properly documented and justified
10. **Documentation Gate**: Complete coverage, type system validation, and architectural compliance documentation

## Risk Mitigation

1. **API Rate Limits**: Implement rate limiting and test scheduling
2. **Provider Variability**: Design tests to handle provider-specific differences
3. **Network Reliability**: Include retry logic and timeout handling
4. **Resource Constraints**: Monitor and limit resource usage during testing
5. **Test Isolation**: Ensure tests don't interfere with each other

## Deliverables

1. **Complete Test Suite**: Comprehensive E2E testing coverage
2. **Performance Benchmarks**: Detailed performance profiles and baselines
3. **API Compliance Report**: Verification of OpenRouter specification adherence
4. **Test Documentation**: Complete coverage and validation documentation
5. **Continuous Integration**: Automated testing pipeline with quality gates

This plan provides a systematic approach to validating the entire tool-calling pipeline, ensuring robust, performant, and specification-compliant behavior across all scenarios.
