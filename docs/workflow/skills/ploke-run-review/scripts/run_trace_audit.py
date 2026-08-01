#!/usr/bin/env python3
"""Summarize ploke-eval run traces for run-review work."""

from __future__ import annotations

import argparse
import gzip
import json
from collections import Counter
from pathlib import Path
from typing import Any


def load_json(path: Path) -> Any:
    if path.suffix == ".gz":
        with gzip.open(path, "rt", encoding="utf-8") as handle:
            return json.load(handle)
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def parse_json_maybe(value: Any) -> Any:
    if not isinstance(value, str):
        return value
    try:
        return json.loads(value)
    except json.JSONDecodeError:
        return value


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    if not path.exists():
        return rows
    with path.open("r", encoding="utf-8") as handle:
        for line in handle:
            line = line.strip()
            if line:
                rows.append(json.loads(line))
    return rows


def classify_tool(tool: str, status: str, args: Any, content: Any, duplicate: bool) -> str:
    if status != "Completed":
        return "transport_failure"
    if duplicate:
        return "duplicate_request"
    if tool == "read_file" and isinstance(content, dict):
        text = content.get("content")
        text_len = len(text) if isinstance(text, str) else 0
        if content.get("ok") is True and content.get("exists") is True and text_len == 0:
            return "empty_completed_read"
        if content.get("truncated") is True and isinstance(args, dict):
            if args.get("start_line") is None and args.get("end_line") is None:
                return "truncated_full_read"
        if text_len > 0:
            return "read_with_content"
    if tool == "request_code_context" and isinstance(content, dict):
        context = content.get("context")
        if isinstance(context, list) and len(context) == 0:
            return "empty_context"
        if isinstance(context, list):
            return "context_results"
    return "completed"


def provider_rows(run_root: Path) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    rows = load_jsonl(run_root / "llm-full-responses.jsonl")
    responses: list[dict[str, Any]] = []
    calls: list[dict[str, Any]] = []
    for row in rows:
        response = row.get("response", {})
        choices = response.get("choices") or []
        choice = choices[0] if choices else {}
        message = choice.get("message") or {}
        usage = response.get("usage") or {}
        response_index = row.get("response_index")
        tool_calls = message.get("tool_calls") or []
        content = message.get("content") or ""
        responses.append(
            {
                "response_index": response_index,
                "finish_reason": choice.get("finish_reason"),
                "content_len": len(content),
                "prompt_tokens": usage.get("prompt_tokens"),
                "completion_tokens": usage.get("completion_tokens"),
                "total_tokens": usage.get("total_tokens"),
                "tool_call_count": len(tool_calls),
            }
        )
        for call in tool_calls:
            fn = call.get("function") or {}
            calls.append(
                {
                    "response_index": response_index,
                    "call_id": call.get("id"),
                    "tool": fn.get("name"),
                    "arguments": parse_json_maybe(fn.get("arguments")),
                }
            )
    return responses, calls


def recorded_rows(run_root: Path) -> list[dict[str, Any]]:
    record_path = run_root / "record.json.gz"
    if not record_path.exists():
        record_path = run_root / "record.json"
    record = load_json(record_path)
    turns = (((record.get("phases") or {}).get("agent_turns")) or [])
    if not turns:
        return []
    calls = turns[0].get("tool_calls") or []
    seen: Counter[str] = Counter()
    rows: list[dict[str, Any]] = []
    for index, call in enumerate(calls):
        request = call.get("request") or {}
        result = call.get("result") or {}
        tool = request.get("tool")
        args_raw = request.get("arguments")
        args = parse_json_maybe(args_raw)
        status = result.get("status")
        content = parse_json_maybe(result.get("content"))
        request_key = json.dumps({"tool": tool, "args": args}, sort_keys=True, default=str)
        seen[request_key] += 1
        duplicate = seen[request_key] > 1
        content_text = content.get("content") if isinstance(content, dict) else None
        context = content.get("context") if isinstance(content, dict) else None
        rows.append(
            {
                "index": index,
                "call_id": request.get("call_id"),
                "tool": tool,
                "status": status,
                "arguments": args,
                "ok": content.get("ok") if isinstance(content, dict) else None,
                "exists": content.get("exists") if isinstance(content, dict) else None,
                "file_path": content.get("file_path") if isinstance(content, dict) else None,
                "truncated": content.get("truncated") if isinstance(content, dict) else None,
                "content_len": len(content_text) if isinstance(content_text, str) else 0,
                "context_len": len(context) if isinstance(context, list) else 0,
                "classification": classify_tool(tool or "", status or "", args, content, duplicate),
            }
        )
    return rows


def audit(run_root: Path) -> dict[str, Any]:
    responses, provider_calls = provider_rows(run_root)
    recorded_calls = recorded_rows(run_root)
    provider_ids = {row["call_id"] for row in provider_calls if row.get("call_id")}
    recorded_ids = {row["call_id"] for row in recorded_calls if row.get("call_id")}
    missing_ids = sorted(provider_ids - recorded_ids)
    return {
        "run_root": str(run_root),
        "responses": responses,
        "provider_calls": provider_calls,
        "recorded_calls": recorded_calls,
        "provider_call_count": len(provider_calls),
        "recorded_call_count": len(recorded_calls),
        "missing_recorded_call_ids": missing_ids,
        "missing_recorded_provider_calls": [
            row for row in provider_calls if row.get("call_id") in missing_ids
        ],
        "extra_recorded_call_ids": sorted(recorded_ids - provider_ids),
        "recorded_classifications": Counter(row["classification"] for row in recorded_calls),
        "finish_reasons": Counter(row["finish_reason"] for row in responses),
    }


def print_markdown(data: dict[str, Any]) -> None:
    print(f"# Trace Audit\n\nRun root: `{data['run_root']}`\n")
    print("## Summary\n")
    print(f"- responses: {len(data['responses'])}")
    print(f"- provider-emitted tool calls: {data['provider_call_count']}")
    print(f"- recorded tool calls: {data['recorded_call_count']}")
    print(f"- missing recorded provider call ids: {len(data['missing_recorded_call_ids'])}")
    print(f"- extra recorded call ids: {len(data['extra_recorded_call_ids'])}")
    print(f"- finish reasons: `{dict(data['finish_reasons'])}`")
    print(f"- recorded classifications: `{dict(data['recorded_classifications'])}`")
    if data["missing_recorded_call_ids"]:
        print("\nMissing recorded provider calls:")
        for row in data["missing_recorded_provider_calls"]:
            args = json.dumps(row["arguments"], sort_keys=True)
            print(
                f"- response {row['response_index']} `{row['call_id']}` "
                f"{row['tool']} `{args}`"
            )
    print("\n## Responses\n")
    print("| index | finish | content_len | prompt | completion | total | tool_calls |")
    print("| --- | --- | ---: | ---: | ---: | ---: | ---: |")
    for row in data["responses"]:
        print(
            "| {response_index} | {finish_reason} | {content_len} | {prompt_tokens} | "
            "{completion_tokens} | {total_tokens} | {tool_call_count} |".format(**row)
        )
    print("\n## Recorded Tool Calls\n")
    print("| idx | tool | status | class | content_len | context_len | truncated | args |")
    print("| ---: | --- | --- | --- | ---: | ---: | --- | --- |")
    for row in data["recorded_calls"]:
        args = json.dumps(row["arguments"], sort_keys=True)
        if len(args) > 120:
            args = args[:117] + "..."
        print(
            f"| {row['index']} | {row['tool']} | {row['status']} | "
            f"{row['classification']} | {row['content_len']} | "
            f"{row['context_len']} | {row['truncated']} | `{args}` |"
        )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("run_root", type=Path)
    parser.add_argument("--markdown", action="store_true")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    data = audit(args.run_root)
    if args.json:
        json_ready = dict(data)
        json_ready["recorded_classifications"] = dict(data["recorded_classifications"])
        json_ready["finish_reasons"] = dict(data["finish_reasons"])
        print(json.dumps(json_ready, indent=2, sort_keys=True))
    else:
        print_markdown(data)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
