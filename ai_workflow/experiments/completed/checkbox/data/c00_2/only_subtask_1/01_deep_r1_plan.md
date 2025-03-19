Let me propose a plan for the type alignment task following your conventions:

# Comprehensive Refactoring Plan: Type Alignment with CozoDB

## 1. Task Definition
**Task**: Refactor type representations in `syn_parser` to align with CozoDB's type system including:
- Replace content hash strings with `Bytes`
- Represent embeddings as fixed-length F32/F64 vectors
- Migrate numeric types to explicit Int/Float variants
- Add proper nullability support

**Purpose**: Enable seamless integration with CozoDB's storage model and query engine

**Success Criteria**:
1. All type representations match CozoDB's column types
2. Serialized output validates against CozoDB schema
3. Backward compatibility maintained via feature flag

## 2. Feature Flag Configuration
**Feature Name**: `cozo_type_system`

```rust
// Example for hash fields
#[cfg(feature = "cozo_type_system")]
struct ValueNode {
    hash: Vec<u8>, // Stored as Bytes in Cozo
}

#[cfg(not(feature = "cozo_type_system"))] 
struct ValueNode {
    hash: String, // Legacy representation
}
```

## 3. Task Breakdown

### 3.1 Analysis & Preparation
- [ ] 3.1.1. Audit current type representations
  - **Purpose**: Identify mismatches with Cozo's type system
  - **Files**: nodes.rs, types.rs, visitor.rs
- [ ] 3.1.2. Design type mapping matrix
  - **Purpose**: Define Rust → Cozo type translations

### 3.2 Core Implementation
- [ ] 3.2.1. Refactor TypeKind enum
  - **Files**: types.rs, nodes.rs
  - **Changes**:
    ```rust
    enum TypeKind {
        Bytes,
        Vector(FloatKind, usize), // (F32/F64, length)
        Int,
        Float,
        // ... other variants
    }
    ```
- [ ] 3.2.2. Update type processing in visitor
  - **Files**: visitor.rs
  - **Reasoning**: Map syn types to Cozo-compatible representations
  - **Testing**: Verify float arrays become FixedVector types

### 3.3 Testing & Integration
- [ ] 3.3.1. Update serialization tests for new types
- [ ] 3.3.2. Add roundtrip tests for binary formats
- [ ] 3.3.3. Validate against Cozo schema constraints

### 3.4 Documentation
- [ ] 3.4.1. Add type mapping table to crate docs
- [ ] 3.4.2. Document feature flag behavior
- [ ] 3.4.3. Update example datasets

## 4. Rollback Strategy
1. Disable `cozo_type_system` feature
2. Revert to previous type definitions
3. Run integration test suite

## 5. Progress Tracking
- [ ] Analysis Phase: 0/2 
- [ ] Implementation Phase: 0/2
- [ ] Testing Phase: 0/3
- [ ] Documentation: 0/3

This plan addresses:
- Strict type alignment per Cozo requirements
- Zero-copy parsing where possible
- Documentation requirements through updated examples
- Feature-flagged transition strategy

Would you like me to elaborate on any specific aspect of this proposal?
