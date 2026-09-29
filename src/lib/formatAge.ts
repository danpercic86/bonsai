// P125-ui §2 / §8.4 — relative-time words for epoch-MILLISECOND stamps.
//
// New rather than shared with `RepoHealthPanel`'s file-local `relativeTime`,
// which works in epoch seconds. Words are always spelled out (`3 hours ago`,
// never `3h`) so the visible text and an accessible name are the same string
// (ui-reference §11 "spell measurements out").

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

function unit(n: number, word: string): string {
  return `${n} ${word}${n === 1 ? '' : 's'}`;
}

/** `just now` (< 1 min) · `{m} minutes ago` · `{h} hours ago` (< 48 h) · `{d} days ago`. */
export function formatAge(atMs: number, nowMs: number): string {
  const diff = Math.max(0, nowMs - atMs);
  if (diff < MINUTE) return 'just now';
  if (diff < HOUR) return `${unit(Math.floor(diff / MINUTE), 'minute')} ago`;
  if (diff < 48 * HOUR) return `${unit(Math.floor(diff / HOUR), 'hour')} ago`;
  return `${unit(Math.floor(diff / DAY), 'day')} ago`;
}

/** `less than a minute` · `{m} minutes` · `{h} hours` — the time until `atMs`. */
export function formatIn(atMs: number, nowMs: number): string {
  const diff = Math.max(0, atMs - nowMs);
  if (diff < MINUTE) return 'less than a minute';
  if (diff < HOUR) return unit(Math.floor(diff / MINUTE), 'minute');
  return unit(Math.floor(diff / HOUR), 'hour');
}
