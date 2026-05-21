use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::commands::{CommandContext, XtaskError};

use super::{Board, BoardArg, BoardLock, OrchestrateOutput, resolve};

/// Lane ownership commands.
#[derive(Debug, Clone, clap::Subcommand)]
pub enum LaneCommand {
    /// Add or replace a lane-owned edit surface group.
    Set(SetLane),
    /// Validate lane surfaces and task edit surfaces.
    Validate(ValidateLanes),
}

impl LaneCommand {
    pub(super) fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
        match self {
            Self::Set(cmd) => cmd.execute(ctx),
            Self::Validate(cmd) => cmd.execute(ctx),
        }
    }

    pub(super) fn board_arg(&self) -> &BoardArg {
        match self {
            Self::Set(cmd) => &cmd.board,
            Self::Validate(cmd) => &cmd.board,
        }
    }

    pub(super) fn usage_key(&self) -> &'static str {
        match self {
            Self::Set(_) => "lane set",
            Self::Validate(_) => "lane validate",
        }
    }
}

/// Add or replace a lane definition.
#[derive(Debug, Clone, clap::Args)]
pub struct SetLane {
    #[command(flatten)]
    board: BoardArg,
    /// Lane id, usually matching task `--lane`.
    id: String,
    /// Owned edit surface for this lane. May be repeated. Omit for read-only coordination lanes.
    #[arg(long = "own")]
    owned_edit: Vec<String>,
    /// Context document for this lane. May be repeated.
    #[arg(long = "doc")]
    docs: Vec<String>,
    /// Short note about the lane boundary. May be repeated.
    #[arg(long = "note")]
    notes: Vec<String>,
}

/// Validate lane ownership boundaries.
#[derive(Debug, Clone, clap::Args)]
pub struct ValidateLanes {
    #[command(flatten)]
    board: BoardArg,
}

/// Lane-owned edit surface group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaneSpec {
    /// Lane id, usually matching task `lane`.
    pub(super) id: String,
    /// Owned edit surfaces. Empty for read-only coordination lanes.
    pub(super) owned_edit: Vec<String>,
    /// Lane context documents.
    pub(super) docs: Vec<String>,
    /// Boundary notes.
    pub(super) notes: Vec<String>,
}

/// Validation result for lane ownership.
#[derive(Debug, Clone, Serialize)]
pub struct LaneValidation {
    /// Whether no overlaps or task ownership issues were found.
    ok: bool,
    /// Lane-to-lane ownership overlaps.
    overlaps: Vec<LaneOverlap>,
    /// Task edit surfaces that do not match lane ownership.
    task_issues: Vec<TaskLaneIssue>,
}

/// Overlap between two lane-owned surfaces.
#[derive(Debug, Clone, Serialize)]
pub struct LaneOverlap {
    /// First lane id.
    lane_a: String,
    /// First surface.
    surface_a: String,
    /// Second lane id.
    lane_b: String,
    /// Second surface.
    surface_b: String,
}

/// Task edit surface issue relative to lane ownership.
#[derive(Debug, Clone, Serialize)]
pub struct TaskLaneIssue {
    /// Task id.
    task: String,
    /// Task lane.
    task_lane: String,
    /// Edit surface from the task, or empty when the issue is lane-level.
    surface: String,
    /// Issue kind.
    kind: TaskLaneIssueKind,
    /// Matching lane ids.
    matching_lanes: Vec<String>,
}

/// Task lane validation issue kind.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskLaneIssueKind {
    /// Task lane has not been defined.
    UndefinedTaskLane,
    /// No lane owns or overlaps the task edit surface.
    NoLaneOwner,
    /// More than one lane overlaps the task edit surface.
    MultipleLaneOwners,
    /// A different lane owns the edit surface.
    TaskLaneMismatch,
}

impl SetLane {
    fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
        let path = resolve(ctx, &self.board.board)?;
        let _lock = BoardLock::acquire(&path)?;
        let mut board = Board::load_or_new(&path)?;
        let lane = LaneSpec {
            id: self.id.clone(),
            owned_edit: self.owned_edit.clone(),
            docs: self.docs.clone(),
            notes: self.notes.clone(),
        };
        board.lanes.insert(lane.id.clone(), lane.clone());
        board.record(format!("lane {} set", lane.id));
        board.save(&path)?;
        Ok(OrchestrateOutput::Lane { lane })
    }
}

impl ValidateLanes {
    fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
        let path = resolve(ctx, &self.board.board)?;
        let board = Board::load(&path)?;
        Ok(OrchestrateOutput::LaneValidation {
            validation: board.validate_lanes(ctx)?,
        })
    }
}

impl Board {
    pub(super) fn validate_lanes(
        &self,
        ctx: &CommandContext,
    ) -> Result<LaneValidation, XtaskError> {
        let mut overlaps = Vec::new();
        let lanes: Vec<_> = self.lanes.values().collect();
        for (left_index, left) in lanes.iter().enumerate() {
            for right in lanes.iter().skip(left_index + 1) {
                for left_surface in &left.owned_edit {
                    for right_surface in &right.owned_edit {
                        if surfaces_overlap(ctx, left_surface, right_surface)? {
                            overlaps.push(LaneOverlap {
                                lane_a: left.id.clone(),
                                surface_a: left_surface.clone(),
                                lane_b: right.id.clone(),
                                surface_b: right_surface.clone(),
                            });
                        }
                    }
                }
            }
        }

        let mut task_issues = Vec::new();
        for task in self.tasks.values() {
            let lane_defined = self.lanes.contains_key(&task.lane);
            if !lane_defined {
                task_issues.push(TaskLaneIssue {
                    task: task.id.clone(),
                    task_lane: task.lane.clone(),
                    surface: String::new(),
                    kind: TaskLaneIssueKind::UndefinedTaskLane,
                    matching_lanes: Vec::new(),
                });
                continue;
            }

            for surface in &task.allowed_edit {
                let matching_lanes = self.matching_lanes(ctx, surface)?;
                let kind = if matching_lanes.is_empty() {
                    Some(TaskLaneIssueKind::NoLaneOwner)
                } else if matching_lanes.len() > 1 {
                    Some(TaskLaneIssueKind::MultipleLaneOwners)
                } else if !matching_lanes.iter().any(|lane| lane == &task.lane) {
                    Some(TaskLaneIssueKind::TaskLaneMismatch)
                } else {
                    None
                };

                if let Some(kind) = kind {
                    task_issues.push(TaskLaneIssue {
                        task: task.id.clone(),
                        task_lane: task.lane.clone(),
                        surface: surface.clone(),
                        kind,
                        matching_lanes,
                    });
                }
            }
        }

        Ok(LaneValidation {
            ok: overlaps.is_empty() && task_issues.is_empty(),
            overlaps,
            task_issues,
        })
    }

    fn matching_lanes(
        &self,
        ctx: &CommandContext,
        surface: &str,
    ) -> Result<Vec<String>, XtaskError> {
        let mut matches = Vec::new();
        for lane in self.lanes.values() {
            let mut overlaps = false;
            for owned in &lane.owned_edit {
                if surfaces_overlap(ctx, owned, surface)? {
                    overlaps = true;
                    break;
                }
            }
            if overlaps {
                matches.push(lane.id.clone());
            }
        }
        Ok(matches)
    }
}

fn surfaces_overlap(ctx: &CommandContext, left: &str, right: &str) -> Result<bool, XtaskError> {
    let left = surface_components(ctx, left)?;
    let right = surface_components(ctx, right)?;
    Ok(component_prefix(&left, &right) || component_prefix(&right, &left))
}

fn component_prefix(prefix: &[String], path: &[String]) -> bool {
    prefix.len() <= path.len() && prefix.iter().zip(path).all(|(left, right)| left == right)
}

fn surface_components(ctx: &CommandContext, raw: &str) -> Result<Vec<String>, XtaskError> {
    let root = ctx.workspace_root()?;
    let path = Path::new(raw);
    let path = if path.is_absolute() {
        path.strip_prefix(root).unwrap_or(path).to_path_buf()
    } else {
        path.to_path_buf()
    };

    Ok(path
        .components()
        .filter_map(|component| match component {
            std::path::Component::CurDir => None,
            std::path::Component::Normal(part) => Some(part.to_string_lossy().to_string()),
            std::path::Component::ParentDir => Some("..".to_string()),
            std::path::Component::RootDir => Some("/".to_string()),
            std::path::Component::Prefix(prefix) => {
                Some(prefix.as_os_str().to_string_lossy().to_string())
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use tempfile::TempDir;

    use super::super::{Board, Task, TaskState};
    use super::*;
    use crate::context::CommandContext;

    fn test_context(temp: &TempDir) -> CommandContext {
        CommandContext::new_with_workspace_root(Some(temp.path().to_path_buf()))
            .expect("test workspace root")
    }

    fn board_with_task(task: Task) -> Board {
        Board {
            schema_version: "orchestrator-board.v1".to_string(),
            created_at: "2026-05-12T00:00:00Z".to_string(),
            updated_at: "2026-05-12T00:00:00Z".to_string(),
            packet_dir: PathBuf::from(".orchestrator/workers"),
            lanes: BTreeMap::new(),
            workers: BTreeMap::new(),
            tasks: BTreeMap::from([(task.id.clone(), task)]),
            blockers: BTreeMap::new(),
            task_sets: BTreeMap::new(),
            events: Vec::new(),
        }
    }

    fn task(id: &str, lane: &str, allowed_edit: Vec<&str>) -> Task {
        Task {
            id: id.to_string(),
            lane: lane.to_string(),
            title: "test task".to_string(),
            priority: 1,
            state: TaskState::NotStarted,
            created_at: "2026-05-12T00:00:00Z".to_string(),
            updated_at: "2026-05-12T00:00:00Z".to_string(),
            assigned_at: None,
            activated_at: None,
            completed_at: None,
            reviewed_at: None,
            blocked_at: None,
            unblocked_at: None,
            allowed_edit: allowed_edit.into_iter().map(str::to_string).collect(),
            forbidden_edit: Vec::new(),
            docs: Vec::new(),
            acceptance: Vec::new(),
            reports: Vec::new(),
            blockers: Vec::new(),
            task_sets: Vec::new(),
        }
    }

    fn lane(id: &str, owned_edit: Vec<&str>) -> LaneSpec {
        LaneSpec {
            id: id.to_string(),
            owned_edit: owned_edit.into_iter().map(str::to_string).collect(),
            docs: Vec::new(),
            notes: Vec::new(),
        }
    }

    #[test]
    fn read_only_task_with_undefined_lane_is_reported() {
        let temp = TempDir::new().expect("tempdir");
        let ctx = test_context(&temp);
        let board = board_with_task(task("retainer-map", "retainer", Vec::new()));

        let validation = board.validate_lanes(&ctx).expect("lane validation");

        assert!(!validation.ok);
        assert_eq!(validation.task_issues.len(), 1);
        assert_eq!(validation.task_issues[0].task, "retainer-map");
        assert_eq!(validation.task_issues[0].task_lane, "retainer");
        assert!(validation.task_issues[0].surface.is_empty());
        assert!(matches!(
            validation.task_issues[0].kind,
            TaskLaneIssueKind::UndefinedTaskLane
        ));
    }

    #[test]
    fn read_only_lane_without_owned_surfaces_validates() {
        let temp = TempDir::new().expect("tempdir");
        let ctx = test_context(&temp);
        let mut board = board_with_task(task("retainer-map", "retainer", Vec::new()));
        board
            .lanes
            .insert("retainer".to_string(), lane("retainer", Vec::new()));

        let validation = board.validate_lanes(&ctx).expect("lane validation");

        assert!(validation.ok);
        assert!(validation.overlaps.is_empty());
        assert!(validation.task_issues.is_empty());
    }

    #[test]
    fn task_edit_surface_must_belong_to_task_lane() {
        let temp = TempDir::new().expect("tempdir");
        let ctx = test_context(&temp);
        let mut board = board_with_task(task(
            "graph-task",
            "loader",
            vec!["crates/ploke-tree/src/graph"],
        ));
        board.lanes.insert(
            "graph".to_string(),
            lane("graph", vec!["crates/ploke-tree/src/graph"]),
        );
        board.lanes.insert(
            "loader".to_string(),
            lane("loader", vec!["crates/ploke-tree/src/store"]),
        );

        let validation = board.validate_lanes(&ctx).expect("lane validation");

        assert!(!validation.ok);
        assert_eq!(validation.task_issues.len(), 1);
        assert!(matches!(
            validation.task_issues[0].kind,
            TaskLaneIssueKind::TaskLaneMismatch
        ));
        assert_eq!(validation.task_issues[0].matching_lanes, vec!["graph"]);
    }
}
