#!/usr/bin/env python3

import json
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

# Determine directories
SCRIPT_DIR = Path(__file__).resolve().parent
POACH_ROOT = SCRIPT_DIR.parent
NIGHTLY_DIR = POACH_ROOT / "nightly"
POACH_BINARY = POACH_ROOT / "target" / "release" / "poach"

def main(benchmark_dir):
  (benchmark_results, failing_benchmarks) = run_benchmarks(benchmark_dir)

  data = {
    "generated_at": datetime.now(timezone.utc).isoformat(),
    "failing_benchmarks": [str(b) for b in failing_benchmarks],
    "passing_benchmarks": benchmark_results
  }
  data_out_path = NIGHTLY_DIR / "output" / "data" / "data.json"
  data_out_path.parent.mkdir(parents=True, exist_ok=True)
  data_out_path.write_text(json.dumps(data, indent=2), encoding="utf-8")

def run_command(cmd, summarize_report):
  started = time.perf_counter_ns()
  cmd_result = subprocess.run(
    cmd,
    cwd=POACH_ROOT,
    capture_output=True,
    text=True # decode stderr/stdout as string instead of raw bytes
  )
  # Clock granularity is ~50-100 ns.
  # Report as micros to avoid reporting false precision.
  time_micros = (time.perf_counter_ns() - started) // 1000
  if cmd_result.returncode != 0:
    return {
      "cmd": " ".join(cmd),
      "status": "error",
      "wall_time_micros": time_micros
    }

  report = json.loads(cmd_result.stderr)

  return {
    "cmd": " ".join(cmd),
    "status": "success",
    "report": summarize_report(report),
    "wall_time_micros": time_micros
  }

def summarize_train_report(report):
  aggregated = {
    "total_micros": 0
  }
  
  # Aggregate time of all steps
  for time_step in report["timings"]:
    aggregated["total_micros"] += time_step["total"]

  # Rekey sizes
  for size in report["sizes"]:
    aggregated[size["name"]] = size["value"]

  return aggregated

def summarize_serve_report(report):
  aggregated = {
    "total_micros": 0
  }
  
  # Aggregate time of all steps
  for time_step in report["timings"]:
    aggregated["total_micros"] += time_step["total"]

  # Rekey sizes
  for size in report["sizes"]:
    aggregated[size["name"]] = size["value"]
  
  return aggregated

def run_benchmarks(benchmark_dir):
  # Find benchmarks
  # benchmark_dir is the root of the benchmark directory 
  benchmarks = list(Path(benchmark_dir).rglob("train/*.egg"))
  
  reports = []
  failing_benchmarks = []
  for benchmark in benchmarks:
    relative_path = benchmark.relative_to(benchmark_dir)
    suite_name = str(relative_path.parent)
    benchmark_name = relative_path.name
    
    # We don't actually save the models, it gets overwritten each iter
    # but that's okay because we don't need it after the serve command each iter
    model_path = NIGHTLY_DIR / "model.model"

    report = {
      "benchmark_name": benchmark_name,
      "suite_name": suite_name
    }

    # Train
    train_command = [
      str(POACH_BINARY),
      "train",
      "--debug",
      str(benchmark),
      str(model_path)
    ]
    train_result = run_command(train_command, summarize_train_report)
    if train_result["status"] == "success":
      report["train"] = train_result
    else:
      failing_benchmarks.append(relative_path)
      continue

    # Serve
    serve_command = [
      str(POACH_BINARY),
      "serve",
      "--debug",
      str(model_path),
      "single",
      str(benchmark)
    ]
    
    serve_result = run_command(serve_command, summarize_serve_report)
    if serve_result["status"] == "success":
      report["serve"] = serve_result
      reports.append(report)
    else:
      failing_benchmarks.append(relative_path)

  return (reports, failing_benchmarks)

if __name__ == "__main__":
  if len(sys.argv) != 2:
    raise SystemExit(f"Usage: nightly.py <benchmark-dir>")
  
  main(sys.argv[1])
