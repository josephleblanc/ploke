# Common Error Patterns in AI-Assisted Development

## Error E0774: Misplaced Derive Attribute
**Description**: Attempting to apply `#[derive]` to trait implementations rather than type definitions.

**Context**: 
- Occurred while implementing the `Visible` trait across multiple node types
- Was trying to keep all related code together but misplaced attributes

**Root Causes**:
1. Pattern matching without context awareness
2. Not distinguishing between type definitions and implementations
3. Working across large files with many similar-looking blocks

**Prevention Strategies**:
- Clear separation of type definitions and trait implementations in code
- Linting rule: "Derives only on type definitions"
- Context-aware editing that understands Rust syntax boundaries

[See Insights](#potential-insights-from-e0774)

## Error E0405: Missing Trait in Scope  
**Description**: Referencing a trait that hasn't been imported or defined.

**Context**:
- Implementing visibility resolution system
- Trait was defined but not properly imported across modules

**Root Causes**:
1. Feature gating complexity
2. Cross-module dependencies not tracked
3. Partial context in AI working memory

**Prevention Strategies**:
- Better visibility tracking for traits
- Automated import suggestions
- Feature flag awareness in tooling

[See Insights](#potential-insights-from-e0405)

## Template for New Error Documentation

### Error [CODE]: Brief Description
**Description**: 1-2 sentence explanation

**Context**:
- What operation was being attempted
- Relevant code sections

**Root Causes**:
1. Primary technical reason
2. Workflow/process factors
3. System limitations

**Prevention Strategies**:
- Specific technical solutions
- Process improvements
- Tooling enhancements

[See Insights](#link-to-relevant-insights)
