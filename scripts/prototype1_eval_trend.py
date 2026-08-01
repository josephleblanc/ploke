#!/usr/bin/env python3
"""Summarize Prototype 1 branch self-evaluation trends for one campaign.

This is an operator/read-only helper. It intentionally reads the campaign's
typed branch-evaluation artifacts and node projections into named Python record
classes before computing the trend report.
"""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Iterable


LOWER_IS_BETTER = (
    "partial_patch_failures",
    "same_file_patch_retry_count",
    "same_file_patch_max_streak",
    "tool_calls_failed",
)

TRUE_IS_BETTER = (
    "convergence",
    "oracle_eligible",
    "nonempty_valid_patch",
    "patch_attempted",
)

FALSE_IS_BETTER = (
    "aborted",
    "aborted_repair_loop",
)


@dataclass(frozen=True)
class Metrics:
    values: dict[str, Any]

    @classmethod
    def from_dict(cls, data: dict[str, Any] | None) -> "Metrics | None":
        if data is None:
            return None
        return cls(values=dict(data))

    def number(self, key: str) -> int:
        value = self.values.get(key, 0)
        if isinstance(value, bool):
            return int(value)
        if isinstance(value, int):
            return value
        return 0

    def flag(self, key: str) -> bool:
        return bool(self.values.get(key, False))


@dataclass(frozen=True)
class BranchEvaluation:
    disposition: str
    reasons: tuple[str, ...] = ()

    @classmethod
    def from_dict(cls, data: dict[str, Any] | None) -> "BranchEvaluation | None":
        if data is None:
            return None
        return cls(
            disposition=str(data.get("disposition", "unknown")),
            reasons=tuple(str(reason) for reason in data.get("reasons", [])),
        )


@dataclass(frozen=True)
class ComparedInstance:
    instance_id: str
    status: str
    baseline_metrics: Metrics | None
    treatment_metrics: Metrics | None
    evaluation: BranchEvaluation | None

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "ComparedInstance":
        return cls(
            instance_id=str(data["instance_id"]),
            status=str(data.get("status", "unknown")),
            baseline_metrics=Metrics.from_dict(data.get("baseline_metrics")),
            treatment_metrics=Metrics.from_dict(data.get("treatment_metrics")),
            evaluation=BranchEvaluation.from_dict(data.get("evaluation")),
        )


@dataclass(frozen=True)
class EvaluationReport:
    branch_id: str
    treatment_campaign_id: str
    overall_disposition: str
    reasons: tuple[str, ...]
    compared_instances: tuple[ComparedInstance, ...]
    path: Path

    @classmethod
    def load(cls, path: Path) -> "EvaluationReport":
        data = json.loads(path.read_text())
        return cls(
            branch_id=str(data["branch_id"]),
            treatment_campaign_id=str(data["treatment_campaign_id"]),
            overall_disposition=str(data.get("overall_disposition", "unknown")),
            reasons=tuple(str(reason) for reason in data.get("reasons", [])),
            compared_instances=tuple(
                ComparedInstance.from_dict(row)
                for row in data.get("compared_instances", [])
            ),
            path=path,
        )


@dataclass(frozen=True)
class NodeRecord:
    node_id: str
    branch_id: str
    generation: int
    parent_node_id: str | None

    @classmethod
    def load(cls, path: Path) -> "NodeRecord":
        data = json.loads(path.read_text())
        return cls(
            node_id=str(data["node_id"]),
            branch_id=str(data["branch_id"]),
            generation=int(data["generation"]),
            parent_node_id=data.get("parent_node_id"),
        )


@dataclass
class GenerationSummary:
    generation: int
    branches: int = 0
    keep: int = 0
    reject: int = 0
    compared_instances: int = 0
    missing_treatment_metrics: int = 0
    metric_totals: dict[str, int] = field(default_factory=dict)
    improvement_reasons: dict[str, int] = field(default_factory=dict)
    regression_reasons: dict[str, int] = field(default_factory=dict)

    def add_reason(self, reason: str) -> None:
        target = (
            self.improvement_reasons
            if " improved:" in reason
            else self.regression_reasons
            if " regressed:" in reason
            else self.regression_reasons
        )
        target[reason] = target.get(reason, 0) + 1

    def add_metric(self, key: str, value: int) -> None:
        self.metric_totals[key] = self.metric_totals.get(key, 0) + value

    def rate(self, numerator: int) -> str:
        if self.branches == 0:
            return "n/a"
        return f"{numerator}/{self.branches} ({(numerator / self.branches) * 100:.1f}%)"

    def metric_rate(self, key: str) -> str:
        if self.compared_instances == 0:
            return "n/a"
        numerator = self.metric_totals.get(key, 0)
        return f"{numerator}/{self.compared_instances} ({(numerator / self.compared_instances) * 100:.1f}%)"


@dataclass(frozen=True)
class BranchRow:
    generation: int
    node_id: str
    branch_id: str
    disposition: str
    compared_instances: int
    improvement_reasons: tuple[str, ...]
    regression_reasons: tuple[str, ...]


def load_nodes(campaign: Path) -> dict[str, NodeRecord]:
    nodes_root = campaign / "prototype1" / "nodes"
    nodes = {}
    for path in nodes_root.glob("*/node.json"):
        node = NodeRecord.load(path)
        nodes[node.branch_id] = node
    return nodes


def load_reports(campaign: Path) -> list[EvaluationReport]:
    evaluations_root = campaign / "prototype1" / "evaluations"
    return sorted(
        (EvaluationReport.load(path) for path in evaluations_root.glob("branch-*.json")),
        key=lambda report: report.branch_id,
    )


def reason_groups(report: EvaluationReport) -> tuple[list[str], list[str]]:
    improvements: list[str] = []
    regressions: list[str] = []
    for row in report.compared_instances:
        if row.evaluation is None:
            continue
        for reason in row.evaluation.reasons:
            if " improved:" in reason:
                improvements.append(f"{row.instance_id}: {reason}")
            elif " regressed:" in reason:
                regressions.append(f"{row.instance_id}: {reason}")
            else:
                regressions.append(f"{row.instance_id}: {reason}")
    for reason in report.reasons:
        if " improved:" in reason:
            improvements.append(reason)
        else:
            regressions.append(reason)
    return improvements, regressions


def summarize(campaign: Path) -> tuple[list[BranchRow], list[GenerationSummary]]:
    nodes = load_nodes(campaign)
    reports = load_reports(campaign)
    by_generation: dict[int, GenerationSummary] = {}
    rows: list[BranchRow] = []

    for report in reports:
        node = nodes.get(report.branch_id)
        generation = node.generation if node is not None else -1
        summary = by_generation.setdefault(generation, GenerationSummary(generation))
        summary.branches += 1
        if report.overall_disposition == "keep":
            summary.keep += 1
        elif report.overall_disposition == "reject":
            summary.reject += 1

        improvements, regressions = reason_groups(report)
        for reason in improvements + regressions:
            summary.add_reason(reason.split(": ", 1)[-1])

        for row in report.compared_instances:
            summary.compared_instances += 1
            if row.treatment_metrics is None:
                summary.missing_treatment_metrics += 1
                continue
            for key in LOWER_IS_BETTER:
                summary.add_metric(key, row.treatment_metrics.number(key))
            for key in TRUE_IS_BETTER + FALSE_IS_BETTER:
                summary.add_metric(key, int(row.treatment_metrics.flag(key)))
            summary.add_metric(
                "tool_calls_total",
                row.treatment_metrics.number("tool_calls_total"),
            )

        rows.append(
            BranchRow(
                generation=generation,
                node_id=node.node_id if node is not None else "(unknown)",
                branch_id=report.branch_id,
                disposition=report.overall_disposition,
                compared_instances=len(report.compared_instances),
                improvement_reasons=tuple(improvements),
                regression_reasons=tuple(regressions),
            )
        )

    rows.sort(key=lambda row: (row.generation, row.branch_id))
    summaries = [by_generation[key] for key in sorted(by_generation)]
    return rows, summaries


def format_top(reasons: dict[str, int], limit: int) -> str:
    if not reasons:
        return "-"
    ranked = sorted(reasons.items(), key=lambda item: (-item[1], item[0]))
    return "; ".join(f"{count}x {reason}" for reason, count in ranked[:limit])


def print_report(campaign: Path, rows: list[BranchRow], summaries: list[GenerationSummary]) -> None:
    print(f"campaign: {campaign.name}")
    print(f"evaluated_branches: {len(rows)}")
    print()
    print("generation trend")
    print(
        "gen branches keep reject compared missing_metrics convergence oracle "
        "nonempty_patch patch_attempted aborted repair_abort failed_tools/total"
    )
    for summary in summaries:
        failed_tools = summary.metric_totals.get("tool_calls_failed", 0)
        total_tools = summary.metric_totals.get("tool_calls_total", 0)
        print(
            f"{summary.generation:>3} "
            f"{summary.branches:>8} "
            f"{summary.keep:>4} "
            f"{summary.reject:>6} "
            f"{summary.compared_instances:>8} "
            f"{summary.missing_treatment_metrics:>15} "
            f"{summary.metric_totals.get('convergence', 0):>11} "
            f"{summary.metric_totals.get('oracle_eligible', 0):>6} "
            f"{summary.metric_totals.get('nonempty_valid_patch', 0):>14} "
            f"{summary.metric_totals.get('patch_attempted', 0):>15} "
            f"{summary.metric_totals.get('aborted', 0):>7} "
            f"{summary.metric_totals.get('aborted_repair_loop', 0):>12} "
            f"{failed_tools}/{total_tools}"
        )
    print()
    print("rate trend")
    print("gen keep_rate reject_rate convergence oracle nonempty_patch patch_attempted aborted repair_abort")
    for summary in summaries:
        print(
            f"{summary.generation:>3} "
            f"{summary.rate(summary.keep):>16} "
            f"{summary.rate(summary.reject):>16} "
            f"{summary.metric_rate('convergence'):>16} "
            f"{summary.metric_rate('oracle_eligible'):>16} "
            f"{summary.metric_rate('nonempty_valid_patch'):>16} "
            f"{summary.metric_rate('patch_attempted'):>16} "
            f"{summary.metric_rate('aborted'):>16} "
            f"{summary.metric_rate('aborted_repair_loop'):>16}"
        )
    print()
    print("branch outcomes")
    print("gen disposition compared node branch")
    for row in rows:
        print(
            f"{row.generation:>3} "
            f"{row.disposition:<11} "
            f"{row.compared_instances:>8} "
            f"{row.node_id} "
            f"{row.branch_id}"
        )
    print()
    print("top reasons by generation")
    for summary in summaries:
        print(f"gen {summary.generation}")
        print(f"  improvements: {format_top(summary.improvement_reasons, 5)}")
        print(f"  regressions:  {format_top(summary.regression_reasons, 5)}")


def campaign_path(value: str) -> Path:
    path = Path(value).expanduser()
    if not path.exists():
        path = Path.home() / ".ploke-eval" / "campaigns" / value
    if not path.exists():
        raise argparse.ArgumentTypeError(f"campaign does not exist: {value}")
    return path


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Summarize Prototype 1 branch self-evaluation trends."
    )
    parser.add_argument("campaign", type=campaign_path)
    args = parser.parse_args(list(argv) if argv is not None else None)

    rows, summaries = summarize(args.campaign)
    print_report(args.campaign, rows, summaries)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
