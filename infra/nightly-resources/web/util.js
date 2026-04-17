function textCell(value) {
  const cell = document.createElement("td");
  cell.textContent = value;
  return cell;
}

function timeMsCell(value) {
  const cell = document.createElement("td");
  cell.textContent = `${value} ms`;
  return cell;
}
