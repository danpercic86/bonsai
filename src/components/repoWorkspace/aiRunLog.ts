/**
 * P68d §C — the AI run LOG line: its shape, its cap, and the one place its kind is
 * decided.
 *
 * Split out of `useAiRuns.ts` so the classification is a pure function the dock
 * (P68e) imports instead of re-sniffing `⚙ ` / `stderr: ` prefixes at render time.
 * Wire-format knowledge belongs on the ingest side of the store, never in a
 * presentational component (P68e §12-A1).
 */

/** Drives the dock's per-line colour; the glyph/prefix carries the meaning. */
export type AiRunLogKind = 'text' | 'tool' | 'stderr' | 'meta';

export interface AiRunLogLine {
  /** The event's own `seq` — monotonic per run, so it is a stable React key. */
  seq: number;
  text: string;
  /** Classified ONCE, at ingest (P68e §12-A1). */
  kind: AiRunLogKind;
}

/**
 * MIRRORS `bonsai_core::ai::stream::MAX_EVENT_TEXT`. Rust truncates every event's
 * text to EXACTLY this many chars and appends `…`, so a line of exactly this length
 * is the signal that something was cut off (the dock's "truncated" chip).
 */
export const AI_EVENT_TEXT_MAX = 2000;

/** D5: how long log lines sit in the ref buffer before ONE state commit. */
export const AI_LOG_FLUSH_MS = 50;

/** D5: retained lines per run; the oldest are dropped and counted in `logDropped`. */
export const AI_LOG_MAX = 500;

/**
 * The prefixes Rust's mapping table (§3.2) puts in front of non-prose lines. Kept
 * here, next to the classifier, because they are the only coupling between the wire
 * format and the UI.
 */
const META_PREFIXES = ['» ', 'session ', 'batch ', 'rate limit: ', 'summary: ', 'system/'];

/**
 * Classify one log line for display (PURE). Total: anything unrecognised is `text`,
 * which is the safe default — a mis-coloured line is a cosmetic bug, a thrown
 * classifier would break the log.
 */
export function classifyLogLine(text: string): AiRunLogKind {
  if (text.startsWith('⚙ ')) return 'tool';
  if (text.startsWith('stderr: ')) return 'stderr';
  if (META_PREFIXES.some((p) => text.startsWith(p))) return 'meta';
  return 'text';
}

/**
 * Append lines to a capped log, reporting how many were dropped off the front.
 *
 * Returns a NEW array (the store is immutable per entry so React sees a change) and
 * the drop count to add to `logDropped`. Called once per 50 ms flush, not per line.
 */
export function appendCapped(
  log: AiRunLogLine[],
  incoming: AiRunLogLine[],
): { log: AiRunLogLine[]; dropped: number } {
  if (incoming.length === 0) return { log, dropped: 0 };
  const merged = log.concat(incoming);
  if (merged.length <= AI_LOG_MAX) return { log: merged, dropped: 0 };
  const dropped = merged.length - AI_LOG_MAX;
  return { log: merged.slice(dropped), dropped };
}
