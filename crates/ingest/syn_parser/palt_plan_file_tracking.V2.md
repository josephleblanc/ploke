# Type-Safe Relational Scope Tracking Plan

## Architectural Principles

### Core Tenets
1. **Type Preservation**:
   - Maintain strong typing through entire pipeline
   - Leverage Rust's type system for validation
   - Avoid stringly-typed database interactions

2. **Separation of Concerns**:
   - `syn_parser`: Focuses on accurate syntax → type conversion
   - `ploke-transform`: Handles type-safe database mapping
   - `ploke-db`: Manages storage and query execution

3. **Validation at Boundaries**:
   - Validate early (during parsing)
   - Validate often (during transformations)
   - Validate late (database constraints)

## Detailed Implementation Plan

### Parser Role Clarification
1. **Primary Responsibility**:
   - Parse Rust source into typed AST nodes
   - Build initial CodeGraph with relations
   - Perform basic syntactic validation

2. **Secondary Responsibility**:
   - Provide validation helpers (visibility, scope)
   - Track source locations precisely
   - Preserve original syntax details

### Key Data Structure Enhancements

#### Node Types (nodes.rs)
1. **Span Tracking**:
   ```rust
   pub struct Span {
       pub file: PathBuf,
       pub start: usize,
       pub end: usize,
       pub line_col: u32,
       pub col: u32
   }
   ```

2. **Enhanced Visibility**:
   ```rust
   pub enum Visibility {
       Public,
       Crate,
       Super,
       In(String), // Path for pub(in path)
       Raw(String) // Original syntax for exact reconstruction
   }
   ```

3. **Import Context**:
   ```rust
   pub struct Import {
       pub path: Vec<String>,
       pub alias: Option<String>,
       pub is_glob: bool,
       pub is_extern: bool,
       pub source_span: Span
   }
   ```

### Parser Workflow Phases

#### Phase 1: Extraction
1. **File Discovery**:
   - Walk project directory
   - Distinguish user code vs dependencies
   - Identify module boundaries

2. **Syntax Parsing**:
   - Use syn for raw parsing
   - Convert to typed nodes immediately
   - Preserve all source locations

3. **Graph Construction**:
   - Build CodeGraph incrementally
   - Establish relations between nodes
   - Track module hierarchies

#### Phase 2: Validation
1. **Scope Checking**:
   - Basic visibility rules
   - Module containment
   - Crate boundaries

2. **Consistency Checks**:
   - Verify all references resolve
   - Check for duplicate items
   - Validate import paths

3. **Error Reporting**:
   - Rich error messages
   - Source location context
   - Suggested fixes

### Integration with ploke-transform

#### Type-Safe Mapping
1. **IntoCozo Trait**:
   ```rust
   pub trait IntoCozo {
       fn as_insert_script(&self) -> String;
       fn schema() -> &'static str;
       fn relation_name() -> &'static str;
   }
   ```

2. **Example Implementation**:
   ```rust
   impl IntoCozo for FunctionNode {
       fn schema() -> &'static str {
           ":create function { 
               id: Uuid, 
               name: String, 
               visibility: String,
               module_id: Uuid,
               span: String 
           }"
       }
       
       fn as_insert_script(&self) -> String {
           format!("?[id, name, visibility, module_id, span] <- [[{:?}, {:?}, {:?}, {:?}, {:?}]]",
               self.id, self.name, self.visibility, self.module_id, self.span)
       }
   }
   ```

### Validation Architecture

#### Pre-Database Validation
1. **Scope Resolution**:
   - Maintain resolve_visibility()
   - Add file-aware checking
   - Support dependency modes

2. **Type Checking**:
   - Basic type existence
   - Trait bounds validation
   - Generic constraints

3. **Path Resolution**:
   - Absolute path calculation
   - Workspace-relative paths
   - Dependency path mapping

### Performance Considerations

1. **Incremental Processing**:
   - Process files in parallel
   - Cache intermediate results
   - Support partial updates

2. **Memory Efficiency**:
   - Use arenas for AST nodes
   - Compact storage for common strings
   - Lazy resolution where possible

3. **Database Optimization**:
   - Batch inserts
   - Prepared statements
   - Index planning

### Error Handling Strategy

1. **Rich Error Types**:
   ```rust
   pub enum ParseError {
       Syntax(syn::Error),
       Scope {
           item: NodeId,
           required_visibility: Visibility,
           current_scope: Vec<String>
       },
       DuplicateItem {
           name: String,
           first_span: Span,
           second_span: Span
       }
   }
   ```

2. **Error Recovery**:
   - Continue after non-fatal errors
   - Track all errors encountered
   - Support partial results

### Testing Approach

1. **Unit Tests**:
   - Individual node parsing
   - Relation building
   - Visibility resolution

2. **Integration Tests**:
   - Full file processing
   - Multi-file projects
   - Dependency scenarios

3. **Golden Tests**:
   - Compare against expected CozoScript
   - Verify round-trip parsing
   - Check error messages

## Rationale for Key Decisions

### Type Preservation
- **Why**: Prevents entire classes of errors
- **How**: Through IntoCozo trait and strong types
- **Benefit**: Catches issues at compile time

### Span Tracking
- **Why**: Essential for error reporting and debugging
- **How**: Capture precise source locations
- **Benefit**: Enables rich IDE features later

### Validation Phases
- **Why**: Catch issues early and often
- **How**: Multi-stage checking
- **Benefit**: Better error messages and faster feedback

### ploke-transform Integration
- **Why**: Clean separation of concerns
- **How**: Dedicated crate for DB mapping
- **Benefit**: Easier to maintain and evolve

## Future Extensions

1. **Incremental Parsing**:
   - Watch mode for file changes
   - Partial graph updates
   - Dependency tracking

2. **IDE Support**:
   - Live error reporting
   - Code completion
   - Refactoring tools

3. **Advanced Queries**:
   - Cross-crate analysis
   - Change impact analysis
   - Architectural linting
