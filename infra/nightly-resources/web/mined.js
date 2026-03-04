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
  const byBenchmark = Object.fromEntries(
    benchmarks.map((b) => {
      const baseline = aggregateTimelinesByCommand(
        GLOBAL_DATA.mineData[b].baseline_timeline,
      );
      const mega = aggregateTimelinesByCommand(
        GLOBAL_DATA.mineData[b].mine_mega_timeline,
      );
      const indiv = aggregateTimelinesByCommand(
        GLOBAL_DATA.mineData[b].mine_indiv_timeline,
      );
      return [
        b,
        {
          baseline: {
            run: aggregate(baseline.run, "total"),
            extract: aggregate(baseline.extract, "total"),
          },
          mega: {
            run: aggregate(mega.run, "total"),
            extract: aggregate(mega.extract, "total"),
          },
          indiv: {
            run: aggregate(indiv.run, "total"),
            extract: aggregate(indiv.extract, "total"),
          },
        },
      ];
    }),
  );

  GLOBAL_DATA.minedChart.data = {
    labels: benchmarks,
    datasets: [
      {
        label: "baseline: run",
        stack: "baseline",
        backgroundColor: "#1e3a8a",
        data: benchmarks.map((b) => byBenchmark[b].baseline.run),
      },
      {
        label: "baseline: extract",
        stack: "baseline",
        backgroundColor: "#60a5fa",
        data: benchmarks.map((b) => byBenchmark[b].baseline.extract),
      },
      {
        label: "mined (mega): run",
        stack: "mega",
        backgroundColor: "#b91c1c",
        data: benchmarks.map((b) => byBenchmark[b].mega.run),
      },
      {
        label: "mined (mega): extract",
        stack: "mega",
        backgroundColor: "#f472b6",
        data: benchmarks.map((b) => byBenchmark[b].mega.extract),
      },
      {
        label: "mined (indiv): run",
        stack: "indiv",
        backgroundColor: "#166534",
        data: benchmarks.map((b) => byBenchmark[b].indiv.run),
      },
      {
        label: "mined (indiv): extract",
        stack: "indiv",
        backgroundColor: "#86efac",
        data: benchmarks.map((b) => byBenchmark[b].indiv.extract),
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
        </tr>
      `;
    });

  tableBody.innerHTML = rows.join("\n");
}
