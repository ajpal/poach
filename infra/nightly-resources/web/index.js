const statusNode = document.querySelector("#status");
const rawDataNode = document.querySelector("#raw-data");

load();

async function load() {
  try {
    const response = await fetch("./data/data.json");
    if (!response.ok) {
      throw new Error(`Failed to load data.json (${response.status})`);
    }

    const data = await response.json();
    statusNode.textContent = "Loaded data/data.json";
    rawDataNode.textContent = JSON.stringify(data, null, 2);
  } catch (error) {
    statusNode.textContent = `Failed to load data/data.json: ${error}`;
    rawDataNode.textContent = "";
  }
}
