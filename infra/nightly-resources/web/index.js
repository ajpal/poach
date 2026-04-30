import { formatMillis, formatSeconds } from "./util.js";

const TIMING_FIELDS = [
  "rule_running_millis",
  "extraction_millis",
  "serialize_millis",
  "other_millis",
  "total_millis",
];

const GLOBAL_DATA = {
  data: null,
  activeSuiteName: null,
  sortKey: "benchmark_path",
  sortDir: "asc",
};

load();

async function load() {
  const statusNode = document.querySelector("#status");
  try {
    const response = await fetch("./data/data.json");
    if (!response.ok) {
      throw new Error(`Failed to load data.json (${response.status})`);
    }
    GLOBAL_DATA.data = await response.json();
    GLOBAL_DATA.activeSuiteName = GLOBAL_DATA.data.suites[0]?.name ?? null;
    statusNode.textContent = "Loaded data/data.json";
    renderSummary();
    renderSuiteTabs();
    renderActiveSuite();
  } catch (err) {
    statusNode.textContent = `Failed to load data/data.json: ${err}`;
  }
}

function renderSummary() {
  const { data } = GLOBAL_DATA;
  const generated = new Date(data.generated_at).toLocaleString();
  const branchSummaries = data.branches
    .map((branch) => {
      const s = data.summary.branches[branch];
      return `<a href="./data/${branch}/data.json">${branch}</a>: ${s.benchmark_count} reports, ${formatSeconds(s.total_time_seconds)}`;
    })
    .join(" | ");
  document.querySelector("#summary-text").innerHTML =
    `Generated ${generated} | ${branchSummaries}`;
}

function renderSuiteTabs() {
  const tabs = document.querySelector("#suite-tabs");
  tabs.innerHTML = GLOBAL_DATA.data.suites
    .map(
      (suite) => `
        <button
          type="button"
          class="suite-tab${suite.name === GLOBAL_DATA.activeSuiteName ? " is-active" : ""}"
          data-suite-name="${suite.name}"
        >
          ${suite.name}
        </button>
      `,
    )
    .join("");

  for (const button of tabs.querySelectorAll(".suite-tab")) {
    button.addEventListener("click", () => {
      GLOBAL_DATA.activeSuiteName = button.dataset.suiteName;
      renderSuiteTabs();
      renderActiveSuite();
    });
  }
}

function renderActiveSuite() {
  const { data, activeSuiteName } = GLOBAL_DATA;
  const suite = data.suites.find((s) => s.name === activeSuiteName);
  const summaryNode = document.querySelector("#active-suite-summary");
  const headerNode = document.querySelector("#benchmarks-header");
  const bodyNode = document.querySelector("#benchmarks-body");

  if (!suite) {
    summaryNode.textContent = "";
    headerNode.innerHTML = "";
    bodyNode.innerHTML = "";
    return;
  }

  const benchmarks = data.benchmarks.filter((b) => b.suite === activeSuiteName);
  const columns = buildColumns(benchmarks);

  const branchLine = data.branches
    .map((branch) => {
      const s = suite.branches[branch];
      return s
        ? `${branch}: ${formatSeconds(s.total_time_seconds)}`
        : `${branch}: —`;
    })
    .join(" | ");
  summaryNode.innerHTML = `
    <div class="suite-header">
      <h3>${suite.name}</h3>
      <p>${benchmarks.length} benchmarks | ${branchLine}</p>
    </div>
  `;

  const topHeaderNode = document.querySelector("#benchmarks-header-top");
  const groupCounts = new Map();
  for (const col of columns) {
    groupCounts.set(col.branch, (groupCounts.get(col.branch) ?? 0) + 1);
  }
  topHeaderNode.innerHTML =
    `<th rowspan="2" data-sort-key="benchmark_path">benchmark</th>` +
    data.branches
      .filter((b) => groupCounts.has(b))
      .map(
        (b) =>
          `<th colspan="${groupCounts.get(b)}" class="branch-start">${b}</th>`,
      )
      .join("");
  headerNode.innerHTML = columns
    .map(
      (col, idx) =>
        `<th data-sort-key="col-${idx}"${col.firstInBranch ? ' class="branch-start"' : ""}>${col.subLabel}</th>`,
    )
    .join("");

  bodyNode.innerHTML = sortBenchmarks(benchmarks, columns)
    .map((bench) => {
      const cells = columns
        .map((col) => {
          const value = col.get(bench);
          const display =
            value === null || value === undefined ? "—" : col.fmt(value);
          const cls = col.firstInBranch ? ' class="branch-start"' : "";
          return `<td${cls}>${display}</td>`;
        })
        .join("");
      return `<tr><td>${bench.benchmark_path}</td>${cells}</tr>`;
    })
    .join("");

  installHeaderSortHandlers();
  updateHeaderIndicators();
}

function buildColumns(benchmarks) {
  // Discover (branch, phase) pairs present in this suite. Branches keep the
  // order from data.branches; phases follow PHASE_ORDER.
  const PHASE_ORDER = ["_", "train", "serve"];
  const branchPhases = [];
  for (const branch of GLOBAL_DATA.data.branches) {
    const phases = new Set();
    for (const benchmark of benchmarks) {
      const phasesForBranch = benchmark.branches[branch]?.phases;
      if (!phasesForBranch) {
        continue;
      }
      for (const phase of Object.keys(phasesForBranch)) {
        phases.add(phase);
      }
    }
    for (const phase of PHASE_ORDER) {
      if (phases.has(phase)) {
        branchPhases.push({ branch, phase });
      }
    }
  }

  const columns = [];
  let lastBranch = null;
  for (const { branch, phase } of branchPhases) {
    const phasePrefix = phase === "_" ? "" : `${phase} `;
    const firstInBranch = branch !== lastBranch;
    lastBranch = branch;

    columns.push({
      branch,
      firstInBranch,
      subLabel: `${phasePrefix}time (s)`,
      get: (b) => b.branches[branch]?.phases[phase]?.time_seconds ?? null,
      fmt: (v) => v.toFixed(2),
    });

    // Only include timing fields that show up for at least one benchmark in
    // this (branch, phase) — keeps the table from sprouting empty columns.
    const presentFields = new Set();
    for (const benchmark of benchmarks) {
      const summary =
        benchmark.branches[branch]?.phases[phase]?.timing_summary;
      if (!summary) {
        continue;
      }
      for (const field of TIMING_FIELDS) {
        if (field in summary) {
          presentFields.add(field);
        }
      }
    }
    for (const field of TIMING_FIELDS) {
      if (!presentFields.has(field)) {
        continue;
      }
      const shortName = field.replace(/_millis$/, "");
      columns.push({
        branch,
        subLabel: `${phasePrefix}${shortName}`,
        get: (b) =>
          b.branches[branch]?.phases[phase]?.timing_summary?.[field] ?? null,
        fmt: (v) => formatMillis(v),
      });
    }
  }
  return columns;
}

function getSortValue(bench, key, columns) {
  if (key === "benchmark_path") {
    return bench.benchmark_path;
  }
  if (key.startsWith("col-")) {
    const idx = Number(key.slice(4));
    return columns[idx]?.get(bench) ?? null;
  }
  return null;
}

function sortBenchmarks(benchmarks, columns) {
  const dir = GLOBAL_DATA.sortDir === "asc" ? 1 : -1;
  const sorted = [...benchmarks];
  sorted.sort((a, b) => {
    const av = getSortValue(a, GLOBAL_DATA.sortKey, columns);
    const bv = getSortValue(b, GLOBAL_DATA.sortKey, columns);
    if (av === null || av === undefined) {
      return 1;
    }
    if (bv === null || bv === undefined) {
      return -1;
    }
    if (typeof av === "string" || typeof bv === "string") {
      return String(av).localeCompare(String(bv)) * dir;
    }
    return (av - bv) * dir;
  });
  return sorted;
}

function installHeaderSortHandlers() {
  for (const th of document.querySelectorAll("thead th[data-sort-key]")) {
    th.style.cursor = "pointer";
    th.addEventListener("click", () => {
      const key = th.dataset.sortKey;
      if (GLOBAL_DATA.sortKey === key) {
        GLOBAL_DATA.sortDir = GLOBAL_DATA.sortDir === "asc" ? "desc" : "asc";
      } else {
        GLOBAL_DATA.sortKey = key;
        GLOBAL_DATA.sortDir = "asc";
      }
      renderActiveSuite();
    });
  }
}

function updateHeaderIndicators() {
  for (const th of document.querySelectorAll("thead th[data-sort-key]")) {
    const label = th.textContent.replace(/[ ▲▼]+$/, "");
    let arrow = "";
    if (th.dataset.sortKey === GLOBAL_DATA.sortKey) {
      arrow = GLOBAL_DATA.sortDir === "asc" ? " ▲" : " ▼";
    }
    th.textContent = label + arrow;
  }
}
