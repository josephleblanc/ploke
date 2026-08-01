#!/usr/bin/env python3
"""Clean local pdftotext extracts for LLM-readable paper notes.

This script is intentionally conservative: it does not try to reconstruct tables,
figures, or equations. It cleans the common artifacts introduced by lightweight
PDF-to-text extraction:

- standalone page-number lines
- repeated arXiv/preprint headers and simple title page headers
- control characters and form-feed leftovers
- wrapped prose lines inside paragraphs
- common spaced-small-caps artifacts such as `S ELF -SOLVE`

Run from the crate root or directly from this directory:

    python docs/scripts/clean_pdf_text.py
    python docs/scripts/clean_pdf_text.py --dry-run
"""

from __future__ import annotations

import argparse
import re
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TEXT_DIR = ROOT / "text"

CHAR_MAP = {
    "\ufb00": "ff",
    "\ufb01": "fi",
    "\ufb02": "fl",
    "\ufb03": "ffi",
    "\ufb04": "ffl",
    "\u00ad": "",
    "\x0c": "\n",
}

WORD_FIXES = [
    ("AGENT CL", "AGENTCL"),
    ("Agent CL", "AGENTCL"),
    ("agent CL", "AGENTCL"),
    ("MEM PROBE", "MemProbe"),
    ("MEMPROBE", "MemProbe"),
    ("O PCD", "O-PCD"),
    ("OPCD", "O-PCD"),
    ("ONE - SHOT", "ONE-SHOT"),
    ("ONE- SHOT", "ONE-SHOT"),
    ("ONE -SHOT", "ONE-SHOT"),
    ("SELF - SOLVE", "SELF-SOLVE"),
    ("SELF- SOLVE", "SELF-SOLVE"),
    ("SELF -SOLVE", "SELF-SOLVE"),
    ("SELF - ASK", "SELF-ASK"),
    ("SELF- ASK", "SELF-ASK"),
    ("SELF -ASK", "SELF-ASK"),
    ("TRAIN - GRPO", "TRAIN-GRPO"),
    ("TRAIN- GRPO", "TRAIN-GRPO"),
    ("TRAIN -GRPO", "TRAIN-GRPO"),
]

FINAL_FIXES = [
    ("ASELF-SOLVE", "A SELF-SOLVE"),
    ("AVESTA Details", "A VESTA Details"),
    ("ARASER workflow", "A RASER workflow"),
]

SECTION_WORDS = {
    "abstract",
    "introduction",
    "related work",
    "method",
    "methods",
    "experiments",
    "results",
    "discussion",
    "conclusion",
    "limitations",
    "references",
    "appendix",
}


def clean_chars(text: str) -> str:
    for old, new in CHAR_MAP.items():
        text = text.replace(old, new)
    text = text.replace("\r\n", "\n").replace("\r", "\n")
    return "".join(ch if ch == "\n" or ch == "\t" or ord(ch) >= 32 else " " for ch in text)


def fix_caps(line: str) -> str:
    """Repair common all-caps spacing artifacts from PDF small-caps fonts."""
    old = None
    while old != line:
        old = line
        line = re.sub(r"\b([A-Z])\s+([A-Z]{2,})(?=\b)", r"\1\2", line)
    line = re.sub(r"\b([A-Z]{2,})\s+-\s+([A-Z]{2,})\b", r"\1-\2", line)
    line = re.sub(r"\b([A-Z]{2,})-\s+([A-Z]{2,})\b", r"\1-\2", line)
    line = re.sub(r"\b([A-Z]{2,})\s+-([A-Z]{2,})\b", r"\1-\2", line)
    for old, new in WORD_FIXES:
        line = line.replace(old, new)
    return line


def norm_line(line: str) -> str:
    line = line.strip()
    line = re.sub(r"[ \t]+", " ", line)
    line = fix_caps(line)
    return line


def title_set(lines: list[str]) -> set[str]:
    heads: list[str] = []
    for line in lines:
        s = norm_line(line)
        if not s:
            continue
        if s.lower() == "abstract":
            break
        if "arxiv:" in s.lower():
            continue
        if re.fullmatch(r"\d{1,3}", s):
            continue
        heads.append(s)
        if len(heads) >= 3:
            break
    found = {h for h in heads if 8 <= len(h) <= 140}
    for idx, head in enumerate(heads[:-1]):
        joined = f"{head} {heads[idx + 1]}"
        if 8 <= len(joined) <= 160:
            found.add(joined)
    return found


def drop_line(line: str, idx: int, heads: set[str]) -> bool:
    low = line.lower()
    if not line:
        return False
    if "arxiv:" in low:
        return True
    if line == "Preprint.":
        return True
    if re.fullmatch(r"\d{1,3}", line):
        return True
    if idx > 20 and line in heads:
        return True
    return False


def is_heading(line: str) -> bool:
    low = line.lower().strip()
    if low in SECTION_WORDS:
        return True
    if re.fullmatch(r"\d+(?:\.\d+)*\s+[A-Z][^.!?]{1,90}", line):
        return True
    if len(line) <= 90 and not re.search(r"[.!?]$", line):
        words = line.split()
        if 1 <= len(words) <= 8:
            caps = sum(1 for w in words if w[:1].isupper() or w.isupper())
            if caps >= max(1, len(words) - 1):
                return True
    return False


def is_item(line: str) -> bool:
    return bool(
        re.match(r"^(?:[-*•]|\d+[.)]|[A-Z][.)])\s+", line)
        or re.match(r"^(?:Figure|Table)\s+\d+", line)
    )


def is_math(line: str) -> bool:
    if len(line) > 140:
        return False
    marks = sum(1 for ch in line if ch in "=<>≤≥→←∑∏√∫{}[]|_^")
    return marks >= 2


def join_pair(left: str, right: str) -> str:
    if re.search(r"[A-Za-z]-$", left) and re.match(r"[a-z]", right):
        return left[:-1] + right
    return left + " " + right


def split_blocks(lines: list[str]) -> list[list[str]]:
    blocks: list[list[str]] = []
    block: list[str] = []
    for line in lines:
        if line:
            block.append(line)
        elif block:
            blocks.append(block)
            block = []
    if block:
        blocks.append(block)
    return blocks


def clean_block(block: list[str]) -> list[str]:
    if len(block) == 1:
        return block
    out: list[str] = []
    cur = ""
    for line in block:
        starts = is_item(line) or is_heading(line) or is_math(line)
        if not cur:
            cur = line
            continue
        if starts:
            out.append(cur)
            cur = line
            continue
        if is_heading(cur) or is_math(cur):
            out.append(cur)
            cur = line
            continue
        if cur.startswith(("Figure ", "Table ")) and re.search(r"[.!?]$", cur):
            out.append(cur)
            cur = line
            continue
        cur = join_pair(cur, line)
    if cur:
        out.append(cur)
    return out


def join_heads(text: str) -> str:
    """Join appendix-letter headings split by PDF extraction."""
    pat = re.compile(r"(^|\n\n)([A-Z])\n\n([A-Z][A-Za-z0-9][^\n=(){}\[\]|]{2,80})")

    def repl(match: re.Match[str]) -> str:
        prefix, letter, head = match.groups()
        if len(head.split()) > 8:
            return match.group(0)
        return f"{prefix}{letter} {head.strip()}"

    return pat.sub(repl, text)


def clean_text(text: str) -> tuple[str, dict[str, int]]:
    text = clean_chars(text)
    raw = text.splitlines()
    heads = title_set(raw)
    lines: list[str] = []
    stats = Counter()
    blank = False
    for idx, raw_line in enumerate(raw):
        line = norm_line(raw_line)
        if drop_line(line, idx, heads):
            stats["dropped"] += 1
            continue
        if not line:
            if lines and not blank:
                lines.append("")
            blank = True
            continue
        lines.append(line)
        blank = False
    blocks = split_blocks(lines)
    cooked: list[str] = []
    for block in blocks:
        before = len(block)
        part = clean_block(block)
        stats["joined"] += max(0, before - len(part))
        cooked.extend(part)
        cooked.append("")
    while cooked and not cooked[-1]:
        cooked.pop()
    result = "\n".join(cooked)
    result = re.sub(r"\n{3,}", "\n\n", result).strip() + "\n"
    result = join_heads(result).strip() + "\n"
    for old, new in FINAL_FIXES:
        result = result.replace(old, new)
    return result, dict(stats)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--text-dir", type=Path, default=TEXT_DIR)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    total = Counter()
    for path in sorted(args.text_dir.glob("*.txt")):
        original = path.read_text(errors="replace")
        cleaned, stats = clean_text(original)
        for key, val in stats.items():
            total[key] += val
        total["files"] += 1
        total["old_lines"] += len(original.splitlines())
        total["new_lines"] += len(cleaned.splitlines())
        total["old_bytes"] += len(original.encode())
        total["new_bytes"] += len(cleaned.encode())
        if not args.dry_run and cleaned != original:
            path.write_text(cleaned)
        print(
            f"{path.name}: lines {len(original.splitlines())} -> {len(cleaned.splitlines())}; "
            f"dropped={stats.get('dropped', 0)} joined={stats.get('joined', 0)}"
        )
    print(
        "summary: "
        f"files={total['files']} lines {total['old_lines']} -> {total['new_lines']} "
        f"bytes {total['old_bytes']} -> {total['new_bytes']} "
        f"dropped={total['dropped']} joined={total['joined']}"
    )


if __name__ == "__main__":
    main()
