const statusNode = document.querySelector("#status");

load();

async function load() {
  try {
    const response = await fetch("./data/data.json");
    if (!response.ok) {
      throw new Error(`Failed to load data.json (${response.status})`);
    }

    const data = await response.json();
    statusNode.textContent = "Loaded data/data.json";
    renderSummary(data.summary);
    renderBenchmarks(data);
  } catch (error) {
    statusNode.textContent = `Failed to load data/data.json: ${error}`;
  }
}

function renderSummary(summary) {
  // TODO
}

function renderBenchmarks(data) {
  // TODO
}
