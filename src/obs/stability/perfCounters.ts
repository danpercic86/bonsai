/**
 * P120 part a — mock-mode lab counters behind `window.__bonsai.perf`. Side-effect
 * free: nothing here runs unless the mock wiring (install.ts, mock.ts, the
 * `<MockProfiler>`) calls it, so it is inert in production.
 */
import { isUnpromptedShift } from './classify';
import type { StabilitySink } from './types';

/** Counts since the last `resetPerfCounters()`. */
export interface PerfSnapshot {
  reactCommits: number;
  ipcCalls: number;
  ipcByCmd: Record<string, number>;
  layoutShiftsUnprompted: number;
  focusLosses: number;
}

let reactCommits = 0;
let ipcCalls = 0;
let ipcByCmd = new Map<string, number>();
let layoutShiftsUnprompted = 0;
let focusLosses = 0;

export function resetPerfCounters(): void {
  reactCommits = 0;
  ipcCalls = 0;
  ipcByCmd = new Map();
  layoutShiftsUnprompted = 0;
  focusLosses = 0;
}

export function perfSnapshot(): PerfSnapshot {
  return {
    reactCommits,
    ipcCalls,
    ipcByCmd: Object.fromEntries(ipcByCmd),
    layoutShiftsUnprompted,
    focusLosses,
  };
}

/** `<Profiler onRender>` target — one bump per React commit. */
export function countReactCommit(): void {
  reactCommits += 1;
}

export function countIpcCall(cmd: string): void {
  ipcCalls += 1;
  ipcByCmd.set(cmd, (ipcByCmd.get(cmd) ?? 0) + 1);
}

/** The counter sink (registered by install.ts in mock mode). */
export const perfCounterSink: StabilitySink = (e) => {
  if (e.type === 'focus-lost') focusLosses += 1;
  else if (isUnpromptedShift(e)) layoutShiftsUnprompted += 1;
};

/** Wrap every method of an IPC object so each call bumps its per-command count.
 *  Mock only — mock.ts wraps `mockIpc` so `instrumentIpc` still sees plain
 *  functions. `this` is preserved as the original object. */
export function countIpcCalls<T extends object>(api: T): T {
  const out: Record<string, unknown> = {};
  for (const [name, value] of Object.entries(api)) {
    if (typeof value !== 'function') {
      out[name] = value;
      continue;
    }
    const fn = value as (...args: unknown[]) => unknown;
    out[name] = (...args: unknown[]): unknown => {
      countIpcCall(name);
      return fn.apply(api, args);
    };
  }
  return out as T;
}
