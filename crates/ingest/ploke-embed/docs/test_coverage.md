# Test Coverage Plan for Embeddings Pipeline

## Overview
This document outlines comprehensive test coverage requirements for the embeddings pipeline, prioritizing:
1. Reliability on consumer hardware (CPU-focused workflows)
2. Clean error propagation to terminal UI
3. Performance optimization (memory/CPU efficiency)
4. Network-handling resilience (for HuggingFace/OpenAI providers)

## Priority Levels
- **P0**: Critical path, required for production readiness
- **P1**: Important functionality, should be implemented
- **P2**: Nice-to-have optimizations, implement if possible

---

## Unit Tests (P0)

### Embedding Processor
- [ ] Test all EmbeddingSource variants generate correctly
- [ ] Test dimension reporting for each provider type
- [ ] Validate error mapping between provider errors and EmbedError

```rust
// Example: Local embedder dimension test
#[test]
fn local_model_dimensions() {
    let model = LocalEmbedder::new(Default::default()).unwrap();
    assert_eq!(model.dimensions(), 384);
}
```

### Local Embedder
- [ ] Test tokenization across edge cases (unicode, empty, very long)
- [ ] Validate proper pooling/normalization logic
- [ ] Test error handling:
  - Invalid model URLs
  - Missing tokenizer.json
  - GPU allocation failures
- [ ] Test configurable batch sizes with small inputs

### HuggingFace/OpenAI Providers
- [ ] Test request serialization for differing snippet counts
- [ ] Test error handling:
  - Invalid credentials
  - Rate limiting
  - Timeouts
  - Malformed responses
- [ ] Test dimension verification against config

### Batch Processing
- [ ] Test cancellation token integration
- [ ] Validate batch progress reporting accuracy
- [ ] Test pipeline continuation after network failure recovery
- [ ] Test dangling batch cleanup

### Error Handling
- [ ] Test truncate_string() edge cases
- [ ] Validate EmbedError -> ploke_error::Error conversion
- [ ] Test error wrapping preserves critical context

---

## Integration Tests (P0-P1)

### Local Embedder Pipeline
1. Download test model (all-MiniLM-L6-v2)
2. Run through sample messages
3. Verify:
   - Output vector dimensions
   - Output normalization (magnitude≈1)
   - Basic semantic similarity

### HuggingFace API Flow
- [ ] Mock server tests validating:
  - Batch request splitting
  - HTTP header formatting
  - Result parsing
  - Handling 429/500 responses

### Embedding -> Storage Flow
1. Load test database fixture
2. Run indexing on small repo
3. Verify:
   - Embedding vectors persisted 
   - Embeddings attached to nodes
   - Progress events dispatched

### Cancellation Flow
- [ ] Indexing startup -> cancellation command -> clean stop
- [ ] Verify cancellation token propagation
- [ ] Test resource cleanup during cancellation

---

## Performance Testing (P1)

### Local Embedder
- [ ] Measure TP95 latency at different batch sizes (1-64)
- [ ] Measure memory consumption during embedding generation
- [ ] CPU utilization tests (single-threaded vs multi)

### Remote Providers
- [ ] Latency measurement at differing connection qualities
- [ ] Parallel request saturation testing
- [ ] Automatic batch sizing for network conditions

### Full Pipeline Profile
- [ ] End-to-end indexing time for:
  - Small crate (10 files)
  - Medium crate (100 files)
- [ ] Profile database write contention (chi-square tests)

---

## Network Reliability Testing (P1)

### Simulated Conditions
- [ ] High latency (300ms+ RTT)
- [ ] Packet loss (1%-5%)
- [ ] Intermittent connectivity failures
- [ ] Rate limitation handling 

### Transition Strategies
- [ ] Local fallback on remote failure
- [ ] Graceful degradation mode
- [ ] Progress checkpointing for resume

---

## Edge Cases (P2)

### Input Validation
- [ ] Empty snippet batches
- [ ] Purely whitespace/comment snippets
- [ ] Oversized snippets (>512 tokens)

### Hardware Constraints
- [ ] CPU-fallback GPU-reliant workflows
- [ ] Operation under constrained memory (<1GB free)
- [ ] Suspend/resume handling

### Model Compatibility
- [ ] Non-standard HF models
- [ ] Different embedding dimensions
- [ ] Model cache purging
