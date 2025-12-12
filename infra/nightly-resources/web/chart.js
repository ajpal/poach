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
        scales: {
          x: {
            type: "linear",
            title: {
              display: true,
              text: "Run Time (ms)",
            },
          },
          y: {
            title: {
              display: true,
              text: "Extract Time (ms)",
            },
          },
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
        plugins: {
          title: {
            display: true,
            text: "Placeholder Title",
            font: {
              size: 20,
            },
          },
        },
        indexAxis: "y",
        scales: {
          x: {
            type: "linear",
            title: {
              display: true,
              text: "",
            },
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
  if (GLOBAL_DATA.serializeChart === null) {
    return;
  }

  const suite = document.querySelector(
    'input[name="suiteToggle"]:checked'
  ).value;

  const runMode = document.querySelector(
    'input[name="runModeToggle"]:checked'
  ).value;

  console.assert(RUN_MODES.includes(runMode));

  const mode = document.querySelector(
    'input[name="serializationMode"]:checked'
  ).value;
  const benchmark = document.getElementById("tests").value;

  const title = benchmark
    ? `Showing all run modes for ${benchmark} (${
        mode === "percentage" ? "% Run Time" : "Total Run Time"
      })`
    : `Showing ${runMode} for ${suite} benchmarks (${
        mode === "percentage" ? "% Run Time" : "Total Run Time"
      })`;

  if (benchmark) {
    // Show all run modes for a single benchmark
    const datasets = Object.fromEntries(
      RUN_MODES.map((runMode) => [
        runMode,
        Object.fromEntries(
          CMDS.map((cmd) => [
            cmd,
            aggregate(
              GLOBAL_DATA.data[suite][runMode]?.[benchmark]?.[cmd],
              "total"
            ),
          ])
        ),
      ]).filter((entry) => Object.values(entry[1]).some((v) => v !== 0))
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
      data: Object.keys(datasets).map((r) => datasets[r]?.[cmd]),
    }));

    GLOBAL_DATA.serializeChart.data = {
      labels: Object.keys(datasets),
      datasets: plotData,
    };
  } else {
    // Show a single run mode for all benchmarks

    const benchmarks = Object.keys(GLOBAL_DATA.data[suite][runMode]);
    const datasets = Object.fromEntries(
      benchmarks.map((bench) => [
        bench,
        Object.fromEntries(
          CMDS.map((cmd) => [
            cmd,
            aggregate(GLOBAL_DATA.data[suite][runMode][bench][cmd], "total"),
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

    GLOBAL_DATA.serializeChart.options.scales.x.title.text = `Run Time (${
      mode === "percentage" ? "%" : "ms"
    })`;

    GLOBAL_DATA.serializeChart.options.plugins.title.text = title;
    GLOBAL_DATA.serializeChart.data = {
      labels: benchmarks,
      datasets: plotData,
    };
  }

  GLOBAL_DATA.serializeChart.update();
}
