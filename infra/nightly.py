#!/usr/bin/env python3

from __future__ import annotations

import json
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parent.parent
NIGHTLY_DIR = REPO_ROOT / "nightly"
OUTPUT_DIR = NIGHTLY_DIR / "output"
DATA_DIR = OUTPUT_DIR / "data"
REPORT_OUTPUT_DIR = DATA_DIR / "reports"
DATA_JSON_PATH = DATA_DIR / "data.json"
POACH_BIN = REPO_ROOT / "target" / "release" / "poach"


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit(f"Usage: {Path(sys.argv[0]).name} <benchmark-dir>")

    benchmark_root = (REPO_ROOT / sys.argv[1]).resolve()
    benchmark_dirs = list(
        path
        for path in benchmark_root.iterdir()
        if path.is_dir() and any((path / "train").glob("*.egg"))
    )
    if not benchmark_dirs:
        raise SystemExit(f"No benchmark suite directories found under {benchmark_root}.")

    benchmark_results = run_benchmarks(benchmark_dirs)
    data = aggregate_reports(benchmark_dirs, benchmark_results)

    DATA_DIR.mkdir(parents=True, exist_ok=True)
    DATA_JSON_PATH.write_text(json.dumps(data, indent=2), encoding="utf-8")
    print(f"Wrote {DATA_JSON_PATH}")

def run_command(command: list[str], *, cwd: Path, report_path: Path) -> dict[str, Any]:
    started = time.perf_counter()
    try:
        completed = subprocess.run(
            command,
            check=True,
            cwd=cwd,
            capture_output=True,
            text=True,
        )
    except subprocess.CalledProcessError as err:
        time_seconds = time.perf_counter() - started
        print(
            f"Command failed after {time_seconds:.3f}s: {' '.join(command)}",
            file=sys.stderr,
        )
        raise err

    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(completed.stderr, encoding="utf-8")

    return {
        "argv": command,
        "cwd": str(cwd),
        "report_path": str(report_path),
        "returncode": completed.returncode,
        "time_seconds": time.perf_counter() - started,
    }

def run_benchmarks(benchmark_dirs: list[Path]) -> list[dict[str, Any]]:
    if REPORT_OUTPUT_DIR.exists():
        shutil.rmtree(REPORT_OUTPUT_DIR)
    REPORT_OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    results: list[dict[str, Any]] = []
    # TODO: invoke the poach commands appropriate for this branch (e.g.
    # `poach train ...` and/or `poach serve ...`) for each benchmark file
    # and append one result dict per command to `results`. Each result
    # must include at least: "suite", "benchmark_path", "report_path",
    # "time_seconds" (plus any branch-specific fields like "phase").

    # for each benchmark:
        # run train/<file.egg> to produce model at out/models/<file.model.json> and report at out/reports/<file.train.json>
        #
        # run serve train/<file.egg> models/<file.model.json> to produce report at out/reports/<file.serve.json>
        # Note that we serve using the train file as well-- the serve file only has extractions and assumes it will
        # be starting from a deserialized egraph

        # add to results:
        {
            "suite": benchmark_dir.name,
            "benchmark_name": benchmark_name,
            "time_seconds": 0, # sum of total time for train and serve
            "train": {
                # aggregate from report
                "rules_time_ms": 0,
                "extract_time_ms": 0,
                "serialize_time_ms": 0,
                "other_time_ms": 0,
                "egraph_size": 0,
                "model_size": 0,
                "best_cache_num_keys": 0,
                "variants_cache_num_keys": 0,
                "avg_num_variants": 0
            },
            "serve": {
                # aggregate from report
                "deserialize_time_ms": 0,
                "rules_time_ms": 0,
                "extract_time_ms": 0, # expect to be 0
                "cache hit rate": 0 # expect to be 100%
            }
        }

        # delete model and reports since we don't need it anymore and keeping them all around gets a bit big

    return results


def display_path(p: Path) -> str:
    # Used for serializing benchmark roots into data.json. Falls back to the
    # absolute path when the benchmarks dir lives outside REPO_ROOT (e.g. when
    # the combined-nightly orchestrator points POACH_BENCHMARKS_DIR at a
    # shared clone above the worktree).
    try:
        return str(p.relative_to(REPO_ROOT))
    except ValueError:
        return str(p)


def summarize_report(report: dict[str, Any]) -> dict[str, int]:
    rule_running_millis = 0
    extraction_millis = 0
    other_millis = 0

    for timing in report["timings"]:
        if "running_rules" in timing["tags"]:
            rule_running_millis += timing["total"]
        elif "extraction" in timing["tags"]:
            extraction_millis += timing["total"]
        else:
            other_millis += timing["total"]

    return {
        "rule_running_millis": rule_running_millis,
        "extraction_millis": extraction_millis,
        "other_millis": other_millis,
        "timing_steps": len(report["timings"]),
    }


def aggregate_reports(benchmark_results: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "benchmark_root": display_path(benchmark_root),
        "summary": {
            "benchmark_count": len(benchmark_results),
            "total_time_seconds": sum(
                result["time_seconds"] for result in benchmark_results
            ),
        },
        "suites": {
            "suite_name": [], # reports for that suite
            # for each suite
        },
    }


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as err:
        print(f"Command failed with exit code {err.returncode}: {err.cmd}", file=sys.stderr)
        raise
