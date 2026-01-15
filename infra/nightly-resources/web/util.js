/**
 * Applies a specified function to an array of times.
 *
 * @param {Array<number>} times - An array of time values.
 * @param {string} mode - The aggregation function: "average", "total", or "max".
 * @returns {number} - The aggregated value based on the selected mode.
 */
function aggregate(times, mode) {
  if (!times || times.length == 0) {
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
 * Extracts the first token from an s-expression.
 */
function getCmd(sexp) {
  const match = sexp.match(/[^\(\s\)]+/);
  if (match) {
    return match[0];
  } else {
    console.warn(`could not parse command from ${sexp}`);
    return null;
  }
}

function getCmdType(cmd) {
  const CMDS = ["run", "extract", "serialize", "deserialize", "read", "write"];
  if (cmd === "run-schedule") {
    return "run";
  } else if (cmd === "multi-extract") {
    return "extract";
  } else if (CMDS.includes(cmd)) {
    return cmd;
  } else {
    return "other";
  }
}
