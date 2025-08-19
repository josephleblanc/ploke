#![allow(unused_variables, unused_imports, dead_code)]
//! # ploke-io
//!
//! `ploke-io` provides a non-blocking I/O actor system for reading
//! file snippets concurrently. It is designed for applications that need to read from
//! many files without blocking.
//!
//! ## Core Components
//!
//! The crate is built around a few key components:
//!
//! - **`IoManagerHandle`**: The public-facing API and the primary entry point for this crate.
//!   It provides a simple, asynchronous interface to the I/O actor. It is a lightweight
//!   handle that can be cloned and shared across threads.
//!
//! - **`IoManager`**: The internal actor that runs in a dedicated background thread. It listens
//!   for requests, manages a pool of file handles, and executes file operations.
//!
//! - **`EmbeddingData`**: A struct that defines a request to read a specific byte range
//!   (a "snippet") from a file. It includes data integrity checks to ensure that the
//!   file content has not changed since it was indexed.
//!
//! ## Runtime Management
//!
//! The `IoManager` runs its own `tokio` runtime on a dedicated OS thread. This design
//! choice offers several advantages:
//!
//! 1.  **Isolation**: I/O operations are completely isolated from the caller's execution
//!     context. This is crucial for applications with their own async runtimes (like a GUI
//!     or a web server), as it prevents I/O-intensive work from blocking the main event loop.
//! 2.  **Dedicated Resources**: The I/O actor has its own set of resources, including a scheduler
//!     and a thread pool, which can be optimized for file operations.
//! 3.  **Simplified API**: Callers do not need to manage the lifecycle of the I/O runtime.
//!     They simply create an `IoManagerHandle` and start sending requests.
//!
//! The `IoManagerHandle::new()` function spawns a new OS thread and initializes a
//! `tokio::runtime::Builder` with `new_current_thread` and `enable_all`. This creates a
//! single-threaded runtime that is efficient for managing a queue of I/O tasks.
//!
//! ## Usage Example
//!
//! Here's how to use `ploke-io` to read snippets from multiple files:
//! TODO: Create example

mod actor;
use builder::IoManagerBuilder;
pub use actor::IoManager;
use actor::IoManagerMessage;
use actor::IoRequest;
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
mod tests_skeleton;
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
