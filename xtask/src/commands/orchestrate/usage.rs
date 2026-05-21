use std::collections::BTreeMap;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::commands::{CommandContext, XtaskError};

use super::{BoardArg, display, now, resolve};

const USAGE_SCHEMA_VERSION: &str = "orchestrator-usage.v1";

/// Local command usage counters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct UsageLedger {
    /// Schema version.
    schema_version: String,
    /// Usage counters by command path.
    #[serde(default)]
    commands: BTreeMap<String, CommandUsage>,
}

/// Usage counter for one command path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandUsage {
    /// Number of recorded calls.
    pub(super) count: u64,
    /// Last recorded call time.
    pub(super) last_used_at: Option<String>,
}

/// Renderable usage summary.
#[derive(Debug, Clone, Serialize)]
pub struct UsageSummary {
    /// Usage metadata path.
    pub(super) path: String,
    /// Command counters.
    pub(super) commands: Vec<UsageCommandSummary>,
}

/// Renderable usage entry.
#[derive(Debug, Clone, Serialize)]
pub struct UsageCommandSummary {
    /// Command path.
    pub(super) command: String,
    /// Number of recorded calls.
    pub(super) count: u64,
    /// Last recorded call time.
    pub(super) last_used_at: Option<String>,
}

impl UsageLedger {
    pub(super) fn record_command_for_board(
        board_path: &Path,
        command: &str,
    ) -> Result<(), XtaskError> {
        let path = usage_path_for_board(board_path);
        let _lock = UsageLock::acquire(&path)?;
        let mut usage = Self::load_or_new(&path)?;
        usage.record(command);
        usage.save(&path)
    }

    pub(super) fn summary_for_board(
        ctx: &CommandContext,
        board_path: &Path,
    ) -> Result<UsageSummary, XtaskError> {
        let path = usage_path_for_board(board_path);
        let usage = Self::load_or_new(&path)?;
        usage.summary(ctx, &path)
    }

    fn new() -> Self {
        Self {
            schema_version: USAGE_SCHEMA_VERSION.to_string(),
            commands: BTreeMap::new(),
        }
    }

    fn load_or_new(path: &Path) -> Result<Self, XtaskError> {
        if path.exists() {
            Self::load(path)
        } else {
            Ok(Self::new())
        }
    }

    fn load(path: &Path) -> Result<Self, XtaskError> {
        let contents = fs::read_to_string(path)?;
        let usage: Self = serde_json::from_str(&contents)?;
        if usage.schema_version != USAGE_SCHEMA_VERSION {
            return Err(XtaskError::validation(format!(
                "Unsupported orchestrator usage schema `{}`",
                usage.schema_version
            ))
            .with_recovery(format!("Expected `{USAGE_SCHEMA_VERSION}`.")));
        }
        Ok(usage)
    }

    fn save(&self, path: &Path) -> Result<(), XtaskError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let body = serde_json::to_string_pretty(self)?;
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, format!("{body}\n"))?;
        fs::rename(tmp, path)?;
        Ok(())
    }

    fn record(&mut self, command: &str) {
        let entry = self
            .commands
            .entry(command.to_string())
            .or_insert(CommandUsage {
                count: 0,
                last_used_at: None,
            });
        entry.count += 1;
        entry.last_used_at = Some(now());
    }

    fn summary(&self, ctx: &CommandContext, path: &Path) -> Result<UsageSummary, XtaskError> {
        let commands = self
            .commands
            .iter()
            .map(|(command, usage)| UsageCommandSummary {
                command: command.clone(),
                count: usage.count,
                last_used_at: usage.last_used_at.clone(),
            })
            .collect();
        Ok(UsageSummary {
            path: display(ctx, path)?,
            commands,
        })
    }
}

pub(super) fn record_usage_best_effort(ctx: &CommandContext, board: &BoardArg, command: &str) {
    let Ok(board_path) = resolve(ctx, board.path()) else {
        return;
    };
    let _ = UsageLedger::record_command_for_board(&board_path, command);
}

pub(super) fn usage_summary(
    ctx: &CommandContext,
    board: &BoardArg,
) -> Result<UsageSummary, XtaskError> {
    let board_path = resolve(ctx, board.path())?;
    UsageLedger::summary_for_board(ctx, &board_path)
}

fn usage_path_for_board(board_path: &Path) -> PathBuf {
    board_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("usage.json")
}

struct UsageLock {
    path: PathBuf,
}

impl UsageLock {
    fn acquire(usage_path: &Path) -> Result<Self, XtaskError> {
        if let Some(parent) = usage_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let lock_path = usage_path.with_extension("lock");
        let started = Instant::now();
        loop {
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
            {
                Ok(mut file) => {
                    write!(file, "pid={}\n", std::process::id())?;
                    return Ok(Self { path: lock_path });
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    if started.elapsed() > Duration::from_secs(10) {
                        return Err(XtaskError::validation(format!(
                            "Timed out waiting for usage lock `{}`",
                            lock_path.display()
                        ))
                        .with_recovery(
                            "Check for a stale usage lock file if no xtask process is running.",
                        ));
                    }
                    thread::sleep(Duration::from_millis(50));
                }
                Err(err) => return Err(err.into()),
            }
        }
    }
}

impl Drop for UsageLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
