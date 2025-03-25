# Ploke DB Component Design

## Decision Summary
- Renamed `database` crate to `ploke-db` for naming consistency
- Established clear responsibility boundaries between crates
- Designed phased implementation approach for embeddings
- Identified key risks and mitigation strategies

## Component Responsibilities

### ploke-db
1. **Core Functions**:
   - Query construction and optimization
   - Result ranking and filtering
   - Performance monitoring

2. **Implementation Benefits**:
   - Encapsulates CozoScript complexity
   - Provides type-safe query building
   - Enables query performance tuning

3. **Example Interface**:
```rust
pub fn find_functions(
    name_pattern: Option<&str>,
    return_type: Option<TypeId>,
    limit: usize
) -> Result<Vec<FunctionNode>> {
    // Builds and executes optimized query
}
```

### ploke_graph
1. **Core Functions**:
   - Schema definition and management
   - Efficient data transformation
   - Bulk insertion operations

2. **Implementation Benefits**:
   - Avoids unnecessary data serialization
   - Maintains data consistency
   - Handles schema migrations

## Rationale

### Why Direct Inserts in ploke_graph?
1. **Performance**:
   - Eliminates intermediate serialization
   - Enables batch optimizations
   - Reduces memory overhead

2. **Consistency**:
   - Single component owns schema
   - Atomic operations easier to manage
   - Clear error handling boundaries

### Why Separate ploke-db?
1. **Abstraction**:
   - Hides database implementation details
   - Provides clean query interface
   - Enables future optimizations

2. **Maintainability**:
   - Isolated query logic
   - Easier to test and benchmark
   - Clear performance boundaries

## Migration Path

1. **Phase 1 (MVP)**:
   - Basic query interface in ploke-db
   - Direct CozoDB embeddings
   - Simple transaction model

2. **Phase 2**:
   - Advanced query building
   - External embedding support
   - Full transaction support

3. **Phase 3**:
   - Query optimization
   - Schema migration system
   - Performance monitoring
