// This module contains tests specifically for the Phase 1 Discovery logic
// introduced as part of the UUID refactor. These tests should only run
// when the `uuid_ids` feature is enabled.

#![cfg(feature = "uuid_ids")]

mod discovery_tests;
// Add other test files within this module if needed
