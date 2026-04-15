#!/usr/bin/env python3

from __future__ import annotations

import json
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parent.parent
NIGHTLY_DIR = REPO_ROOT / "nightly"
TMP_DIR = NIGHTLY_DIR / "tmp"
OUTPUT_DIR = NIGHTLY_DIR / "output"
DATA_DIR = OUTPUT_DIR / "data"
BENCHMARK_REPO_DIR = REPO_ROOT / "tests" / "passing"
REPORT_OUTPUT_DIR = DATA_DIR / "reports"
DATA_JSON_PATH = DATA_DIR / "data.json"
POACH_BIN = REPO_ROOT / "target" / "release" / "poach"


def main() -> None:
    ensure_prerequisites()

    benchmark_files = sorted(BENCHMARK_REPO_DIR.rglob("*.egg"))
    if not benchmark_files:
        raise SystemExit(
            f"No .egg benchmark files found under {BENCHMARK_REPO_DIR}."
        )

    run_batch_reports()
    data = aggregate_reports()

    DATA_DIR.mkdir(parents=True, exist_ok=True)
    DATA_JSON_PATH.write_text(json.dumps(data, indent=2), encoding="utf-8")
    print(f"Wrote {DATA_JSON_PATH}")


def ensure_prerequisites() -> None:
    if not BENCHMARK_REPO_DIR.is_dir():
        raise SystemExit(
            f"Benchmark path not found at {BENCHMARK_REPO_DIR}."
        )
    if not POACH_BIN.is_file():
        raise SystemExit(
            f"Expected release binary at {POACH_BIN}. "
            "Run cargo build --release before nightly.py."
        )
    DATA_DIR.mkdir(parents=True, exist_ok=True)

def run_batch_reports() -> None:
    if REPORT_OUTPUT_DIR.exists():
        shutil.rmtree(REPORT_OUTPUT_DIR)
    REPORT_OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    command = [
        str(POACH_BIN),
        "serve",
        "empty.model",
        "batch",
        str(BENCHMARK_REPO_DIR),
        str(REPORT_OUTPUT_DIR),
    ]
    print("Running benchmarks:", " ".join(command))
    subprocess.run(command, check=True, cwd=REPO_ROOT)


def aggregate_reports() -> dict[str, Any]:
    report_files = sorted(REPORT_OUTPUT_DIR.rglob("*.report.json"))
    if not report_files:
        raise SystemExit(f"No report files were generated under {REPORT_OUTPUT_DIR}")

    benchmarks = []
    running_rules_total = 0
    extraction_total = 0
    other_total = 0

    for report_path in report_files:
        report = json.loads(report_path.read_text(encoding="utf-8"))
        relative_report_path = report_path.relative_to(OUTPUT_DIR).as_posix()
        relative_benchmark_path = report_path.relative_to(REPORT_OUTPUT_DIR).with_suffix(".egg")

        timing = report.get("timing", {})
        steps = timing.get("steps", [])
        running_rules_ms = sum(
            step.get("total", 0)
            for step in steps
            if "running_rules" in step.get("tags", [])
        )
        extraction_ms = sum(
            step.get("total", 0)
            for step in steps
            if "extraction" in step.get("tags", [])
        )
        other_ms = sum(
            step.get("total", 0)
            for step in steps
            if "other" in step.get("tags", [])
        )

        running_rules_total += running_rules_ms
        extraction_total += extraction_ms
        other_total += other_ms

        metrics = extract_size_metrics(report.get("sizes", []))
        benchmarks.append(
            {
                "name": relative_benchmark_path.stem,
                "path": relative_benchmark_path.as_posix(),
                "report_path": relative_report_path,
                "running_rules_ms": running_rules_ms,
                "extraction_ms": extraction_ms,
                "other_ms": other_ms,
                "total_tagged_ms": running_rules_ms + extraction_ms + other_ms,
                "report_total_ms": timing.get("total", 0),
                "metrics": metrics,
                "steps": steps,
            }
        )

    benchmarks.sort(key=lambda row: row["total_tagged_ms"], reverse=True)

    return {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "suite": BENCHMARK_REPO_DIR.name,
        "benchmark_root": str(BENCHMARK_REPO_DIR.relative_to(REPO_ROOT)),
        "summary": {
            "benchmark_count": len(benchmarks),
            "running_rules_ms": running_rules_total,
            "extraction_ms": extraction_total,
            "other_ms": other_total,
            "total_tagged_ms": running_rules_total + extraction_total + other_total,
        },
        "benchmarks": benchmarks,
    }


def extract_size_metrics(size_entries: list[dict[str, Any]]) -> dict[str, int]:
    metrics: dict[str, int] = {}
    for entry in size_entries:
        name = entry.get("name")
        value = entry.get("value", {})
        if not name or not isinstance(value, dict):
            continue
        if "Count" in value:
            metrics[name] = value["Count"]
        elif "Bytes" in value:
            metrics[name] = value["Bytes"]
    return metrics

if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as err:
        print(f"Command failed with exit code {err.returncode}: {err.cmd}", file=sys.stderr)
        raise
