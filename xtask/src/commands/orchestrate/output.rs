use serde::Serialize;

use super::{
    Blocker, BoardStatus, BoundedStatus, LaneSpec, LaneValidation, Task, TaskSet, UsageSummary,
    WorkerSlot,
};

/// Command output for orchestration commands.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OrchestrateOutput {
    /// Board was initialized.
    Initialized {
        /// Board path.
        board: String,
        /// Packet directory.
        packet_dir: String,
    },
    /// Board status summary.
    Status(BoardStatus),
    /// Bounded routine board status.
    StatusBrief(BoundedStatus),
    /// Worker was added or updated.
    Worker {
        /// Worker slot after the change.
        worker: WorkerSlot,
    },
    /// Task was added.
    Task {
        /// Task after the change.
        task: Task,
    },
    /// Assignment changed.
    Assigned {
        /// Worker slot after assignment.
        worker: WorkerSlot,
    },
    /// Task completed.
    Completed {
        /// Task after completion.
        task: Task,
    },
    /// Task reviewed.
    Reviewed {
        /// Task after review.
        task: Task,
    },
    /// Blocker added.
    Blocked {
        /// Task after blocker attachment.
        task: Task,
        /// Newly created blocker.
        blocker: Blocker,
    },
    /// Worker packet was written.
    Packet {
        /// Worker id.
        worker: String,
        /// Packet path.
        path: String,
    },
    /// Lane was added or replaced.
    Lane {
        /// Lane definition after the change.
        lane: LaneSpec,
    },
    /// Lane validation result.
    LaneValidation {
        /// Validation result.
        validation: LaneValidation,
    },
    /// Task set was created.
    TaskSet {
        /// Task-set metadata.
        task_set: TaskSet,
    },
    /// Task-set membership changed.
    TaskSetMembership {
        /// Task after membership change.
        task: Task,
    },
    /// Task-set list.
    TaskSets {
        /// Known task sets.
        task_sets: Vec<TaskSet>,
    },
    /// Local usage counter summary.
    Usage(UsageSummary),
}
