# Review of CozoDB Datatypes Documentation

The `source/datatypes.rst` document provides essential information about the type system in CozoDB, which is crucial for developing our schema. Here are the key points:

## Runtime Types in CozoDB

CozoDB supports these runtime types:
- `Null`
- `Bool`
- `Number` (can be `Float` or `Int`)
- `String`
- `Bytes`
- `Uuid`
- `List`
- `Vector` (fixed-length, contains floats - F32 or F64 versions)
- `Json`
- `Validity` (for time travel queries)

## Type Comparison and Ordering

- Types are compared in the order listed above
- Within each type, values have specific comparison rules:
  - `false < true`
  - Numbers follow natural ordering
  - Lists are compared lexicographically
  - Strings are compared lexicographically by UTF-8 bytes
  - UUIDs have special sorting for UUIDv1 with similar timestamps

## Important Warning

There's a critical warning about equality vs. identity:
- `1 == 1.0` evaluates to `true` (equality)
- But `1` and `1.0` are distinct values (different identity)
- This can cause confusion when using floating point numbers in keys

## Literals

The document describes syntax for literals:
- Standard notations for `null`, `false`, `true`
- Various number formats (decimal, hex, octal, binary)
- String literals with different quoting options
- List literals with square brackets
- No literal syntax for `Bytes`, `Uuid`, `Vector`, or `Validity`

## Column Types for Stored Relations

For our schema design, these are the available column types:
- Atomic types: `Int`, `Float`, `Bool`, `String`, `Bytes`, `Uuid`, `Json`, `Validity`
- Nullable types: Add `?` after type (e.g., `Int?`)
- Homogeneous lists: `[Type]` or `[Type; N]` for fixed length
- Tuples: `(Type1, Type2, ...)` for heterogeneous fixed-length lists
- Vectors: `<F32; 1024>` or `<F64; 1024>` for fixed-length vectors
- Special type `Any` or `Any?` to allow all values

## How This Helps Our Schema Development

This information is crucial for our schema design because:

1. **Type Mapping**: We need to map our Rust `CodeGraph` types to appropriate CozoDB types:
   - `NodeId` and `TypeId` (usize) → `Int`
   - Names and paths → `String`
   - Attributes and docstrings → `String`
   - Visibility information → Enum represented as `String` or `Int`
   - Lists of related items → `[Int]` for lists of IDs

2. **Key Design**: The warning about number equality vs. identity is important for our key design. We should:
   - Use consistent types for keys (prefer `Int` for IDs)
   - Avoid floating point in keys
   - Be careful with type conversions

3. **Relation Structure**: We can now design relations with appropriate column types:
   - Functions relation with columns for name, visibility, parameters, etc.
   - Types relation for storing struct/enum definitions
   - Relationships relation for connecting nodes

4. **Vector Support**: For future embedding functionality, we can use the vector types:
   - Store code embeddings as `<F32; N>` or `<F64; N>` vectors
   - Enable similarity search on code snippets

With this understanding, we can now design a schema that properly represents our `CodeGraph` structure in CozoDB, ensuring type safety and efficient querying.
