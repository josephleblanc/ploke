#!/usr/bin/env python3
"""Read-only Prototype 1 run artifact summarizer.

This script intentionally avoids opening sqlite/db files. It summarizes JSON,
JSONL, log, and filesystem mtime evidence for a Prototype 1 campaign.
"""

from __future__ import annotations

import argparse
import json
import re
from collections import Counter
from datetime import datetime
from pathlib import Path
from typing import Any, Iterable


def local_time_from_epoch(value: float) -> str:
    return datetime.fromtimestamp(value).astimezone().isoformat(timespec="seconds")


def local_time_from_ms(value: int | float | None) -> str:
    if value is None:
        return ""
    return local_time_from_epoch(float(value) / 1000.0)


def read_json(path: Path) -> Any:
    with path.open() as handle:
        return json.load(handle)


def read_jsonl(path: Path) -> Iterable[Any]:
    with path.open() as handle:
        for line in handle:
            line = line.strip()
            if not line:
                continue
            try:
                yield json.loads(line)
            except json.JSONDecodeError:
                continue


def short(value: Any, limit: int = 240) -> str:
    text = str(value).replace("\n", " ")
    if len(text) <= limit:
        return text
    return text[: limit - 3] + "..."


def print_profile(campaign_root: Path) -> None:
    print("## Profile")
    for path in [campaign_root / "campaign.json", campaign_root / "prototype1" / "run-profile.toml"]:
        print(f"- {path}")
        if not path.exists():
            print("  missing")
            continue
        if path.suffix == ".json":
            data = read_json(path)
            print(f"  model_id={data.get('model_id')} route_source={data.get('route_source')}")
            protocol = data.get("protocol") or {}
            if protocol:
                print(
                    "  protocol "
                    f"model_id={protocol.get('model_id')} "
                    f"route_source={protocol.get('route_source')} "
                    f"max_tokens={protocol.get('max_tokens')}"
                )
        else:
            interesting = (
                "max_generations",
                "max_total_nodes",
                "min_children",
                "max_children",
                "parallel_targets",
                "max_attempts",
                "fresh_slots_per_child",
                "require_keep_for_continuation",
                "explore_from_rejected",
                "stop_on_first_keep",
                "observe_child_stale_after_secs",
            )
            for line in path.read_text().splitlines():
                stripped = line.strip()
                if any(stripped.startswith(key) for key in interesting):
                    print(f"  {stripped}")


def print_journal(proto: Path) -> None:
    print("\n## Transition Journal")
    journal = proto / "transition-journal.jsonl"
    if not journal.exists():
        print("missing")
        return
    entries = list(read_jsonl(journal))
    print(f"- path={journal}")
    print(f"- entries={len(entries)} mtime={local_time_from_epoch(journal.stat().st_mtime)}")
    print(f"- last_kind_counts={dict(Counter(entry.get('kind') for entry in entries[-40:]))}")
    for entry in entries[-12:]:
        node = entry.get("node_id") or (entry.get("refs") or {}).get("node_id")
        state = entry.get("state") or entry.get("result") or entry.get("refs")
        print(
            "- "
            f"{local_time_from_ms(entry.get('recorded_at'))} "
            f"kind={entry.get('kind')} phase={entry.get('phase', '')} "
            f"generation={entry.get('generation', '')} node={node} "
            f"{short(state)}"
        )


def print_nodes(proto: Path) -> None:
    print("\n## Nodes")
    for path in sorted((proto / "nodes").glob("*/node.json")):
        data = read_json(path)
        print(
            "- "
            f"{data.get('node_id')} gen={data.get('generation')} "
            f"branch={data.get('branch_id')} status={data.get('status')} "
            f"updated_at={data.get('updated_at')}"
        )


def print_branch_registry(proto: Path) -> None:
    print("\n## Branch Registry")
    path = proto / "branches.json"
    if not path.exists():
        print("missing")
        return
    rows = list(read_jsonl(path))
    for row in rows:
        body = row.get("body") or {}
        summary = body.get("summary") or {}
        print(
            "- "
            f"{row.get('recorded_at')} branch={body.get('branch_id')} "
            f"parent={body.get('parent_branch_id')} candidate={body.get('candidate_id')} "
            f"target={body.get('target_relpath')} disposition={summary.get('overall_disposition')}"
        )


def summarize_run_roots(instances_root: Path) -> None:
    print("\n## Agent Traces")
    for trace in sorted(instances_root.glob("**/agent-turn-trace.json")):
        data = read_json(trace)
        parts = trace.parts
        branch = "baseline"
        if "treatments" in parts:
            branch = parts[parts.index("treatments") + 1]
        run = trace.parent.name
        events = data.get("events") or []
        term = data.get("terminal_record") or {}
        print(
            "- "
            f"{branch} {run} trace_mtime={local_time_from_epoch(trace.stat().st_mtime)} "
            f"events={len(events)} model={data.get('selected_model')} "
            f"outcome={term.get('outcome')} attempts={term.get('attempts')}"
        )

    print("\n## LLM Responses")
    for path in sorted(instances_root.glob("**/llm-full-responses.jsonl")):
        parts = path.parts
        branch = "baseline"
        if "treatments" in parts:
            branch = parts[parts.index("treatments") + 1]
        created: list[float] = []
        models: Counter[str] = Counter()
        finishes: Counter[str] = Counter()
        count = 0
        for row in read_jsonl(path):
            response = row.get("response") or {}
            count += 1
            if response.get("created") is not None:
                created.append(float(response["created"]))
            if response.get("model"):
                models[response["model"]] += 1
            for choice in response.get("choices") or []:
                finishes[str(choice.get("finish_reason"))] += 1
        first = local_time_from_epoch(min(created)) if created else ""
        last = local_time_from_epoch(max(created)) if created else ""
        print(
            "- "
            f"{branch} {path.parent.name} responses={count} first={first} last={last} "
            f"models={dict(models)} finish={dict(finishes)}"
        )


def summarize_protocol(protocol_root: Path) -> None:
    print("\n## Protocol")
    for run_dir in sorted(path for path in protocol_root.glob("**/runs/*") if path.is_dir()):
        files = list(run_dir.glob("*.json"))
        if not files:
            continue
        parts = run_dir.parts
        branch = "baseline"
        if "treatments" in parts:
            branch = parts[parts.index("treatments") + 1]
        kinds: Counter[str] = Counter()
        overall: Counter[str] = Counter()
        failed_focal = 0
        mtimes: list[float] = []
        for file in files:
            mtimes.append(file.stat().st_mtime)
            name = file.name
            if "tool_call_intent_segmentation" in name:
                kinds["tool_call_intent_segmentation"] += 1
            elif "tool_call_segment_review" in name:
                kinds["tool_call_segment_review"] += 1
            elif "tool_call_review" in name:
                kinds["tool_call_review"] += 1
            else:
                kinds["other"] += 1
            data = read_json(file)
            output = data.get("output") or {}
            if isinstance(output.get("overall"), str):
                overall[output["overall"]] += 1
            focal = (data.get("input") or {}).get("focal")
            if isinstance(focal, dict) and focal.get("failed"):
                failed_focal += 1
        print(
            "- "
            f"{branch} {run_dir.name} count={len(files)} kinds={dict(kinds)} "
            f"first={local_time_from_epoch(min(mtimes))} last={local_time_from_epoch(max(mtimes))} "
            f"overall={dict(overall)} failed_focal={failed_focal}"
        )


def print_evaluations(proto: Path) -> None:
    print("\n## Evaluations")
    for path in sorted((proto / "evaluations").glob("*.json")):
        data = read_json(path)
        compared = (data.get("compared_instances") or [{}])[0]
        base = compared.get("baseline_metrics") or {}
        treatment = compared.get("treatment_metrics") or {}
        evaluation = compared.get("evaluation") or {}
        print(
            "- "
            f"branch={data.get('branch_id')} disposition={data.get('overall_disposition')} "
            f"base_failed={base.get('tool_calls_failed')} treat_failed={treatment.get('tool_calls_failed')} "
            f"base_retry={base.get('same_file_patch_retry_count')} "
            f"treat_retry={treatment.get('same_file_patch_retry_count')} "
            f"base_streak={base.get('same_file_patch_max_streak')} "
            f"treat_streak={treatment.get('same_file_patch_max_streak')} "
            f"eval_disposition={evaluation.get('disposition')}"
        )


def print_history_selection(proto: Path) -> None:
    print("\n## History Selection Formula Rows")
    block = proto / "history" / "blocks" / "segment-000000.jsonl"
    if not block.exists():
        print("missing")
        return
    found = 0
    for row in read_jsonl(block):
        text = json.dumps(row)
        if "score_child_prop" not in text:
            continue
        found += 1
        metrics = []
        def walk(value: Any) -> None:
            if isinstance(value, dict):
                formula = value.get("formula")
                if isinstance(formula, dict) and formula.get("kind") == "score_child_prop":
                    metrics.append(formula)
                for child in value.values():
                    walk(child)
            elif isinstance(value, list):
                for child in value:
                    walk(child)
        walk(row)
        for formula in metrics:
            print(
                "- "
                f"selected={formula.get('selected_candidate')} total_weight={formula.get('total_weight')} "
                f"sample={formula.get('sample')} selected_index={formula.get('selected_index')}"
            )
            for row_data in formula.get("rows") or []:
                print(
                    "  - "
                    f"candidate={row_data.get('candidate')} node={row_data.get('node_id')} "
                    f"branch={row_data.get('branch_id')} disposition={row_data.get('branch_disposition')} "
                    f"performance={row_data.get('performance')} child_count={row_data.get('child_count')} "
                    f"weight={row_data.get('weight')} selected={row_data.get('selected')}"
                )
    if not found:
        print("no score_child_prop rows found")


def print_timeout_markers(campaign_root: Path) -> None:
    print("\n## Timeout/Error Markers")
    pattern = re.compile(
        r"timed out|timeout after|deadline|stale|INVALID_MODEL_RESPONSE|WorkspacePathMismatch",
        re.I,
    )
    logs = sorted((campaign_root / "prototype1" / "nodes").glob("**/*.log"))
    matches = 0
    for path in logs:
        try:
            for index, line in enumerate(path.read_text(errors="replace").splitlines(), start=1):
                if not pattern.search(line):
                    continue
                if "timeout_ms=" in line and " WARN " not in line and " ERROR " not in line:
                    continue
                matches += 1
                print(f"- {path}:{index}: {short(line, 300)}")
                if matches >= 40:
                    print("- truncated after 40 markers")
                    return
        except OSError:
            continue
    if not matches:
        print("no timeout/error markers found in scanned logs")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--campaign", required=True)
    parser.add_argument("--home", default="~/.ploke-eval")
    args = parser.parse_args()

    home = Path(args.home).expanduser()
    campaign_root = home / "campaigns" / args.campaign
    proto = campaign_root / "prototype1"
    instances_root = home / "instances" / "prototype1" / args.campaign
    protocol_root = home / "protocol" / "prototype1" / args.campaign

    print(f"# Prototype 1 Run Summary: {args.campaign}")
    print(f"generated_at={datetime.now().astimezone().isoformat(timespec='seconds')}")
    print(f"campaign_root={campaign_root}")
    print(f"worktree={home / 'worktrees' / args.campaign}")
    print(f"instances_root={instances_root}")
    print(f"protocol_root={protocol_root}")

    print_profile(campaign_root)
    print_journal(proto)
    print_nodes(proto)
    print_branch_registry(proto)
    print_evaluations(proto)
    summarize_run_roots(instances_root)
    summarize_protocol(protocol_root)
    print_history_selection(proto)
    print_timeout_markers(campaign_root)


if __name__ == "__main__":
    main()
