function initializeGlobalData() {
  GLOBAL_DATA.data = Object.fromEntries(
    BENCH_SUITES.map((suite) => [
      suite.dir,
      { ...suite, ...Object.fromEntries(RUN_MODES.map((mode) => [mode, {}])) },
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
        times[getCmdType(cmd)].push(time_micros);
      });
    });

    GLOBAL_DATA.data[suite][runMode][benchmark] = times;
  });
}
