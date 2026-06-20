//! Human-readable typestate shape metadata derived from alias declarations.
//!
//! The walk debugger needs to print `Runtime<...>` shapes and axis deltas for
//! humans. The metadata here is intentionally structural: alias declarations use
//! `stringify!` once at the same site that defines the real type alias, and the
//! walk UI renders/diffs those axis strings instead of maintaining a second copy
//! of each `Runtime<...>` alias.

use std::collections::BTreeSet;

const TYPE_LINE_LIMIT: usize = 72;

/// Stringified top-level axes of one `Runtime<...>` typestate alias.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeShape {
    pub(crate) phase: &'static str,
    pub(crate) role: &'static str,
    pub(crate) context: &'static str,
    pub(crate) plan: &'static str,
    pub(crate) children: &'static str,
    pub(crate) history: &'static str,
    pub(crate) evidence: &'static str,
    pub(crate) continuation: &'static str,
    pub(crate) report: &'static str,
}

/// One changed top-level `Runtime<...>` axis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeAxisDelta {
    pub(crate) label: &'static str,
    pub(crate) from: String,
    pub(crate) to: String,
}

#[derive(Debug, Clone, Copy)]
struct AxisShape {
    label: &'static str,
    value: &'static str,
    multiline: bool,
}

impl RuntimeAxisDelta {
    /// Type expressions present in the previous axis value but absent now.
    pub(crate) fn removed_structures(&self) -> Vec<String> {
        structure_diff(&self.from, &self.to)
    }

    /// Type expressions present in the current axis value but absent before.
    pub(crate) fn added_structures(&self) -> Vec<String> {
        structure_diff(&self.to, &self.from)
    }
}

impl RuntimeShape {
    /// Render this shape as the familiar `Runtime<...>;` block.
    pub(crate) fn render(self) -> String {
        let mut lines = vec!["Runtime<".to_string()];
        for axis in self.axes() {
            if axis.multiline {
                lines.extend(render_multiline_axis(axis.value, 4));
            } else {
                lines.push(format!("    {},", compact_type(axis.value)));
            }
        }
        lines.push(">;".to_string());
        lines.join("\n")
    }

    /// Return changed axes between two shapes.
    pub(crate) fn axis_deltas_from(self, from: RuntimeShape) -> Vec<RuntimeAxisDelta> {
        self.axes()
            .into_iter()
            .zip(from.axes())
            .filter_map(|(to, from)| {
                let from_value = compact_type(from.value);
                let to_value = compact_type(to.value);
                if from_value == to_value {
                    None
                } else {
                    Some(RuntimeAxisDelta {
                        label: to.label,
                        from: from_value,
                        to: to_value,
                    })
                }
            })
            .collect()
    }

    /// Render changed axes between two shapes.
    pub(crate) fn render_deltas_from(self, from: RuntimeShape) -> Vec<String> {
        self.axis_deltas_from(from)
            .into_iter()
            .map(|delta| render_delta(delta.label, &delta.from, &delta.to))
            .collect()
    }

    fn axes(self) -> [AxisShape; 9] {
        [
            AxisShape {
                label: "phase",
                value: self.phase,
                multiline: false,
            },
            AxisShape {
                label: "role",
                value: self.role,
                multiline: false,
            },
            AxisShape {
                label: "context",
                value: self.context,
                multiline: false,
            },
            AxisShape {
                label: "plan",
                value: self.plan,
                multiline: true,
            },
            AxisShape {
                label: "children",
                value: self.children,
                multiline: false,
            },
            AxisShape {
                label: "history",
                value: self.history,
                multiline: true,
            },
            AxisShape {
                label: "evidence",
                value: self.evidence,
                multiline: true,
            },
            AxisShape {
                label: "continuation",
                value: self.continuation,
                multiline: true,
            },
            AxisShape {
                label: "report",
                value: self.report,
                multiline: false,
            },
        ]
    }
}

fn render_delta(label: &str, from: &str, to: &str) -> String {
    let combined = format!("{label}: {from} -> {to}");
    if combined.len() <= 96 {
        combined
    } else {
        format!("{label}:\n{from}\n-> {to}")
    }
}

pub(crate) fn render_type_expr(value: &str, indent: usize) -> Vec<String> {
    render_type_expr_with(value, indent, false, false)
}

fn render_multiline_axis(value: &str, indent: usize) -> Vec<String> {
    render_type_expr_with(value, indent, true, true)
}

fn render_type_expr_with(
    value: &str,
    indent: usize,
    trailing: bool,
    force_multiline: bool,
) -> Vec<String> {
    let value = compact_type(value);
    let prefix = " ".repeat(indent);
    let suffix = if trailing { "," } else { "" };
    let Some((head, args)) = split_generic(&value) else {
        return vec![format!("{prefix}{value}{suffix}")];
    };
    let args = split_top_level_args(args);
    if !force_multiline && value.len() <= TYPE_LINE_LIMIT {
        return vec![format!("{prefix}{value}{suffix}")];
    }
    let mut lines = vec![format!("{prefix}{head}<")];
    for arg in args {
        lines.extend(render_type_expr_with(&arg, indent + 4, true, false));
    }
    lines.push(format!("{prefix}>{suffix}"));
    lines
}

fn split_generic(value: &str) -> Option<(&str, &str)> {
    let open = value.find('<')?;
    if !value.ends_with('>') {
        return None;
    }
    Some((&value[..open], &value[open + 1..value.len() - 1]))
}

fn split_top_level_args(value: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut depth = 0_i32;
    let mut start = 0_usize;
    for (index, ch) in value.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => depth -= 1,
            ',' if depth == 0 => {
                let arg = value[start..index].trim();
                if !arg.is_empty() {
                    args.push(arg.to_string());
                }
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    let tail = value[start..].trim();
    if !tail.is_empty() {
        args.push(tail.to_string());
    }
    args
}

fn compact_type(value: &str) -> String {
    let mut compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    for (from, to) in [
        ("< ", "<"),
        (" >", ">"),
        (" ,", ","),
        (", >", ",>"),
        (":: ", "::"),
        (" ::", "::"),
    ] {
        compact = compact.replace(from, to);
    }
    compact
}

fn structure_diff(value: &str, other: &str) -> Vec<String> {
    let value = extract_structures(value);
    let other = extract_structures(other);
    value.difference(&other).cloned().collect()
}

fn extract_structures(value: &str) -> BTreeSet<String> {
    let mut structures = BTreeSet::new();
    collect_structures(&compact_type(value), &mut structures);
    structures
}

fn collect_structures(value: &str, structures: &mut BTreeSet<String>) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    structures.insert(value.to_string());
    if let Some((_, args)) = split_generic(value) {
        for arg in split_top_level_args(args) {
            collect_structures(&arg, structures);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeShape;

    #[test]
    fn renders_runtime_history_axis_multiline() {
        let shape = RuntimeShape {
            phase: "phase::R0",
            role: "RuntimeRole<role::Unknown, role::Unresolved>",
            context: "Context<context::Command<Prototype1StateCommand>>",
            plan: "Plan<plan::authority::None, plan::schedule::None>",
            children: "Children<children::set::None, children::attempt::None>",
            history: "History<history_axis::startup::None, history_axis::head::Unobserved, history_axis::epoch::None>",
            evidence: "Evidence<evidence::parent_start::None, evidence::baseline::None>",
            continuation: "Continuation<continuation::selection::None, continuation::decision::None>",
            report: "Report<report::None>",
        };

        let rendered = shape.render();

        assert!(rendered.contains("History<\n        history_axis::startup::None,"));
        assert!(rendered.contains("Context<context::Command<Prototype1StateCommand>>,"));
    }

    #[test]
    fn renders_plan_axis_with_nested_schedule_multiline() {
        let shape = RuntimeShape {
            phase: "phase::R13a",
            role: "parent_role::Parent<parent_role::Selectable>",
            context: "Context<context::Collected<RunShape, CampaignConfig>>",
            plan: "Plan<plan::authority::Received<Received<parent_role::ChildPlan>>, plan::schedule::Ready<Prototype1ChildBudget, Prototype1ChildScheduleMode>>",
            children: "Children<children::set::Report<PlannedChildOutcome>, children::attempt::Complete>",
            history: "History<history_axis::startup::Validated<history_axis::startup::Any>, history_axis::head::Read, history_axis::epoch::None>",
            evidence: "Evidence<evidence::parent_start::Recorded<ParentStartedEntry>, evidence::baseline::Ready<CompleteBaseline>, evidence::policy::Ready<Prototype1SearchPolicy, Prototype1ChildBudget>, evidence::selection::Evidence<SelectionSealMaterial>, evidence::completion::None>",
            continuation: "Continuation<continuation::selection::Maybe<SuccessorDecision>, continuation::decision::Stopped<Prototype1ContinuationDecision>, continuation::handoff::None>",
            report: "Report<report::Facts>",
        };

        let rendered = shape.render();

        assert!(rendered.contains(
            "Plan<\n        plan::authority::Received<Received<parent_role::ChildPlan>>,"
        ));
        assert!(
            rendered.contains("        plan::schedule::Ready<\n            Prototype1ChildBudget,")
        );
    }

    #[test]
    fn renders_long_type_expr_multiline() {
        let rendered = super::render_type_expr(
            "Evidence<evidence::parent_start::Recorded<ParentStartedEntry>, evidence::baseline::Ready<CompleteBaseline>, evidence::policy::Ready<Prototype1SearchPolicy, Prototype1ChildBudget>, evidence::selection::Evidence<SelectionSealMaterial>, evidence::completion::Recorded>",
            4,
        )
        .join("\n");

        assert!(rendered.starts_with("    Evidence<"));
        assert!(rendered.contains("        evidence::completion::Recorded,"));
    }

    #[test]
    fn reports_added_and_removed_structures() {
        let delta = super::RuntimeAxisDelta {
            label: "context",
            from: "Context<context::Command<Prototype1StateCommand>>".to_string(),
            to: "Context<context::Collected<RunShape, CampaignConfig>>".to_string(),
        };

        let removed = delta.removed_structures();
        let added = delta.added_structures();

        assert!(removed.contains(&"context::Command<Prototype1StateCommand>".to_string()));
        assert!(removed.contains(&"Prototype1StateCommand".to_string()));
        assert!(added.contains(&"context::Collected<RunShape, CampaignConfig>".to_string()));
        assert!(added.contains(&"RunShape".to_string()));
        assert!(added.contains(&"CampaignConfig".to_string()));
    }

    #[test]
    fn renders_long_axis_delta_multiline() {
        let from = RuntimeShape {
            phase: "phase::R0",
            role: "role::A",
            context: "Context<context::Command<Prototype1StateCommand>>",
            plan: "Plan<A, B>",
            children: "Children<A, B>",
            history: "History<A, B, C>",
            evidence: "Evidence<A, B, C, D, E>",
            continuation: "Continuation<A, B, C>",
            report: "Report<A>",
        };
        let to = RuntimeShape {
            context: "Context<context::Collected<RunShape, CampaignConfig>>",
            phase: "phase::R1",
            ..from
        };

        let deltas = to.render_deltas_from(from);

        assert!(
            deltas
                .iter()
                .any(|delta| delta == "phase: phase::R0 -> phase::R1")
        );
        assert!(deltas.iter().any(|delta| delta.contains("context:\nContext<context::Command<Prototype1StateCommand>>\n-> Context<context::Collected<RunShape, CampaignConfig>>")));
    }
}
