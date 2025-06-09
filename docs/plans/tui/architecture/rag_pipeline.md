# RAG Pipeline Architecture Details

## 1. Vector Storage Implementation

### Storage Architecture
```rust
pub struct VectorStore {
    // Underlying storage using HNSW algorithm
    index: hnswlib::Hnsw<f32>,
    // Metadata storage in cozo
    metadata: CozoStore,
    // Local cache for frequent retrievals
    cache: LruCache<Uuid, Vec<f32>>,
}

pub struct VectorMetadata {
    id: Uuid,
    file_path: PathBuf,
    node_type: NodeType,
    embedding_type: EmbeddingType,
    timestamp: DateTime<Utc>,
    chunk_info: Option<ChunkInfo>,
}
```

### Embedding Strategy
- Multiple embedding types per node:
  1. Full node content
  2. Semantic summary
  3. API/interface only
  4. Dependencies/usage context

### Storage Operations
```rust
impl VectorStore {
    async fn upsert(&mut self, 
        content: &str,
        metadata: VectorMetadata,
        embedding_type: EmbeddingType
    ) -> Result<(), StoreError>;

    async fn search(&self,
        query: &str,
        filters: &SearchFilters,
        limit: usize
    ) -> Vec<SearchResult>;
    
    async fn delete(&mut self, id: Uuid) -> Result<(), StoreError>;
    
    async fn rebuild_index(&mut self) -> Result<(), StoreError>;
}
```

## 2. Reranking System

### Multi-Stage Ranking
1. Initial Retrieval (HNSW)
2. Semantic Reranking
3. Code-Aware Scoring
4. Intent-Based Boosting

### Scoring Components
```rust
pub struct RankingScore {
    // Base similarity score from vector search
    vector_score: f32,
    // Semantic relevance from small LLM
    semantic_score: f32,
    // Code structure relevance
    structural_score: f32,
    // Intent alignment score
    intent_score: f32,
    // Final combined score
    final_score: f32,
}

impl RankingScore {
    fn compute_final_score(&mut self, weights: &RankingWeights) {
        self.final_score = weights.vector * self.vector_score
            + weights.semantic * self.semantic_score
            + weights.structural * self.structural_score
            + weights.intent * self.intent_score;
    }
}
```

### Reranking Pipeline
```rust
pub trait Reranker: Send + Sync {
    async fn rerank(
        &self,
        results: Vec<SearchResult>,
        query: &str,
        intent: &Intent,
    ) -> Vec<RankedResult>;
}

pub struct MultiStageReranker {
    semantic_model: Box<dyn SemanticScorer>,
    code_analyzer: Box<dyn CodeScorer>,
    intent_analyzer: Box<dyn IntentScorer>,
    weights: RankingWeights,
}
```

## 3. Context Window Management

### Window Types
```rust
pub enum ContextWindow {
    // Fixed size token window
    TokenBased {
        max_tokens: usize,
        buffer: VecDeque<Token>,
    },
    // Semantic unit window
    UnitBased {
        units: Vec<SemanticUnit>,
        total_tokens: usize,
    },
    // Hybrid approach
    Hybrid {
        primary: Box<ContextWindow>,
        overflow: Box<ContextWindow>,
    },
}
```

### Window Management
```rust
impl ContextWindow {
    fn add_context(
        &mut self,
        content: &str,
        importance: f32,
    ) -> Result<(), WindowError>;

    fn optimize(&mut self, target_size: usize) -> Vec<String>;

    fn get_summary(&self) -> String;
}
```

### Priority Management
- Token budget allocation
- Important context preservation
- Dynamic window resizing
- Context relevance scoring

## 4. Chunking Strategies

### Chunk Types
```rust
pub enum ChunkType {
    // Single complete item (function, struct, etc)
    CompleteItem,
    // Fixed size chunks
    FixedSize { size: usize },
    // Semantic boundaries
    SemanticBoundary,
    // Hybrid approach
    Hybrid { 
        primary: Box<ChunkType>,
        fallback: Box<ChunkType>,
    },
}
```

### Example Chunking Patterns

1. Function-Level Chunking:
```rust
// Original
pub fn process_items<T>(items: &[T]) -> Result<Vec<T>, Error> {
    let mut processed = Vec::new();
    for item in items {
        processed.push(process_item(item)?);
    }
    Ok(processed)
}

// Chunk Metadata
ChunkInfo {
    type: ChunkType::CompleteItem,
    start_line: 1,
    end_line: 7,
    context: "Function process_items",
    dependencies: ["process_item"],
}
```

2. Module-Level Chunking:
```rust
// Original
mod processing {
    pub struct Processor {
        config: Config,
    }
    
    impl Processor {
        pub fn new(config: Config) -> Self {
            Self { config }
        }
    }
}

// Chunks
ChunkInfo {
    type: ChunkType::SemanticBoundary,
    chunks: vec![
        Chunk {
            content: "pub struct Processor {...}",
            context: "Module processing, struct definition",
        },
        Chunk {
            content: "impl Processor {...}",
            context: "Module processing, implementation",
        },
    ],
}
```

### Chunking Implementation
```rust
pub trait Chunker: Send + Sync {
    fn chunk_content(
        &self,
        content: &str,
        chunk_type: ChunkType,
    ) -> Vec<Chunk>;

    fn merge_chunks(
        &self,
        chunks: &[Chunk],
        max_size: usize,
    ) -> Vec<Chunk>;
}
```

## 5. Integration Points

### With Code Graph
- Graph-aware chunking
- Relationship preservation
- Cross-reference handling

### With Intent Processing
- Intent-driven chunk selection
- Context relevance scoring
- Query-specific chunking

### With LLM Interface
- Token optimization
- Context formatting
- Prompt integration

## 6. Performance Optimization

### Caching Strategy
- Embedding cache
- Chunk cache
- Result cache
- Metadata cache

### Parallel Processing
- Concurrent embedding generation
- Parallel chunk processing
- Async retrieval pipeline

### Resource Management
- Memory-efficient storage
- Batch processing
- Index optimization
- Cache eviction policies
