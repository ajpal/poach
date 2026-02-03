function initialize() {
  console.log("here");
  initializeGlobalData().then(initializeCharts).then(plotMine);
}

function plotMine() {
  const mined = GLOBAL_DATA.data.easteregg.mine;
  const baseline = GLOBAL_DATA.data.easteregg.timeline;

  if (GLOBAL_DATA.minedChart === null) {
    return;
  }

  const benchmarks = Object.keys(mined);

  const data = {};

  benchmarks.forEach((b) => {
    data[b] = {};

    data[b].baseline = benchmarkTotalTime(baseline[b]);
    data[b].mined = benchmarkTotalTime(mined[b]);
  });

  GLOBAL_DATA.minedChart.data = {
    labels: benchmarks,
    datasets: [
      {
        label: "baseline",
        data: Object.values(data).map((d) => d.baseline),
      },
      {
        label: "mined",
        data: Object.values(data).map((d) => d.mined),
      },
    ],
  };

  GLOBAL_DATA.minedChart.update();
}
