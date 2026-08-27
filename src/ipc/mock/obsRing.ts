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

/** The JSONL text an exported file would contain. */
export function ringDump(): string {
  return ring.map((r) => JSON.stringify(r)).join('\n');
}

declare global {
  interface Window {
    __bonsaiDumpLogs?: () => string;
  }
}

/** Installed once by the mock IPC module so the harness can read the buffer. */
export function installLogDump(): void {
  if (typeof window === 'undefined') return;
  window.__bonsaiDumpLogs = ringDump;
}
