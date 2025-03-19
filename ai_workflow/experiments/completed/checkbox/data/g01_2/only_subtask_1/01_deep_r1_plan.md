| Subtask ID | Description | Estimated Time (hours) | Cfg Flag Required? | Dependencies | Potential Issues |
|---|---|---|---|---|---|
| 1 | Analyze type mappings between existing system and CozoDB types | 2 | No | - | Different type semantics (e.g., Cozo's strict F32/F64 vs Rust floats) may complicate mapping |
| 2 | Update TypeNode and TypeKind to support CozoDB types | 3 | Yes (feature_ai_task_type_alignment) | 1 | Breaking changes to type representation require flag |
| 3 | Modify get_or_create_type in visitor.rs for Cozo type conversion | 4 | Yes (feature_ai_task_type_alignment) | 2 | Complex type resolution logic might introduce parsing errors |
| 4 | Add serialization handlers for Cozo types in ron.rs | 3 | Yes (feature_ai_task_type_alignment) | 3 | Serialization format changes could break existing integrations |
| 5 | Update test cases with Cozo type assertions | 2 | No | 4 | Existing tests might fail due to type representation changes |
| 6 | Add validation for Cozo type constraints in type_graph | 1.5 | No | 2 | Vector dimension checks and numeric type validation complexity |

**Configuration Flag Rationale:** The `feature_ai_task_type_alignment` flag is required for subtasks 2-4 because: 
1. Changing TypeKind variants affects all type-sensitive operations
2. New serialization format could break existing consumers
3. Type resolution changes might alter parsing behavior
4. Allows gradual rollout and A/B testing of new type system

**Key Challenges:**
- Cozo's strict numeric separation (Int vs Float) vs Rust's type coercion
- Handling embedded vectors with fixed dimensions in a flexible type system
- Maintaining backward compatibility during transition period
- Different null handling semantics between the systems
