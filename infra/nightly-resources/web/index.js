import { formatMillis } from "./util.js";

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
  summaryNode.textContent = `${data.benchmarks.length} benchmarks`;
}

function renderBenchmarks(data) {
  benchmarkTableBodyNode.innerHTML = "";

  for (const benchmark of data.benchmarks) {
    const train = summarizeTiming(benchmark.train);
    const serve = summarizeTiming(benchmark.serve);

    const rowNode = document.createElement("tr");
    rowNode.innerHTML = `
      <td>${benchmark.path}</td>
      <td>${formatTagTotal(train, "running_rules")}</td>
      <td>${formatTagTotal(train, "extraction")}</td>
      <td>${formatTagTotal(train, "serialize")}</td>
      <td>${formatMillis(benchmark.train.timing.total)}</td>
      <td>${formatTagTotal(serve, "running_rules")}</td>
      <td>${formatTagTotal(serve, "extraction")}</td>
      <td>${formatMillis(benchmark.serve.timing.total)}</td>
    `;
    benchmarkTableBodyNode.appendChild(rowNode);
  }
}

function summarizeTiming(report) {
  const timingsByTag = {};

  for (const step of report.timing.steps) {
    const tags = step.tags.length === 0 ? ["other"] : step.tags;

    for (const tag of tags) {
      if (timingsByTag[tag] === undefined) {
        timingsByTag[tag] = [];
      }
      timingsByTag[tag].push(step.total);
    }
  }

  return timingsByTag;
}

function formatTagTotal(timingsByTag, tag) {
  const totals = timingsByTag[tag];
  if (totals === undefined) {
    return "-";
  }
  return formatMillis(totals.reduce((total, value) => total + value, 0));
}
