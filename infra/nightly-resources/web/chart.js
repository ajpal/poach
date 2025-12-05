// Set up chart containers
// Seems important for Chart.js to change the data but not
// create a new chart object to avoid some weird rendering flicekrs.
function initializeCharts() {
  console.assert(GLOBAL_DATA.runExtractChart === null);

  GLOBAL_DATA.runExtractChart = new Chart(
    document.getElementById("run-extract-chart").getContext("2d"),
    {
      type: "scatter",
      data: { datasets: [] },
      options: {
        title: {
          display: false,
        },
      },
    }
  );

  console.assert(GLOBAL_DATA.serializeChart === null);

  GLOBAL_DATA.serializeChart = new Chart(
    document.getElementById("serialize-chart"),
    {
      type: "bar",
      data: {},
      options: {
        indexAxis: "y",
        scales: {
          x: {
            stacked: true,
          },
          y: {
            stacked: true,
          },
        },
      },
    }
  );
}

/**
 * Plots the loaded benchmark data on a scatter chart.
 */
function plotTimeline() {
  console.assert(GLOBAL_DATA.runExtractChart !== null);

  const mode = document.querySelector('input[name="mode"]:checked').value;

  const datasets = Object.values(GLOBAL_DATA.data).map((suite) => ({
    label: suite.name,
    // todo other run modes
    data: Object.values(suite.timeline).map((entry) => ({
      x: aggregate(entry.run, mode),
      y: aggregate(entry.extract, mode),
    })),
    backgroundColor: suite.color,
    pointRadius: 4,
  }));

  GLOBAL_DATA.runExtractChart.data.datasets = datasets;

  GLOBAL_DATA.runExtractChart.update();
}

/**
 * Plots a stacked bar chart, showing time spent in each phase (run, extract, serialize, deserialize, read, write)
 * across the egglog tests benchmarks
 *
 * TODO: Use the dropdown value to zoom in one benchmark
 * TODO: Toggle to switch between absolute and percentage
 */
function plotSerialization(benchmark) {
  console.assert(GLOBAL_DATA.serializeChart !== null);
  const benchmarks = Object.keys(GLOBAL_DATA.data.tests.sequential);

  GLOBAL_DATA.serializeChart.data = {
    labels: benchmarks,
    datasets: [
      ...CMDS.map((cmd) => ({
        label: cmd,
        data: benchmarks.map((b) =>
          aggregate(GLOBAL_DATA.data.tests.sequential[b][cmd], "total")
        ),
      })),
    ],
  };

  GLOBAL_DATA.serializeChart.update();
}
