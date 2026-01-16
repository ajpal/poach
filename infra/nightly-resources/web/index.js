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

/**
 * Temporary for timing new extraction
 */
function compareExtractionTypes() {
  kdData = Object.fromEntries(
    BENCH_SUITES.map((suite) => [suite.dir, { ...suite, data: [] }])
  );

  fetch("data/kd/data.json")
    .then((response) => response.json())
    .then((blob) => {
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

        kdData[suite].data.push(times);

        const tableData = BENCH_SUITES.map(suite => ({
          benchmarks: suite.dir,
          bf: aggregate(loadedData[suite.dir].data.map(
            (data) => aggregate(data.extract, "total")
          ), "total"),
          kd: aggregate(kdData[suite.dir].data.map(
            (data) => aggregate(data.extract, "total")
          ), "total"),
        }));

        let html = '<table style="border-collapse: collapse;"><thead><tr>';

        headers = ["Benchmarks", "Bellman-Ford (ms)", "Knuth (ms)"]

        headers.forEach(header => {
          html += `<th style="padding: 8px 16px; border: 1px solid #ddd;">${header}</th>`;
        });
        html += '</tr></thead><tbody>';

        tableData.forEach(row => {
          html += '<tr>';
          html += `<td style="padding: 8px 16px; border: 1px solid #ddd;">${row.benchmarks}</td>`;
          html += `<td style="padding: 8px 16px; border: 1px solid #ddd;">${row.bf}</td>`;
          html += `<td style="padding: 8px 16px; border: 1px solid #ddd;">${row.kd}</td>`;
          html += '</tr>';
        });

        html += '</tbody></table>';

        document.getElementById('extraction-types-table').innerHTML = html;
      });
    });
}
