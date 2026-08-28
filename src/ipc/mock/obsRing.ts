/**
 * P91 §6 "Mock mode" — the browser harness's in-memory log sink.
 *
 * The Rust sink writes JSONL to disk; in the harness there is no disk, so the
 * same records land in a bounded ring buffer and `window.__bonsaiDumpLogs()`
 * returns them as the SAME JSONL text. That is what lets the harness assert
 * schema and (from increment 5) anomalies with no Tauri at all.
 *
 * Its own module, not part of `handlers/obs.ts`: the ring is state that the
 * fixtures, the handlers and the harness console all read, and it must not grow
 * into the handler file.
 */
import type { LogRecord } from '../types';
import { analyzeAnomalies } from './obsAnomaly';
import { slowCommandFixtures, spanFixtures } from './obs';

/** Mirrors the Rust `sync_channel(4096)` bound (§6): oldest is dropped first. */
export const MOCK_RING_CAPACITY = 4096;

const ring: LogRecord[] = [];
let seq = 0;
let dropped = 0;

/** Appends records, assigning `seq` in arrival order exactly as the Rust writer
 *  thread does — the harness must see the same authoritative ordering. */
export function ringAppend(records: LogRecord[]): void {
  for (const r of records) {
    seq += 1;
    ring.push({ ...r, seq });
    if (ring.length > MOCK_RING_CAPACITY) {
      ring.shift();
      dropped += 1;
    }
  }
}

export function ringRecords(): readonly LogRecord[] {
  return ring;
}

export function ringStats(): { records: number; dropped: number } {
  return { records: seq, dropped };
}

export function ringClear(): void {
  ring.length = 0;
  seq = 0;
  dropped = 0;
}

/** The `anomaly` records the harness-only batch analyzer derives from the current
 *  ring (increment 5b). Computed on demand; the ring itself is never mutated, so
 *  repeated dumps are stable and `refs` still resolve to the real record seqs. */
export function ringAnomalies(): LogRecord[] {
  return analyzeAnomalies(ring);
}

/** The JSONL text an exported file would contain, with the derived `anomaly`
 *  records appended so the browser gate can assert anomalies with no Tauri. */
export function ringDump(): string {
  return [...ring, ...ringAnomalies()].map((r) => JSON.stringify(r)).join('\n');
}

declare global {
  interface Window {
    __bonsaiDumpLogs?: () => string;
    __bonsaiClearLogs?: () => void;
    __bonsaiLogStats?: () => { records: number; dropped: number; buffered: number };
    /** §4/§5.1 — seed synthesised `span` records (incl. one slow `graph.get`)
     *  so the harness can exercise the performance rules with no Tauri. */
    __bonsaiSeedSpans?: () => number;
  }
}

/**
 * Installed once by the mock IPC module so the harness can read the buffer.
 *
 * P91 increment 2 adds the two companions the scripted-scenario gate needs: a
 * clear (so a scenario starts from an empty ring instead of one polluted by
 * boot-time IPC) and a stats read (so an assertion can prove records arrived
 * before it asserts on their content — an empty ring otherwise makes every
 * "no raw path appears" check pass vacuously).
 */
export function installLogDump(): void {
  if (typeof window === 'undefined') return;
  window.__bonsaiDumpLogs = ringDump;
  window.__bonsaiClearLogs = ringClear;
  window.__bonsaiLogStats = () => ({ ...ringStats(), buffered: ring.length });
  window.__bonsaiSeedSpans = () => {
    // Spans FIRST so the slow `graph.get` span precedes its `ipc.result` in seq
    // order (the analyzer's slow-phase correlation needs the span already seen).
    const spans = spanFixtures();
    const results = slowCommandFixtures();
    ringAppend(spans);
    ringAppend(results);
    return spans.length + results.length;
  };
}
