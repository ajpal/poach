function initializeExtract() {
  initializeGlobalData()
    .then(initializeExtractOptions)  
    .then(initializeCharts)
    .then(plotExtract);
}

function initializeExtractOptions() {
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
}


function plotExtract() {

  const suite = document.querySelector(
    'input[name="suiteToggle"]:checked'
  ).value;

  if (!suite) {
    return;
  }

  const all_data = GLOBAL_DATA.data[suite].extract;

  if (GLOBAL_DATA.extractChart === null) {
    return;
  }

  const benchmarks = Object.keys(all_data);

  const data = {};

  benchmarks.forEach((b) => {
    data[b] = {};

    // hack: we don't distinguish between the two sets of extracts,
    // but we know the first half are from the vanilla run, and the
    // second half are from the deserialized (POACH) e-graph
    const extracts = all_data[b].extract;
    console.assert(extracts.length % 2 === 0);
    const midpoint = extracts.length / 2;

    data[b].vanillaRun = aggregate(all_data[b].run, "total");
    data[b].vanillaExtract = aggregate(extracts.slice(0, midpoint), "total");
    data[b].vanillaTotal = data[b].vanillaRun + data[b].vanillaExtract;

    data[b].poachExtract = aggregate(extracts.slice(midpoint), "total");
    data[b].poachDeser = aggregate(all_data[b].deserialize, "total");
    data[b].poachTotal = data[b].poachDeser + data[b].poachExtract;

    data[b].difference = data[b].vanillaTotal - data[b].poachTotal;
  });

  GLOBAL_DATA.differenceChart.data = {
    labels: benchmarks,
    datasets: [
      {
        label: "poach - vanilla",
        data: Object.values(data).map((d) => d.difference),
        backgroundColor: Object.values(data).map((d) => {
          return d.difference >= 0
            ? "rgba(54, 162, 235, 0.7)"
            : "rgba(255, 99, 132, 0.7)";
        }),
      },
    ],
  };

  GLOBAL_DATA.differenceChart.update();

  GLOBAL_DATA.extractChart.data = {
    labels: benchmarks,
    datasets: [
      // Vanilla Egglog
      {
        label: "Run",
        data: Object.values(data).map((d) => d.vanillaRun),
        backgroundColor: "rgba(75, 192, 192, 0.8)",
        stack: "vanilla",
      },
      {
        label: "Extract",
        data: Object.values(data).map((d) => d.vanillaExtract),
        backgroundColor: "rgba(75, 192, 192, 0.4)",
        stack: "vanilla",
      },

      // POACH
      {
        label: "Deserialize",
        data: Object.values(data).map((d) => d.poachDeser),
        backgroundColor: "rgba(255, 159, 64, 0.8)",
        stack: "poach",
      },
      {
        label: "Extract",
        data: Object.values(data).map((d) => d.poachExtract),
        backgroundColor: "rgba(255, 159, 64, 0.4)",
        stack: "poach",
      },
    ],
  };

  GLOBAL_DATA.extractChart.update();  
}
