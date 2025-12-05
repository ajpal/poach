const GLOBAL_DATA = {};

function initializePage() {
  initializeGlobalData()
    .then(initializeSerializationDropdown)
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
  "no-io",
];

const CMDS = ["run", "extract", "serialize", "deserialize", "read", "write"];

function initializeSerializationDropdown() {
  const files = Object.keys(GLOBAL_DATA.data.tests.sequential).sort();
  const dropdownElt = document.getElementById("tests");
  files.forEach((file) => {
    const opt = document.createElement("option");
    opt.value = file;
    opt.textContent = file;
    dropdownElt.appendChild(opt);
  });
}

// Handlers

function serializationDropdownChange(e) {
  plotSerialization(e.target.value);
}
