#!/usr/bin/env python3
"""
Fetch OpenRouter model metadata and materialize the pricing slice consumed by ploke-tui tests.

Usage:
    ./scripts/openrouter_pricing_sync.py

Outputs:
    crates/ploke-tui/data/models/all_pricing_parsed.json
"""
from __future__ import annotations

import json
import pathlib
import sys
import urllib.request


OPENROUTER_MODELS_URL = "https://openrouter.ai/api/v1/models"
OUTPUT_PATH = pathlib.Path("crates/ploke-tui/data/models/all_pricing_parsed.json")


def fetch_models() -> dict:
    with urllib.request.urlopen(OPENROUTER_MODELS_URL) as resp:
        if resp.status != 200:
            raise RuntimeError(f"Unexpected status {resp.status} fetching {OPENROUTER_MODELS_URL}")
        return json.load(resp)


def extract_pricing(models_payload: dict) -> list[dict]:
    data = models_payload.get("data", [])
    return [entry.get("pricing", {}) for entry in data]


def write_pricing(pricing: list[dict]) -> None:
    OUTPUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT_PATH.write_text(json.dumps(pricing, indent=2))
    print(f"Wrote {len(pricing)} pricing entries to {OUTPUT_PATH}")


def main() -> int:
    payload = fetch_models()
    pricing = extract_pricing(payload)
    if not pricing:
        print("No pricing data returned from OpenRouter response.", file=sys.stderr)
        return 1
    write_pricing(pricing)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
