export function formatTime(rawValue, mode) {
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

export function formatBytes(rawValue, mode) {
  const ONE_GB = 1024 * 1024 * 1024;
  const ONE_MB = 1024 * 1024;
  const ONE_KB = 1024;
  if (mode === "raw") return `${rawValue} B`;
  console.assert(mode === "readable");
  if (rawValue >= ONE_GB) return `${(rawValue / ONE_GB).toFixed(2)} GB`;
  if (rawValue >= ONE_MB) return `${(rawValue / ONE_MB).toFixed(2)} MB`;
  if (rawValue >= ONE_KB) return `${(rawValue / ONE_KB).toFixed(2)} KB`;
  return `${rawValue} B`;
}

export const fmtSize = (v) =>
  v === null ? "—" : v.toLocaleString(undefined, { maximumFractionDigits: 0 });

export function unwrapCount(v) {
  if (typeof v === "number") return v;
  if (v && typeof v === "object" && "Count" in v) return v.Count;
  return 0;
}

export function unwrapBytes(v) {
  if (typeof v === "number") return v;
  if (v && typeof v === "object" && "Bytes" in v) return v.Bytes;
  return 0;
}
