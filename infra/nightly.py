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

    benchmark_root = benchmark_dirs[0].parent
    results = []
    for benchmark_dir in benchmark_dirs:
        benchmark_files = list((benchmark_dir / "train").glob("*.egg"))
        for benchmark_file in benchmark_files:
            relative_benchmark = benchmark_file.relative_to(benchmark_dir / "train")
            bench_out_dir = REPORT_OUTPUT_DIR / benchmark_dir.name / relative_benchmark.with_suffix("")
            bench_out_dir.mkdir(parents=True, exist_ok=True)

            train_command = [
                str(POACH_BIN),
                "train",
                "--debug",
                str(benchmark_file),
                str(bench_out_dir / "model.fbs"),
            ]
            print("Running benchmark train:", " ".join(train_command))
            train_result = run_command(train_command, cwd=REPO_ROOT, report_path=(bench_out_dir / "train.report.json"))
            results.append({
                "suite": benchmark_dir.name,
                "benchmark_path": str(relative_benchmark),
                "phase": "train",
                **{k: train_result[k] for k in ("argv", "cwd", "report_path", "returncode", "time_seconds")},
            })

            serve_command = [
                str(POACH_BIN),
                "serve",
                "--debug",
                str(bench_out_dir / "model.fbs"),
                "single",
                str(benchmark_dir / "serve" / relative_benchmark),
            ]
            print("Running benchmark serve:", " ".join(serve_command))
            serve_result = run_command(serve_command, cwd=REPO_ROOT, report_path=(bench_out_dir / "serve.report.json"))
            results.append({
                "suite": benchmark_dir.name,
                "benchmark_path": str(relative_benchmark),
                "phase": "serve",
                **{k: serve_result[k] for k in ("argv", "cwd", "report_path", "returncode", "time_seconds")},
            })

            # Serialized egraphs are large; drop each one as soon as serve
            # has consumed it so the nightly's disk footprint stays bounded.
            model_path = bench_out_dir / "model.fbs"
            if model_path.exists():
                model_path.unlink()
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
    serialize_millis = 0
    deserialize_millis = 0
    other_millis = 0
    total_millis = 0

    for timing in report["timings"]:
        total_millis += timing["total"]
        if "running_rules" in timing["tags"]:
            rule_running_millis += timing["total"]
        elif "extraction" in timing["tags"]:
            extraction_millis += timing["total"]
        elif timing["name"] == "serialize_model":
            serialize_millis += timing["total"]
            other_millis += timing["total"]
        elif timing["name"] == "deserialize_model":
            deserialize_millis += timing["total"]
            other_millis += timing["total"]
        else:
            other_millis += timing["total"]

    # report["sizes"] is a list of {"name": str, "value": {"Bytes"|"Count": int}}
    # Re-key by name for direct lookup.
    sizes = {size["name"]: size["value"] for size in report.get("sizes", [])}
    model_size_bytes = sizes.get("model_bytes", {}).get("Bytes", 0)
    egraph_tuples = sizes.get("egraph_tuples", {}).get("Count", 0)

    return {
        "rule_running_millis": rule_running_millis,
        "extraction_millis": extraction_millis,
        "serialize_millis": serialize_millis,
        "deserialize_millis": deserialize_millis,
        "other_millis": other_millis,
        "total_millis": total_millis,
        "timing_steps": len(report["timings"]),
        "model_size_bytes": model_size_bytes,
        "egraph_tuples": egraph_tuples,
    }


def aggregate_reports(
    benchmark_dirs: list[Path], benchmark_results: list[dict[str, Any]]
) -> dict[str, Any]:
    if not benchmark_results:
        raise SystemExit(f"No report files were generated under {REPORT_OUTPUT_DIR}")

    benchmark_root = benchmark_dirs[0].parent
    suites = []
    for benchmark_dir in benchmark_dirs:
        suite_results = [
            result for result in benchmark_results if result["suite"] == benchmark_dir.name
        ]
        suites.append(
            {
                "name": benchmark_dir.name,
                "benchmark_root": display_path(benchmark_dir),
                "summary": {
                    "total_time_seconds": sum(
                        result["time_seconds"] for result in suite_results
                    ),
                },
                "reports": [
                    {
                        "suite": benchmark_dir.name,
                        "benchmark_path": result["benchmark_path"],
                        "phase": result["phase"],
                        "path": str(
                            Path(result["report_path"]).relative_to(OUTPUT_DIR)
                        ),
                        "time_seconds": result["time_seconds"],
                        "timing_summary": summarize_report(
                            json.loads(
                                Path(result["report_path"]).read_text(
                                    encoding="utf-8"
                                )
                            )
                        ),
                    }
                    for result in suite_results
                ],
            }
        )

    return {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "benchmark_root": display_path(benchmark_root),
        "summary": {
            "benchmark_count": len(benchmark_results),
            "total_time_seconds": sum(
                result["time_seconds"] for result in benchmark_results
            ),
        },
        "suites": suites,
        "reports": [report for suite in suites for report in suite["reports"]],
    }


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as err:
        print(f"Command failed with exit code {err.returncode}: {err.cmd}", file=sys.stderr)
        raise
