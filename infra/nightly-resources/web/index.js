const GLOBAL_DATA = {};
function initializePage() {
  initializeGlobalData();
  loadData()
    .then(loadSerializationDropdown)
    .then(initializeCharts)
    .then(plotTimeline)
    .then(() => plotSerialization("AVERAGE"));
}

/**
 * The benchmark suites to include in the visualization.
 */
const BENCH_SUITES = [
  {
    name: "Herbie (Hamming)",
    dir: "herbie-hamming",
    color: "blue",
  },
  {
    name: "Easteregg",
    dir: "easteregg",
    color: "red",
  },
  {
    name: "Herbie (Math rewrite)",
    dir: "herbie-math-rewrite",
    color: "green",
  },
  {
    name: "Herbie (Math taylor)",
    dir: "herbie-math-taylor",
    color: "purple",
  },
  {
    name: "Egglog Tests",
    dir: "tests",
    color: "orange",
  },
];

const RUN_MODES = [
  "sequential",
  "interleaved",
  "old-serialize",
  "idempotent",
  "timeline",
];

function initializeGlobalData() {
  GLOBAL_DATA.data = Object.fromEntries(
    BENCH_SUITES.map((suite) => [
      suite.dir,
      { ...suite, ...Object.fromEntries(RUN_MODES.map((mode) => [mode, []])) },
    ])
  );
  GLOBAL_DATA.runExtractChart = null;
  GLOBAL_DATA.serializeChart = null;
}

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

function loadData() {
  return fetch("data/data.json")
    .then((response) => response.json())
    .then(processRawData);
}

function loadSerializationDropdown() {
  const files = GLOBAL_DATA.data.tests.sequential
    .map((x) => x.benchmark)
    .sort();
  const dropdownElt = document.getElementById("tests");
  files.forEach((file) => {
    const opt = document.createElement("option");
    opt.value = file;
    opt.textContent = file;
    dropdownElt.appendChild(opt);
  });
}

function serializationDropdownChange(e) {
  plotSerialization(e.target.value);
}

function getCmd(sexp) {
  const match = sexp.match(/[^\(\s\)]+/);
  if (match) {
    return match[0];
  } else {
    console.warn(`could not parse command from ${sexp}`);
    return null;
  }
}

/**
 * Expected data layout:
 * key is the name of the benchmark egg file
 * value is the array of timelines where each timeline contains an array of events
 *
 * Populated `GLOBAL_DATA` map with aggregated/processed data for chart
 * key is the benchmark category
 * value contains an array of data points, corresponding to all of the individual
 * egg files in the benchmark category.
 * Each data point contains arrays of times for run, extract, serialize, and deserialize events
 */
function processRawData(blob) {
  const RUN_CMDS = ["run", "run-schedule"];
  const EXT_CMDS = ["extract", "multi-extract"];
  const SERIALIZE_CMDS = ["serialize"];
  const DESERIALIZE_CMDS = ["deserialize"];

  Object.entries(blob).forEach(([name, timelines]) => {
    const [suite, runMode, benchmark, _] = name.split("/");
    if (!GLOBAL_DATA.data[suite]) {
      return;
    }
    // Aggregate commands across all timelines
    const times = {
      benchmark,
      run: [],
      extract: [],
      serialize: [],
      deserialize: [],
      other: [],
    };

    timelines.forEach(({ events, sexps }) => {
      events.forEach((time_ms, idx) => {
        const cmd = getCmd(sexps[idx]);

        // group commands by type (run, extract, (de)serialize, other)
        if (RUN_CMDS.includes(cmd)) {
          times.run.push(time_ms);
        } else if (EXT_CMDS.includes(cmd)) {
          times.extract.push(time_ms);
        } else if (SERIALIZE_CMDS.includes(cmd)) {
          times.serialize.push(time_ms);
        } else if (DESERIALIZE_CMDS.includes(cmd)) {
          times.deserialize.push(time_ms);
        } else {
          times.other.push(time_ms);
        }
      });
    });

    GLOBAL_DATA.data[suite][runMode].push(times);
  });
}

/**
 * Applies a specified function to an array of times.
 *
 * @param {Array<number>} times - An array of time values.
 * @param {string} mode - The aggregation function: "average", "total", or "max".
 * @returns {number} - The aggregated value based on the selected mode.
 */
function aggregate(times, mode) {
  if (times.length == 0) {
    return 0;
  }
  switch (mode) {
    case "average":
      return times.reduce((a, b) => a + b) / times.length;

    case "total":
      return times.reduce((a, b) => a + b);

    case "max":
      return Math.max(...times);

    default:
      console.warn("Unknown selection:", mode);
      return 0;
  }
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
