# Recent Changes

## 2025-07-05 20:21

 1 Consistent hash generation using PROJECT_NAMESPACE_UUID
 2 Proper UUID generation in concurrency test
 3 Added synchronization delays in shutdown tests
 4 Maintained all other existing fixes

## 2025-07-05 20:25

 1 Rename tracking_hash to include path parameter
 2 Add new tracking_hash_with_path helper
 3 Update tests to use actual file paths
 4 Maintain backwards compatibility for tests that don't need path-specific hashing

## 2025-07-05 21:57

 1 Added checkmarks for UTF-8 decoding errors and permission denied errors (both
   passing)
 2 Marked UTF-8 invalid sequences as covered
 3 Confirmed all shutdown-related tests are passing
 4 Added checkmarks for error conversion paths (Utf8 and FileOperation)
 5 Noted that large file (>1MB) test is still needed
 6 Identified parse error test as still missing
