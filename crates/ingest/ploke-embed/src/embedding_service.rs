pub type EmbeddingVector = Vec<f32>;

pub trait EmbeddingService {
    fn compute_embedding(&self, snippet: &str) -> BoxFuture<'_, Result<EmbeddingVector, ploke_error::Error> >;

    fn dimensions(&self) -> usize;
}

// ```mermaid
// graph LR
// A[First-time User] --> B[Install CUDA?]
// B --> C{GPU Available?}
// C -->|Yes| D[Download 450MB Model]
// C -->|No| E[Slow CPU Execution]
// D --> F[Compile with CUDA Flags]
// ```
