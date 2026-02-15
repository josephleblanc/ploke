# ploke-graph Current Coverage and Improvement Areas

## Current Implementation Status

The `ploke-graph` crate provides comprehensive coverage of the types parsed by `syn_parser`, with a few remaining areas for improvement.

### Schema Coverage

| syn_parser Type       | ploke-graph Relation      | Notes |
|-----------------------|---------------------------|-------|
| FunctionNode          | functions + function_params | Full support |
| StructNode            | structs + struct_fields   | Full support |
| EnumNode              | enums + enum_variants     | Full support |
| TraitNode             | traits                   | Full support |
| ImplNode              | impls                    | Full support |
| ModuleNode            | modules + module_relationships | Partial import support |
| TypeNode              | types + type_relations    | Full support |
| ValueNode             | values                   | Full support |
| MacroNode             | macros                   | Full support |
| GenericParamNode      | generic_params           | Partial support |
| Attribute             | attributes               | Basic support |

### Transform Coverage

The `transform.rs` handles conversion of all core CodeGraph types:
- Functions, parameters, return types
- Structs, enums, unions with their fields/variants  
- Traits and implementations
- Modules and their relationships
- Values (constants/statics) and macros
- Type system details

## Key Areas for Improvement

### 1. Generic Parameter Handling
- Currently transforms basic generics but could better handle:
  - Complex where clauses
  - Lifetime bounds  
  - Const generics
  - Default type parameters

### 2. Attribute Processing
- Could expand to better capture:
  - Attribute arguments
  - Nested attribute values
  - Common attributes (derive, cfg, etc.)
  - Custom attribute syntax

### 3. Import Relationships
- Currently missing transformation of ImportNode details
- Could add relations for:
  - use statements
  - extern crates
  - re-exports

### 4. Advanced Type Features
- Could better represent:
  - Trait objects with multiple bounds
  - Impl trait in return position
  - Complex associated types
  - Type projections
  - Never type (!)

### 5. Test Coverage Expansion
- Add tests for edge cases:
  - Complex generic types
  - Nested modules
  - Advanced trait bounds
  - Macro rules and expansions
  - Visibility edge cases

## Implementation Notes

The core schema and transformation capabilities are well-matched to the syn_parser types. The remaining gaps primarily involve:

1. More complete handling of Rust's advanced type system features
2. Better representation of module import/export relationships  
3. More detailed attribute processing
4. Expanded test coverage for edge cases

These improvements would make the graph representation even more precise and useful for code analysis tasks.
