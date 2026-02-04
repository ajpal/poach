function initialize() {
  console.log("here");
  initializeGlobalData().then(initializeCharts).then(plotMine);
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
