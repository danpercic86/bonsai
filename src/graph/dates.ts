/** Pure date/oid formatting helpers for the graph draw + hover layers (P51b).
 *  No canvas, no React — unit-tested directly. */

/** Relative date: "now", "5m", "3h", "4d", "2mo", "1y". Pure, unit-testable.
 *  Moved here from draw.ts (P51b); draw.ts re-exports it so the many existing
 *  `import { relativeDate } from '../graph/draw'` call sites keep working. */
export function relativeDate(ts: number, now: number): string {
  const s = Math.max(0, now - ts);
  if (s < 60) return 'now';
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h`;
  const d = Math.floor(h / 24);
  if (d < 30) return `${d}d`;
  const mo = Math.floor(d / 30);
  if (mo < 12) return `${mo}mo`;
  return `${Math.max(1, Math.floor(d / 365))}y`;
}

/** First `len` (default 7) hex chars of a full 40-char oid — the short-SHA
 *  column text. Pure slice; short oids/empty strings pass through unchanged. */
export function shortSha(id: string, len = 7): string {
  return id.slice(0, len);
}

/** Absolute LOCAL timestamp "YYYY-MM-DD HH:mm" for the date hover tooltip
 *  (P51b §5). A fixed format (NOT `toLocaleString`) so it is deterministic and
 *  unit-testable; it reads local time so it matches the user's clock — the unit
 *  test builds its expected value from the same local `Date`, so the timezone
 *  cancels and the assertion is machine-independent. */
export function formatAbsolute(tsSeconds: number): string {
  const d = new Date(tsSeconds * 1000);
  const p = (n: number): string => String(n).padStart(2, '0');
  return (
    `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ` +
    `${p(d.getHours())}:${p(d.getMinutes())}`
  );
}
