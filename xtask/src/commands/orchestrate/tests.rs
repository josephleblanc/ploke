use std::fs;
use std::path::PathBuf;

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

fn task_state_count(status: &BoundedStatus, state: &str) -> Option<usize> {
    status
        .tasks_by_state
        .iter()
        .find(|entry| entry.state == state)
        .map(|entry| entry.count)
}

fn usage_count(summary: &UsageSummary, command: &str) -> Option<u64> {
    summary
        .commands
        .iter()
        .find(|entry| entry.command == command)
        .map(|entry| entry.count)
}
