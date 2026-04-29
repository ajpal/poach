import { formatMillis, formatSeconds } from "./util.js";

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
    renderSummary(data.summary);
    renderBenchmarks(data);
  } catch (error) {
    statusNode.textContent = `Failed to load data/data.json: ${error}`;
  }
}

function renderSummary(summary) {
  document.querySelector("#summary").textContent =
    `${summary.benchmark_count} Benchmarks | Total time: ${formatSeconds(summary.total_time_seconds)}`;
}

function renderBenchmarks(data) {
  const rows = [];
  data.benchmarks.forEach((benchmark) => {
    const row = document.createElement("tr");
    const train = getTimingTotals(benchmark.train_report);
    const serve = getTimingTotals(benchmark.serve_report);

    [
      benchmark.path,
      formatMillis(train.ruleRunningMillis),
      formatMillis(train.extractionMillis),
      formatMillis(train.serializeMillis),
      formatMillis(train.totalMillis),
      formatMillis(serve.ruleRunningMillis),
      formatMillis(serve.extractionMillis),
      formatMillis(serve.totalMillis),
    ].forEach((value) => {
      const cell = document.createElement("td");
      cell.textContent = value;
      row.append(cell);
    });

    rows.push(row);
  });
  document.querySelector("#benchmarks tbody").replaceChildren(...rows);
}

function getTimingTotals(report) {
  let ruleRunningMillis = 0;
  let extractionMillis = 0;
  let serializeMillis = 0;
  let totalMillis = 0;

  for (const timing of report.timings) {
    totalMillis += timing.total;
    if (timing.tags.includes("running_rules")) {
      ruleRunningMillis += timing.total;
    } else if (timing.tags.includes("extraction")) {
      extractionMillis += timing.total;
    } else if (timing.name === "serialize_model") {
      serializeMillis += timing.total;
    }
  }

  return {
    ruleRunningMillis,
    extractionMillis,
    serializeMillis,
    totalMillis,
  };
}
