/** P121 — shared obs plumbing for render-count tests: attach an in-memory sink,
 *  enable React capture at trace level, and sum `render.tally` renders per
 *  component. Mirrors the setup in `Sidebar.churn.test.tsx`. Call
 *  `startRenderTally()` in `beforeEach` and `stopRenderTally()` in `afterEach`. */
import { configureObs, resetObsConfigForTests } from '../obs/enabled';
import { attachSink, flushNow, resetBatcherForTests } from '../obs/batcher';
import { clearSessionSalt, setSessionSalt } from '../obs/redact';
import { __resetRenderTally, flushRenderTally } from '../obs/renderTally';
import type { LogRecord } from '../obs/types';

const SALT = '00112233445566778899aabbccddeeff';
let sunk: LogRecord[] = [];

export function startRenderTally(): void {
  sunk = [];
  resetObsConfigForTests();
  resetBatcherForTests();
  __resetRenderTally();
  attachSink({
    async logAppend(records) {
      sunk.push(...records);
    },
    async logSessionInfo() {
      return { salt: SALT };
    },
  });
  setSessionSalt(SALT);
  configureObs({
    enabled: true,
    level: 'trace',
    captureIpc: true,
    captureReact: true,
    captureFrames: false,
    includeRawNames: false,
  });
}

export function stopRenderTally(): void {
  resetObsConfigForTests();
  resetBatcherForTests();
  __resetRenderTally();
  clearSessionSalt();
}

/** Flush the open tally window into the sink. */
export async function drainRenderTally(): Promise<void> {
  flushRenderTally();
  await flushNow();
}

/** Drop everything counted so far (e.g. mount churn). */
export function resetRenderTally(): void {
  sunk = [];
  __resetRenderTally();
}

/** Sum of `renders` across the drained `render.tally` records of `component`. */
export function tallyRenders(component: string): number {
  return sunk
    .filter((r) => r.kind === 'render.tally' && r.component === component)
    .reduce((n, r) => n + (r.renders as number), 0);
}
