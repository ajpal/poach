const PERF_SUMMARY_PATH = "perf/perf-summary.json";

function formatNumber(value) {
  if (value === null || value === undefined) {
    return "n/a";
  }
  return value.toLocaleString();
}

function formatMs(value) {
  if (value === null || value === undefined) {
    return "n/a";
  }
  return `${value.toFixed(2)} ms`;
}

function formatPercent(value) {
  if (value === null || value === undefined) {
    return "n/a";
  }
  return `${value.toFixed(2)}%`;
}

function safePercent(part, whole) {
  if (!whole || whole <= 0) {
    return 0;
  }
  return (part * 100.0) / whole;
}

function estimateMsFromSamples(samples, sampleFreqHz) {
  if (!sampleFreqHz || sampleFreqHz <= 0) {
    return null;
  }
  return (samples * 1000.0) / sampleFreqHz;
}

function metricEstimatedMs(metric, sampleFreqHz) {
  if (metric && metric.estimated_ms !== null && metric.estimated_ms !== undefined) {
    return metric.estimated_ms;
  }
  return estimateMsFromSamples(metric?.sample_record_count ?? 0, sampleFreqHz);
}

function suiteAllSampleCounts(summary) {
  const totals = new Map();
  for (const benchmark of Object.values(summary.benchmarks || {})) {
    const suite = benchmark.suite || "unknown";
    const prior = totals.get(suite) || 0;
    totals.set(suite, prior + (benchmark.sample_record_count || 0));
  }
  return totals;
}

function parseLegacySuiteAndBenchmark(filePath) {
  const normalized = filePath.replace(/\\/g, "/");
  const marker = "/perf/";
  const markerIdx = normalized.indexOf(marker);
  if (markerIdx < 0) {
    return { suite: "unknown", benchmark: normalized };
  }
  const rel = normalized.slice(markerIdx + marker.length);
  const parts = rel.split("/");
  const suite = parts[0] || "unknown";
  const benchmarkPath = parts.slice(1).join("/");
  const benchmark = benchmarkPath.replace(/\.perf\.data$/, "");
  return { suite, benchmark };
}

function normalizeLegacySummary(rawSummary) {
  const calleeSymbols = Array.isArray(rawSummary.callee_symbols)
    ? rawSummary.callee_symbols
    : [];

  const benchmarks = {};
  const suites = {};

  for (const entry of rawSummary.files || []) {
    const identity = parseLegacySuiteAndBenchmark(entry.file || "");
    const rootCount = entry.function_focus?.root_sample_record_count ?? 0;
    const rootPeriod = entry.function_focus?.root_period_sum ?? 0;
    const callees = [];

    for (const calleeSymbol of calleeSymbols) {
      const match = (entry.function_focus?.callees || []).find(
        (callee) => callee.symbol === calleeSymbol
      );
      const sampleCount = match?.sample_record_count ?? 0;
      const periodSum = match?.period_sum ?? 0;
      callees.push({
        symbol: calleeSymbol,
        sample_record_count: sampleCount,
        period_sum: periodSum,
        estimated_ms: null,
        percent_of_root_samples:
          rootCount === 0 ? 0 : (sampleCount * 100.0) / rootCount,
        percent_of_root_period:
          rootPeriod === 0 ? 0 : (periodSum * 100.0) / rootPeriod,
      });
    }

    const benchmarkKey = `${identity.suite}/${identity.benchmark}`;
    benchmarks[benchmarkKey] = {
      suite: identity.suite,
      benchmark: identity.benchmark,
      file: entry.file ?? "",
      sample_record_count: entry.sample_record_count ?? 0,
      parsed_event_record_count: entry.parsed_event_record_count ?? 0,
      root: {
        sample_record_count: rootCount,
        period_sum: rootPeriod,
        estimated_ms: null,
      },
      callees,
    };

    if (!suites[identity.suite]) {
      suites[identity.suite] = {
        benchmark_count: 0,
        root: { sample_record_count: 0, period_sum: 0, estimated_ms: null },
        callees: calleeSymbols.map((symbol) => ({
          symbol,
          sample_record_count: 0,
          period_sum: 0,
          estimated_ms: null,
          percent_of_root_samples: 0,
          percent_of_root_period: 0,
        })),
      };
    }

    suites[identity.suite].benchmark_count += 1;
    suites[identity.suite].root.sample_record_count += rootCount;
    suites[identity.suite].root.period_sum += rootPeriod;
    for (let i = 0; i < calleeSymbols.length; i++) {
      suites[identity.suite].callees[i].sample_record_count +=
        callees[i].sample_record_count;
      suites[identity.suite].callees[i].period_sum += callees[i].period_sum;
    }
  }

  for (const suite of Object.values(suites)) {
    for (const callee of suite.callees) {
      callee.percent_of_root_samples =
        suite.root.sample_record_count === 0
          ? 0
          : (callee.sample_record_count * 100.0) / suite.root.sample_record_count;
      callee.percent_of_root_period =
        suite.root.period_sum === 0
          ? 0
          : (callee.period_sum * 100.0) / suite.root.period_sum;
    }
  }

  return {
    meta: {
      perf_dir: rawSummary.perf_dir ?? "nightly/output/perf",
      event_name: null,
      sample_freq_hz: null,
      sampling_policy: null,
      root_symbol: rawSummary.root_symbol ?? "run_extract_command",
      callee_symbols: calleeSymbols,
      total_files: rawSummary.total_files ?? Object.keys(benchmarks).length,
    },
    suites,
    benchmarks,
  };
}

function normalizeSummary(rawSummary) {
  if (rawSummary && rawSummary.meta && rawSummary.suites && rawSummary.benchmarks) {
    return rawSummary;
  }
  return normalizeLegacySummary(rawSummary);
}

function setLoadedState(sourcePath) {
  document.getElementById("before-load").style.display = "none";
  document.getElementById("on-error").style.display = "none";
  document.getElementById("after-load").style.display = "block";
  document.getElementById("summary-source").textContent = `Source: ${sourcePath}`;
}

function setErrorState(message) {
  document.getElementById("before-load").style.display = "none";
  document.getElementById("on-error").style.display = "block";
  document.getElementById("after-load").style.display = "none";
  console.error(message);
}

function renderMeta(meta) {
  const rows = [
    ["perf_dir", meta.perf_dir],
    ["event_name", meta.event_name],
    ["sample_freq_hz", meta.sample_freq_hz],
    ["sampling_policy", meta.sampling_policy],
    ["root_symbol", meta.root_symbol],
    ["callee_symbols", (meta.callee_symbols || []).join(", ")],
    ["total_files", meta.total_files],
  ];

  const table = document.getElementById("meta-table");
  table.innerHTML = `
    <thead>
      <tr>
        <th class="cell-text">Field</th>
        <th class="cell-text">Value</th>
      </tr>
    </thead>
    <tbody>
      ${rows
        .map(
          ([key, value]) =>
            `<tr><td class="cell-text">${key}</td><td class="cell-text">${value ?? "n/a"}</td></tr>`
        )
        .join("")}
    </tbody>
  `;
}

function calleeHeaderCells(calleeSymbols) {
  return calleeSymbols
    .map(
      (symbol) =>
        `<th>${symbol} samples</th><th>${symbol} est.</th><th>${symbol} %root</th>`
    )
    .join("");
}

function calleeValueCells(callees, calleeSymbols, sampleFreqHz) {
  const bySymbol = new Map((callees || []).map((callee) => [callee.symbol, callee]));
  return calleeSymbols
    .map((symbol) => {
      const callee = bySymbol.get(symbol);
      const estimatedMs = metricEstimatedMs(callee, sampleFreqHz);
      return `<td>${formatNumber(callee?.sample_record_count ?? 0)}</td><td>${formatMs(
        estimatedMs
      )}</td><td>${formatPercent(callee?.percent_of_root_samples ?? 0)}</td>`;
    })
    .join("");
}

function renderSuitesTable(summary) {
  const calleeSymbols = summary.meta.callee_symbols || [];
  const sampleFreqHz = summary.meta.sample_freq_hz;
  const suiteAllSamples = suiteAllSampleCounts(summary);
  const suiteRows = Object.entries(summary.suites || {}).sort((a, b) => {
    return (b[1]?.root?.sample_record_count ?? 0) - (a[1]?.root?.sample_record_count ?? 0);
  });
  let totalBenchmarkCount = 0;
  let totalAllSamples = 0;
  let totalRootSamples = 0;
  const totalCalleeSamples = new Map(calleeSymbols.map((symbol) => [symbol, 0]));

  for (const [suiteName, suite] of suiteRows) {
    totalBenchmarkCount += suite.benchmark_count || 0;
    totalAllSamples += suiteAllSamples.get(suiteName) || 0;
    totalRootSamples += suite.root?.sample_record_count || 0;

    const suiteCalleeBySymbol = new Map(
      (suite.callees || []).map((callee) => [callee.symbol, callee])
    );
    for (const symbol of calleeSymbols) {
      const prior = totalCalleeSamples.get(symbol) || 0;
      totalCalleeSamples.set(
        symbol,
        prior + (suiteCalleeBySymbol.get(symbol)?.sample_record_count || 0)
      );
    }
  }

  const totalCallees = calleeSymbols.map((symbol) => {
    const count = totalCalleeSamples.get(symbol) || 0;
    return {
      symbol,
      sample_record_count: count,
      estimated_ms: null,
      percent_of_root_samples: totalRootSamples === 0 ? 0 : (count * 100.0) / totalRootSamples,
    };
  });

  const table = document.getElementById("suite-table");
  table.innerHTML = `
    <thead>
      <tr>
        <th class="cell-text">Suite</th>
        <th>Benchmarks</th>
        <th>All samples</th>
        <th>All est.</th>
        <th>Root samples</th>
        <th>Root est.</th>
        <th>Root % all</th>
        ${calleeHeaderCells(calleeSymbols)}
      </tr>
    </thead>
    <tbody>
      ${suiteRows
        .map(([suiteName, suite]) => {
          const allSamples = suiteAllSamples.get(suiteName) || 0;
          const allEstimatedMs = estimateMsFromSamples(allSamples, sampleFreqHz);
          const rootSamples = suite.root.sample_record_count || 0;
          return `
            <tr>
              <td class="cell-text">${suiteName}</td>
              <td>${formatNumber(suite.benchmark_count)}</td>
              <td>${formatNumber(allSamples)}</td>
              <td>${formatMs(allEstimatedMs)}</td>
              <td>${formatNumber(rootSamples)}</td>
              <td>${formatMs(metricEstimatedMs(suite.root, sampleFreqHz))}</td>
              <td>${formatPercent(safePercent(rootSamples, allSamples))}</td>
              ${calleeValueCells(suite.callees, calleeSymbols, sampleFreqHz)}
            </tr>
          `;
        })
        .join("")}
      <tr class="total-row">
        <td class="cell-text">TOTAL</td>
        <td>${formatNumber(totalBenchmarkCount)}</td>
        <td>${formatNumber(totalAllSamples)}</td>
        <td>${formatMs(estimateMsFromSamples(totalAllSamples, sampleFreqHz))}</td>
        <td>${formatNumber(totalRootSamples)}</td>
        <td>${formatMs(estimateMsFromSamples(totalRootSamples, sampleFreqHz))}</td>
        <td>${formatPercent(safePercent(totalRootSamples, totalAllSamples))}</td>
        ${calleeValueCells(totalCallees, calleeSymbols, sampleFreqHz)}
      </tr>
    </tbody>
  `;
}

function renderBenchmarksTable(summary) {
  const calleeSymbols = summary.meta.callee_symbols || [];
  const sampleFreqHz = summary.meta.sample_freq_hz;
  const benchmarkRows = Object.entries(summary.benchmarks || {}).sort((a, b) => {
    return (b[1]?.root?.sample_record_count ?? 0) - (a[1]?.root?.sample_record_count ?? 0);
  });

  const table = document.getElementById("benchmark-table");
  table.innerHTML = `
    <thead>
      <tr>
        <th class="cell-text">Benchmark</th>
        <th class="cell-text">Suite</th>
        <th>All samples</th>
        <th>All est.</th>
        <th>Root samples</th>
        <th>Root est.</th>
        <th>Root % all</th>
        ${calleeHeaderCells(calleeSymbols)}
      </tr>
    </thead>
    <tbody>
      ${benchmarkRows
        .map(([benchmarkKey, benchmark]) => {
          const allSamples = benchmark.sample_record_count || 0;
          const allEstimatedMs = estimateMsFromSamples(allSamples, sampleFreqHz);
          const rootSamples = benchmark.root.sample_record_count || 0;
          return `
            <tr>
              <td class="cell-text">${benchmarkKey}</td>
              <td class="cell-text">${benchmark.suite}</td>
              <td>${formatNumber(allSamples)}</td>
              <td>${formatMs(allEstimatedMs)}</td>
              <td>${formatNumber(rootSamples)}</td>
              <td>${formatMs(metricEstimatedMs(benchmark.root, sampleFreqHz))}</td>
              <td>${formatPercent(safePercent(rootSamples, allSamples))}</td>
              ${calleeValueCells(benchmark.callees, calleeSymbols, sampleFreqHz)}
            </tr>
          `;
        })
        .join("")}
    </tbody>
  `;
}

async function loadLatestSummary() {
  const cacheBuster = `ts=${Date.now()}`;
  const requestPath = `${PERF_SUMMARY_PATH}?${cacheBuster}`;
  const response = await fetch(requestPath, { cache: "no-store" });
  if (!response.ok) {
    throw new Error(
      `Could not load ${PERF_SUMMARY_PATH}. Run nightly generation to produce the latest perf summary.`
    );
  }
  return { sourcePath: PERF_SUMMARY_PATH, data: await response.json() };
}

async function initializePerfSummary() {
  try {
    const loaded = await loadLatestSummary();
    const summary = normalizeSummary(loaded.data);
    renderMeta(summary.meta);
    renderSuitesTable(summary);
    renderBenchmarksTable(summary);
    setLoadedState(loaded.sourcePath);
  } catch (error) {
    setErrorState(error);
  }
}
