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

  const mode = document.querySelector(
    'input[name="timelineMode"]:checked'
  ).value;

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
function plotSerialization() {
  console.assert(GLOBAL_DATA.serializeChart !== null);

  const runMode = document.querySelector(
    'input[name="runModeToggle"]:checked'
  ).value;
  console.assert(RUN_MODES.includes(runMode));

  const mode = document.querySelector(
    'input[name="serializationMode"]:checked'
  ).value;
  const benchmark = document.getElementById("tests").value;

  if (benchmark) {
    // Show all run modes for a single benchmark
    const datasets = Object.fromEntries(
      RUN_MODES.map((runMode) => [
        runMode,
        Object.fromEntries(
          CMDS.map((cmd) => [
            cmd,
            aggregate(GLOBAL_DATA.data.tests[runMode][benchmark][cmd], "total"),
          ])
        ),
      ])
    );

    if (mode === "percentage") {
      Object.keys(datasets).forEach((entry) => {
        const total = aggregate(
          CMDS.map((cmd) => datasets[entry][cmd]),
          "total"
        );
        CMDS.forEach((cmd) => {
          datasets[entry][cmd] /= total;
        });
      });
    }

    const plotData = CMDS.map((cmd) => ({
      label: cmd,
      data: RUN_MODES.map((r) => datasets[r][cmd]),
    }));

    GLOBAL_DATA.serializeChart.data = {
      labels: RUN_MODES,
      datasets: plotData,
    };
  } else {
    // Show a single run mode for all benchmarks

    const benchmarks = Object.keys(GLOBAL_DATA.data.tests[runMode]);
    const datasets = Object.fromEntries(
      benchmarks.map((bench) => [
        bench,
        Object.fromEntries(
          CMDS.map((cmd) => [
            cmd,
            aggregate(GLOBAL_DATA.data.tests[runMode][bench][cmd], "total"),
          ])
        ),
      ])
    );

    if (mode === "percentage") {
      Object.keys(datasets).forEach((entry) => {
        const total = aggregate(
          CMDS.map((cmd) => datasets[entry][cmd]),
          "total"
        );
        CMDS.forEach((cmd) => {
          datasets[entry][cmd] /= total;
        });
      });
    }

    const plotData = CMDS.map((cmd) => ({
      label: cmd,
      data: benchmarks.map((b) => datasets[b][cmd]),
    }));

    GLOBAL_DATA.serializeChart.data = {
      labels: benchmarks,
      datasets: plotData,
    };
  }

  GLOBAL_DATA.serializeChart.update();
}
