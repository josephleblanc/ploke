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
- [ ] UTF-8 decoding errors
- [ ] Permission denied errors
- [x] Concurrency throttling verification
- [ ] Read during shutdown
- [ ] Send request during shutdown

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
