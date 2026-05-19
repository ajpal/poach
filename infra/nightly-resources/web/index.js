import { convertToTable } from "./table.js";
import {
  formatTime,
  fmtSpeedup,
  fmtPct,
  fmtSize,
  unwrapCount,
  average,
} from "./util.js";

const STATE = {
  activeSuite: null,
  timeDisplay: "readable",
};

const GLOBAL_DATA = {
  data: null,
  suites: null,
};

load();

async function load() {
  const statusNode = document.querySelector("#status");

  const response = await fetch("./data/data.json");
  if (!response.ok) {
    statusNode.textContent = `Failed to load data/data.json: ${response.status} ${response.statusText}`;
    return;
  }

  GLOBAL_DATA.data = await response.json();
  statusNode.textContent = "Loaded data/data.json";

  GLOBAL_DATA.suites = [
    ...new Set(GLOBAL_DATA.data.passing_benchmarks.map((x) => x.suite_name)),
  ].sort();
  STATE.activeSuite = GLOBAL_DATA.suites[0] ?? null;

  setupSuiteSelectors();
  setupTimeDisplaySelector();
  render();
}

// ─── Formatting ─────────────────────────────────────────────────────────

const displayTime = (v) => formatTime(v, STATE.timeDisplay);

// ─── Metrics ───────────────────────────────────────────────────────────-

function speedup(b) {
  const t = b.train.report.run_program;
  const s = b.serve.wall_time_micros;
  return t === 0 || s === 0 ? null : t / s;
}

function cacheHitRate(b) {
  const hits = unwrapCount(b.serve.report.cache_hits);
  const misses = unwrapCount(b.serve.report.cache_misses);
  const total = hits + misses;
  return total === 0 ? null : (hits / total) * 100;
}

function totalTime(benchmarks) {
  return benchmarks.reduce(
    (sum, b) => sum + b.train.wall_time_micros + b.serve.wall_time_micros,
    0,
  );
}

// ─── Interaction ────────────────────────────────────────────────────────

function setupTimeDisplaySelector() {
  for (const radio of document.querySelectorAll('input[name="time-display"]')) {
    radio.addEventListener("change", () => {
      if (radio.checked) {
        STATE.timeDisplay = radio.value;
        render();
      }
    });
  }
}

function setupSuiteSelectors() {
  document.querySelector("#suite-tabs").innerHTML = GLOBAL_DATA.suites
    .map(
      (suite) =>
        `<button
    type="button"
    class="suite-tab ${suite === STATE.activeSuite ? " is-active" : ""}"
    data-suite-name="${suite}"
      >
      ${suite}
      </button>
      `,
    )
    .join("");

  for (const button of document.querySelectorAll(".suite-tab")) {
    button.addEventListener("click", () => {
      STATE.activeSuite = button.dataset.suiteName;
      for (const btn of document.querySelectorAll(".suite-tab")) {
        btn.classList.toggle(
          "is-active",
          btn.dataset.suiteName === STATE.activeSuite,
        );
      }
      renderTable();
    });
  }
}

// ─── Rendering ──────────────────────────────────────────────────────────

function render() {
  renderSummary();
  renderTable();
}

function renderSummary() {
  const passing = GLOBAL_DATA.data.passing_benchmarks;
  const numFailing = GLOBAL_DATA.data.failing_benchmarks.length;

  const speedups = passing.map(speedup).filter((v) => v !== null);
  const egraphSizes = passing.map((x) =>
    unwrapCount(x.serve.report.egraph_size),
  );

  document.querySelector("#summary-text").textContent =
    `Passing Benchmarks: ${passing.length} | ` +
    `Failing Benchmarks: ${numFailing} | ` +
    `Total time: ${displayTime(totalTime(passing))} | ` +
    `Min speedup: ${fmtSpeedup(speedups.length ? Math.min(...speedups) : null)} | ` +
    `Max speedup: ${fmtSpeedup(speedups.length ? Math.max(...speedups) : null)} | ` +
    `Avg speedup: ${fmtSpeedup(average(speedups))} | ` +
    `Avg egraph size: ${fmtSize(average(egraphSizes))}`;
}

function renderTable() {
  const benchmarks = GLOBAL_DATA.data.passing_benchmarks.filter(
    (x) => x.suite_name === STATE.activeSuite,
  );
  document.querySelector("#active-suite-summary").innerHTML = `
  <div>
    <h3>${STATE.activeSuite}</h3>
    <p>${benchmarks.length} benchmarks | ${displayTime(totalTime(benchmarks))}</p>
  </div>`;

  const columns = [
    "Benchmark",
    "Program Run Time",
    "Train Total Time",
    "Serve Total Time",
    "Speedup",
  ];

  const rows = benchmarks.map((b) => ({
    Benchmark: b.benchmark_name,
    "Program Run Time": b.train.report.run_program,
    "Train Total Time": b.train.wall_time_micros,
    "Serve Total Time": b.serve.wall_time_micros,
    Speedup: speedup(b),
    _benchmark: b,
  }));

  const displayFns = {
    "Program Run Time": displayTime,
    "Train Total Time": displayTime,
    "Serve Total Time": displayTime,
    Speedup: fmtSpeedup,
  };

  const tableDiv = document.querySelector("#active-suite-table");
  tableDiv.innerHTML = "";
  tableDiv.appendChild(
    convertToTable(columns, rows, displayFns, renderBenchmarkDetail),
  );
}

function renderBenchmarkDetail(row) {
  const train = row._benchmark.train.report;
  const serve = row._benchmark.serve.report;

  const elt = document.createElement("div");
  elt.className = "benchmark-detail";
  elt.innerHTML = `
    <div class="detail-columns">
      <div class="detail-column">
        <h4>Train</h4>
        <ul>
          <li>Running Rules: ${displayTime(train.rule_micros)}</li>
          <li>Extraction: ${displayTime(train.extraction_micros)}</li>
          <li>Serialization: ${displayTime(train.serialize)}</li>
          <li>EGraph size: ${fmtSize(unwrapCount(train.egraph_num_tuples))}</li>
        </ul>
      </div>
      <div class="detail-column">
        <h4>Serve</h4>
        <ul>
          <li>Deserialization: ${displayTime(serve.deserialize)}</li>
          <li>Cache overhead: ${displayTime(serve.cache_overhead)}</li>
          <li>Running Rules: ${displayTime(serve.rule_micros)}</li>
          <li>Extraction: ${displayTime(serve.extraction_micros)}</li>
          <li>EGraph size: ${fmtSize(unwrapCount(serve.egraph_size))}</li>
          <li>Cache hit rate: ${fmtPct(cacheHitRate(row._benchmark))}</li>
        </ul>
      </div>
    </div>
  `;
  return elt;
}
