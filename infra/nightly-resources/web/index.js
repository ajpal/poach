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
      const t = x.train.wall_time_micros;
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
    "Train Total Time",
    "Serve Total Time",
    "Speedup",
    "EGraph Size",
    "Cache Hit %",
  ];

  const rows = benchmarks.map((b) => {
    const hits = unwrapCount(b.serve.report.cache_hits);
    const misses = unwrapCount(b.serve.report.cache_misses);
    const total = hits + misses;
    const trainTime = b.train.wall_time_micros;
    const serveTime = b.serve.wall_time_micros;
    return {
      Benchmark: b.benchmark_name,
      "Train Total Time": trainTime,
      "Serve Total Time": serveTime,
      Speedup:
        trainTime === 0 || serveTime === 0 ? null : trainTime / serveTime,
      "EGraph Size": unwrapCount(b.serve.report.egraph_size),
      "Cache Hit %": total === 0 ? null : (hits / total) * 100,
    };
  });

  const displayFns = {
    "Train Total Time": displayTime,
    "Serve Total Time": displayTime,
    Speedup: (v) => (v === null ? "—" : `${v.toFixed(2)}×`),
    "Cache Hit %": (v) => (v === null ? "—" : `${v.toFixed(1)}%`),
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
  elt.innerText = `Details for ${row.Benchmark}`;

  return elt;
}

function render() {
  renderSummary();
  renderTable();
}
