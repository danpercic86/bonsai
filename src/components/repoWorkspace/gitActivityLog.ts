/**
 * P87b — the git-activity LOG line: its shape, its cap, and the batched append.
 *
 * Split out of `useGitActivity.ts` (file-size discipline, mirrors `aiRunLog.ts`).
 * Wire-format knowledge (the 2000-char per-line cap mirrored from Rust) lives on
 * the ingest side of the store, never in a presentational component.
 */

/** One captured hook/CLI output line, classified by stream at ingest. */
export interface GitActivityLine {
  /** The event's own `seq` — monotonic per run, so it is a stable React key. */
  seq: number;
  stream: 'stdout' | 'stderr';
  text: string;
}

/**
 * MIRRORS `bonsai_core::git::activity::MAX_ACTIVITY_LINE_CHARS`. Rust truncates
 * every output line to EXACTLY this many chars and appends `…`, so a line of
 * exactly this length is the signal that something was cut off (the row's
 * "truncated" chip).
 */
export const GIT_ACTIVITY_LINE_MAX_CHARS = 2000;

/** D5: how long log lines / progress sit in the ref buffer before ONE commit. */
export const GIT_ACTIVITY_FLUSH_MS = 50;

/** Retained output lines per run; the oldest are dropped and counted. */
export const GIT_ACTIVITY_LINES_MAX = 500;

/**
 * Append lines to a capped log, reporting how many were dropped off the front.
 *
 * Returns a NEW array (the store is immutable per entry so React sees a change)
 * and the drop count to add to `linesDropped`. Called once per 50 ms flush.
 */
export function appendCappedLines(
  log: GitActivityLine[],
  incoming: GitActivityLine[],
): { log: GitActivityLine[]; dropped: number } {
  if (incoming.length === 0) return { log, dropped: 0 };
  const merged = log.concat(incoming);
  if (merged.length <= GIT_ACTIVITY_LINES_MAX) return { log: merged, dropped: 0 };
  const dropped = merged.length - GIT_ACTIVITY_LINES_MAX;
  return { log: merged.slice(dropped), dropped };
}
