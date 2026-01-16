const GLOBAL_DATA = {};

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
  "no-io",
  "extract",
];

const CMDS = ["run", "extract", "serialize", "deserialize", "read", "write"];

function initializeGlobalData() {
  GLOBAL_DATA.data = Object.fromEntries(
    BENCH_SUITES.map((suite) => [
      suite.dir,
      { ...suite, ...Object.fromEntries(RUN_MODES.map((mode) => [mode, {}])) },
    ])
  );
  GLOBAL_DATA.runExtractChart = null;
  GLOBAL_DATA.serializeChart = null;
  GLOBAL_DATA.extractChart = null;
  GLOBAL_DATA.differenceChart = null;
  return fetch("data/data.json")
    .then((response) => response.json())
    .then(processRawData);
}

/**
 * Expected data layout:
 * key is the name of the benchmark egg file
 * value is the array of timelines where each timeline contains an array of events
 */
function processRawData(blob) {
  Object.entries(blob).forEach(([name, timelines]) => {
    const [suite, runMode, benchmark, _] = name.split("/");
    if (!GLOBAL_DATA.data[suite]) {
      return;
    }
    // Aggregate commands across all timelines
    const times = {
      benchmark,
      ...Object.fromEntries(CMDS.map((cmd) => [cmd, []])),
      other: [],
    };

    timelines.forEach(({ events, sexps }) => {
      events.forEach((time_micros, idx) => {
        const cmd = getCmd(sexps[idx]);

        // Group times by command type
        times[getCmdType(cmd)].push(time_micros / 1000); // we measure microseconds, but for charts, it's nicer to show in ms
      });
    });

    GLOBAL_DATA.data[suite][runMode][benchmark] = times;
  });
}
