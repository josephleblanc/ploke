# Task Tracker - Type Alignment

This file tracks the progress of tasks and subtasks related to type alignment in the `syn_parser` crate.

## Task List

| Task ID | Description | Finished | Tests Created | Tests Pass |
|---|---|---|---|---|
| **1 - Analyze and Map Types** | Analyze `syn_parser`'s existing types and map them to CozoDB types. | ☐ | ☐ | ☐ |
| 1.1 | Review `TypeKind` enum and document CozoDB mapping. | ☐ | ☐ | ☐ |
| 1.2 | Analyze usage of `TypeId` and `TypeKind` in AST nodes. | ☐ | ☐ | ☐ |
| 1.3 | Investigate implications of using `Bytes` for identifiers. | ☐ | ☐ | ☐ |
| 1.4 | Create detailed mapping table (gemma_workflow/type_mappings.md). | ☐ | ☐ | ☐ |
| 1.5 | Document potential data loss during type conversion. | ☐ | ☐ | ☐ |
| 1.6 | Review documentation alignment with CozoDB best practices. | ☐ | ☐ | ☐ |
| 1.7 | Document CozoDB Type Limitations. | ☐ | ☐ | ☐ |
| **2** | Modify `FunctionNode` to use `Bytes` for `name`. | ☐ | ☐ | ☐ |
| **3** | Modify `StructNode` and `EnumNode` to use `Bytes` for `name`. | ☐ | ☐ | ☐ |
| **4 - Replace TypeId with CozoDbType** | Replace `TypeId` with a new `CozoDbType` enum. | ☐ | ☐ | ☐ |
| 4.1 | Define the `CozoDbType` enum. | ☐ | ☐ | ☐ |
| 4.2 | Update instances of `TypeId` in `syn_parser/src/parser/nodes.rs`. | ☐ | ☐ | ☐ |
| 4.3 | Modify `VisitorState` to use `CozoDbType`. | ☐ | ☐ | ☐ |
| 4.4 | Update `type_map` in `VisitorState`. | ☐ | ☐ | ☐ |
| 4.5 | Add unit tests for `CozoDbType`. | ☐ | ☐ | ☐ |
| **5** | Update `FieldNode` to use `CozoDbType`. | ☐ | ☐ | ☐ |
| **6** | Update `ParameterNode` to use `CozoDbType`. | ☐ | ☐ | ☐ |
| **7** | Modify `Attribute` to store `value` as `Option<Bytes>`. | ☐ | ☐ | ☐ |
| **8** | Add unit tests for all changes. | ☐ | ☐ | ☐ |

