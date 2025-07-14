# Using CozoDB HNSW Vector Search

This document outlines the correct way to use HNSW vector search with CozoDB, based on findings from fixing failing tests in `src/index.rs`.

## Key Learnings

1.  **Index Creation Requires `fields`**: The `::hnsw create` command *must* include the `fields` parameter to specify which column(s) in the relation contain the vectors to be indexed. The command fails with `Cannot create HNSW index without vector fields` if this is omitted.
2.  **Vector Type Syntax**: The column being indexed must be defined with the correct vector type syntax in the `:create` statement. The correct syntax is `<DTYPE; DIM>`, for example `<F32; 384>` for 32-bit floats.
3.  **HNSW Requires F32**: The HNSW index implementation in CozoDB specifically requires `F32` vectors. Using `F64` will result in a `Cannot create HNSW index with field ... of type F64 (expected F32)` error.
4.  **Explicit Vector Casting in Queries**: When performing a similarity search, the query parameter containing the vector must be explicitly cast using the `vec()` function within the datalog query. Binding the parameter directly will result in an `Expected vector, got [...]` error.

    *Correct usage:*
    ```cozo
    :create documents {
        id: Int,
        content: String,
        embedding: <F32; 384>
    }
    
    ::hnsw create documents:embedding {
        fields: [embedding],
        dim: 384,
        dtype: F32,
        m: 32,
        ef_construction: 200,
        distance: L2
    }

    ?[id, content, distance] := 
        ~documents:embedding{id, content | 
            query: q, 
            k: $k, 
            ef: $ef,
            bind_distance: distance
        }, q = vec($query_embedding)
    ```

## Test Fix Checklist

- [x] `test_empty_search`
- [x] `test_create_index_and_insert`
- [x] `test_similarity_search`
- [ ] `test_index_rebuild`
- [ ] `test_index_stats`
