/**
 * P120 §3.2 — stability lifecycle. `startStability()` is idempotent and is
 * called once at boot (main.tsx).
 *
 * Part a: in mock mode (`VITE_MOCK_IPC=1`) it registers the lab counter sink
 * (behind `window.__bonsai.perf`) and the `?perfInject*` triggers. It registers
 * at MODULE level rather than from a React effect: StrictMode's
 * mount→unmount→mount would otherwise take the sink count 1→0→1 and the second
 * attach would lose the buffered boot replay (A7). Part b adds the dev-mode sink
 * here, gated on `onObsConfigChange`. Outside mock mode part a registers
 * nothing, so no observer or listener is attached.
 */
import { installPerfInject } from './mockInject';
import { perfCounterSink } from './perfCounters';
import { addStabilitySink } from './sinks';

const MOCK_MODE = import.meta.env.VITE_MOCK_IPC === '1';

let stop: (() => void) | null = null;

export function startStability(): void {
  if (stop !== null) return;
  const parts: Array<() => void> = [];
  if (MOCK_MODE) {
    parts.push(addStabilitySink(perfCounterSink));
    parts.push(installPerfInject(window.location.search));
  }
  stop = () => {
    for (const p of parts.reverse()) p();
  };
}

/** Inverse of `startStability()` (tests / teardown). */
export function stopStability(): void {
  stop?.();
  stop = null;
}
