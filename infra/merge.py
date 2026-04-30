#!/usr/bin/env python3

"""Merge per-branch nightly data.json files into a unified data.json.

Each per-branch run (driven from infra/nightly.sh on this combined branch)
drops its data.json under nightly/output/data/<branch>/. This script reads
those, joins benchmarks across branches by (suite, benchmark_path), and
writes a unified nightly/output/data/data.json that the combined frontend
consumes.

The shape per-benchmark is:

    {
      "suite": ...,
      "benchmark_path": ...,
      "branches": {
        "<branch>": {
          "phases": {
            "<phase or '_'>": <per-branch report dict, with `path` rewritten
                              to live under data/<branch>/...>
          }
        },
        ...
      }
    }

"phase" is preserved when present (e.g. the serialize branch emits "train"
and "serve") and falls back to "_" when absent (vanilla branch). Fields are
unioned across branches; missing entries are simply absent.
"""

from __future__ import annotations

import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parent.parent
DATA_DIR = REPO_ROOT / "nightly" / "output" / "data"
DATA_JSON_PATH = DATA_DIR / "data.json"


def main() -> None:
    if len(sys.argv) < 2:
        raise SystemExit(
            f"Usage: {Path(sys.argv[0]).name} <branch> [<branch> ...]"
        )
    branches = sys.argv[1:]

    per_branch: dict[str, dict[str, Any]] = {}
    for branch in branches:
        path = DATA_DIR / branch / "data.json"
        if not path.is_file():
            raise SystemExit(f"ERROR: missing per-branch data.json: {path}")
        per_branch[branch] = json.loads(path.read_text(encoding="utf-8"))

    merged = merge(branches, per_branch)
    DATA_JSON_PATH.write_text(json.dumps(merged, indent=2), encoding="utf-8")
    print(f"Wrote {DATA_JSON_PATH}")


def merge(
    branches: list[str], per_branch: dict[str, dict[str, Any]]
) -> dict[str, Any]:
    benchmarks: dict[tuple[str, str], dict[str, Any]] = {}

    for branch, data in per_branch.items():
        for report in data.get("reports", []):
            suite = report.get("suite", "")
            benchmark_path = report.get("benchmark_path", "")
            phase = report.get("phase")
            phase_key = phase if phase is not None else "_"

            entry = benchmarks.setdefault(
                (suite, benchmark_path),
                {"suite": suite, "benchmark_path": benchmark_path, "branches": {}},
            )
            branch_entry = entry["branches"].setdefault(branch, {"phases": {}})

            rebased = dict(report)
            if "path" in rebased and rebased["path"]:
                rebased["path"] = f"{branch}/{rebased['path']}"
            branch_entry["phases"][phase_key] = rebased

    suites: dict[str, dict[str, Any]] = {}
    for branch, data in per_branch.items():
        for suite in data.get("suites", []):
            entry = suites.setdefault(
                suite["name"], {"name": suite["name"], "branches": {}}
            )
            entry["branches"][branch] = {
                "total_time_seconds": suite.get("summary", {}).get(
                    "total_time_seconds", 0
                ),
                "report_count": len(suite.get("reports", [])),
            }

    return {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "branches": branches,
        "summary": {
            "branches": {
                branch: {
                    "benchmark_count": data.get("summary", {}).get(
                        "benchmark_count", 0
                    ),
                    "total_time_seconds": data.get("summary", {}).get(
                        "total_time_seconds", 0
                    ),
                }
                for branch, data in per_branch.items()
            },
        },
        "suites": sorted(suites.values(), key=lambda s: s["name"]),
        "benchmarks": sorted(
            benchmarks.values(),
            key=lambda b: (b["suite"], b["benchmark_path"]),
        ),
    }


if __name__ == "__main__":
    main()
