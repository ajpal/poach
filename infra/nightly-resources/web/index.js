import { convertToTable } from "./table.js";
import { formatTime, formatBytes, unwrapBytes } from "./util.js";

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
  statusNode.innerHTML = 'Loaded <a href="./data/data.json">data/data.json</a>';

  GLOBAL_DATA.suites = [
    ...new Set(GLOBAL_DATA.data.passing_benchmarks.map((x) => x.suite_name)),
  ].sort();
  STATE.activeSuite = GLOBAL_DATA.suites[0] ?? null;

  // Set up interactive elements
  setupSuiteSelectors();
  setupTimeDisplaySelector();

  render();
}

const displayTime = (v) => formatTime(v, STATE.timeDisplay);
const displayBytes = (v) => formatBytes(v, STATE.timeDisplay);

function totalTime(benchmarks) {
  return benchmarks.reduce(
    (sum, b) => sum + b.train.wall_time_micros + b.serve.wall_time_micros,
    0,
  );
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

function renderSummary() {
  let trainMicros = 0;
  let serveMicros = 0;

  for (const b of GLOBAL_DATA.data.passing_benchmarks) {
    trainMicros += b.train.wall_time_micros;
    serveMicros += b.serve.wall_time_micros;
  }

  const numPassing = GLOBAL_DATA.data.passing_benchmarks.length;
  const numFailing = GLOBAL_DATA.data.failing_benchmarks.length;

  document.querySelector("#summary-text").textContent =
    `Passing Benchmarks: ${numPassing} | ` +
    `Failing Benchmarks: ${numFailing} | ` +
    `Total time: ${displayTime(trainMicros + serveMicros)} | ` +
    `Total train time: ${displayTime(trainMicros)} | ` +
    `Total serve time: ${displayTime(serveMicros)}`;
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

function renderTable() {
  const benchmarks = GLOBAL_DATA.data.passing_benchmarks.filter(
    (x) => x.suite_name === STATE.activeSuite,
  );

  document.querySelector("#active-suite-summary").innerHTML = `
  <div>
    <h3>${STATE.activeSuite}</h3>
    <p>${benchmarks.length} benchmarks | ${displayTime(totalTime(benchmarks))} </p>
  </div>`;

  const columns = ["Benchmark", "Train Time", "Model Size", "Serve Time"];

  const rows = benchmarks.map((b) => ({
    Benchmark: b.benchmark_name,
    "Train Time": b.train.wall_time_micros,
    "Model Size": unwrapBytes(b.train.report.model_bytes),
    "Serve Time": b.serve.wall_time_micros,
    _benchmark: b,
  }));

  const displayFns = {
    "Train Time": displayTime,
    "Model Size": displayBytes,
    "Serve Time": displayTime,
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
          <li>Rules: ${displayTime(train.rule_micros)}</li>
          <li>Extract: ${displayTime(train.extraction_micros)}</li>
          <li>Serialize: ${displayTime(train.serialize)}</li>
        </ul>
      </div>
      <div class="detail-column">
        <h4>Serve</h4>
        <ul>
          <li>Deserialize: ${displayTime(serve.deserialize)}</li>
          <li>Rules: ${displayTime(serve.rule_micros)}</li>
          <li>Extract: ${displayTime(serve.extraction_micros)}</li>
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
