//! Human-readable typestate shape metadata derived from alias declarations.
//!
//! The walk debugger needs to print `Runtime<...>` shapes and axis deltas for
//! humans. The metadata here is intentionally structural: alias declarations use
//! `stringify!` once at the same site that defines the real type alias, and the
//! walk UI renders/diffs those axis strings instead of maintaining a second copy
//! of each `Runtime<...>` alias.

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

#[derive(Debug, Clone, Copy)]
struct AxisShape {
    label: &'static str,
    value: &'static str,
    multiline: bool,
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

    /// Render changed axes between two shapes.
    pub(crate) fn render_deltas_from(self, from: RuntimeShape) -> Vec<String> {
        self.axes()
            .into_iter()
            .zip(from.axes())
            .filter_map(|(to, from)| {
                let from_value = compact_type(from.value);
                let to_value = compact_type(to.value);
                if from_value == to_value {
                    None
                } else {
                    Some(render_delta(to.label, &from_value, &to_value))
                }
            })
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
                multiline: false,
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

fn render_multiline_axis(value: &str, indent: usize) -> Vec<String> {
    let value = compact_type(value);
    let Some((head, args)) = split_generic(&value) else {
        return vec![format!("{}{},", " ".repeat(indent), value)];
    };
    let prefix = " ".repeat(indent);
    let item_prefix = " ".repeat(indent + 4);
    let mut lines = vec![format!("{prefix}{head}<")];
    for arg in split_top_level_args(args) {
        lines.push(format!("{item_prefix}{arg},"));
    }
    lines.push(format!("{prefix}>,"));
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
