const statusNode = document.querySelector("#status");
const summaryNode = document.querySelector("#summary");
const tableBodyNode = document.querySelector("#benchmark-table-body");

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
    renderBenchmarks(data.benchmarks);
  } catch (error) {
    statusNode.textContent = `Failed to load data/data.json: ${error}`;
  }
}

function renderSummary(summary) {
  summaryNode.textContent =
    `Benchmarks: ${summary.benchmark_count} | ` +
    `Running Rules: ${summary.running_rules_ms} ms | ` +
    `Extraction: ${summary.extraction_ms} ms | ` +
    `Other: ${summary.other_ms} ms | ` +
    `Total: ${summary.total_tagged_ms} ms`;
}

function renderBenchmarks(benchmarks) {
  tableBodyNode.innerHTML = "";

  const sortedBenchmarks = [...benchmarks].sort(
    (a, b) => b.total_tagged_ms - a.total_tagged_ms,
  );

  for (const benchmark of sortedBenchmarks) {
    const row = document.createElement("tr");
    row.appendChild(textCell(benchmark.name));
    row.appendChild(timeMsCell(benchmark.running_rules_ms));
    row.appendChild(timeMsCell(benchmark.extraction_ms));
    row.appendChild(timeMsCell(benchmark.other_ms));
    row.appendChild(timeMsCell(benchmark.total_tagged_ms));
    tableBodyNode.appendChild(row);
  }
}

function textCell(value) {
  const cell = document.createElement("td");
  cell.textContent = value;
  return cell;
}

function timeMsCell(value) {
  const cell = document.createElement("td");
  cell.textContent = `${value} ms`;
  return cell;
}
