//! File with invalid use statements.
//! Useful for testing error handling.

// DELIBERATELY INVALID RUST SYNTAX - FOR ERROR HANDLING TESTS ONLY

// 1. Empty path segments (invalid)
use ::::invalid;

// 2. Unterminated glob import (invalid)
use std::collections::*;

// 3. Malformed rename (invalid)
use std::fmt as;
