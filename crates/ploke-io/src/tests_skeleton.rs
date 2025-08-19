// Test inventory to guide migration (comment-only)

// Read Path Tests:
// - test_get_snippets_batch_preserves_order
// - test_content_mismatch
// - test_io_errors
// - test_concurrency_throttling
// - test_seek_errors
// - test_zero_length_snippet
// - test_partial_failure_handling
// - test_concurrent_modification
// - test_utf8_error
// - test_zero_byte_files
// - test_multi_byte_unicode_boundaries
// - test_invalid_byte_ranges
// - test_exact_semaphore_limit
// - test_permission_denied
// - test_large_file_snippet_extraction
// - test_mixed_batch_hash_mismatch_per_request
// - test_parse_error_invalid_rust
// - test_reject_relative_path
// - test_roots_enforcement_basic (shared with path policy)

// Scan Path Tests:
// - test_scan_changes_preserves_input_order
// - test_scan_changes_bounded_concurrency

// Builder/Config Tests:
// - test_fd_limit_precedence_and_clamp
// - test_fd_limit_env_applied_when_no_builder
// - test_fd_limit_default_from_soft
// - test_fd_limit_default_on_error

// Actor/Runtime Tests:
// - test_actor_shutdown_during_ops
// - test_read_during_shutdown
// - test_send_during_shutdown
// - test_handle_read_snippet_batch
// - test_handle_request (ignored)

// Misc/Hashing Semantics:
// - test_token_stream_sensitivity
