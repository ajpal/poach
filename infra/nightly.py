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
OUTPUT_DIR = NIGHTLY_DIR / "output"
DATA_DIR = OUTPUT_DIR / "data"
REPORT_OUTPUT_DIR = DATA_DIR / "reports"
DATA_JSON_PATH = DATA_DIR / "data.json"
POACH_BIN = REPO_ROOT / "target" / "release" / "poach"
MODEL_FILE = REPO_ROOT / "empty.model"


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit(f"Usage: {Path(sys.argv[0]).name} <benchmark-dir>")

    benchmark_dir = (REPO_ROOT / sys.argv[1]).resolve()

    benchmark_files = list(benchmark_dir.rglob("*.egg"))
    if not benchmark_files:
        raise SystemExit(
            f"No .egg benchmark files found under {benchmark_dir}."
        )

    run_benchmarks(benchmark_dir)
    data = aggregate_reports(benchmark_dir)

    DATA_DIR.mkdir(parents=True, exist_ok=True)
    DATA_JSON_PATH.write_text(json.dumps(data, indent=2), encoding="utf-8")
    print(f"Wrote {DATA_JSON_PATH}")

def run_benchmarks(benchmark_dir: Path) -> None:
    if REPORT_OUTPUT_DIR.exists():
        shutil.rmtree(REPORT_OUTPUT_DIR)
    REPORT_OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    for benchmark_file in benchmark_dir.rglob("*.egg"):
        relative_path = benchmark_file.relative_to(benchmark_dir)
        report_path = REPORT_OUTPUT_DIR / relative_path.with_suffix(".report.json")
        report_path.parent.mkdir(parents=True, exist_ok=True)

        command = [
            str(POACH_BIN),
            "serve",
            "--debug",
            str(MODEL_FILE),
            "single",
            str(benchmark_file),
        ]
        print("Running benchmark:", " ".join(command))
        with report_path.open("w", encoding="utf-8") as report_file:
            subprocess.run(
                command,
                check=True,
                cwd=REPO_ROOT,
                stdout=subprocess.DEVNULL,
                stderr=report_file,
            )
def aggregate_reports(benchmark_dir: Path) -> dict[str, Any]:
    report_files = list(REPORT_OUTPUT_DIR.rglob("*.report.json"))
    if not report_files:
        raise SystemExit(f"No report files were generated under {REPORT_OUTPUT_DIR}")

    reports = []
    total_runtime_ms = 0
    for report_file in report_files:
        report = json.loads(report_file.read_text(encoding="utf-8"))
        reports.append({
            "path": str(report_file.relative_to(REPORT_OUTPUT_DIR)),
            "report": report,
        })
        total_runtime_ms += report.get("timing", {}).get("total", 0)

    return {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "suite": benchmark_dir.name,
        "benchmark_root": str(benchmark_dir.relative_to(REPO_ROOT)),
        "summary": {
            "benchmarks": len(reports),
            "total_runtime_ms": total_runtime_ms,
        },
        "reports": reports,
    }


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as err:
        print(f"Command failed with exit code {err.returncode}: {err.cmd}", file=sys.stderr)
        raise
