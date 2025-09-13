//! RAG handlers and tool integrations for the TUI layer.
//!
//! This module provides asynchronous handlers that:
//! - Dispatch LLM tool calls and return results via the EventBus as System events
//!   (ToolCallCompleted / ToolCallFailed) and SysInfo chat messages.
//! - Stage source-code edits as proposals (including human-readable previews) and
//!   apply/deny them on user command.
//! - Run sparse (BM25), dense, and hybrid search via an optional RagService stored
//!   on AppState, and surface results as chat messages.
//! - Assemble a prompt context from retrieved snippets and the current conversation,
//!   emitting a constructed prompt event for the LLM subsystem.
//!
//! Notable characteristics:
//! - Handlers accept Arc<AppState> and Arc<EventBus) and communicate by sending AppEvent
//!   instances via the EventBus (realtime/background). No implicit global state is modified,
//!   aside from fields within AppState accessed through provided references.
//! - When required services or inputs are missing, functions emit SysInfo chat messages and/or
//!   ToolCallFailed events instead of panicking.
//! - IO and database operations are delegated to other subsystems (IoManager, Database, RagService);
//!   this module validates inputs, constructs requests, and forwards results as events/messages.
//!
//! Context in the larger project:
//! - Events and system integration types come from the crate root (AppEvent, SystemEvent, llm::Event).
//! - The module is invoked by the state manager/dispatcher and other handlers; it does not own
//!   background loops itself. It relies on the EventBus for communicating with the UI and LLM manager.

pub mod context;
pub mod editing;
pub mod search;
pub mod tools;
pub mod utils;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AppEvent, EventBus,
    app_state::{AppState, events::SystemEvent},
};
