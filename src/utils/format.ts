// Shared human formatting helpers (P29c: extracted from CloneDialog and
// extended with GiB per the repo-health contract §8).

/** Human-readable byte size: B / KiB / MiB / GiB, one decimal above bytes. */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const kib = bytes / 1024;
  if (kib < 1024) return `${kib.toFixed(1)} KiB`;
  const mib = kib / 1024;
  if (mib < 1024) return `${mib.toFixed(1)} MiB`;
  return `${(mib / 1024).toFixed(1)} GiB`;
}
