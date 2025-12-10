const GLOBAL_DATA = {};

function initializePage() {
  initializeGlobalData()
    .then(initializeSerializationOptions)
    .then(initializeCharts)
    .then(plotTimeline)
    .then(plotSerialization);
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

function updateSerializationOptions() {
  const suite = document.querySelector(
    'input[name="suiteToggle"]:checked'
  ).value;

  if (!suite) {
    return;
  }

  const files = Object.keys(GLOBAL_DATA.data[suite].sequential).sort();

  const dropdownElt = document.getElementById("tests");
  dropdownElt.options.length = 0;

  const opt = document.createElement("option");
  opt.value = "";
  opt.textContent = "Show All Benchmarks";
  dropdownElt.appendChild(opt);

  files.forEach((file) => {
    const opt = document.createElement("option");
    opt.value = file;
    opt.textContent = file;
    dropdownElt.appendChild(opt);
  });

  plotSerialization();
}

function initializeSerializationOptions() {
  const suiteElt = document.getElementById("suite");
  Object.keys(GLOBAL_DATA.data).forEach((suite, idx) => {
    const label = document.createElement("label");
    const input = document.createElement("input");

    input.type = "radio";
    input.name = "suiteToggle";
    input.value = suite;

    if (idx === 0) {
      input.checked = true; // select first run mode
    }

    label.appendChild(input);
    label.append(" " + suite);

    suiteElt.appendChild(label);
  });

  updateSerializationOptions();

  // Add run modes as radio buttons
  const formElt = document.getElementById("runModeToggle");
  RUN_MODES.forEach((runMode, idx) => {
    const label = document.createElement("label");
    const input = document.createElement("input");

    input.type = "radio";
    input.name = "runModeToggle";
    input.value = runMode;

    if (idx === 0) {
      input.checked = true; // select first run mode
    }

    label.appendChild(input);
    label.append(" " + runMode);

    formElt.appendChild(label);
  });
}
