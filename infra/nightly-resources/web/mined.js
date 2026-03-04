function initialize() {
  Promise.all([initializeGlobalData(), loadMineData()])
    .then(initializeCharts)
    .then(() => {
      plotMine();
      renderMineSummaryTable();
    });
}

function loadMineData() {
  return fetch("data/mine-data.json")
    .then((response) => response.json())
    .then((data) => {
      GLOBAL_DATA.mineData = data;
    })
    .catch((error) => {
      console.error("Failed to load mine-data.json", error);
      GLOBAL_DATA.mineData = {};
    });
}

function plotMine() {
  if (GLOBAL_DATA.minedChart === null) {
    return;
  }

  const benchmarks = Object.keys(GLOBAL_DATA.mineData).sort();

  GLOBAL_DATA.minedChart.data = {
    labels: benchmarks,
    datasets: [
      {
        label: "baseline",
        data: benchmarks.map((b) =>
          runtimeMsFromTimelines(
            GLOBAL_DATA.mineData[b].baseline_timeline,
            new Set(["serialize", "write"]),
          ),
        ),
      },
      {
        label: "mined (mega)",
        data: benchmarks.map((b) =>
          runtimeMsFromTimelines(GLOBAL_DATA.mineData[b].mine_mega_timeline),
        ),
      },
      {
        label: "mined (indiv)",
        data: benchmarks.map((b) =>
          runtimeMsFromTimelines(GLOBAL_DATA.mineData[b].mine_indiv_timeline),
        ),
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

  const rows = Object.entries(GLOBAL_DATA.mineData)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([benchmarkName, data]) => {
      const entries = data.mine_mega_extracts || [];
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
      return `
        <tr>
          <td>${benchmarkName}</td>
          <td>${extractCount}</td>
          <td>${avgInitialCost}</td>
          <td>${avgFinalCost}</td>
          <td>${avgInitialCost - avgFinalCost}</d>
        </tr>
      `;
    });

  tableBody.innerHTML = rows.join("\n");
}

function runtimeMsFromTimelines(timelines, ignoredCmdTypes = new Set()) {
  return (
    aggregate(
      (timelines || []).flatMap((timeline) =>
        (timeline.events || [])
          .map((timeMicros, i) => {
            const cmdType = getCmdType(getCmd((timeline.sexps || [])[i] || ""));
            return ignoredCmdTypes.has(cmdType) ? null : timeMicros;
          })
          .filter((timeMicros) => timeMicros !== null),
      ),
      "total",
    ) / 1000
  );
}
