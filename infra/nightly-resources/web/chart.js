// Set up chart containers
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
          xAxes: [
            {
              type: "linear",
              position: "bottom",
              scaleLabel: {
                display: true,
                labelString: "Run Time (ms)",
              },
            },
          ],
          yAxes: [
            {
              scaleLabel: {
                display: true,
                labelString: "Extract Time (ms)",
              },
            },
          ],
        },
      },
    }
  );

  console.assert(GLOBAL_DATA.serializeChart === null);

  GLOBAL_DATA.serializeChart = new Chart(
    document.getElementById("serialize-chart").getContext("2d"),
    {
      type: "bar",
      data: { labels: ["Serialize", "Deserialize"], datasets: [] },
      options: {
        scales: {
          y: {
            beginAtZero: true,
            title: {
              display: true,
              text: "Time (ms)",
            },
          },
          x: {},
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

function plotSerialization(benchmark) {
  console.assert(GLOBAL_DATA.serializeChart !== null);

  const data = {
    sequential: { serialize: 0, deserialize: 0 },
    interleaved: { serialize: 0, deserialize: 0 },
  };
  if (benchmark === "AVERAGE") {
    data.sequential.serialize = GLOBAL_DATA.data.tests.sequential
      .map((x) => {
        if (x.serialize.length !== 1) {
          console.warn(`Expected one serialize event, found ${x.serialize}`);
        }
        return x.serialize[0];
      })
      .reduce((acc, curr) => acc + curr, 0);
    data.sequential.deserialize = GLOBAL_DATA.data.tests.sequential
      .map((x) => {
        if (x.deserialize.length !== 1) {
          console.warn(
            `Expected one deserialize event, found ${x.deserialize}`
          );
        }
        return x.deserialize[0];
      })
      .reduce((acc, curr) => acc + curr, 0);

    data.interleaved.serialize = GLOBAL_DATA.data.tests.interleaved
      .map((x) => {
        if (x.serialize.length !== 1) {
          console.warn(`Expected one serialize event, found ${x.serialize}`);
        }
        return x.serialize[0];
      })
      .reduce((acc, curr) => acc + curr, 0);
    data.interleaved.deserialize = GLOBAL_DATA.data.tests.interleaved
      .map((x) => {
        if (x.deserialize.length !== 1) {
          console.warn(
            `Expected one deserialize event, found ${x.deserialize}`
          );
        }
        return x.deserialize[0];
      })
      .reduce((acc, curr) => acc + curr, 0);
  } else {
    const sequential = GLOBAL_DATA.data.tests.sequential.find(
      (b) => b.benchmark == benchmark
    );
    const interleaved = GLOBAL_DATA.data.tests.interleaved.find(
      (b) => b.benchmark == benchmark
    );
    if (!sequential || !interleaved) {
      console.warn(`Couldn't find serialization data for ${benchmark}`);
      return;
    }

    if (
      sequential.serialize.length !== 1 ||
      sequential.deserialize.length !== 1 ||
      interleaved.serialize.length !== 1 ||
      interleaved.deserialize.length !== 1
    ) {
      console.warn("Unexpected number of serialize/deserialize events");
      return;
    }

    data.sequential.serialize = sequential.serialize[0];
    data.sequential.deserialize = sequential.deserialize[0];
    data.interleaved.serialize = interleaved.serialize[0];
    data.interleaved.deserialize = interleaved.deserialize[0];
  }

  GLOBAL_DATA.serializeChart.data.datasets = [
    {
      label: "Sequential",
      data: [data.sequential.serialize, data.sequential.deserialize],
      backgroundColor: "red",
    },
    {
      label: "Interleaved",
      data: [data.interleaved.serialize, data.interleaved.deserialize],
      backgroundColor: "blue",
    },
  ];

  GLOBAL_DATA.serializeChart.update();
}
