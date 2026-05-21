use std::fs;
use std::path::PathBuf;

use super::task_sets;
use super::unblock;
use super::*;

fn temp_ctx() -> (tempfile::TempDir, CommandContext) {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = CommandContext::new_with_workspace_root(Some(dir.path().to_path_buf()))
        .expect("temp workspace context");
    (dir, ctx)
}

fn board_arg() -> BoardArg {
    BoardArg {
        board: PathBuf::from(DEFAULT_BOARD_PATH),
    }
}

#[test]
fn complete_summary_writes_report_file_and_attaches_path() {
    let (dir, ctx) = temp_ctx();

    Init {
        board: board_arg(),
        packet_dir: PathBuf::from(DEFAULT_PACKET_DIR),
    }
    .execute(&ctx)
    .expect("init board");
    WorkerCommand {
        board: board_arg(),
        id: "worker-a".to_string(),
        role: WorkerRole::Worker,
        refresh_after_questions: 5,
    }
    .execute(&ctx)
    .expect("add worker");
    AddTask {
        board: board_arg(),
        id: "task/a".to_string(),
        lane: "tooling".to_string(),
        title: "Improve complete ergonomics".to_string(),
        priority: 3,
        allowed_edit: vec!["xtask/src/commands/orchestrate.rs".to_string()],
        forbidden_edit: Vec::new(),
        docs: Vec::new(),
        acceptance: Vec::new(),
    }
    .execute(&ctx)
    .expect("add task");
    Assign {
        board: board_arg(),
        task: "task/a".to_string(),
        worker: "worker-a".to_string(),
        active: true,
    }
    .execute(&ctx)
    .expect("assign task");

    let output = Complete {
        board: board_arg(),
        task: "task/a".to_string(),
        report: None,
        summary: Some("Changed files: xtask/src/commands/orchestrate.rs".to_string()),
    }
    .execute(&ctx)
    .expect("complete with summary");

    let OrchestrateOutput::Completed { task } = output else {
        panic!("expected completed output");
    };
    assert_eq!(task.reports.len(), 1);
    assert!(task.reports[0].starts_with(".orchestrator/reports/task_a-complete-"));

    let report = fs::read_to_string(dir.path().join(&task.reports[0])).expect("summary report");
    assert!(report.contains("# Orchestrator complete Summary"));
    assert!(report.contains("- task: task/a"));
    assert!(report.contains("Changed files: xtask/src/commands/orchestrate.rs"));
}

#[test]
fn usage_command_counts_orchestrate_command_paths() {
    let (_dir, ctx) = temp_ctx();

    Orchestrate::Init(Init {
        board: board_arg(),
        packet_dir: PathBuf::from(DEFAULT_PACKET_DIR),
    })
    .execute(&ctx)
    .expect("init board");
    Orchestrate::Status(Status {
        board: board_arg(),
        brief: false,
        task_set: None,
    })
    .execute(&ctx)
    .expect("status board");

    let output = Orchestrate::Usage(Usage { board: board_arg() })
        .execute(&ctx)
        .expect("usage summary");

    let OrchestrateOutput::Usage(summary) = output else {
        panic!("expected usage output");
    };
    assert_eq!(summary.path, ".orchestrator/usage.json");
    assert_eq!(usage_count(&summary, "init"), Some(1));
    assert_eq!(usage_count(&summary, "status"), Some(1));
    assert_eq!(usage_count(&summary, "usage"), Some(1));
}

#[test]
fn usage_is_best_effort_and_records_status_set_after_success() {
    let (dir, ctx) = temp_ctx();

    Init {
        board: board_arg(),
        packet_dir: PathBuf::from(DEFAULT_PACKET_DIR),
    }
    .execute(&ctx)
    .expect("init board");
    fs::write(dir.path().join(".orchestrator/usage.json"), "not json\n").expect("write bad usage");

    let output = Orchestrate::Status(Status {
        board: board_arg(),
        brief: true,
        task_set: None,
    })
    .execute(&ctx)
    .expect("status ignores bad usage metadata");
    assert!(matches!(output, OrchestrateOutput::StatusBrief(_)));

    fs::remove_file(dir.path().join(".orchestrator/usage.json")).expect("remove bad usage");
    Orchestrate::TaskSet(task_sets::TaskSetCommand::Create(
        task_sets::CreateTaskSet {
            board: board_arg(),
            id: "current-thread".to_string(),
            description: None,
        },
    ))
    .execute(&ctx)
    .expect("create task set");
    let failed = Orchestrate::Status(Status {
        board: board_arg(),
        brief: true,
        task_set: Some("missing-set".to_string()),
    })
    .execute(&ctx);
    assert!(failed.is_err());
    Orchestrate::Status(Status {
        board: board_arg(),
        brief: true,
        task_set: Some("current-thread".to_string()),
    })
    .execute(&ctx)
    .expect("status set");

    let output = Orchestrate::Usage(Usage { board: board_arg() })
        .execute(&ctx)
        .expect("usage summary");
    let OrchestrateOutput::Usage(summary) = output else {
        panic!("expected usage output");
    };
    assert_eq!(usage_count(&summary, "status --set"), Some(1));
    assert_eq!(usage_count(&summary, "status --brief"), None);
}

#[test]
fn status_brief_returns_bounded_projection() {
    let (_dir, ctx) = temp_ctx();

    Orchestrate::Init(Init {
        board: board_arg(),
        packet_dir: PathBuf::from(DEFAULT_PACKET_DIR),
    })
    .execute(&ctx)
    .expect("init board");
    Orchestrate::Worker(WorkerCommand {
        board: board_arg(),
        id: "worker-a".to_string(),
        role: WorkerRole::Worker,
        refresh_after_questions: 5,
    })
    .execute(&ctx)
    .expect("add worker");
    Orchestrate::Add(AddTask {
        board: board_arg(),
        id: "status/brief".to_string(),
        lane: "tooling".to_string(),
        title: "Add bounded status".to_string(),
        priority: 3,
        allowed_edit: vec!["xtask/src/commands/orchestrate".to_string()],
        forbidden_edit: Vec::new(),
        docs: Vec::new(),
        acceptance: Vec::new(),
    })
    .execute(&ctx)
    .expect("add task");
    Orchestrate::Assign(Assign {
        board: board_arg(),
        task: "status/brief".to_string(),
        worker: "worker-a".to_string(),
        active: true,
    })
    .execute(&ctx)
    .expect("assign task");

    let output = Orchestrate::Status(Status {
        board: board_arg(),
        brief: true,
        task_set: None,
    })
    .execute(&ctx)
    .expect("brief status");

    let OrchestrateOutput::StatusBrief(status) = output else {
        panic!("expected brief status output");
    };
    assert_eq!(status.board, ".orchestrator/board.json");
    assert_eq!(status.blockers, 0);
    assert_eq!(status.workers.len(), 1);
    assert_eq!(status.workers[0].id, "worker-a");
    assert_eq!(status.workers[0].active.as_deref(), Some("status/brief"));
    assert_eq!(status.workers[0].queue_len, 0);
    assert_eq!(task_state_count(&status, "assigned_active"), Some(1));
}

#[test]
fn status_set_filters_to_task_set_members() {
    let (_dir, ctx) = temp_ctx();

    Orchestrate::Init(Init {
        board: board_arg(),
        packet_dir: PathBuf::from(DEFAULT_PACKET_DIR),
    })
    .execute(&ctx)
    .expect("init board");
    Orchestrate::Add(AddTask {
        board: board_arg(),
        id: "task/current".to_string(),
        lane: "tooling".to_string(),
        title: "Current thread task".to_string(),
        priority: 3,
        allowed_edit: vec!["xtask/src/commands/orchestrate".to_string()],
        forbidden_edit: Vec::new(),
        docs: Vec::new(),
        acceptance: Vec::new(),
    })
    .execute(&ctx)
    .expect("add current task");
    Orchestrate::Add(AddTask {
        board: board_arg(),
        id: "task/other".to_string(),
        lane: "tooling".to_string(),
        title: "Other thread task".to_string(),
        priority: 3,
        allowed_edit: vec!["docs/active/agents".to_string()],
        forbidden_edit: Vec::new(),
        docs: Vec::new(),
        acceptance: Vec::new(),
    })
    .execute(&ctx)
    .expect("add other task");
    Orchestrate::TaskSet(task_sets::TaskSetCommand::Create(
        task_sets::CreateTaskSet {
            board: board_arg(),
            id: "current-thread".to_string(),
            description: Some("current thread".to_string()),
        },
    ))
    .execute(&ctx)
    .expect("create task set");
    Orchestrate::TaskSet(task_sets::TaskSetCommand::Add(task_sets::AddTaskToSet {
        board: board_arg(),
        set: "current-thread".to_string(),
        task: "task/current".to_string(),
    }))
    .execute(&ctx)
    .expect("add task to set");

    let output = Orchestrate::Status(Status {
        board: board_arg(),
        brief: true,
        task_set: Some("current-thread".to_string()),
    })
    .execute(&ctx)
    .expect("filtered status");

    let OrchestrateOutput::StatusBrief(status) = output else {
        panic!("expected brief status output");
    };
    assert_eq!(status.task_set.as_deref(), Some("current-thread"));
    assert_eq!(task_state_count(&status, "not_started"), Some(1));
}

#[test]
fn status_set_preserves_worker_occupancy_outside_filter() {
    let (_dir, ctx) = temp_ctx();

    Orchestrate::Init(Init {
        board: board_arg(),
        packet_dir: PathBuf::from(DEFAULT_PACKET_DIR),
    })
    .execute(&ctx)
    .expect("init board");
    Orchestrate::Worker(WorkerCommand {
        board: board_arg(),
        id: "worker-a".to_string(),
        role: WorkerRole::Worker,
        refresh_after_questions: 5,
    })
    .execute(&ctx)
    .expect("add worker");
    Orchestrate::Add(AddTask {
        board: board_arg(),
        id: "task/current".to_string(),
        lane: "tooling".to_string(),
        title: "Current thread task".to_string(),
        priority: 3,
        allowed_edit: vec!["xtask/src/commands/orchestrate".to_string()],
        forbidden_edit: Vec::new(),
        docs: Vec::new(),
        acceptance: Vec::new(),
    })
    .execute(&ctx)
    .expect("add current task");
    Orchestrate::Add(AddTask {
        board: board_arg(),
        id: "task/other".to_string(),
        lane: "tooling".to_string(),
        title: "Other thread task".to_string(),
        priority: 3,
        allowed_edit: vec!["docs/active/agents".to_string()],
        forbidden_edit: Vec::new(),
        docs: Vec::new(),
        acceptance: Vec::new(),
    })
    .execute(&ctx)
    .expect("add other task");
    Orchestrate::Assign(Assign {
        board: board_arg(),
        task: "task/other".to_string(),
        worker: "worker-a".to_string(),
        active: true,
    })
    .execute(&ctx)
    .expect("assign outside-set task");
    Orchestrate::TaskSet(task_sets::TaskSetCommand::Create(
        task_sets::CreateTaskSet {
            board: board_arg(),
            id: "current-thread".to_string(),
            description: None,
        },
    ))
    .execute(&ctx)
    .expect("create task set");
    Orchestrate::TaskSet(task_sets::TaskSetCommand::Add(task_sets::AddTaskToSet {
        board: board_arg(),
        set: "current-thread".to_string(),
        task: "task/current".to_string(),
    }))
    .execute(&ctx)
    .expect("add task to set");

    let output = Orchestrate::Status(Status {
        board: board_arg(),
        brief: true,
        task_set: Some("current-thread".to_string()),
    })
    .execute(&ctx)
    .expect("filtered status");
    let OrchestrateOutput::StatusBrief(status) = output else {
        panic!("expected brief status output");
    };
    assert_eq!(status.workers.len(), 1);
    assert_eq!(status.workers[0].active.as_deref(), Some("task/other"));
    assert_eq!(status.workers[0].active_in_filter, Some(false));
    assert_eq!(status.workers[0].queue_len, 0);
    assert_eq!(status.workers[0].queue_in_filter, Some(0));
    assert_eq!(task_state_count(&status, "not_started"), Some(1));
    assert_eq!(task_state_count(&status, "assigned_active"), None);
}

#[test]
fn unblock_resolves_blocker_and_restores_task_state() {
    let (_dir, ctx) = temp_ctx();

    Orchestrate::Init(Init {
        board: board_arg(),
        packet_dir: PathBuf::from(DEFAULT_PACKET_DIR),
    })
    .execute(&ctx)
    .expect("init board");
    Orchestrate::Add(AddTask {
        board: board_arg(),
        id: "blocked/task".to_string(),
        lane: "tooling".to_string(),
        title: "Blocked task".to_string(),
        priority: 3,
        allowed_edit: vec!["xtask/src/commands/orchestrate".to_string()],
        forbidden_edit: Vec::new(),
        docs: Vec::new(),
        acceptance: Vec::new(),
    })
    .execute(&ctx)
    .expect("add task");
    Orchestrate::Block(Block {
        board: board_arg(),
        task: "blocked/task".to_string(),
        id: "blocker-a".to_string(),
        kind: BlockerKind::Dependency,
        summary: "waiting on dependency".to_string(),
        evidence: Vec::new(),
        unblock: Some("dependency done".to_string()),
    })
    .execute(&ctx)
    .expect("block task");

    let output = Orchestrate::Unblock(Unblock {
        board: board_arg(),
        blocker: "blocker-a".to_string(),
        summary: "dependency done".to_string(),
        evidence: vec!["test evidence".to_string()],
        next: unblock::UnblockNext::NotStarted,
        worker: None,
    })
    .execute(&ctx)
    .expect("unblock task");

    let OrchestrateOutput::Unblocked { task, blocker } = output else {
        panic!("expected unblock output");
    };
    assert!(matches!(task.state, TaskState::NotStarted));
    assert!(task.blockers.is_empty());
    assert!(blocker.resolved_at.is_some());
    assert_eq!(
        blocker.resolution_summary.as_deref(),
        Some("dependency done")
    );

    let output = Orchestrate::Status(Status {
        board: board_arg(),
        brief: true,
        task_set: None,
    })
    .execute(&ctx)
    .expect("brief status");
    let OrchestrateOutput::StatusBrief(status) = output else {
        panic!("expected brief status output");
    };
    assert_eq!(status.blockers, 0);
}

#[test]
fn lifecycle_rejects_review_before_complete_and_assign_while_blocked() {
    let (_dir, ctx) = temp_ctx();

    Orchestrate::Init(Init {
        board: board_arg(),
        packet_dir: PathBuf::from(DEFAULT_PACKET_DIR),
    })
    .execute(&ctx)
    .expect("init board");
    Orchestrate::Worker(WorkerCommand {
        board: board_arg(),
        id: "worker-a".to_string(),
        role: WorkerRole::Worker,
        refresh_after_questions: 5,
    })
    .execute(&ctx)
    .expect("add worker");
    Orchestrate::Add(AddTask {
        board: board_arg(),
        id: "blocked/task".to_string(),
        lane: "tooling".to_string(),
        title: "Blocked task".to_string(),
        priority: 3,
        allowed_edit: vec!["xtask/src/commands/orchestrate".to_string()],
        forbidden_edit: Vec::new(),
        docs: Vec::new(),
        acceptance: Vec::new(),
    })
    .execute(&ctx)
    .expect("add task");

    let review = Orchestrate::Review(Review {
        board: board_arg(),
        task: "blocked/task".to_string(),
        report: None,
    })
    .execute(&ctx);
    assert!(review.is_err());

    Orchestrate::Block(Block {
        board: board_arg(),
        task: "blocked/task".to_string(),
        id: "blocker-a".to_string(),
        kind: BlockerKind::Dependency,
        summary: "waiting".to_string(),
        evidence: Vec::new(),
        unblock: None,
    })
    .execute(&ctx)
    .expect("block task");

    let assign = Orchestrate::Assign(Assign {
        board: board_arg(),
        task: "blocked/task".to_string(),
        worker: "worker-a".to_string(),
        active: true,
    })
    .execute(&ctx);
    assert!(assign.is_err());
    let complete = Orchestrate::Complete(Complete {
        board: board_arg(),
        task: "blocked/task".to_string(),
        report: None,
        summary: None,
    })
    .execute(&ctx);
    assert!(complete.is_err());
}

#[test]
fn unblock_keeps_task_blocked_until_all_open_blockers_resolve() {
    let (_dir, ctx) = temp_ctx();

    Orchestrate::Init(Init {
        board: board_arg(),
        packet_dir: PathBuf::from(DEFAULT_PACKET_DIR),
    })
    .execute(&ctx)
    .expect("init board");
    Orchestrate::Add(AddTask {
        board: board_arg(),
        id: "blocked/task".to_string(),
        lane: "tooling".to_string(),
        title: "Blocked task".to_string(),
        priority: 3,
        allowed_edit: vec!["xtask/src/commands/orchestrate".to_string()],
        forbidden_edit: Vec::new(),
        docs: Vec::new(),
        acceptance: Vec::new(),
    })
    .execute(&ctx)
    .expect("add task");
    for blocker_id in ["blocker-a", "blocker-b"] {
        Orchestrate::Block(Block {
            board: board_arg(),
            task: "blocked/task".to_string(),
            id: blocker_id.to_string(),
            kind: BlockerKind::Dependency,
            summary: "waiting".to_string(),
            evidence: Vec::new(),
            unblock: None,
        })
        .execute(&ctx)
        .expect("block task");
    }

    let output = Orchestrate::Unblock(Unblock {
        board: board_arg(),
        blocker: "blocker-a".to_string(),
        summary: "first done".to_string(),
        evidence: Vec::new(),
        next: unblock::UnblockNext::NotStarted,
        worker: None,
    })
    .execute(&ctx)
    .expect("unblock first blocker");
    let OrchestrateOutput::Unblocked { task, .. } = output else {
        panic!("expected unblock output");
    };
    assert!(matches!(
        task.state,
        TaskState::Blocked { ref blocker } if blocker == "blocker-b"
    ));
    assert_eq!(task.blockers, vec!["blocker-b"]);
}

#[test]
fn lifecycle_transitions_record_task_timestamps() {
    let (dir, ctx) = temp_ctx();

    Orchestrate::Init(Init {
        board: board_arg(),
        packet_dir: PathBuf::from(DEFAULT_PACKET_DIR),
    })
    .execute(&ctx)
    .expect("init board");
    Orchestrate::Worker(WorkerCommand {
        board: board_arg(),
        id: "worker-a".to_string(),
        role: WorkerRole::Worker,
        refresh_after_questions: 5,
    })
    .execute(&ctx)
    .expect("add worker");
    let output = Orchestrate::Add(AddTask {
        board: board_arg(),
        id: "timed/task".to_string(),
        lane: "tooling".to_string(),
        title: "Timed task".to_string(),
        priority: 3,
        allowed_edit: vec!["xtask/src/commands/orchestrate".to_string()],
        forbidden_edit: Vec::new(),
        docs: Vec::new(),
        acceptance: Vec::new(),
    })
    .execute(&ctx)
    .expect("add task");
    let OrchestrateOutput::Task { task } = output else {
        panic!("expected task output");
    };
    assert_eq!(task.created_at, task.updated_at);
    assert!(task.assigned_at.is_none());

    Orchestrate::Assign(Assign {
        board: board_arg(),
        task: "timed/task".to_string(),
        worker: "worker-a".to_string(),
        active: true,
    })
    .execute(&ctx)
    .expect("assign task");
    let board = Board::load(&dir.path().join(DEFAULT_BOARD_PATH)).expect("load board");
    let task = board.tasks.get("timed/task").expect("task");
    assert!(task.assigned_at.is_some());
    assert!(task.activated_at.is_some());

    let output = Orchestrate::Complete(Complete {
        board: board_arg(),
        task: "timed/task".to_string(),
        report: None,
        summary: None,
    })
    .execute(&ctx)
    .expect("complete task");
    let OrchestrateOutput::Completed { task } = output else {
        panic!("expected completed output");
    };
    assert!(task.completed_at.is_some());

    let output = Orchestrate::Review(Review {
        board: board_arg(),
        task: "timed/task".to_string(),
        report: None,
    })
    .execute(&ctx)
    .expect("review task");
    let OrchestrateOutput::Reviewed { task } = output else {
        panic!("expected reviewed output");
    };
    assert!(task.reviewed_at.is_some());
}

#[test]
fn status_warns_about_missing_and_stale_worker_packets() {
    let (_dir, ctx) = temp_ctx();

    Orchestrate::Init(Init {
        board: board_arg(),
        packet_dir: PathBuf::from(DEFAULT_PACKET_DIR),
    })
    .execute(&ctx)
    .expect("init board");
    Orchestrate::Worker(WorkerCommand {
        board: board_arg(),
        id: "worker-a".to_string(),
        role: WorkerRole::Worker,
        refresh_after_questions: 5,
    })
    .execute(&ctx)
    .expect("add worker");
    for task_id in ["timed/active", "timed/queued"] {
        Orchestrate::Add(AddTask {
            board: board_arg(),
            id: task_id.to_string(),
            lane: "tooling".to_string(),
            title: "Timed task".to_string(),
            priority: 3,
            allowed_edit: vec!["xtask/src/commands/orchestrate".to_string()],
            forbidden_edit: Vec::new(),
            docs: Vec::new(),
            acceptance: Vec::new(),
        })
        .execute(&ctx)
        .expect("add task");
    }
    Orchestrate::Assign(Assign {
        board: board_arg(),
        task: "timed/active".to_string(),
        worker: "worker-a".to_string(),
        active: true,
    })
    .execute(&ctx)
    .expect("assign active task");

    let output = Orchestrate::Status(Status {
        board: board_arg(),
        brief: true,
        task_set: None,
    })
    .execute(&ctx)
    .expect("brief status");
    let OrchestrateOutput::StatusBrief(status) = output else {
        panic!("expected brief status");
    };
    assert!(has_warning(&status, "missing_worker_packet", "worker-a"));

    Orchestrate::Packet(Packet {
        board: board_arg(),
        worker: "worker-a".to_string(),
    })
    .execute(&ctx)
    .expect("generate packet");
    let output = Orchestrate::Status(Status {
        board: board_arg(),
        brief: true,
        task_set: None,
    })
    .execute(&ctx)
    .expect("brief status");
    let OrchestrateOutput::StatusBrief(status) = output else {
        panic!("expected brief status");
    };
    assert!(!has_warning(&status, "missing_worker_packet", "worker-a"));
    assert!(!has_warning(&status, "stale_worker_packet", "worker-a"));
    assert!(status.workers[0].packet_generated_at.is_some());

    Orchestrate::Assign(Assign {
        board: board_arg(),
        task: "timed/queued".to_string(),
        worker: "worker-a".to_string(),
        active: false,
    })
    .execute(&ctx)
    .expect("queue task after packet");
    let output = Orchestrate::Status(Status {
        board: board_arg(),
        brief: true,
        task_set: None,
    })
    .execute(&ctx)
    .expect("brief status");
    let OrchestrateOutput::StatusBrief(status) = output else {
        panic!("expected brief status");
    };
    assert!(has_warning(&status, "stale_worker_packet", "worker-a"));
}

#[test]
fn status_warns_about_stale_active_and_review_tasks() {
    let (dir, ctx) = temp_ctx();
    let board_path = dir.path().join(DEFAULT_BOARD_PATH);
    let old = "2026-05-19T00:00:00Z".to_string();

    Orchestrate::Init(Init {
        board: board_arg(),
        packet_dir: PathBuf::from(DEFAULT_PACKET_DIR),
    })
    .execute(&ctx)
    .expect("init board");
    Orchestrate::Worker(WorkerCommand {
        board: board_arg(),
        id: "worker-a".to_string(),
        role: WorkerRole::Worker,
        refresh_after_questions: 5,
    })
    .execute(&ctx)
    .expect("add worker");
    Orchestrate::Add(AddTask {
        board: board_arg(),
        id: "timed/task".to_string(),
        lane: "tooling".to_string(),
        title: "Timed task".to_string(),
        priority: 3,
        allowed_edit: vec!["xtask/src/commands/orchestrate".to_string()],
        forbidden_edit: Vec::new(),
        docs: Vec::new(),
        acceptance: Vec::new(),
    })
    .execute(&ctx)
    .expect("add task");
    Orchestrate::Assign(Assign {
        board: board_arg(),
        task: "timed/task".to_string(),
        worker: "worker-a".to_string(),
        active: true,
    })
    .execute(&ctx)
    .expect("assign task");
    {
        let mut board = Board::load(&board_path).expect("load board");
        let task = board.tasks.get_mut("timed/task").expect("task");
        task.activated_at = Some(old.clone());
        task.updated_at = old.clone();
        board.save(&board_path).expect("save board");
    }
    let output = Orchestrate::Status(Status {
        board: board_arg(),
        brief: true,
        task_set: None,
    })
    .execute(&ctx)
    .expect("brief status");
    let OrchestrateOutput::StatusBrief(status) = output else {
        panic!("expected brief status");
    };
    assert!(has_warning(&status, "stale_active_task", "timed/task"));

    Orchestrate::Complete(Complete {
        board: board_arg(),
        task: "timed/task".to_string(),
        report: None,
        summary: None,
    })
    .execute(&ctx)
    .expect("complete task");
    {
        let mut board = Board::load(&board_path).expect("load board");
        let task = board.tasks.get_mut("timed/task").expect("task");
        task.completed_at = Some(old.clone());
        task.updated_at = old;
        board.save(&board_path).expect("save board");
    }
    let output = Orchestrate::Status(Status {
        board: board_arg(),
        brief: true,
        task_set: None,
    })
    .execute(&ctx)
    .expect("brief status");
    let OrchestrateOutput::StatusBrief(status) = output else {
        panic!("expected brief status");
    };
    assert!(has_warning(&status, "stale_review_task", "timed/task"));
}

fn task_state_count(status: &BoundedStatus, state: &str) -> Option<usize> {
    status
        .tasks_by_state
        .iter()
        .find(|entry| entry.state == state)
        .map(|entry| entry.count)
}

fn has_warning(status: &BoundedStatus, kind: &str, subject: &str) -> bool {
    status
        .warnings
        .iter()
        .any(|warning| warning.kind == kind && warning.subject == subject)
}

fn usage_count(summary: &UsageSummary, command: &str) -> Option<u64> {
    summary
        .commands
        .iter()
        .find(|entry| entry.command == command)
        .map(|entry| entry.count)
}
