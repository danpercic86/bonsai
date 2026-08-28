/**
 * P91 §12 row 4 — refresh + echo causality (obs §2.4/§2.5).
 *
 * These exercise the REAL async handler flow (await IPC → threaded refresh), not
 * a synchronous fixture: the trace is captured at the handler's synchronous entry
 * (under the `withTrace` ambient) and threaded by value into `refreshAll`, which
 * survives the post-await continuation. The negative control proves an UNBOUND
 * post-await refresh logs `causedBy: undefined` — never a stale/wrong trace.
 */
import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { configureObs, resetObsConfigForTests } from '../../obs/enabled';
import { attachSink, flushNow, resetBatcherForTests } from '../../obs/batcher';
import { clearSessionSalt, setSessionSalt } from '../../obs/redact';
import { currentTrace, resetTraceForTests, withTrace } from '../../obs/trace';
import { __resetEchoSuppression } from './echoSuppression';
import { useCoalescedRefresh } from './useCoalescedRefresh';
import type { DevSettings } from '../../ipc/types/settings';
import type { LogRecord } from '../../obs/types';

const DEV_ON: DevSettings = {
  enabled: true,
  level: 'debug',
  captureIpc: true,
  captureReact: true,
  captureFrames: false,
  includeRawNames: false,
};

let sunk: LogRecord[] = [];

beforeEach(() => {
  sunk = [];
  resetObsConfigForTests();
  resetBatcherForTests();
  resetTraceForTests();
  __resetEchoSuppression();
  clearSessionSalt();
  attachSink({
    async logAppend(records) {
      sunk.push(...records);
    },
    async logSessionInfo() {
      return { salt: '00112233445566778899aabbccddeeff' };
    },
  });
  setSessionSalt('00112233445566778899aabbccddeeff');
  configureObs(DEV_ON);
});

afterEach(() => {
  vi.useRealTimers();
  resetObsConfigForTests();
  resetBatcherForTests();
  clearSessionSalt();
});

const byKind = (k: string) => sunk.filter((r) => r.kind === k);

/** A realistic mutation handler: capture the trace at the synchronous entry, await
 *  an IPC round-trip, THEN refresh with the threaded trace. */
async function mutate(
  refresh: (o: 'mutation', s: 'full', t?: string) => Promise<void>,
): Promise<void> {
  // The ambient IS present during the SYNCHRONOUS extent of withTrace, so the
  // handler captures its trace there and threads it by value past the await.
  const captured = withTrace('click', 'sidebar.branch.checkout', () => currentTrace()?.trace);
  await Promise.resolve(); // stand-in for `await ipc.mutate(...)`
  await refresh('mutation', 'full', captured);
}

describe('acceptance (a) — fs echo attributed to its mutation', () => {
  it('a dropped watcher refresh carries suppressed + causedBy = mutation trace', async () => {
    const { result } = renderHook(() => useCoalescedRefresh('repo-1', async () => undefined));
    await act(async () => {
      await mutate(result.current.refresh);
    });
    // The echo lands while the window is armed (mutation still settling / tail).
    await act(async () => {
      await result.current.refresh('watcher', 'full');
    });
    await flushNow();

    const gesture = byKind('gesture').find((r) => r.gesture === 'sidebar.branch.checkout');
    expect(gesture?.trace).toBeTruthy();
    const suppressed = byKind('watcher').find((r) => r.suppressed === true);
    expect(suppressed).toBeTruthy();
    expect(suppressed?.suppressReason).toBe('echo');
    expect(suppressed?.causedBy).toBe(gesture?.trace);
    expect(suppressed?.fired).toBe(false);
    // (b) half — the one mutation produced EXACTLY one refresh round (its echo
    // was suppressed, so it added no second round).
    expect(byKind('refresh')).toHaveLength(1);
    expect(byKind('refresh')[0]?.contributingTraces).toContain(gesture?.trace);
  });
});

describe('acceptance (b) — one refresh record lists every collapsed trace', () => {
  it('mutations that collapse into a single trailing round list every trace', async () => {
    // Gate the LEADING round so two later mutations arrive while it is in flight
    // and coalesce into ONE trailing round — the real double-trigger collapse.
    let release: () => void = () => undefined;
    const gate = new Promise<void>((r) => {
      release = r;
    });
    let runs = 0;
    const run = async (): Promise<void> => {
      runs += 1;
      if (runs === 1) await gate;
    };
    const { result } = renderHook(() => useCoalescedRefresh('repo-1', run));

    const traceA = withTrace('click', 'commit.commit.submit', () => currentTrace()?.trace);
    const pA = result.current.refresh('mutation', 'full', traceA); // leading, blocks on gate
    // B + C arrive during the leading round → both collapse into the trailing round.
    const traceB = withTrace('click', 'commit.commit.submit', () => currentTrace()?.trace);
    const pB = result.current.refresh('mutation', 'full', traceB);
    const traceC = withTrace('click', 'commit.commit.submit', () => currentTrace()?.trace);
    const pC = result.current.refresh('mutation', 'full', traceC);

    await act(async () => {
      release();
      await Promise.all([pA, pB, pC]);
    });
    await flushNow();

    const refreshes = byKind('refresh');
    const collapsed = refreshes.find((r) => (r.contributingTraces as string[]).length >= 2);
    expect(collapsed).toBeTruthy();
    expect(collapsed?.contributingTraces).toContain(traceB);
    expect(collapsed?.contributingTraces).toContain(traceC);
    expect(collapsed?.collapsed as number).toBeGreaterThanOrEqual(1);
  });
});

describe('negative control — never a stale/wrong trace', () => {
  it('an UNBOUND post-await refresh logs causedBy: undefined, not a leftover trace', async () => {
    const { result } = renderHook(() => useCoalescedRefresh('repo-2', async () => undefined));
    // Offender shape: read the ambient AFTER the await (it is already cleared), so
    // no trace is threaded. A stale-trace implementation would wrongly attribute.
    await act(async () => {
      await withTrace('click', 'sidebar.branch.delete', async () => undefined);
      await Promise.resolve();
      // No trace threaded — mutation arms with undefined.
      await result.current.refresh('mutation', 'full');
    });
    await act(async () => {
      await result.current.refresh('watcher', 'full');
    });
    await flushNow();
    const suppressed = byKind('watcher').find((r) => r.suppressed === true);
    expect(suppressed).toBeTruthy();
    expect(suppressed?.causedBy).toBeUndefined();
  });
});
