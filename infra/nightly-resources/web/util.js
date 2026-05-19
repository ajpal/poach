export function formatTime(rawValue, mode) {
  if (rawValue === undefined) return "-";
  const ONE_MIN = 60000000;
  const ONE_SEC = 1000000;
  const ONE_MILLI = 1000;
  if (mode === "raw") return `${rawValue} μs`;
  console.assert(mode === "readable");
  if (rawValue >= ONE_MIN) return `${(rawValue / ONE_MIN).toFixed(2)} min`;
  if (rawValue >= ONE_SEC) return `${(rawValue / ONE_SEC).toFixed(2)} s`;
  if (rawValue >= ONE_MILLI) return `${(rawValue / ONE_MILLI).toFixed(2)} ms`;
  return `${rawValue} μs`;
}

export const fmtSpeedup = (v) => (v === null ? "—" : `${v.toFixed(2)}×`);
export const fmtPct = (v) => (v === null ? "—" : `${v.toFixed(1)}%`);
export const fmtSize = (v) =>
  v === null ? "—" : v.toLocaleString(undefined, { maximumFractionDigits: 0 });

export function unwrapCount(v) {
  if (!v || typeof v !== "object" || !("Count" in v)) {
    throw new Error(
      `unwrapCount expected {Count: N}, got ${JSON.stringify(v)}`,
    );
  }
  return v.Count;
}

export function average(xs) {
  return xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : null;
}
