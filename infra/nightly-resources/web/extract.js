function initializeExtract() {
  initializeGlobalData().then(initializeCharts).then(plotExtract);
}

function plotExtract() {
  const all_data = GLOBAL_DATA.data.tests.extract;

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

    data[b].difference = data[b].poachTotal - data[b].vanillaTotal;
  });

  GLOBAL_DATA.differenceChart.data = {
    labels: benchmarks,
    datasets: [
      {
        label: "poach - vanilla",
        data: Object.values(data).map((d) => d.difference),
        backgroundColor: Object.values(data).map((d) => {
          if (Math.abs(d.difference) > 25) {
            return "gray";
          } else {
            return d.difference >= 0
              ? "rgba(255, 99, 132, 0.7)"
              : "rgba(54, 162, 235, 0.7)";
          }
        }),
      },
    ],
  };

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
}
