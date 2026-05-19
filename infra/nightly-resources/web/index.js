import { convertToTable } from "./table.js";

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

  // Set up interactive elements
  setupSuiteSelectors();
  setupTimeDisplaySelector();

  render();
}

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

function displayTime(rawValue) {
  const ONE_MIN = 60000000;
  const ONE_SEC = 1000000;
  const ONE_MILLI = 1000;
  if (STATE.timeDisplay === "raw") {
    return `${rawValue} μs`;
  } else {
    console.assert(STATE.timeDisplay === "readable");
    if (rawValue >= ONE_MIN) {
      return `${(rawValue / ONE_MIN).toFixed(2)} min`;
    } else if (rawValue >= ONE_SEC) {
      return `${(rawValue / ONE_SEC).toFixed(2)} s`;
    } else if (rawValue >= ONE_MILLI) {
      return `${(rawValue / ONE_MILLI).toFixed(2)} ms`;
    } else {
      return `${rawValue} μs`;
    }
  }
}

function renderSummary() {
  const passing = GLOBAL_DATA.data.passing_benchmarks;
  const numPassing = passing.length;
  const numFailing = GLOBAL_DATA.data.failing_benchmarks.length;

  const totalTime = passing
    .map((x) => x.train.wall_time_micros + x.serve.wall_time_micros)
    .reduce((a, b) => a + b, 0);

  const speedups = passing
    .map((x) => {
      const t = x.train.report.run_program;
      const s = x.serve.wall_time_micros;
      return t === 0 || s === 0 ? null : t / s;
    })
    .filter((v) => v !== null);

  const minSpeedup = speedups.length ? Math.min(...speedups) : null;
  const maxSpeedup = speedups.length ? Math.max(...speedups) : null;
  const avgSpeedup = speedups.length
    ? speedups.reduce((a, b) => a + b, 0) / speedups.length
    : null;

  const egraphSizes = passing
    .map((x) => unwrapCount(x.serve.report.egraph_size))
    .filter((v) => v !== undefined && v !== null);
  const avgEgraphSize = egraphSizes.length
    ? egraphSizes.reduce((a, b) => a + b, 0) / egraphSizes.length
    : null;

  const fmtSpeedup = (v) => (v === null ? "—" : `${v.toFixed(2)}×`);
  const fmtSize = (v) =>
    v === null
      ? "—"
      : v.toLocaleString(undefined, { maximumFractionDigits: 0 });

  document.querySelector("#summary-text").textContent =
    `Passing Benchmarks: ${numPassing} | ` +
    `Failing Benchmarks: ${numFailing} | ` +
    `Total time: ${displayTime(totalTime)} | ` +
    `Min speedup: ${fmtSpeedup(minSpeedup)} | ` +
    `Max speedup: ${fmtSpeedup(maxSpeedup)} | ` +
    `Avg speedup: ${fmtSpeedup(avgSpeedup)} | ` +
    `Avg egraph size: ${fmtSize(avgEgraphSize)}`;
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

function unwrapCount(v) {
  if (typeof v === "number") return v;
  if (v && typeof v === "object" && "Count" in v) return v.Count;
  return 0;
}

function renderTable() {
  const benchmarks = GLOBAL_DATA.data.passing_benchmarks.filter(
    (x) => x.suite_name === STATE.activeSuite,
  );
  const totalTime = benchmarks
    .map((x) => x.train.wall_time_micros + x.serve.wall_time_micros)
    .reduce((a, b) => a + b, 0);

  document.querySelector("#active-suite-summary").innerHTML = `
  <div>
    <h3>${STATE.activeSuite}</h3>
    <p>${benchmarks.length} benchmarks | ${displayTime(totalTime)}</p>
  </div>`;

  const columns = [
    "Benchmark",
    "Program Run Time",
    "Train Total Time",
    "Serve Total Time",
    "Speedup",
  ];

  const rows = benchmarks.map((b) => {
    const trainTime =
      b.train.report.rule_micros +
      b.train.report.extraction_micros +
      b.train.report.serialize;
    const serveTime = b.serve.wall_time_micros;
    const runProgramTime = b.train.report.run_program;
    return {
      Benchmark: b.benchmark_name,
      "Train Total Time": trainTime,
      "Program Run Time": runProgramTime,
      "Serve Total Time": serveTime,
      Speedup:
        runProgramTime === 0 || serveTime === 0
          ? null
          : runProgramTime / serveTime,
      _benchmark: b,
    };
  });

  const displayFns = {
    "Train Total Time": displayTime,
    "Program Run Time": displayTime,
    "Serve Total Time": displayTime,
    Speedup: (v) => (v === null ? "—" : `${v.toFixed(2)}×`),
  };

  const tableDiv = document.querySelector("#active-suite-table");
  tableDiv.innerHTML = "";
  tableDiv.appendChild(
    convertToTable(columns, rows, displayFns, renderBenchmarkDetail),
  );
}

function renderBenchmarkDetail(row) {
  const elt = document.createElement("div");
  elt.className = "benchmark-detail";

  const train = row._benchmark.train.report;
  const serve = row._benchmark.serve.report;

  const hits = unwrapCount(serve.cache_hits);
  const misses = unwrapCount(serve.cache_misses);
  const totalLookups = hits + misses;
  const cacheHitRate = totalLookups === 0 ? null : (hits / totalLookups) * 100;

  const fmtSize = (v) =>
    v.toLocaleString(undefined, { maximumFractionDigits: 0 });
  const fmtPct = (v) => (v === null ? "—" : `${v.toFixed(1)}%`);

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
          <li>Cache hit rate: ${fmtPct(cacheHitRate)}</li>
        </ul>
      </div>
    </div>
  `;

  return elt;
}

function render() {
  renderSummary();
  renderTable();
}
