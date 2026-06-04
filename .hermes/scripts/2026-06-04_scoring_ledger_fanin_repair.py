#!/usr/bin/env python3
"""Repair Q2 scoring comparison canonical ledgers from reviewed shards.

Reads reviewed shard records under /home/brasides/wiki/queries/ploke/selection-scoring/shards
and rewrites only the six canonical ledgers under .../ledgers.
"""
from __future__ import annotations

from pathlib import Path
import csv
import io
import re
from typing import Iterable

BASE = Path("/home/brasides/wiki/queries/ploke/selection-scoring")
SHARDS = BASE / "shards"
LEDGERS = BASE / "ledgers"
TODAY = "2026-06-04"


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def lines(path: Path) -> list[str]:
    return read(path).splitlines()


def section(path: Path, heading: str) -> list[str]:
    data = lines(path)
    start = None
    for i, line in enumerate(data):
        if line.strip() == heading:
            start = i + 1
            break
    if start is None:
        return []
    end = len(data)
    for j in range(start, len(data)):
        if data[j].startswith("## "):
            end = j
            break
    return data[start:end]


def strip_quotes(value: str) -> str:
    value = value.strip()
    if len(value) >= 2 and value[0] == value[-1] and value[0] in "\"'":
        return value[1:-1]
    return value


def parse_array(value: str) -> list[str] | None:
    v = value.strip()
    if not (v.startswith("[") and v.endswith("]")):
        return None
    inner = v[1:-1].strip()
    if not inner:
        return []
    reader = csv.reader(io.StringIO(inner), skipinitialspace=True)
    try:
        return [strip_quotes(x.strip()) for x in next(reader)]
    except Exception:
        return [strip_quotes(x.strip()) for x in inner.split(",") if x.strip()]


def clean_value(value: str, key: str | None = None) -> str:
    v = value.strip()
    arr = parse_array(v)
    if arr is not None:
        if key == "papers_using_it":
            return "[" + ", ".join(arr) + "]" if arr else "[]"
        return ", ".join(arr) if arr else "none"
    v = strip_quotes(v)
    v = v.replace("827-834for", "827-834 for")
    v = re.sub(r"(:\d+(?:-\d+)?(?:,\d+(?:-\d+)?)?)(\()", r"\1 \2", v)
    v = re.sub(r"\s+", " ", v).strip()
    return v if v else "not specified"


def cell(value: object, key: str | None = None) -> str:
    v = clean_value(str(value), key=key)
    # Escape markdown table separators inside cells.
    v = v.replace("|", r"\|")
    return v


def row(fields: Iterable[object], keys: Iterable[str] | None = None) -> str:
    key_list = list(keys) if keys is not None else [None] * len(list(fields))
    field_list = list(fields)
    if len(key_list) != len(field_list):
        key_list = [None] * len(field_list)
    return "| " + " | ".join(cell(v, k) for v, k in zip(field_list, key_list)) + " |"


def parse_table(path: Path, heading: str) -> list[dict[str, str]]:
    sec = section(path, heading)
    header: list[str] | None = None
    out: list[dict[str, str]] = []
    for line in sec:
        if not line.startswith("|"):
            continue
        parts = [p.strip() for p in line.strip().strip("|").split("|")]
        if not parts:
            continue
        if all(set(p) <= {"-", ":", " "} for p in parts):
            continue
        if header is None:
            header = parts
            continue
        if parts == header:
            continue
        out.append({h: clean_value(v, h) for h, v in zip(header, parts)})
    return out


START_KEYS = {
    "kind",  # M3 benchmark records start with kind before benchmark_id.
    "mechanism_id",
    "benchmark_id",
    "paper_id",
    "source_paper_id",
    "source_id",
}


def parse_list_records(path: Path, heading: str) -> list[dict[str, str]]:
    sec = section(path, heading)
    records: list[dict[str, str]] = []
    current: dict[str, str] | None = None

    def finish() -> None:
        nonlocal current
        if current:
            records.append(current)
            current = None

    for raw in sec:
        line = raw.rstrip()
        stripped = line.strip()
        if not stripped or stripped.startswith("```"):
            continue
        m_title = re.match(r"^-\s+(\d{4}\.\d+)\s+—\s+(.+)$", stripped)
        if m_title:
            finish()
            current = {"paper_id": m_title.group(1), "record_title": clean_value(m_title.group(2))}
            continue
        m_start = re.match(r"^-\s+([A-Za-z0-9_]+):\s*(.*)$", stripped)
        if m_start:
            key, value = m_start.group(1), m_start.group(2)
            if key in START_KEYS and current is not None and key in current:
                finish()
            if current is None:
                current = {}
            current[key] = clean_value(value, key)
            continue
        m_nested_bullet = re.match(r"^\s*-\s+([A-Za-z0-9_]+):\s*(.*)$", line)
        if m_nested_bullet and current is not None:
            key, value = m_nested_bullet.group(1), m_nested_bullet.group(2)
            current[key] = clean_value(value, key)
            continue
        m_field = re.match(r"^\s{2,}([A-Za-z0-9_]+):\s*(.*)$", line)
        if m_field and current is not None:
            key, value = m_field.group(1), m_field.group(2)
            current[key] = clean_value(value, key)
            continue
    finish()
    # Keep only records with at least one real id-like field.
    return [r for r in records if any(k in r for k in START_KEYS)]


def inventory_seed_block() -> str:
    current = read(LEDGERS / "mechanism-ledger.md")
    m = re.search(r"### Inventory-derived seeds\n\n(?P<body>.*?)(?=\n### Formal and nearby mechanism rows)", current, re.S)
    if not m:
        raise RuntimeError("Could not find inventory seed block")
    return m.group("body").rstrip()


def mechanism_rows() -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    rows.extend(parse_table(SHARDS / "M2-formal-mechanisms.md", "## Mechanism records"))
    for rec in parse_list_records(SHARDS / "M7-hyperagents.md", "## Mechanism records"):
        rows.append({
            "mechanism_id": rec.get("mechanism_id", "not specified"),
            "paper_id": rec.get("paper_id", "not specified"),
            "family": rec.get("mechanism_family", rec.get("family", "not specified")),
            "equation_or_rule": rec.get("equation_or_rule", "not specified"),
            "exactness_label": rec.get("exactness_label", "not specified"),
            "score_target": rec.get("score_target", "not specified"),
            "selection_effect": rec.get("selection_effect", "not specified"),
            "source_anchor": rec.get("source_anchor", "not specified"),
            "Ploke axis": rec.get("related_ploke_axis", "not specified"),
            "confidence": rec.get("confidence", "medium"),
            "record_status": rec.get("record_status", "draft"),
        })
    # M7 equation map uses imp-at-k as a report-only mechanism id; define it so the map has no dangling id.
    rows.append({
        "mechanism_id": "imp-at-k",
        "paper_id": "2603.19461",
        "family": "benchmark metric / reporting",
        "equation_or_rule": "maximum improvement a fixed meta agent can obtain by generating up to k variants",
        "exactness_label": "faithful-formalization",
        "score_target": "generated task agents",
        "selection_effect": "reports only",
        "source_anchor": "/home/brasides/code/ploke/.agents/hyper-agents.txt:2248-2266",
        "Ploke axis": "budgeted improvement projection",
        "confidence": "high",
        "record_status": "draft",
    })
    for rec in parse_list_records(SHARDS / "M8-june03-nearby.md", "## Mechanism records"):
        rows.append({
            "mechanism_id": rec.get("mechanism_id", "not specified"),
            "paper_id": rec.get("paper_id", "not specified"),
            "family": rec.get("mechanism_family", rec.get("family", "not specified")),
            "equation_or_rule": rec.get("equation_or_rule", "not specified"),
            "exactness_label": rec.get("exactness_label", "not specified"),
            "score_target": rec.get("score_target", "not specified"),
            "selection_effect": rec.get("selection_effect", "not specified"),
            "source_anchor": rec.get("source_anchor", "not specified"),
            "Ploke axis": rec.get("related_ploke_axis", "not specified"),
            "confidence": rec.get("confidence", "medium"),
            "record_status": rec.get("record_status", "draft"),
        })
    return rows


def benchmark_rows() -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    for rec in parse_list_records(SHARDS / "M3-benchmarks.md", "## Benchmark records"):
        if rec.get("kind") == "metric-surface":
            continue
        rows.append(rec)
    rows.extend(parse_list_records(SHARDS / "M7-hyperagents.md", "## Benchmark records"))
    rows.append({
        "benchmark_id": "multi-domain-bundle",
        "benchmark_name": "HyperAgents multi-domain bundle",
        "aliases": "Polyglot, Paper Review, Robotics Reward Design, IMO Grading",
        "domain": "multi-domain self-improvement evaluation",
        "task_unit": "domain task bundle",
        "metric_names": "average performance",
        "split_usage": "combines the HyperAgents domain splits for coding, paper review, robotics reward design, and IMO grading",
        "staged_gate": "yes",
        "ground_truth_or_judge": "domain-specific evaluators and held-out test tasks",
        "papers_using_it": "[2603.19461]",
        "source_anchor": "/home/brasides/code/ploke/.agents/hyper-agents.txt:338-347; /home/brasides/code/ploke/.agents/hyper-agents.txt:456-463; /home/brasides/code/ploke/.agents/hyper-agents.txt:1695-1698; /home/brasides/code/ploke/.agents/hyper-agents.txt:2213-2223",
        "record_status": "draft",
        "confidence": "medium",
        "notes": "Defined from the reviewed M7 equation edge so dgm-h-cross-domain-average has a benchmark id in the ledger; this is a bundle label, not a new public benchmark title.",
    })
    rows.extend(parse_list_records(SHARDS / "M8-june03-nearby.md", "## Benchmark records"))
    return rows


def model_rows() -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    rows.extend(parse_list_records(SHARDS / "M4-model-prompts-tools.md", "## Model/prompt/tool records"))
    rows.extend(parse_list_records(SHARDS / "M7-hyperagents.md", "## Model/prompt/tool records"))
    rows.extend(parse_list_records(SHARDS / "M8-june03-nearby.md", "## Model/prompt/tool records"))
    return rows


def repo_rows() -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    rows.extend(parse_list_records(SHARDS / "M5-repos.md", "## Repo/code records"))
    for rec in parse_list_records(SHARDS / "M7-hyperagents.md", "## Repo/code records"):
        rec = dict(rec)
        if rec.get("paper_id") == "2603.19461" and rec.get("code_status") == "paper-only":
            rec["code_status"] = "paper-stated URL; not visited"
        rows.append(rec)
    rows.extend(parse_list_records(SHARDS / "M8-june03-nearby.md", "## Repo/code records"))
    return rows


def citation_rows() -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    rows.extend(parse_list_records(SHARDS / "M6-citations.md", "## Citation edges"))
    rows.extend(parse_list_records(SHARDS / "M7-hyperagents.md", "## Citation edges"))
    rows.extend(parse_list_records(SHARDS / "M8-june03-nearby.md", "## Citation edges"))
    return rows


def equation_rows() -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    rows.extend(parse_list_records(SHARDS / "M7-hyperagents.md", "## Equation-benchmark-model edges"))
    for rec in parse_list_records(SHARDS / "M8-june03-nearby.md", "## Equation-benchmark-model edges"):
        rec = dict(rec)
        rec.setdefault("confidence", "medium")
        rec.setdefault("record_status", "draft")
        rows.append(rec)
    return rows


def write_mechanism() -> None:
    keys = ["mechanism_id", "paper_id", "family", "equation_or_rule", "exactness_label", "score_target", "selection_effect", "source_anchor", "Ploke axis", "confidence", "record_status"]
    body = [
        "---",
        "title: Mechanism Ledger",
        f"created: {TODAY}",
        f"updated: {TODAY}",
        "type: query",
        "tags: [ploke, scoring, source-ledger, research]",
        "sources:",
        "  - queries/ploke/selection-scoring/WORKER_CONTRACT.md",
        "  - queries/ploke/selection-scoring/comparison-index.md",
        "confidence: medium",
        "contested: false",
        "---",
        "# Mechanism Ledger",
        "",
        "Packet hub: [[queries/ploke/selection-scoring/comparison-index|comparison-index]]. Contract: [[queries/ploke/selection-scoring/WORKER_CONTRACT|WORKER_CONTRACT]].",
        "",
        "Status: merged from reviewed shard outputs after Q1 repair review and Q2 fan-in repair. Extraction workers must not edit canonical ledgers.",
        "",
        "## Mechanism records",
        "",
        "### Inventory-derived seeds",
        "",
        inventory_seed_block(),
        "",
        "### Formal and nearby mechanism rows",
        "",
        row(keys),
        row(["---"] * len(keys)),
    ]
    for rec in mechanism_rows():
        body.append(row([rec.get(k, "not specified") for k in keys], keys))
    body.extend([
        "",
        "## Fan-in notes",
        "- Preserve source anchors from shards.",
        "- Mark duplicate or alias records explicitly rather than silently merging uncertain rows.",
        "- Use `needs_high_review` when a field affects Ploke design transfer but the source is ambiguous.",
        "- M1 inventory rows remain source inventory, not canonical mechanism promotion.",
        "- `imp-at-k` is included as a report-only metric mechanism because the reviewed M7 equation map uses it as an edge id; it is not live selection authority.",
    ])
    (LEDGERS / "mechanism-ledger.md").write_text("\n".join(body) + "\n", encoding="utf-8")


def write_benchmark() -> None:
    keys = ["benchmark_id", "benchmark_name", "aliases", "domain", "task_unit", "metric_names", "split_usage", "staged_gate", "ground_truth_or_judge", "papers_using_it", "source_anchor", "record_status", "confidence", "notes"]
    body = [
        "---",
        "title: Benchmark Ledger",
        f"created: {TODAY}",
        f"updated: {TODAY}",
        "type: query",
        "tags: [ploke, scoring, source-ledger, research, benchmark]",
        "sources:",
        "  - queries/ploke/selection-scoring/WORKER_CONTRACT.md",
        "  - queries/ploke/selection-scoring/comparison-index.md",
        "confidence: medium",
        "contested: false",
        "---",
        "# Benchmark Ledger",
        "",
        "Packet hub: [[queries/ploke/selection-scoring/comparison-index|comparison-index]]. Contract: [[queries/ploke/selection-scoring/WORKER_CONTRACT|WORKER_CONTRACT]].",
        "",
        "Status: merged from reviewed shard outputs after Q1 repair review and Q2 fan-in repair. Extraction workers must not edit canonical ledgers.",
        "",
        "## Benchmark records",
        "",
        row(keys),
        row(["---"] * len(keys)),
    ]
    for rec in benchmark_rows():
        body.append(row([rec.get(k, "not specified") for k in keys], keys))
    body.extend([
        "",
        "## Fan-in notes",
        "- Preserve split/gate/judge uncertainty instead of normalizing it away.",
        "- Resolve aliases explicitly.",
        "- Metric-surface rows from M3 remain on the shard; this canonical ledger keeps benchmark substrates and benchmark-audit rows only.",
        "- `multi-domain-bundle` is a bundle label derived from the reviewed M7 equation edge so the equation map has a defined benchmark id.",
    ])
    (LEDGERS / "benchmark-ledger.md").write_text("\n".join(body) + "\n", encoding="utf-8")


def write_model() -> None:
    keys = ["paper_id", "model_role", "model_name", "model_state", "tool_access", "modification_scope", "prompt_or_appendix_anchor", "safety_or_sandbox", "cost_or_budget", "source_anchor", "confidence", "record_status", "notes"]
    body = [
        "---",
        "title: Model Prompt Tool Ledger",
        f"created: {TODAY}",
        f"updated: {TODAY}",
        "type: query",
        "tags: [ploke, scoring, source-ledger, research]",
        "sources:",
        "  - queries/ploke/selection-scoring/WORKER_CONTRACT.md",
        "  - queries/ploke/selection-scoring/comparison-index.md",
        "confidence: medium",
        "contested: false",
        "---",
        "# Model Prompt Tool Ledger",
        "",
        "Packet hub: [[queries/ploke/selection-scoring/comparison-index|comparison-index]]. Contract: [[queries/ploke/selection-scoring/WORKER_CONTRACT|WORKER_CONTRACT]].",
        "",
        "Status: merged from reviewed shard outputs after Q1 repair review and Q2 fan-in repair. Extraction workers must not edit canonical ledgers.",
        "",
        "## Model/prompt/tool records",
        "",
        row(keys),
        row(["---"] * len(keys)),
    ]
    for rec in model_rows():
        body.append(row([rec.get(k, "not specified") for k in keys], keys))
    body.extend([
        "",
        "## Fan-in notes",
        "- Preserve one row per role/surface where the shard split duplicate paper ids by role.",
        "- Use `unknown` and `not specified` rather than inventing model strings, prompts, or tool bindings.",
        "- M4 prompt anchors marked `not specified` remain explicitly unspecified.",
    ])
    (LEDGERS / "model-prompt-ledger.md").write_text("\n".join(body) + "\n", encoding="utf-8")


def write_repo() -> None:
    keys = ["paper_id", "repo_url", "code_status", "license", "commit_or_release", "artifact_scope", "verification_method", "source_anchor", "confidence", "record_status", "notes"]
    body = [
        "---",
        "title: Repo Code Ledger",
        f"created: {TODAY}",
        f"updated: {TODAY}",
        "type: query",
        "tags: [ploke, scoring, source-ledger, research, implementation-reference]",
        "sources:",
        "  - queries/ploke/selection-scoring/WORKER_CONTRACT.md",
        "  - queries/ploke/selection-scoring/comparison-index.md",
        "confidence: medium",
        "contested: false",
        "---",
        "# Repo Code Ledger",
        "",
        "Packet hub: [[queries/ploke/selection-scoring/comparison-index|comparison-index]]. Contract: [[queries/ploke/selection-scoring/WORKER_CONTRACT|WORKER_CONTRACT]].",
        "",
        "Status: merged from reviewed shard outputs after Q1 repair review and Q2 fan-in repair. Extraction workers must not edit canonical ledgers.",
        "",
        "## Repo/code records",
        "",
        row(keys),
        row(["---"] * len(keys)),
    ]
    for rec in repo_rows():
        body.append(row([rec.get(k, "not specified") for k in keys], keys))
    body.extend([
        "",
        "## Fan-in notes",
        "- Keep paper-stated code availability separate from verified URLs.",
        "- Record license and commit/release only when visible.",
        "- M8 `not specified` / `unknown` rows are included because Q2 fan-in QA required explicit handling rather than silent exclusion.",
    ])
    (LEDGERS / "repo-ledger.md").write_text("\n".join(body) + "\n", encoding="utf-8")


def write_citation() -> None:
    keys = ["source_paper_id", "target_paper_id_or_title", "target_in_corpus", "relation_type", "evidence_anchor", "confidence", "record_status", "notes"]
    body = [
        "---",
        "title: Citation Ledger",
        f"created: {TODAY}",
        f"updated: {TODAY}",
        "type: query",
        "tags: [ploke, scoring, source-ledger, research]",
        "sources:",
        "  - queries/ploke/selection-scoring/WORKER_CONTRACT.md",
        "  - queries/ploke/selection-scoring/comparison-index.md",
        "confidence: medium",
        "contested: false",
        "---",
        "# Citation Ledger",
        "",
        "Packet hub: [[queries/ploke/selection-scoring/comparison-index|comparison-index]]. Contract: [[queries/ploke/selection-scoring/WORKER_CONTRACT|WORKER_CONTRACT]].",
        "",
        "Status: merged from reviewed shard outputs after Q1 repair review and Q2 fan-in repair. Extraction workers must not edit canonical ledgers.",
        "",
        "## Citation edges",
        "",
        row(keys),
        row(["---"] * len(keys)),
    ]
    for rec in citation_rows():
        body.append(row([rec.get(k, "not specified") for k in keys], keys))
    body.extend([
        "",
        "## Fan-in notes",
        "- Distinguish related-work-only citations from benchmark reuse, method extension, critique, shared dataset, shared repo, or shared model edges.",
    ])
    (LEDGERS / "citation-ledger.md").write_text("\n".join(body) + "\n", encoding="utf-8")


def write_equation() -> None:
    keys = ["mechanism_id", "benchmark_id", "metric_name", "model_roles", "paper_claim", "evidence_anchor", "transfer_question", "confidence", "record_status"]
    body = [
        "---",
        "title: Equation Benchmark Model Map",
        f"created: {TODAY}",
        f"updated: {TODAY}",
        "type: query",
        "tags: [ploke, scoring, source-ledger, research, benchmark]",
        "sources:",
        "  - queries/ploke/selection-scoring/WORKER_CONTRACT.md",
        "  - queries/ploke/selection-scoring/comparison-index.md",
        "confidence: medium",
        "contested: false",
        "---",
        "# Equation Benchmark Model Map",
        "",
        "Packet hub: [[queries/ploke/selection-scoring/comparison-index|comparison-index]]. Contract: [[queries/ploke/selection-scoring/WORKER_CONTRACT|WORKER_CONTRACT]].",
        "",
        "Status: merged from reviewed shard outputs after Q1 repair review and Q2 fan-in repair. Extraction workers must not edit canonical ledgers.",
        "",
        "## Equation-benchmark-model edges",
        "",
        row(keys),
        row(["---"] * len(keys)),
    ]
    for rec in equation_rows():
        body.append(row([rec.get(k, "not specified") for k in keys], keys))
    body.extend([
        "",
        "## Fan-in notes",
        "- Only include edges with source anchors.",
        "- Separate `improves`, `evaluates`, `trains`, `filters`, `selects`, and `reports only` paper claims.",
        "- M8 edges did not carry confidence/status in the shard, so Q2 fan-in sets them to medium/draft as required by Q1.",
    ])
    (LEDGERS / "equation-benchmark-map.md").write_text("\n".join(body) + "\n", encoding="utf-8")


def table_counts(path: Path) -> tuple[int, list[int]]:
    counts = []
    for line in lines(path):
        if line.startswith("|"):
            counts.append(line.strip().strip("|").count("|") + 1)
    return len(counts), sorted(set(counts))


def table_col_errors(path: Path) -> list[tuple[int, int, int]]:
    errors: list[tuple[int, int, int]] = []
    expected: int | None = None
    for lineno, line in enumerate(lines(path), 1):
        if not line.startswith("|"):
            expected = None
            continue
        cols = line.strip().strip("|").count("|") + 1
        if expected is None:
            expected = cols
        elif cols != expected:
            errors.append((lineno, expected, cols))
    return errors


def table_records_by_first_col(path: Path, first_col: str) -> list[dict[str, str]]:
    out: list[dict[str, str]] = []
    header: list[str] | None = None
    collecting = False
    for line in lines(path):
        if not line.startswith("|"):
            collecting = False
            header = None
            continue
        parts = [p.strip() for p in line.strip().strip("|").split("|")]
        if not parts:
            continue
        if all(set(p) <= {"-", ":", " "} for p in parts):
            continue
        if parts[0] == first_col:
            header = parts
            collecting = True
            continue
        if collecting and header is not None and len(parts) == len(header):
            out.append({h: clean_value(v, h) for h, v in zip(header, parts)})
    return out


def validate() -> dict[str, object]:
    files = [
        "mechanism-ledger.md",
        "benchmark-ledger.md",
        "model-prompt-ledger.md",
        "repo-ledger.md",
        "citation-ledger.md",
        "equation-benchmark-map.md",
    ]
    table_summary = {f: table_counts(LEDGERS / f) for f in files}
    bad_columns = {f: table_col_errors(LEDGERS / f) for f in files if table_col_errors(LEDGERS / f)}

    mech_records = table_records_by_first_col(LEDGERS / "mechanism-ledger.md", "mechanism_id")
    bench_records = table_records_by_first_col(LEDGERS / "benchmark-ledger.md", "benchmark_id")
    edge_records = table_records_by_first_col(LEDGERS / "equation-benchmark-map.md", "mechanism_id")
    mech_ids = {r["mechanism_id"] for r in mech_records}
    bench_ids = {r["benchmark_id"] for r in bench_records}
    dangling_mech = sorted({e["mechanism_id"] for e in edge_records if e.get("mechanism_id") not in mech_ids})
    dangling_bench = sorted({e["benchmark_id"] for e in edge_records if e.get("benchmark_id") not in bench_ids})

    all_text = "\n".join(read(LEDGERS / f) for f in files)
    old_bad = [
        "model-prompt-tool-ledger.md",
        "/home/brasides/wiki/queries/ploke/selection-scoring/ledgers/mechanism-ledger.md | archive admission",
        "/home/brasides/wiki/queries/ploke/selection-scoring/ledgers/citation-ledger.md",
    ]
    bad_patterns = [p for p in old_bad if p in all_text]

    row_counts = {
        "mechanism_rows": len(mech_records),
        "benchmark_rows": len(bench_records),
        "model_rows": len(table_records_by_first_col(LEDGERS / "model-prompt-ledger.md", "paper_id")),
        "repo_rows": len(table_records_by_first_col(LEDGERS / "repo-ledger.md", "paper_id")),
        "citation_rows": len(table_records_by_first_col(LEDGERS / "citation-ledger.md", "source_paper_id")),
        "equation_rows": len(edge_records),
    }
    return {
        "row_counts": row_counts,
        "table_columns": table_summary,
        "bad_column_files": bad_columns,
        "dangling_mechanism_ids": dangling_mech,
        "dangling_benchmark_ids": dangling_bench,
        "old_bad_patterns": bad_patterns,
    }


def main() -> None:
    LEDGERS.mkdir(parents=True, exist_ok=True)
    write_mechanism()
    write_benchmark()
    write_model()
    write_repo()
    write_citation()
    write_equation()
    result = validate()
    print(result)
    if result["bad_column_files"] or result["dangling_mechanism_ids"] or result["dangling_benchmark_ids"] or result["old_bad_patterns"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
