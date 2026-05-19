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
  let trainMicros = 0;
  let serveMicros = 0;

  for (const b of GLOBAL_DATA.data.passing_benchmarks) {
    trainMicros += b.train.wall_time_micros;
    serveMicros += b.serve.wall_time_micros;
  }

  const numPassing = GLOBAL_DATA.data.passing_benchmarks.length;
  const numFailing = GLOBAL_DATA.data.failing_benchmarks.length;
  const totalTime = trainMicros + serveMicros;

  document.querySelector("#summary-text").textContent =
    `Passing Benchmarks: ${numPassing} | ` +
    `Failing Benchmarks: ${numFailing} | ` +
    `Total time: ${displayTime(totalTime)} | ` +
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
  const totalTime = benchmarks
    .map((b) => b.train.wall_time_micros + b.serve.wall_time_micros)
    .reduce((a, b) => a + b, 0);

  document.querySelector("#active-suite-summary").innerHTML = `
  <div>
    <h3>${STATE.activeSuite}</h3>
    <p>${benchmarks.length} benchmarks | ${displayTime(totalTime)} </p>
  </div>`;

  const columns = ["Benchmark", "Train Time", "Serve Time"];

  const rows = benchmarks.map((b) => ({
    Benchmark: b.benchmark_name,
    "Train Time": b.train.wall_time_micros,
    "Serve Time": b.serve.wall_time_micros,
  }));

  const displayFns = {
    "Train Time": displayTime,
    "Serve Time": displayTime,
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
