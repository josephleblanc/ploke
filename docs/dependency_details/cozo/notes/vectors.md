# Using Vector Embeddings in CozoDB for Ploke

This document explains how vector embeddings and similarity search will be
implemented in Ploke using the CozoDB database. The information is based on the
official CozoDB documentation for HNSW indices.

## 1. Storing Embeddings

Vector embeddings will not be stored in their own separate table. Instead, a
new field will be added to each of the existing primary node relations (e.g.,
`function`, `struct`, `enum`, etc.). This keeps the embedding directly
associated with the code entity it represents.

This new field, let's call it `embedding`, will be of a vector type. For
example, if we use 384-dimensional embeddings, the type in CozoDB would be
`<F32; 384>`. The field will be nullable, as embeddings will be generated and
added asynchronously after the initial parsing of the code.

Here is how we would modify a schema definition in our `ploke-transform` crate:

```rust // crates/ingest/ploke-transform/src/schema/primary_nodes.rs

define_schema!(FunctionNodeSchema { "function", id: "Uuid", name: "String", //
... other fields embedding: "<F32; 384>?" // New nullable vector field }); ```

## 2. Indexing Embeddings for Similarity Search

To perform efficient similarity searches, we cannot just query the `embedding`
field directly. We need to create a specialized index. CozoDB provides the
Hierarchical Navigable Small World (HNSW) index for this purpose.

A single HNSW index will be created that covers the `embedding` field across
*all* the primary node relations that contain it. This is a powerful feature of
CozoDB that allows us to search for similar nodes across different types (e.g.,
find functions that are semantically similar to a struct).

The command to create the index will look something like this:

```datalog ::hnsw create nodes:embedding_idx { dim: 384, dtype: F32, fields:
[embedding], distance: Cosine, } ```

-   `nodes:embedding_idx`: The name of our index.
-   `dim: 384`: The dimension of our vectors. This must match the dimension of
the vectors we store.
-   `dtype: F32`: The data type of our vector elements.
-   `fields: [embedding]`: The name of the field to index.
-   `distance: Cosine`: The distance metric to use for similarity. Cosine
similarity is a common choice for semantic search with text embeddings.

This index will be created once when the database is initialized.

## 3. Interacting with the Database

There are two main interactions with the database concerning embeddings:

### a. Populating the Embeddings

After a code entity is parsed and stored in the database, its `embedding` field
will be `null`. The embedding generation pipeline will:

1.  Query for nodes where `embedding` is `null`.
2.  Generate the embedding for each node.
3.  Update the node's relation with the new embedding vector using a standard
`:update` command.

### b. Performing Similarity Search

When we need to find code entities similar to a given query (e.g., for
retrieval-augmented generation), we will perform a vector search on our HNSW
index.

The query will look like this:

```datalog ?[dist, id, name] := ~nodes:embedding_idx{ id, name | query:
$query_embedding, k: 10, bind_distance: dist } ```

-   `~nodes:embedding_idx`: The `~` signifies a vector search on our index.
-   `query: $query_embedding`: We provide the embedding of our search query as
a parameter.
-   `k: 10`: We ask for the top 10 most similar results.
-   `bind_distance: dist`: We bind the similarity score to the variable `dist`.
-   `id, name`: We can retrieve other fields from the original relations in the
results.

This query will efficiently return the `k` most similar nodes from our entire
database, regardless of their type, based on the semantic meaning captured in
their vector embeddings.
