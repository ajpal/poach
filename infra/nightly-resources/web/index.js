import { formatMillis, formatSeconds } from "./util.js";

const statusNode = document.querySelector("#status");
const summaryNode = document.querySelector("#summary");
const benchmarkTableBodyNode = document.querySelector("#benchmarks tbody");

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
    renderBenchmarks(data);
  } catch (error) {
    statusNode.textContent = `Failed to load data/data.json: ${error}`;
  }
}

function renderSummary(data) {
  let runningRulesTotal = 0;
  let extractionTotal = 0;
  let otherTotal = 0;

  for (const entry of data.reports) {
    for (const step of entry.report.timing.steps) {
      const category = step.tags[0] === undefined ? "other" : step.tags[0];
      if (category === "running_rules") {
        runningRulesTotal += step.total;
      } else if (category === "extraction") {
        extractionTotal += step.total;
      } else {
        otherTotal += step.total;
      }
    }
  }

  const totalTime = runningRulesTotal + extractionTotal + otherTotal;
  const totalCommandTime = data.summary?.total_time_seconds ?? 0;
  summaryNode.textContent =
    `${data.reports.length} benchmarks | ` +
    `Nightly time: ${formatSeconds(totalCommandTime)} | ` +
    `Rule running: ${formatMillis(runningRulesTotal)} | ` +
    `Extraction: ${formatMillis(extractionTotal)} | ` +
    `Other: ${formatMillis(otherTotal)} | ` +
    `Report total: ${formatMillis(totalTime)}`;
}

function renderBenchmarks(data) {
  benchmarkTableBodyNode.innerHTML = "";

  for (const entry of data.reports) {
    let runningRulesTotal = 0;
    let extractionTotal = 0;
    let otherTotal = 0;

    for (const step of entry.report.timing.steps) {
      const category = step.tags[0] === undefined ? "other" : step.tags[0];
      if (category === "running_rules") {
        runningRulesTotal += step.total;
      } else if (category === "extraction") {
        extractionTotal += step.total;
      } else {
        otherTotal += step.total;
      }
    }

    const rowNode = document.createElement("tr");
    rowNode.innerHTML = `
      <td>${entry.path}</td>
      <td>${formatMillis(runningRulesTotal)}</td>
      <td>${formatMillis(extractionTotal)}</td>
      <td>${formatMillis(otherTotal)}</td>
      <td>${formatSeconds(entry.total_time_seconds)}</td>
    `;
    benchmarkTableBodyNode.appendChild(rowNode);
  }
}
