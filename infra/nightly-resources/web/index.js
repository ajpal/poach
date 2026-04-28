import { formatMillis } from "./util.js";

const statusNode = document.querySelector("#status");

load();

async function load() {
  try {
    const response = await fetch("./data/data.json");
    if (!response.ok) {
      throw new Error(`Failed to load data.json (${response.status})`);
    }

    const data = await response.json();
    statusNode.textContent = "Loaded data/data.json";
    renderSummary(data);
    renderBenchmarks(data.reports);
  } catch (error) {
    statusNode.textContent = `Failed to load data/data.json: ${error}`;
  }
}

function renderSummary(data) {
  let ruleRunningMillis = 0;
  let extractionMillis = 0;
  let otherMillis = 0;

  for (const { report } of data.reports) {
    const totals = getTimingTotals(report);
    ruleRunningMillis += totals.ruleRunningMillis;
    extractionMillis += totals.extractionMillis;
    otherMillis += totals.otherMillis;
  }

  document.querySelector("#summary-text").textContent =
    `${data.reports.length} benchmarks | ` +
    `Nightly time: ${data.summary.total_time_seconds.toFixed(1)} s | ` +
    `Rule running: ${ruleRunningMillis} ms | ` +
    `Extraction: ${extractionMillis} ms | ` +
    `Other: ${otherMillis} ms`;
}

function renderBenchmarks(reports) {
  document.querySelector("#benchmarks-body").innerHTML = reports
    .map(({ path, time_seconds, report }) => {
      const totals = getTimingTotals(report);

      return `
        <tr>
          <td>${path}</td>
          <td>${time_seconds.toFixed(3)} s</td>
          <td>${formatMillis(totals.ruleRunningMillis)}</td>
          <td>${formatMillis(totals.extractionMillis)}</td>
          <td>${formatMillis(totals.otherMillis)}</td>
          <td>${report.timings.length}</td>
        </tr>
      `;
    })
    .join("");
}

function getTimingTotals(report) {
  let ruleRunningMillis = 0;
  let extractionMillis = 0;
  let otherMillis = 0;

  for (const timing of report.timings) {
    if (timing.tags.includes("running_rules")) {
      ruleRunningMillis += timing.total;
    } else if (timing.tags.includes("extraction")) {
      extractionMillis += timing.total;
    } else {
      otherMillis += timing.total;
    }
  }

  return { ruleRunningMillis, extractionMillis, otherMillis };
}
