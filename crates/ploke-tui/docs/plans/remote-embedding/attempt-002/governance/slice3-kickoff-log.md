The "model registry" mentioned in the plans, particularly in Slice 3 of `execution_plan.md`, is indeed intended to be a **runtime dynamic registry**, not a compile-time hardcoded list.

The phrasing "dispatch vectors to the per-dimension relation reported by the registry (creating it first if missing)" is key here. This implies a system that can:
1.  **Dynamically discover or be informed of model characteristics:** When a user selects a model (even a new or unknown one), the system needs to determine its properties like the vector dimension, provider, and specific model ID.
2.  **Lazily provision resources:** If an embedding set (defined by `provider`, `model`, and `shape`) is encountered for the first time, the system should be able to create the necessary database relations (`embedding_vectors_<dims>`) for it.

### Compelling Reason for "Registry" Functionality

There is a compelling reason for *some form* of "registry" functionality, even if it's not implemented as a heavy, standalone `ModelRegistry` type. Its necessity stems from bridging several critical gaps:

1.  **Mapping User/Provider Identifiers to Internal Parameters:** Users (or external providers) interact with models using human-readable names or specific API identifiers (e.g., "openai/text-embedding-ada-002"). The internal system, however, needs concrete technical parameters like the vector `dimension` (e.g., 1536) to select the correct storage (e.g., `embedding_vectors_1536`) and retrieval mechanisms. The "registry" provides this essential lookup.

2.  **Validation and Invariant Enforcement:** When a model is chosen, the system needs to ensure its characteristics are compatible with the database. For example, if a model claims to produce 1536-dimension vectors, the system needs to ensure that the actual vectors it processes are indeed 1536-dimensional and that the database is configured to handle them (or can be configured lazily).

3.  **HNSW Index Management:** The retrieval part of the system (HNSW indexing) needs to know which specific vector relation to query. This is determined by the model's `EmbeddingSetId`.

If this mapping and validation logic were removed, the user-facing commands would lack the necessary intelligence to interact with the multi-embedding database effectively. Every user command would need to explicitly specify not only the model but also its technical characteristics (like dimension), which is cumbersome and error-prone.

The "registry" as conceived in the plan could be implemented in a lightweight manner:
*   As an API or set of functions that derive `EmbeddingSetId` from user input (model name, provider).
*   By leveraging `UserConfig` to store and retrieve user-defined models and their characteristics.
*   By querying the `EmbeddingProcessor` itself, which knows its own `shape`, `provider`, and `model`.

It serves as the conceptual layer that manages the lifecycle and characteristics of available (or dynamically provisioned) embedding sets. Removing this conceptual layer would push the complexity of mapping and validation back into every call site or user command, which would be less maintainable and less robust.