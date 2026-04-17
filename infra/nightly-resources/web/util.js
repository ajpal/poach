export function textCell(value) {
  const cell = document.createElement("td");
  cell.textContent = value;
  return cell;
}

export function timeMsCell(value) {
  const cell = document.createElement("td");
  cell.textContent = `${value} ms`;
  return cell;
}
