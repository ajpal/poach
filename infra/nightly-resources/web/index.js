import { formatMillis } from "./util.js";

let suites = [];
let activeSuiteName = null;
load();

async function load() {
  const statusNode = document.querySelector("#status");

  try {
    const response = await fetch("./data/data.json");
    if (!response.ok) {
      throw new Error(`Failed to load data.json (${response.status})`);
    }

    const data = await response.json();
    suites = data.suites;
    activeSuiteName = suites[0]?.name ?? null;
    statusNode.textContent = "Loaded data/data.json";
    renderSummary(data);
    renderSuites();
  } catch (error) {
    statusNode.textContent = `Failed to load data/data.json: ${error}`;
  }
}

function renderSummary(data) {
  let ruleRunningMillis = 0;
  let extractionMillis = 0;
  let otherMillis = 0;

  for (const { timing_summary } of data.reports) {
    ruleRunningMillis += timing_summary.rule_running_millis;
    extractionMillis += timing_summary.extraction_millis;
    otherMillis += timing_summary.other_millis;
  }

  document.querySelector("#summary-text").textContent =
    `${data.summary.benchmark_count} benchmarks across ${data.suites.length} suites | ` +
    `Nightly time: ${data.summary.total_time_seconds.toFixed(1)} s | ` +
    `Rule running: ${ruleRunningMillis} ms | ` +
    `Extraction: ${extractionMillis} ms | ` +
    `Other: ${otherMillis} ms`;
}

function renderSuites() {
  document.querySelector("#suite-tabs").innerHTML = suites
    .map((suite) => {
      return `
        <button
          type="button"
          class="suite-tab${suite.name === activeSuiteName ? " is-active" : ""}"
          data-suite-name="${suite.name}"
        >
          ${suite.name}
        </button>
      `;
    })
    .join("");

  for (const button of document.querySelectorAll(".suite-tab")) {
    button.addEventListener("click", () => {
      activeSuiteName = button.dataset.suiteName;
      renderSuites();
    });
  }

  const activeSuite = suites.find((suite) => suite.name === activeSuiteName);
  if (!activeSuite) {
    document.querySelector("#active-suite-summary").textContent = "";
    document.querySelector("#benchmarks-body").innerHTML = "";
    return;
  }

  document.querySelector("#active-suite-summary").innerHTML = `
    <div class="suite-header">
      <h3>${activeSuite.name}</h3>
      <p>${activeSuite.reports.length} benchmarks | ${activeSuite.summary.total_time_seconds.toFixed(1)} s</p>
    </div>
  `;
  document.querySelector("#benchmarks-body").innerHTML = renderRows(
    activeSuite.reports,
  );
}

function renderRows(reports) {
  return reports
    .map(({ benchmark_path, time_seconds, timing_summary }) => {
      return `
        <tr>
          <td>${benchmark_path}</td>
          <td>${time_seconds.toFixed(3)} s</td>
          <td>${formatMillis(timing_summary.rule_running_millis)}</td>
          <td>${formatMillis(timing_summary.extraction_millis)}</td>
          <td>${formatMillis(timing_summary.other_millis)}</td>
          <td>${timing_summary.timing_steps}</td>
        </tr>
      `;
    })
    .join("");
}
