# Cozo Schema Reference

Status: Draft

## Purpose

TODO: Describe what this chapter covers.

## Scope

TODO: Define what is in scope and out of scope for this chapter.

## Notes

TODO: Add key references and links.

## Schema

The database schema contains two categories of entries, nodes and edges. The
nodes are, for the most part, directly taken from the the code items detailed
in the Rust Reference, such as "Const", "Function", "Struct", and so on.
Additionally, there are some meta-data nodes that contain information on the
target crate. Edges are the relationships between the nodes, where the edge
that most nodes have is "Contains" to represent the dependency relationship
between a module and the items it contains.

### Nodes

Nodes represent code items, and there are several categories to differentiate the kind of node. These categories indicate which kind of context a node may be found.

1. Primary: A code item which may exist in a module without other encapsulating scope.
  - FunctionNode
  - ConstNode
  - EnumNode
  - ImplNode
  - ImportNode ("use" statements)
  - MacroNode
  - ModuleNode
  - StaticNode
  - StructNode
  - TraitNode
  - TypeAliasNode
  - UnionNode

2. Secondary: A code item which may only exist within the scope of a primary node item, and not alone within the scope of a module.
  - ParamNode: A function parameter
  - VariantNode: An enum or struct variant
  - FieldNode: A struct field
  - AttributeNode: Attributes prepending the node
  - GenericTypeNode: todo
  - GenericLifetimeNode: todo
  - GenericCostNode: todo

3. Assoc: A code item which may be found within the scope of an Impl or Trait block.
  - MethodNode

4. Type Nodes: Nodes that represent a specific type, and containing ID
   references to related types. Each of thes types has an ID, so for example a
`Vec<i32>` would have the same type node but have different rows from a
`Vec<u32>`.
  - NamedType
  - ReferenceType
  - SliceType
  - ArrayType
  - TupleType
  - FunctionType
  - NeverType
  - InferredType
  - RawPointerType
  - TraitObjectType
  - ImplTraitType
  - ParenType
  - MacroType
  - UnknownType (temporary and fallback while debugging)

4. Special Metadata Cases: These nodes do not fit into the divisions in the
   code items, but are still useful in grouping the code items.
  - CrateContext: Contains an ID for the crate, along with the root path and
  files in the crate.
  - Bm25Meta: Metadata for bm25 search

### Edges

Edges define the relationship between nodes. We enforce the rules for which
relationships may exist between nodes in the parsing process through type
definitions on each edge type.

The schema includes two types of edges, which represent different layers in the
code graph. The first category of edges is "Syntactic" edges (or relations),
and the second is "Semantic" edges (at the time of writing not yet
implemented).

Syntactic edges represent the relations that are present in the syntax of the
code, which is more precies and verbose than the "Semantic" representation. For
example, in a semantic representation of the code graph, we might not
differentiate between two impl blocks, as they are functionally one impl block.
However, if we are to edit an impl block, it is important to be able to
differentiate between two impl blocks.

#### Syntactic Edges

Syntactic edges are defined in the comments as shown below:

```rust,ignore
{{#include ../../../../crates/ingest/syn_parser/src/parser/relations.rs:syntactic_relation}}
```


