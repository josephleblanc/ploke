# ARCH_REVIEW_TRIAGE

## Ideas to evaluate

1. **Immediate Changes**:
   - Make embedding dimensions configurable
   - Consolidate `error`+`security` into `utils`
   - Replace UUIDv7 with content-addressable versioning

2. **Risk Mitigation**:
   - Implement feature toggles for controversial components
   - Establish performance baseline metrics
   - Conduct threat modeling session

## Address Franz's Review

### A: **Valid Concerns** (Critical Observations Worth Addressing):

1. **Schema Inflexibility**:
   - Hardcoded embedding dimensions in DB schema
   - Redundant storage of content hashes + type versions
   - *Recommended Fix*: Use schema versioning + generic embedding column type

2. **Versioning Strategy**:
   - UUIDv7 proliferation risks storage bloat
   - *Suggested Alternative*: Content-addressable version chaining with merkle DAG

3. **Concurrency Model Risks**:
   - Mixing Tokio/Rayon executors creates deadlock potential
   - DashMap contention for write-heavy workloads
   - *Recommended*: Benchmark actor model vs. pure async approaches

4. **Testing Gaps**:
   - Insufficient graph traversal tests
   - No performance benchmarking strategy
   - *Suggested Addition*: Fuzz testing for graph operations

### B: **Debatable Points** (Require Further Analysis):

1. **Cryptographic Choices**:
   - Blake3 vs MD5 requires benchmark data
   - XChaCha20 for models needs threat modeling

2. **Crate Granularity**:
   - Multi-crate structure *could* aid long-term maintenance
   - *Suggested Analysis*: Compile-time impact study

3. **IDE Watcher Isolation**:
   - Separate crate makes sense for cross-platform support
   - *Recommended*: Validate with prototype

### C: **Missed Opportunities** in Original Review:

1. **Database Choice Implications**:
   - CozoDB's consistency guarantees vs operational needs
   - Migration strategy for schema changes

2. **Hardware Acceleration**:
   - CUDA/OpenCL flags need clear use case justification

3. **Security Tradeoffs**:
   - Merkle validation overhead vs trust requirements

### D: **Critical Questions Requiring Answers**:

1. What are the actual scale targets?
   - Nodes/year? Query latency SLA? Concurrency needs?

2. What operational constraints exist?
   - Self-hosted vs cloud? Hardware budgets?

3. What are the failure domain boundaries?
   - How crucial is real-time vs batch processing?

