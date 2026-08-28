/**
 * P91 §12 gate row 5 — headless END-TO-END proof of the frontend obs pipeline.
 *
 * No browser: the `dom` vitest project runs with VITE_MOCK_IPC=1, so `ipc` is the
 * production-instrumented `mockIpc` and `attachSink` targets the SAME in-memory
 * ring the real sink would. This drives the WHOLE chain — activation
 * (`configureObs`), capture (the `instrumentIpc` proxy over real mock invokes),
 * flush (the batcher → `mockIpc.logAppend` → ring), and the harness-only anomaly
 * analyzer — and asserts against `window.__bonsaiDumpLogs()` exactly as the
 * scripted browser gate would, giving the orchestrator Bash-runnable evidence.
 *
 * dup-ipc is driven with REAL instrumented invokes (proving activation+capture).
 * The slow pair is SEEDED via `__bonsaiSeedSpans()` (a >10 s / calibrated latency
 * cannot be produced by a real invoke in jsdom) — the intended fixture path,
 * mirroring `obsAnomaly.test.ts`.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ipc } from '../ipc';
import { mockIpc } from '../ipc/mock';
import { configureObs, obsEnabled, resetObsConfigForTests } from './enabled';
import {
  attachSink,
  flushNow,
  pendingRestart,
  resetBatcherForTests,
} from './batcher';
import { resetIpcProxyForTests } from './ipcProxy';
import { clearSessionSalt, redactionReady } from './redact';
import type { DevSettings } from '../ipc/types/settings';
import type { LogRecord } from './types';

const DEV_ON: DevSettings = {
  enabled: true,
  level: 'debug',
  captureIpc: true,
  captureReact: true,
  captureFrames: false,
  includeRawNames: false,
};

function dump(): LogRecord[] {
  const text = window.__bonsaiDumpLogs?.() ?? '';
  if (text === '') return [];
  return text.split('\n').map((l) => JSON.parse(l) as LogRecord);
}

const bySeq = (recs: LogRecord[], seq: number) => recs.find((r) => r.seq === seq);

beforeEach(async () => {
  resetObsConfigForTests();
  resetBatcherForTests();
  resetIpcProxyForTests();
  clearSessionSalt();
  window.__bonsaiClearLogs?.();
  // Re-establish the ring sink that production attaches at module load (the
  // batcher reset above dropped it), with the RAW mock api — never the proxy.
  attachSink(mockIpc);
  // Mock persistence Dev-mode ON, so `logAppend` actually writes to the ring and
  // `logSessionInfo` returns a real salt (both gate on it, mirroring Rust).
  await mockIpc.setUiSettings({ dev: DEV_ON });
});

afterEach(() => {
  vi.useRealTimers();
  resetObsConfigForTests();
  resetBatcherForTests();
  clearSessionSalt();
  window.__bonsaiClearLogs?.();
});

describe('obs pipeline drives a real dup-ipc anomaly (increment 7d, §12 row 5)', () => {
  it('two identical instrumented invokes trip exactly one dup-ipc whose refs resolve', async () => {
    // Activate via the real wire; the batcher re-salts on enable.
    configureObs(DEV_ON);
    await pendingRestart();
    expect(obsEnabled()).toBe(true);
    expect(redactionReady()).toBe(true);

    // Two IDENTICAL calls (same cmd, no args ⇒ same argsHash) within 300 ms, no
    // intervening mutation — the dup-ipc trigger, through the REAL proxy.
    await ipc.getUiSettings();
    await ipc.getUiSettings();
    await flushNow();

    const recs = dump();
    const dups = recs.filter((r) => r.kind === 'anomaly' && r.rule === 'dup-ipc');
    expect(dups).toHaveLength(1);

    const refs = (dups[0].refs ?? []) as number[];
    expect(refs).toHaveLength(2);
    const [a, b] = refs.map((s) => bySeq(recs, s));
    expect(a?.kind).toBe('ipc.call');
    expect(b?.kind).toBe('ipc.call');
    expect(a?.cmd).toBe('getUiSettings');
    expect(b?.cmd).toBe('getUiSettings');
    // Same digest is what makes them a duplicate pair.
    expect(a?.argsHash).toBe(b?.argsHash);
  });
});

describe('obs pipeline surfaces the slow pair (increment 7d, §12 row 5)', () => {
  it('seeding the slow fixture yields slow-command + slow-phase in one dump, refs resolving', () => {
    // The slow fixtures are seeded straight into the ring (a >10 s latency is not
    // producible by a real invoke); analysis runs at dump time regardless of enable.
    expect(window.__bonsaiSeedSpans?.()).toBeGreaterThan(0);

    const recs = dump();
    const slowCmd = recs.filter((r) => r.kind === 'anomaly' && r.rule === 'slow-command');
    const slowPhase = recs.filter((r) => r.kind === 'anomaly' && r.rule === 'slow-phase');
    expect(slowCmd).toHaveLength(1);
    expect(slowPhase).toHaveLength(1);

    // slow-phase refs = [span seq, result seq]; both resolve to real ring records.
    const refs = (slowPhase[0].refs ?? []) as number[];
    expect(refs).toHaveLength(2);
    const span = bySeq(recs, refs[0]);
    const result = bySeq(recs, refs[1]);
    expect(span?.kind).toBe('span');
    expect(result?.kind).toBe('ipc.result');
    // The slow-command references that same result — same dump, one coherent story.
    const cmdRefs = (slowCmd[0].refs ?? []) as number[];
    expect(cmdRefs[0]).toBe(result?.seq);
  });
});
