//! Usage tracking for xtask commands.
//!
//! This module provides comprehensive usage tracking and analytics for xtask commands,
//! including:
//! - Command execution statistics
//! - JSONL persistence format
//! - Rolling suggestions (triggered every 50 runs)
//! - Statistics generation and reporting

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::XtaskError;

/// Tracks command usage for analytics and suggestions.
///
/// The tracker maintains an in-memory buffer of recent usage records
/// and persists them to a JSONL file for long-term storage.
pub struct UsageTracker {
    /// Path to the usage log file (JSONL format).
    log_path: PathBuf,

    /// In-memory buffer for recent usage records.
    buffer: Mutex<Vec<UsageRecord>>,

    /// Threshold for showing suggestions (every N runs).
    suggestion_threshold: usize,

    /// Last suggestion timestamp to prevent duplicate suggestions.
    last_suggestion: Mutex<Option<Instant>>,

    /// Buffer size threshold before flushing to disk.
    buffer_flush_threshold: usize,
}

/// A single usage record representing one command execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    /// When the command was executed.
    pub timestamp: DateTime<Utc>,

    /// Name of the command that was executed.
    pub command_name: String,

    /// Duration of execution in milliseconds.
    pub duration_ms: u64,

    /// Whether the command succeeded.
    pub success: bool,

    /// Exit code if available (0 for success, non-zero for failure).
    pub exit_code: Option<i32>,
}

/// Marker for the start of a command execution.
///
/// Used to calculate duration when recording completion.
#[derive(Debug)]
pub struct UsageStart {
    /// Name of the command being executed.
    pub command_name: String,

    /// When the command started.
    pub timestamp: DateTime<Utc>,
}

/// Usage statistics report aggregating multiple records.
#[derive(Debug, Clone, Default)]
pub struct UsageStats {
    /// Total number of commands executed.
    pub total_commands: usize,

    /// Number of successful commands.
    pub total_success: usize,

    /// Number of failed commands.
    pub total_failure: usize,

    /// Per-command statistics.
    pub command_breakdown: std::collections::HashMap<String, CommandStats>,

    /// Average duration across all commands (in milliseconds).
    pub average_duration_ms: f64,
}

/// Statistics for a single command type.
#[derive(Debug, Clone, Default)]
pub struct CommandStats {
    /// Total executions of this command.
    pub count: usize,

    /// Successful executions.
    pub success_count: usize,

    /// Failed executions.
    pub failure_count: usize,

    /// Average duration in milliseconds.
    pub average_duration_ms: f64,

    /// When this command was last used.
    pub last_used: DateTime<Utc>,
}

impl UsageTracker {
    /// Create a new usage tracker.
    ///
    /// # Arguments
    /// * `log_path` - Optional path for the usage log file. If not provided,
    ///   defaults to the platform's data directory.
    ///
    /// # Errors
    /// Returns an error if the log directory cannot be created.
    pub fn new(log_path: Option<PathBuf>) -> Result<Self, XtaskError> {
        let log_path = log_path.unwrap_or_else(default_usage_log_path);

        // Ensure directory exists
        if let Some(parent) = log_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                XtaskError::new(format!("Failed to create usage log directory: {e}"))
            })?;
        }

        Ok(Self {
            log_path,
            buffer: Mutex::new(Vec::new()),
            suggestion_threshold: 50, // As per requirements: every 50 runs
            last_suggestion: Mutex::new(None),
            buffer_flush_threshold: 10, // Flush every 10 records
        })
    }

    /// Create a new usage tracker with a custom suggestion threshold.
    ///
    /// This is primarily useful for testing.
    #[cfg(test)]
pub fn with_threshold(log_path: Option<PathBuf>, threshold: usize) -> Result<Self, XtaskError> {
        let mut tracker = Self::new(log_path)?;
        tracker.suggestion_threshold = threshold;
        Ok(tracker)
    }

    /// Record the start of a command execution.
    ///
    /// Returns a `UsageStart` marker that should be passed to `record_completion`.
    pub fn record_start(&self, command_name: &str) -> UsageStart {
        UsageStart {
            command_name: command_name.to_string(),
            timestamp: Utc::now(),
        }
    }

    /// Record the completion of a command execution.
    ///
    /// # Arguments
    /// * `start` - The start marker returned by `record_start`
    /// * `success` - Whether the command succeeded
    pub fn record_completion(&self, start: UsageStart, success: bool) {
        let duration = Utc::now().signed_duration_since(start.timestamp);

        let record = UsageRecord {
            timestamp: start.timestamp,
            command_name: start.command_name,
            duration_ms: duration.num_milliseconds() as u64,
            success,
            exit_code: if success { Some(0) } else { Some(1) },
        };

        // Buffer the record
        {
            let mut buffer = self.buffer.lock().unwrap();
            buffer.push(record);

            // Flush if buffer is large enough
            if buffer.len() >= self.buffer_flush_threshold {
                let records_to_flush: Vec<_> = buffer.drain(..).collect();
                drop(buffer); // Release lock before I/O
                self.flush_buffer(&records_to_flush);
            }
        }
    }

    /// Flush the in-memory buffer to disk.
    ///
    /// This is called automatically when the buffer reaches its threshold,
    /// but can also be called manually for explicit persistence.
    pub fn flush(&self) {
        let records: Vec<_> = {
            let mut buffer = self.buffer.lock().unwrap();
            if buffer.is_empty() {
                return;
            }
            buffer.drain(..).collect()
        };
        self.flush_buffer(&records);
    }

    /// Internal method to write records to the log file.
    fn flush_buffer(&self, records: &[UsageRecord]) {
        if records.is_empty() {
            return;
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path);

        match file {
            Ok(mut file) => {
                for record in records {
                    if let Ok(json) = serde_json::to_string(record) {
                        if writeln!(file, "{}", json).is_err() {
                            tracing::warn!("Failed to write usage record to log");
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to open usage log file: {}", e);
            }
        }
    }

    /// Get the total number of commands recorded.
    ///
    /// This includes both buffered and persisted records.
    pub fn total_command_count(&self) -> Result<usize, XtaskError> {
        let buffered_count = self.buffer.lock().unwrap().len();

        let file_count = match File::open(&self.log_path) {
            Ok(file) => {
                let reader = BufReader::new(file);
                reader.lines().count()
            }
            Err(_) => 0, // File might not exist yet
        };

        Ok(buffered_count + file_count)
    }

    /// Check if we should show a suggestion.
    ///
    /// Suggestions are triggered every `suggestion_threshold` runs,
    /// but only once per threshold period to avoid being annoying.
    pub fn should_show_suggestion(&self) -> bool {
        let count = self.total_command_count().unwrap_or(0);

        if count > 0 && count % self.suggestion_threshold == 0 {
            let last = *self.last_suggestion.lock().unwrap();
            // Only show once per threshold (with 60-second cooldown)
            last.map(|t| t.elapsed() > std::time::Duration::from_secs(60))
                .unwrap_or(true)
        } else {
            false
        }
    }

    /// Show the rolling suggestion.
    ///
    /// This updates the last suggestion timestamp and prints the suggestion.
    pub fn show_suggestion(&self) {
        *self.last_suggestion.lock().unwrap() = Some(Instant::now());

        eprintln!("\n💡 Auto-generated suggestion:");
        eprintln!("   Have feedback or suggestions for xtask commands?");
        eprintln!(
            "   Check {} and don't forget to be honest!\n",
            self.feedback_file_path().display()
        );
    }

    /// Get the path to the feedback file.
    fn feedback_file_path(&self) -> PathBuf {
        self.log_path.with_file_name("xtask_feedback.md")
    }

    /// Generate a comprehensive usage statistics report.
    ///
    /// This aggregates all persisted usage records into statistics.
    pub fn generate_stats(&self) -> Result<UsageStats, XtaskError> {
        let file = File::open(&self.log_path)?;
        let reader = BufReader::new(file);

        let mut total_commands = 0usize;
        let mut total_success = 0usize;
        let mut total_duration = 0u64;
        let mut command_stats: std::collections::HashMap<String, CommandStats> =
            std::collections::HashMap::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            if let Ok(record) = serde_json::from_str::<UsageRecord>(&line) {
                total_commands += 1;
                if record.success {
                    total_success += 1;
                }
                total_duration += record.duration_ms;

                let stats = command_stats
                    .entry(record.command_name.clone())
                    .or_insert_with(|| CommandStats {
                        count: 0,
                        success_count: 0,
                        failure_count: 0,
                        average_duration_ms: 0.0,
                        last_used: record.timestamp,
                    });

                stats.count += 1;
                if record.success {
                    stats.success_count += 1;
                } else {
                    stats.failure_count += 1;
                }

                // Rolling average calculation
                stats.average_duration_ms = (stats.average_duration_ms * (stats.count - 1) as f64
                    + record.duration_ms as f64)
                    / stats.count as f64;

                if record.timestamp > stats.last_used {
                    stats.last_used = record.timestamp;
                }
            }
        }

        Ok(UsageStats {
            total_commands,
            total_success,
            total_failure: total_commands - total_success,
            command_breakdown: command_stats,
            average_duration_ms: if total_commands > 0 {
                total_duration as f64 / total_commands as f64
            } else {
                0.0
            },
        })
    }

    /// Get the path to the usage log file.
    pub fn log_path(&self) -> &PathBuf {
        &self.log_path
    }

    /// Clear all usage records (for testing).
    #[cfg(test)]
    pub fn clear(&self) -> Result<(), XtaskError> {
        self.buffer.lock().unwrap().clear();
        if self.log_path.exists() {
            fs::remove_file(&self.log_path)?;
        }
        Ok(())
    }
}

impl Drop for UsageTracker {
    fn drop(&mut self) {
        // Ensure all buffered records are persisted on drop
        self.flush();
    }
}

/// Get the default path for the usage log file.
///
/// Uses the platform's data directory:
/// - Linux: `~/.local/share/ploke/xtask_usage.jsonl`
/// - macOS: `~/Library/Application Support/ploke/xtask_usage.jsonl`
/// - Windows: `%APPDATA%/ploke/xtask_usage.jsonl`
///
/// Falls back to `./xtask_usage.jsonl` if the data directory cannot be determined.
fn default_usage_log_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ploke")
        .join("xtask_usage.jsonl")
}

/// Get a summary of recent usage for display.
///
/// Returns a formatted string showing recent command usage patterns.
pub fn format_usage_summary(stats: &UsageStats) -> String {
    let mut output = String::new();

    output.push_str(&format!("Total commands: {}\n", stats.total_commands));
    output.push_str(&format!(
        "Success rate: {:.1}%\n",
        if stats.total_commands > 0 {
            (stats.total_success as f64 / stats.total_commands as f64) * 100.0
        } else {
            0.0
        }
    ));
    output.push_str(&format!(
        "Average duration: {:.1}ms\n",
        stats.average_duration_ms
    ));

    if !stats.command_breakdown.is_empty() {
        output.push_str("\nCommand breakdown:\n");

        // Sort commands by usage count (descending)
        let mut commands: Vec<_> = stats.command_breakdown.iter().collect();
        commands.sort_by(|a, b| b.1.count.cmp(&a.1.count));

        for (name, cmd_stats) in commands {
            output.push_str(&format!(
                "  {}: {} runs ({} ok, {} fail)\n",
                name, cmd_stats.count, cmd_stats.success_count, cmd_stats.failure_count
            ));
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_tracker() -> (UsageTracker, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("usage.jsonl");
        let tracker = UsageTracker::new(Some(log_path)).unwrap();
        (tracker, temp_dir)
    }

    #[test]
    fn test_usage_tracker_new() {
        let (tracker, _temp) = create_test_tracker();
        assert_eq!(tracker.total_command_count().unwrap(), 0);
    }

    #[test]
    fn test_record_start_completion() {
        let (tracker, _temp) = create_test_tracker();

        let start = tracker.record_start("test-command");
        assert_eq!(start.command_name, "test-command");

        // Simulate some work
        std::thread::sleep(std::time::Duration::from_millis(10));

        tracker.record_completion(start, true);

        // Should have one buffered record
        assert_eq!(tracker.buffer.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_flush_buffer() {
        let (tracker, temp) = create_test_tracker();

        // Add records
        for i in 0..5 {
            let start = tracker.record_start(&format!("cmd-{}", i));
            tracker.record_completion(start, true);
        }

        // Manually flush
        tracker.flush();

        // Buffer should be empty
        assert!(tracker.buffer.lock().unwrap().is_empty());

        // File should exist and have content
        let log_path = temp.path().join("usage.jsonl");
        assert!(log_path.exists());

        let content = fs::read_to_string(&log_path).unwrap();
        assert_eq!(content.lines().count(), 5);
    }

    #[test]
    fn test_generate_stats() {
        let (tracker, _temp) = create_test_tracker();

        // Create some test records
        for i in 0..10 {
            let start = tracker.record_start("test-cmd");
            tracker.record_completion(start, i < 8); // 8 success, 2 failure
        }

        // Flush to ensure records are persisted
        tracker.flush();

        let stats = tracker.generate_stats().unwrap();
        assert_eq!(stats.total_commands, 10);
        assert_eq!(stats.total_success, 8);
        assert_eq!(stats.total_failure, 2);
        assert!(stats.command_breakdown.contains_key("test-cmd"));
    }

    #[test]
    fn test_should_show_suggestion() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("usage.jsonl");

        // Create tracker with threshold of 5 for testing
        let tracker = UsageTracker::with_threshold(Some(log_path), 5).unwrap();

        // Should not show suggestion initially
        assert!(!tracker.should_show_suggestion());

        // Add 4 records
        for _ in 0..4 {
            let start = tracker.record_start("cmd");
            tracker.record_completion(start, true);
        }

        // Still should not show (not at threshold)
        assert!(!tracker.should_show_suggestion());

        // Add 5th record (now at threshold)
        let start = tracker.record_start("cmd");
        tracker.record_completion(start, true);
        tracker.flush();

        // Should show suggestion
        assert!(tracker.should_show_suggestion());

        // After showing, should not show again immediately
        tracker.show_suggestion();
        assert!(!tracker.should_show_suggestion());
    }

    #[test]
    fn test_usage_record_serialization() {
        let record = UsageRecord {
            timestamp: Utc::now(),
            command_name: "test".to_string(),
            duration_ms: 100,
            success: true,
            exit_code: Some(0),
        };

        let json = serde_json::to_string(&record).unwrap();
        let deserialized: UsageRecord = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.command_name, "test");
        assert_eq!(deserialized.duration_ms, 100);
        assert!(deserialized.success);
    }

    #[test]
    fn test_format_usage_summary() {
        let stats = UsageStats {
            total_commands: 10,
            total_success: 8,
            total_failure: 2,
            command_breakdown: {
                let mut map = std::collections::HashMap::new();
                map.insert(
                    "test-cmd".to_string(),
                    CommandStats {
                        count: 10,
                        success_count: 8,
                        failure_count: 2,
                        average_duration_ms: 50.0,
                        last_used: Utc::now(),
                    },
                );
                map
            },
            average_duration_ms: 50.0,
        };

        let summary = format_usage_summary(&stats);
        assert!(summary.contains("Total commands: 10"));
        assert!(summary.contains("Success rate: 80.0%"));
        assert!(summary.contains("test-cmd: 10 runs"));
    }

    #[test]
    fn test_default_usage_log_path() {
        let path = default_usage_log_path();
        assert!(path.to_string_lossy().contains("xtask_usage.jsonl"));
        assert!(path.to_string_lossy().contains("ploke"));
    }

    #[test]
    fn test_feedback_file_path() {
        let (tracker, _temp) = create_test_tracker();
        let feedback_path = tracker.feedback_file_path();
        assert_eq!(feedback_path.file_name().unwrap(), "xtask_feedback.md");
    }

    #[test]
    fn test_usage_start_creation() {
        let start = UsageStart {
            command_name: "my-cmd".to_string(),
            timestamp: Utc::now(),
        };
        assert_eq!(start.command_name, "my-cmd");
    }
}
