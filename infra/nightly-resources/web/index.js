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

  for (const { timing_summary } of data.reports) {
    ruleRunningMillis += timing_summary.rule_running_millis;
    extractionMillis += timing_summary.extraction_millis;
    otherMillis += timing_summary.other_millis;
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
    .map(({ path, time_seconds, timing_summary }) => {
      return `
        <tr>
          <td>${path}</td>
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
