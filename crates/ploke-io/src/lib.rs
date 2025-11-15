#![allow(
    unused_variables,
    unused_imports,
    dead_code,
    clippy::duplicated_attributes
)]
//! ploke-io — Async I/O actor for safe, concurrent file operations
//!
//! ploke-io provides an actor-style I/O service that runs on its own Tokio runtime
//! in a dedicated thread. It focuses on correctness, safety, and backpressure while
//! supporting three core workflows:
//!
//! - Read snippets: UTF-8 safe byte slicing with per-request content-hash verification.
//! - Scan for changes: recompute file hashes and report changes deterministically.
//! - Write snippets: splice byte ranges with atomic temp-write + fsync + rename,
//!   serialized per-file via async locks (see feature notes below).
//!
//! Optional feature:
//! - watcher: broadcast debounced file events using notify via a background thread.
//!
//! Key properties
//! - Bounded concurrency based on FD heuristics or explicit builder configuration.
//! - Path policy enforcement: absolute-path requirement and root normalization
//!   with configurable symlink policy (DenyCrossRoot by default when roots are set).
//! - Clear error mapping into ploke_error types.
//!
//! Getting started
//! - Use IoManagerHandle::new() for sensible defaults, or the builder for configuration.
//! - Use ploke_core types for requests and results (EmbeddingData, FileData, WriteSnippetData, etc).
//!
//! Quick start (read snippets)
//! ```rust,ignore
//! use ploke_io::IoManagerHandle;
//! use ploke_core::{EmbeddingData, TrackingHash, PROJECT_NAMESPACE_UUID};
//! use uuid::Uuid;
//! use quote::ToTokens;
//!
//! // Prepare a file and compute its tracking hash from tokens.
//! let dir = tempfile::tempdir().unwrap();
//! let file = dir.path().join("ex.rs");
//! std::fs::write(&file, "fn demo() { let x = 1; }\n").unwrap();
//! let file_ast = syn::parse_file("fn demo() { let x = 1; }\n").unwrap();
//! let tokens = file_ast.into_token_stream();
//! let file_hash = TrackingHash::generate(PROJECT_NAMESPACE_UUID, &file, &tokens);
//!
//! // Read the "demo" identifier
//! let content = std::fs::read_to_string(&file).unwrap();
//! let start = content.find("demo").unwrap();
//! let end = start + "demo".len();
//!
//! let req = EmbeddingData {
//!     id: Uuid::new_v4(),
//!     name: "demo_fn".into(),
//!     file_path: file.clone(),
//!     file_tracking_hash: file_hash,
//!     start_byte: start,
//!     end_byte: end,
//!     node_tracking_hash: TrackingHash(Uuid::new_v4()),
//!     namespace: PROJECT_NAMESPACE_UUID,
//! };
//!
//! # tokio::runtime::Runtime::new().unwrap().block_on(async {
//! let handle = IoManagerHandle::new();
//! let results = handle.get_snippets_batch(vec![req]).await.unwrap();
//! assert_eq!(results.len(), 1);
//! assert_eq!(results[0].as_ref().unwrap(), "demo");
//! handle.shutdown().await;
//! # });
//! ```
//!
//! Scanning for changes
//! ```rust,ignore
//! use ploke_io::IoManagerHandle;
//! use ploke_core::{FileData, TrackingHash, PROJECT_NAMESPACE_UUID};
//! use quote::ToTokens;
//! use uuid::Uuid;
//!
//! let dir = tempfile::tempdir().unwrap();
//! let file = dir.path().join("s.rs");
//! let initial = "fn a() {}\n";
//! std::fs::write(&file, initial).unwrap();
//!
//! let tokens = syn::parse_file(initial).unwrap().into_token_stream();
//! let old_hash = TrackingHash::generate(PROJECT_NAMESPACE_UUID, &file, &tokens);
//!
//! let req = FileData {
//!     id: Uuid::new_v4(),
//!     namespace: PROJECT_NAMESPACE_UUID,
//!     file_tracking_hash: old_hash,
//!     file_path: file.clone(),
//! };
//!
//! // Change the file
//! std::fs::write(&file, "fn a() { let _y = 1; }\n").unwrap();
//!
//! # tokio::runtime::Runtime::new().unwrap().block_on(async {
//! let handle = IoManagerHandle::new();
//! let changed = handle.scan_changes_batch(vec![req]).await.unwrap().unwrap();
//! assert!(changed[0].is_some());
//! handle.shutdown().await;
//! # });
//! ```
//!
//! Writing snippets (atomic and serialized per file)
//! ```rust,ignore
//! use ploke_io::IoManagerHandle;
//! use ploke_core::{WriteSnippetData, TrackingHash, PROJECT_NAMESPACE_UUID};
//! use quote::ToTokens;
//! use uuid::Uuid;
//!
//! let dir = tempfile::tempdir().unwrap();
//! let file = dir.path().join("w.rs");
//! let initial = "fn hello() {}\n";
//! std::fs::write(&file, initial).unwrap();
//!
//! let tokens = syn::parse_file(initial).unwrap().into_token_stream();
//! let expected = TrackingHash::generate(PROJECT_NAMESPACE_UUID, &file, &tokens);
//! let start = initial.find("hello").unwrap();
//! let end = start + "hello".len();
//!
//! let req = WriteSnippetData {
//!     id: Uuid::new_v4(),
//!     name: "node".into(),
//!     file_path: file.clone(),
//!     expected_file_hash: expected,
//!     start_byte: start,
//!     end_byte: end,
//!     replacement: "goodbye".into(),
//!     namespace: PROJECT_NAMESPACE_UUID,
//! };
//!
//! # tokio::runtime::Runtime::new().unwrap().block_on(async {
//! let handle = IoManagerHandle::new();
//! let resp = handle.write_snippets_batch(vec![req]).await.unwrap();
//! assert!(resp[0].is_ok());
//! handle.shutdown().await;
//! # });
//! ```
//!
//! Configuration via builder
//! ```rust,ignore
//! use ploke_io::IoManagerHandle;
//! use ploke_io::path_policy::SymlinkPolicy;
//!
//! let handle = ploke_io::IoManagerHandle::builder()
//!     .with_fd_limit(64)                // bound concurrency
//!     .with_roots([std::env::current_dir().unwrap()])
//!     .with_symlink_policy(SymlinkPolicy::DenyCrossRoot)
//!     .build();
//! # futures::executor::block_on(async { handle.shutdown().await; });
//! ```
//!
//! Feature flags
//! - watcher: enable file system watcher integration and subscribe_file_events() API.
//!
//! Error model
//! - Channel/shutdown errors surface as IoError::Recv and map to ploke_error::Error::Internal.
//! - File, parse, range, and path policy violations surface as Fatal variants via mapping.
//!
//! See docs/production_plan.md for a full roadmap and design details.
use ploke_core::PROJECT_NAMESPACE_UUID;
mod actor;
pub use actor::IoManager;
use actor::IoManagerMessage;
use actor::IoRequest;
use builder::IoManagerBuilder;
mod builder;
pub mod errors;
pub use errors::IoError;
pub use errors::RecvError;
pub mod handle;
pub use handle::IoManagerHandle;
pub mod path_policy;
pub mod read;
pub mod scan;
#[cfg(feature = "watcher")]
pub mod watcher;
#[cfg(feature = "watcher")]
pub use watcher::{FileChangeEvent, FileEventKind};
mod create;
mod tests_skeleton;
mod write;
#[cfg(test)]
mod write_tests;
use futures::future::join_all;
use itertools::Itertools;
use ploke_core::ChangedFileData;
use ploke_core::EmbeddingData;
use ploke_core::FileData;
use ploke_core::TrackingHash;
use ploke_error::fatal::FatalError;
use ploke_error::Error as PlokeError;
use quote::ToTokens;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot, Semaphore};
