/**
 * Expected data layout:
 * data/data.json : big JSON blob containing all of the timeline data
 * key is the name of the benchmark egg file
 * value is the array of timelines where each timeline contains an array of events
 */

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

let chart = null;

// Aggregated/processed data for chart
// key is the benchmark category
// value contains an array of data points, corresponding to all of the individual
// egg files in the benchmark category.
// Each data point contains arrays of times for run, extract, serialize, and deserialize events
let loadedData = Object.fromEntries(
  BENCH_SUITES.map((suite) => [suite.dir, { ...suite, data: [] }])
);

/**
 * Loads the timeline page.
 */
function loadTimeline() {
  fetch("data/data.json")
    .then((response) => response.json())
    .then(processRawData)
    .then(plot);
}

function getCmd(sexp) {
  const match = sexp.match(/[^\(\s]+/);
  if (match) {
    return match[0];
  } else {
    console.warn(`could not parse command from ${sexp}`);
    return null;
  }
}

function processRawData(blob) {
  const RUN_CMDS = ["run", "run-schedule"];
  const EXT_CMDS = ["extract", "multi-extract"];
  const SERIALIZE_CMDS = ["serialize"];
  const DESERIALIZE_CMDS = ["deserialize"];

  Object.entries(blob).forEach(([name, timelines]) => {
    const [suite, benchmark, _] = name.split("/");
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

    loadedData[suite].data.push(times);
  });
}

function getDatapoint(name) {
  const RUN_CMDS = ["run", "run-schedule"];
  const EXT_CMDS = ["extract", "multi-extract"];
  const SERIALIZE_CMDS = ["serialize"];
  const DESERIALIZE_CMDS = ["deserialize"];

  return fetch(`data/${name}`)
    .then((response) => response.json())
    .then((data) => {
      [suite, benchmark, _] = name.split("/");

      // Aggregate commands across all timelines
      const times = {
        benchmark,
        run: [],
        extract: [],
        serialize: [],
        deserialize: [],
        other: [],
      };

      data.forEach((timeline) => {
        timeline.evts.forEach((event) => {
          const ms = event.total_time_ms;
          const cmd = event.cmd;

          // group commands by type (run, extract, (de)serialize, other)
          if (RUN_CMDS.includes(cmd)) {
            times.run.push(ms);
          } else if (EXT_CMDS.includes(cmd)) {
            times.extract.push(ms);
          } else if (SERIALIZE_CMDS.includes(cmd)) {
            times.serialize.push(ms);
          } else if (DESERIALIZE_CMDS.includes(cmd)) {
            times.deserialize.push(ms);
          } else {
            times.other.push(ms);
          }
        });
      });

      loadedData[suite].data.push(times);
    });
}

/**
 * Fetches and processes datapoints for a given benchmark suite.
 *
 * @param {string} suite - The directory name of the benchmark suite.
 * @param {Array<string>} names - The list of JSON filenames to fetch data from.
 * @returns {Promise<Array<Object>>} - A promise that resolves to an array of processed datapoints,
 *                                    grouped by command type (run, extract, other).
 */
function getDatapoints(suite, names) {
  const RUN_CMDS = ["run", "run-schedule"];
  const EXT_CMDS = ["extract", "multi-extract"];

  const datapoints = names.map((name) =>
    fetch(`data/${suite}/${name}`)
      .then((response) => response.json())
      // Currently, all of our tests run a new egraph for each .egg file.
      // However, it is possible to run a single egraph on multiple .egg files,
      // in which case each file will correspond to an entry in the JSON array.
      .then((data) => data[0].evts)
      .then((events) => {
        const times = {
          runs: [],
          exts: [],
          others: [],
        };

        events.forEach((entry) => {
          const ms = entry.total_time_ms;
          const cmd = entry.cmd;

          // group commands by type (run, extract, other)
          if (RUN_CMDS.includes(cmd)) {
            times.runs.push(ms);
          } else if (EXT_CMDS.includes(cmd)) {
            times.exts.push(ms);
          } else {
            times.others.push(ms);
          }
        });

        return times;
      })
  );

  return Promise.all(datapoints);
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
function plot() {
  if (chart === null) {
    const ctx = document.getElementById("chart").getContext("2d");

    chart = new Chart(ctx, {
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
    });
  }

  const mode = document.querySelector('input[name="mode"]:checked').value;

  const datasets = Object.values(loadedData).map((suite) => ({
    label: suite.name,
    data: Object.values(suite.data).map((entry) => {
      return {
        x: aggregate(entry.run, mode),
        y: aggregate(entry.extract, mode),
      };
    }),
    backgroundColor: suite.color,
    pointRadius: 4,
  }));

  chart.data.datasets = datasets;

  chart.update();
}
