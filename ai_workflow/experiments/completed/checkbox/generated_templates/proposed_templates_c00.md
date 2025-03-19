claude - all default

# LLM Task Breakdown Templates with Feature Flag Guidance

## Overview
Based on your workflow needs and the examples provided, I've created three templates to help break down complex tasks into trackable subtasks. Each template includes guidance on when to use feature flags (cfg attributes) to maintain a working codebase during multi-step refactoring.

## Feature Flag Naming Conventions
For each template, I've used a different feature naming pattern that follows Rust conventions:
- Simple: `feature_taskname` (e.g., `feature_type_alignment`)
- Structured: `impl_taskname` (e.g., `impl_sync_types`)
- Comprehensive: `refactor_component` (e.g., `refactor_codegraph`)

## Template 1: Simple Task Tracker
This minimalist template focuses on clear, concise task breakdown with minimal formatting overhead.

```markdown
# Task Breakdown: [Task Name]

## Main Task Description:
[Brief description of the overall task]

## Feature Flag: `feature_[task_name]`
Apply this feature flag to any code changes that would break compilation if not fully implemented.

Example: `#[cfg(feature = "feature_[task_name]")]`

## Subtasks:
- [ ] 1. [Description of first subtask]
- [ ] 2. [Description of second subtask]
  - [ ] 2.1. [Sub-step if needed]
  - [ ] 2.2. [Sub-step if needed]
- [ ] 3. [Description of third subtask]

## Testing Strategy:
[Brief notes on how to test this change]
```

**Reasoning**: This template provides just enough structure to organize thoughts without overhead. It's ideal for straightforward tasks or when you're in a hurry and need something simple but effective.

## Template 2: Structured Implementation Plan
This template provides more organization around different phases of implementation.

```markdown
# Structured Implementation Plan: [Task Name]

## Task Overview
**Objective**: [Brief description of what needs to be accomplished]
**Impact Areas**: [Files/modules that will be affected]
**Estimated Complexity**: [Low/Medium/High]

## Feature Flag Strategy
- Feature name: `impl_[task_name]`
- When to use: Apply feature flag ONLY when changes would break the codebase without completing all related subtasks
- Usage example: 
  ```rust
  #[cfg(feature = "impl_[task_name]")]
  pub fn new_function() { /* implementation */ }
  ```

## Subtasks

### Phase 1: Analysis
- [ ] 1.1. [Review existing implementation]
- [ ] 1.2. [Identify affected components]

### Phase 2: Implementation
- [ ] 2.1. [First implementation step]
  - [ ] 2.1.1. [Implementation detail]
  - [ ] 2.1.2. [Implementation detail]
- [ ] 2.2. [Second implementation step]

### Phase 3: Testing & Validation
- [ ] 3.1. [Create tests for new functionality]
- [ ] 3.2. [Verify existing functionality works]

## Dependencies
- Subtask 2.1 depends on completing 1.2
- [Other dependency relationships]

## Implementation Notes
[Reasoning behind key design decisions]
```

**Reasoning**: This template introduces phases and dependency tracking, which helps manage complex tasks by grouping related subtasks together. It encourages thinking about the analytical phase before jumping into implementation.

## Template 3: Comprehensive Refactoring Plan
This template provides the most thorough breakdown for complex refactoring tasks.

```markdown
# Comprehensive Refactoring Plan: [Task Name]

## 1. Task Definition
**Task**: [Detailed description of the task]
**Purpose**: [Why this task is necessary]
**Success Criteria**: [How to determine when the task is complete]

## 2. Feature Flag Configuration
**Feature Name**: `refactor_[component_name]`

**Implementation Guide:**
```rust
// New implementation with feature flag
#[cfg(feature = "refactor_[component_name]")]
impl SomeStruct {
    pub fn new_method() { /* implementation */ }
}

// Maintain backward compatibility
#[cfg(not(feature = "refactor_[component_name]"))]
impl SomeStruct {
    pub fn old_method() { /* implementation */ }
}
```

## 3. Task Breakdown

### 3.1 Analysis & Preparation
- [ ] 3.1.1. Review current implementation
  - **Purpose**: [Why this step is necessary]
  - **Expected Outcome**: [What should be produced]
  - **Files to Examine**: [List of relevant files]
- [ ] 3.1.2. Create implementation plan
  - **Purpose**: [Why this step is necessary]

### 3.2 Core Implementation
- [ ] 3.2.1. [First implementation step]
  - **Files to Modify**: [List of files]
  - **Reasoning**: [Why this approach was chosen]
  - **Testing Approach**: [How to test this change]
- [ ] 3.2.2. [Second implementation step]
  - **Code Changes**:
    ```rust
    // Example of expected changes
    ```

### 3.3 Testing & Integration
- [ ] 3.3.1. Update existing tests
- [ ] 3.3.2. Add new tests for changed functionality
- [ ] 3.3.3. Test with and without feature flag enabled

### 3.4 Documentation & Knowledge Preservation
- [ ] 3.4.1. Update code documentation with reasoning
- [ ] 3.4.2. Document design decisions
- [ ] 3.4.3. Create commit message template capturing key changes

## 4. Rollback Strategy
[Instructions on how to revert changes if issues are found]

## 5. Progress Tracking
- [ ] Analysis Phase: 0/2 complete
- [ ] Implementation Phase: 0/2 complete
- [ ] Testing Phase: 0/3 complete
- [ ] Documentation Phase: 0/3 complete
```

**Reasoning**: This template provides comprehensive guidance for tasks that require significant refactoring. It emphasizes knowledge preservation, testing strategies, and clearly documenting the reasoning behind changes—addressing your concern about preserving the LLM's thinking process.

## Uncertainties in My Response

I'm uncertain about:
1. The ideal level of detail for feature flag examples - I've included code snippets, but these may need adjustment based on your specific needs
2. Whether specific git commit message guidance should be more prominently featured in the templates
3. If the testing strategy sections are detailed enough for your use case

## Example Usage

To use these templates, you would:
1. Choose the template that best fits the complexity of your task
2. Fill in the task details and ask the LLM to complete the template
3. Use the generated subtask list to track progress as you work with the LLM

Would you like me to adjust any aspects of these templates or would you like an example of one filled out for a specific task?
