/**
 * P120 §3.2 — stability lifecycle; `startStability()` is idempotent, called once
 * from main.tsx at MODULE level (a React effect would take the sink count
 * 1→0→1 under StrictMode and lose the buffered boot replay, A7).
 *
 * Mock mode: the lab counter sink + `?perfInject*` triggers (part a). Both
 * modes: the dev sink (`devSink.ts`) is registered iff `obsEnabled()`, tracked
 * via `onObsConfigChange`; its buckets are flushed before it unregisters and on
 * `pagehide`, a listener that exists only while it is registered (§1, AC 12).
 */
import { obsEnabled, onObsConfigChange } from '../enabled';
import { createDevSink } from './devSink';
import type { DevSink } from './devSink';
import { installPerfInject } from './mockInject';
import { perfCounterSink } from './perfCounters';
import { addStabilitySink } from './sinks';

const MOCK_MODE = import.meta.env.VITE_MOCK_IPC === '1';

let stop: (() => void) | null = null;
let dev: DevSink | null = null;
let unregisterDev: (() => void) | null = null;

function onPageHide(): void {
  dev?.flushAll();
}

function syncDevSink(): void {
  if (obsEnabled() && unregisterDev === null) {
    dev ??= createDevSink();
    const removeSink = addStabilitySink(dev.sink);
    window.addEventListener('pagehide', onPageHide);
    unregisterDev = () => {
      window.removeEventListener('pagehide', onPageHide);
      dev?.flushAll();
      removeSink();
    };
  } else if (!obsEnabled() && unregisterDev !== null) {
    unregisterDev();
    unregisterDev = null;
  }
}

export function startStability(): void {
  if (stop !== null) return;
  const parts: Array<() => void> = [];
  if (MOCK_MODE) {
    parts.push(addStabilitySink(perfCounterSink));
    parts.push(installPerfInject(window.location.search));
  }
  parts.push(onObsConfigChange(syncDevSink));
  syncDevSink();
  parts.push(() => {
    unregisterDev?.();
    unregisterDev = null;
  });
  stop = () => {
    for (const p of parts.reverse()) p();
  };
}

/** Inverse of `startStability()` (tests / teardown). */
export function stopStability(): void {
  stop?.();
  stop = null;
}
