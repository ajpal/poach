import { formatMillis, formatSeconds } from "./util.js";

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
    `${data.summary.benchmark_count} reports across ${data.suites.length} suites | ` +
    `Nightly time: ${formatSeconds(data.summary.total_time_seconds)} | ` +
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

  const benchmarks = groupByBenchmark(activeSuite.reports);
  document.querySelector("#active-suite-summary").innerHTML = `
    <div class="suite-header">
      <h3>${activeSuite.name}</h3>
      <p>${benchmarks.length} benchmarks | ${formatSeconds(activeSuite.summary.total_time_seconds)}</p>
    </div>
  `;
  document.querySelector("#benchmarks-body").innerHTML = renderRows(benchmarks);
}

function groupByBenchmark(reports) {
  const grouped = new Map();
  for (const report of reports) {
    if (!grouped.has(report.benchmark_path)) {
      grouped.set(report.benchmark_path, {
        benchmark_path: report.benchmark_path,
      });
    }
    grouped.get(report.benchmark_path)[report.phase] = report.timing_summary;
  }
  return Array.from(grouped.values());
}

function renderRows(benchmarks) {
  return benchmarks
    .map(({ benchmark_path, train, serve }) => {
      return `
        <tr>
          <td>${benchmark_path}</td>
          <td>${formatMillis(train?.rule_running_millis ?? 0)}</td>
          <td>${formatMillis(train?.extraction_millis ?? 0)}</td>
          <td>${formatMillis(train?.serialize_millis ?? 0)}</td>
          <td>${formatMillis(train?.total_millis ?? 0)}</td>
          <td>${formatMillis(serve?.rule_running_millis ?? 0)}</td>
          <td>${formatMillis(serve?.extraction_millis ?? 0)}</td>
          <td>${formatMillis(serve?.total_millis ?? 0)}</td>
        </tr>
      `;
    })
    .join("");
}
