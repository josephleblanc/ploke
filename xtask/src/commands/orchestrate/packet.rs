use std::fs;
use std::path::PathBuf;

use crate::commands::{CommandContext, XtaskError};

use super::{Board, Task, WorkerRole, WorkerSlot, resolve};

impl Board {
    pub(super) fn write_packet(
        &self,
        ctx: &CommandContext,
        worker_id: &str,
    ) -> Result<PathBuf, XtaskError> {
        let worker = self.workers.get(worker_id).expect("checked");
        let packet_dir = resolve(ctx, &self.packet_dir)?;
        fs::create_dir_all(&packet_dir)?;
        let path = packet_dir.join(format!("{worker_id}.md"));
        fs::write(&path, self.packet_body(worker))?;
        Ok(path)
    }

    fn packet_body(&self, worker: &WorkerSlot) -> String {
        let mut out = String::new();
        out.push_str(&format!("# Worker Packet: {}\n\n", worker.id));
        out.push_str(&format!("- role: {:?}\n", worker.role));
        if worker.role == WorkerRole::Retainer {
            out.push_str(&format!(
                "- refresh_after_questions: {}\n",
                worker.refresh_after_questions
            ));
        }
        out.push_str("\n## Active Task\n\n");
        match worker
            .active
            .as_ref()
            .and_then(|task_id| self.tasks.get(task_id))
        {
            Some(task) => push_task(&mut out, task),
            None => out.push_str("No active task assigned.\n"),
        }

        out.push_str("\n## Queued Tasks\n\n");
        if worker.queue.is_empty() {
            out.push_str("No queued tasks.\n");
        } else {
            for task_id in &worker.queue {
                if let Some(task) = self.tasks.get(task_id) {
                    push_task(&mut out, task);
                    out.push('\n');
                }
            }
        }

        out.push_str("\n## Report Contract\n\n");
        out.push_str("- Report changed files or `none`.\n");
        out.push_str("- Report verification commands and outcomes.\n");
        out.push_str("- Report blockers with exact file paths or evidence.\n");
        out.push_str("- Do not edit outside allowed surfaces.\n");
        out.push_str("- Do not touch forbidden files.\n");
        out
    }
}

fn push_task(out: &mut String, task: &Task) {
    out.push_str(&format!("### {}: {}\n\n", task.id, task.title));
    out.push_str(&format!("- lane: {}\n", task.lane));
    out.push_str(&format!("- priority: {}\n", task.priority));
    out.push_str(&format!("- state: {:?}\n", task.state));
    push_list(out, "allowed_edit", &task.allowed_edit);
    push_list(out, "forbidden_edit", &task.forbidden_edit);
    push_list(out, "docs", &task.docs);
    push_list(out, "acceptance", &task.acceptance);
    push_list(out, "blockers", &task.blockers);
}

fn push_list(out: &mut String, label: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    out.push_str(&format!("- {label}:\n"));
    for item in items {
        out.push_str(&format!("  - {item}\n"));
    }
}
