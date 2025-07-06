# Ploke-IO Test Coverage Plan

## Assessment Summary

### Strong Areas
- Batch snippet retrieval with order preservation
- Content validation with tracking hashes
- File grouping and concurrency handling
- Basic I/O error handling (missing files, wrong paths)
- Byte range validation (out-of-bound checks)
- Partial failure handling

### Weak Areas
- Semaphore acquisition failures
- Runtime initialization failures
- Shutdown paths without operations
- Permission-related I/O errors
- Zero file/no file edge cases
- Multi-byte Unicode handling
- Exact semaphore limit handling
- File handle lifetime management
- Error conversion paths

## Test Plan Checklist

### Core Functionality
- [x] Basic file read \<100KB
- [x] Basic file read \>1MB
- [x] Zero-length snippet handling
- [x] Multiple snippets on single file
- [ ] Empty batch handling

### Error Handling
- [x] Content changed detection
- [x] File not found errors
- [x] Byte range exceeds file size
- [x] Parse errors
- [x] UTF-8 decoding errors
- [x] Permission denied errors
- [x] Concurrency throttling verification
- [x] Read during shutdown
- [x] Send request during shutdown

### Edge Cases
- [ ] Zero-byte files
- [ ] Files with invalid UTF-8 sequences
- [ ] Files with multi-byte Unicode sequences
- [ ] Requests with start_byte > end_byte
- [ ] End byte exactly at EOF

### Infrastructure
- [ ] Runtime initialization failure
- [ ] Semaphore acquisition failure
- [ ] Empty file groups processing
- [ ] Shutdown with no active operations
- [x] Early shutdown during heavy operations
- [ ] Exactly semaphore-limit concurrent files

### Error Conversion Paths
- [x] IoError::Recv conversion
- [x] IoError::ContentMismatch conversion
- [x] IoError::OutOfRange conversion
- [ ] IoError::Utf8 conversion
- [ ] IoError::FileOperation conversion

## Recommended Actions

1. Add `rstest` for parameterized testing
```toml
[rdev-dependencies]
rstest = "0.18"
```

2. Fix ignored tests by providing proper `id` fields
3. Generate test data programmatically using `rand`
4. Add explicit UTf-8 decoding tests
5. Test permission errors using temporary inaccessible files

## Questions

1. Should we add `rstest` for parameterized testing?
2. What specific hash consistency requirements exist?
3. What file descriptor limit thresholds should we validate?
=======
# Ploke-IO Test Coverage Plan

## Assessment Summary

### Strong Areas
- Order-preserving batch snippet retrieval
- Content validation via token-based tracking hashes
- File grouping and concurrency optimization
- Basic I/O error handling (missing files, invalid paths)
- Byte range validation and error handling
- Partial failure handling in mixed success/failure batches

### Weak Areas
- Semaphore acquisition edge cases
- Runtime initialization failures
- Permission-related I/O errors
- Zero-file/no-file edge cases
- UTF-8 decoding and Unicode handling
- Cross-platform hash consistency
- Controlled shutdown scenarios
- Semaphore limit validation
- Token stream sensitivity verification

## Test Plan Checklist

### Core Functionality
- [x] Basic file read <100KB
- [ ] Basic file read >1MB  <!-- Still needs test -->
- [x] Zero-length snippet handling
- [x] Multiple snippets on single file
- [ ] Empty batch handling  <!-- Still needs test -->

### Error Handling
- [x] Content changed detection
- [x] File not found errors
- [x] Byte range exceeds file size
- [ ] Parse errors  <!-- Still needs test -->
- [x] UTF-8 decoding errors
- [x] Permission denied errors
- [x] Concurrency throttling verification
- [x] Read during shutdown
- [x] Send request during shutdown

### Edge Cases
- [ ] Zero-byte files  <!-- Still needs test -->
- [x] Files with invalid UTF-8 sequences  <!-- Added checkmark -->
- [ ] Files with multi-byte Unicode sequences  <!-- Still needs test -->
- [ ] Requests with start_byte > end_byte  <!-- Still needs test -->
- [ ] End byte exactly at EOF  <!-- Still needs test -->

### Infrastructure
- [ ] Runtime initialization failure  <!-- Still needs test -->
- [ ] Semaphore acquisition failure  <!-- Still needs test -->
- [ ] Empty file groups processing  <!-- Still needs test -->
- [ ] Shutdown with no active operations  <!-- Still needs test -->
- [x] Early shutdown during heavy operations
- [ ] Exactly semaphore-limit concurrent files  <!-- Still needs test -->

### Error Conversion Paths
- [x] IoError::Recv conversion
- [x] IoError::ContentMismatch conversion
- [x] IoError::OutOfRange conversion
- [x] IoError::Utf8 conversion  <!-- Added checkmark -->
- [x] IoError::FileOperation conversion  <!-- Added checkmark -->

## Recommended Technologies

```toml
[dev-dependencies]
rstest = "0.18"    # Parameterized testing
mockall = "0.12"   # Mock system calls/I/O services
proptest = "1.4.0" # Property-based tests (optional)
filetime = "0.2"    # Modify file metadata for cache tests
```

## Action Plan

### Phase 1: Fix Existing Tests
1. ~~Provide valid `id` fields in `EmbeddingNode` using real UUIDs~~ (Done)
2. ~~Implement mock content generation with `rand`~~ (Done)
3. **Redesign order preservation test** to handle NodeId properly
   
### Phase 2: Parameterized Tests (`rstest`)
- Core functionality variations
- Multi-byte Unicode edge cases
- Permission denial scenarios

### Phase 3: Mocking (`mockall`)
- Simulate file permission errors
- Mock `getrlimit()` failures/configurations
- Simulate UTF-8 decoding failures

### Phase 4: System-Level Testing
- Actual permission denial tests
- File handle leak testing
- Cross-platform canonicalization tests

## File Descriptor Validation Targets

| Scenario | Soft Limit | Expected Slots |
|----------|------------|----------------|
| High availability | 300 | 100 |
| Moderate availability | 246 | 82 |
| Low availability | 30 | 10 |
| Error case | N/A | 50 |

## Hash Verification Plan
1. Verify identical token streams → identical UUIDv5
2. Verify comment-only changes → same token stream → unchanged hash
3. Verify addition of functional token → hash change
4. Verify same token streams + different paths → different hashes
5. Verify same tokens + path + different namespaces → different hashes
