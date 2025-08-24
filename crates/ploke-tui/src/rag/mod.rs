pub mod tools;
pub mod context;
pub mod dispatcher;
pub mod search;
pub mod utils;
pub mod editing;

use std::sync::Arc;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AppEvent, EventBus, app_state::AppState, llm, system::SystemEvent};

