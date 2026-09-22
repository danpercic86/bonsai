/**
 * P117 §2.2 / AC2-12 — the repo dimension on the `refresh` record, asserted at
 * the WIRE BOUNDARY.
 *
 * The assertion is deliberately made on what reaches `logAppend`, not on a
 * record the test builds itself: a locally-constructed record would pass even if
 * the UI redacted `repo` on its way out. §2.2 point 3 makes "no UI-side
 * redaction of `repo`, on any path" normative, because a `ui:path#3` here would
 * never equal the Rust `graph.get` span's raw `repoId` — so `cache-collapse`
 * would fire immediately after a real mutation, inverting the whole suppression
 * argument. The repoId used here is path-shaped AND contains a space, which is
 * exactly what a UI redactor would mangle.
 */
import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { configureObs, resetObsConfigForTests } from '../../obs/enabled';
import { attachSink, flushNow, resetBatcherForTests } from '../../obs/batcher';
import { clearSessionSalt, setSessionSalt } from '../../obs/redact';
import { resetTraceForTests } from '../../obs/trace';
import { __resetEchoSuppression } from './echoSuppression';
import { useCoalescedRefresh } from './useCoalescedRefresh';
import type { DevSettings } from '../../ipc/types/settings';
import type { LogRecord } from '../../obs/types';

const SALT = '00112233445566778899aabbccddeeff';
const REPO = 'D:\\Repos\\my project';
const OTHER = 'D:\\Repos\\bonsai';

const DEV_ON: DevSettings = {
  enabled: true,
  level: 'debug',
  captureIpc: true,
  captureReact: true,
  captureFrames: false,
  includeRawNames: false,
};

let sunk: LogRecord[] = [];
let appended: LogRecord[][] = [];

beforeEach(() => {
  sunk = [];
  appended = [];
  resetObsConfigForTests();
  resetBatcherForTests();
  resetTraceForTests();
  __resetEchoSuppression();
  clearSessionSalt();
  attachSink({
    async logAppend(records) {
      // The spy is the boundary: this is the array the Rust sink receives.
      appended.push(records);
      sunk.push(...records);
    },
    async logSessionInfo() {
      return { salt: SALT };
    },
  });
  setSessionSalt(SALT);
  configureObs(DEV_ON);
});

afterEach(() => {
  vi.useRealTimers();
  resetObsConfigForTests();
  resetBatcherForTests();
  clearSessionSalt();
});

const refreshRecords = (): LogRecord[] => sunk.filter((r) => r.kind === 'refresh');

it('a refresh round reaches logAppend carrying the RAW repoId', async () => {
  const { result } = renderHook(() => useCoalescedRefresh(REPO, async () => undefined));
  await act(async () => {
    await result.current.refresh('manual', 'full');
  });
  await flushNow();

  expect(appended.length).toBeGreaterThan(0);
  const rounds = refreshRecords();
  expect(rounds).toHaveLength(1);
  // Byte-identical to the hook's prop: not `ui:path#N`, not masked, not absent.
  expect(rounds[0].repo).toBe(REPO);
  expect(rounds[0].scope).toBe('full');
});

it('never emits a redacted or masked form of the repoId', async () => {
  const { result } = renderHook(() => useCoalescedRefresh(REPO, async () => undefined));
  await act(async () => {
    await result.current.refresh('manual', 'full');
  });
  await flushNow();

  const wire = JSON.stringify(appended);
  expect(wire).not.toContain('ui:path#');
  expect(wire).not.toContain('<home>');
  expect(wire).not.toContain('repo#');
});

it('attributes a round to the CURRENT repo after a repo switch', async () => {
  const { result, rerender } = renderHook(
    ({ repoId }: { repoId: string }) => useCoalescedRefresh(repoId, async () => undefined),
    { initialProps: { repoId: REPO } },
  );
  await act(async () => {
    await result.current.refresh('manual', 'full');
  });
  rerender({ repoId: OTHER });
  await act(async () => {
    await result.current.refresh('manual', 'full');
  });
  await flushNow();

  // The coalescer closure is built once, so a captured `repoId` would attribute
  // BOTH rounds to the repo that was open at mount.
  expect(refreshRecords().map((r) => r.repo)).toEqual([REPO, OTHER]);
});
