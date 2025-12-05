function initializeGlobalData() {
  GLOBAL_DATA.data = Object.fromEntries(
    BENCH_SUITES.map((suite) => [
      suite.dir,
      { ...suite, ...Object.fromEntries(RUN_MODES.map((mode) => [mode, []])) },
    ])
  );
  GLOBAL_DATA.runExtractChart = null;
  GLOBAL_DATA.serializeChart = null;
  return fetch("data/data.json")
    .then((response) => response.json())
    .then(processRawData);
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
  const CMDS = ["run", "extract", "serialize", "deserialize", "read", "write"];

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
      events.forEach((time_ms, idx) => {
        const cmd = getCmd(sexps[idx]);

        // Group times by command type
        times[getCmdType(cmd)].push(time_ms);
      });
    });

    GLOBAL_DATA.data[suite][runMode].push(times);
  });
}
