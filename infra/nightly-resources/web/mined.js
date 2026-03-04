function initialize() {
  Promise.all([initializeGlobalData(), loadMineExtracts()])
    .then(initializeCharts)
    .then(() => {
      plotMine();
      renderMineSummaryTable();
    });
}

function loadMineExtracts() {
  return fetch("data/mine-extracts.json")
    .then((response) => response.json())
    .then((data) => {
      GLOBAL_DATA.mineExtracts = data;
    })
    .catch((error) => {
      console.error("Failed to load mine-extracts.json", error);
      GLOBAL_DATA.mineExtracts = {};
    });
}

function plotMine() {
  const mega_mined = GLOBAL_DATA.data.easteregg["mine-mega"];
  const indiv_mined = GLOBAL_DATA.data.easteregg["mine-indiv"];
  const baseline = GLOBAL_DATA.data.easteregg.timeline;

  if (GLOBAL_DATA.minedChart === null) {
    return;
  }

  const benchmarks = Object.keys(baseline);

  const data = {};

  benchmarks.forEach((b) => {
    data[b] = {};

    data[b].baseline = benchmarkTotalTime(baseline[b]);
    data[b].mega_mined = benchmarkTotalTime(mega_mined[b]);
    data[b].indiv_mined = benchmarkTotalTime(indiv_mined[b]);
  });

  GLOBAL_DATA.minedChart.data = {
    labels: benchmarks,
    datasets: [
      {
        label: "baseline",
        data: Object.values(data).map((d) => d.baseline),
      },
      {
        label: "mined (mega)",
        data: Object.values(data).map((d) => d.mega_mined),
      },
      {
        label: "mined (indiv)",
        data: Object.values(data).map((d) => d.indiv_mined),
      },
    ],
  };

  GLOBAL_DATA.minedChart.update();
}

function renderMineSummaryTable() {
  const tableBody = document.getElementById("mine-summary-body");
  if (!tableBody) {
    return;
  }

  const summaries = GLOBAL_DATA.mineExtracts["mine-mega"] || {};
  const rows = Object.entries(summaries)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([benchmarkName, entries]) => {
      const extractCount = entries.length;
      const initialTotal = entries.reduce(
        (sum, entry) => sum + entry.initial_cost,
        0,
      );
      const finalTotal = entries.reduce(
        (sum, entry) => sum + entry.final_cost,
        0,
      );
      const avgInitialCost =
        extractCount === 0 ? 0 : initialTotal / extractCount;
      const avgFinalCost = extractCount === 0 ? 0 : finalTotal / extractCount;
      const avgCostDifference = avgInitialCost - avgFinalCost;
      return `
        <tr>
          <td>${benchmarkName}</td>
          <td>${extractCount}</td>
          <td>${avgInitialCost}</td>
          <td>${avgFinalCost}</td>
          <td>${avgCostDifference}</td>
        </tr>
      `;
    });

  tableBody.innerHTML = rows.join("\n");
}
