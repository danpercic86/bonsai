/**
 * P91 §9.3 — `frame` records must name the dimension they measured.
 *
 * The paint and gap recorders are separate (§4.7) and each fills the OTHER
 * dimension with `0`. Without the `dim` discriminator a consumer reads
 * `gapMs: 0` on a paint record as "zero gap measured" — a datum nobody
 * recorded. These tests pin the discriminator and the filler convention.
 */
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { attachSink, flushNow, resetBatcherForTests } from '../obs/batcher';
import { configureObs, resetObsConfigForTests } from '../obs/enabled';
import { clearSessionSalt, setSessionSalt } from '../obs/redact';
import { newGapRecorder, newPaintRecorder } from './graphObs';
import type { DevSettings } from '../ipc/types/settings';
import type { LogRecord } from '../obs/types';

const SALT = '00112233445566778899aabbccddeeff';

const DEV: DevSettings = {
  enabled: true,
  level: 'trace',
  captureIpc: true,
  captureReact: true,
  captureFrames: true,
  includeRawNames: false,
};

let sunk: LogRecord[] = [];

beforeEach(() => {
  sunk = [];
  resetObsConfigForTests();
  resetBatcherForTests();
  attachSink({
    async logAppend(records) {
      sunk.push(...records);
    },
    async logSessionInfo() {
      return { salt: SALT };
    },
  });
  setSessionSalt(SALT);
  configureObs(DEV);
});

afterEach(() => {
  vi.useRealTimers();
  resetObsConfigForTests();
  resetBatcherForTests();
  clearSessionSalt();
});

const frames = () => sunk.filter((r) => r.kind === 'frame');

it('a paint window is tagged dim: paint, with gapMs as an explicit filler', async () => {
  const rec = newPaintRecorder();
  rec.record(10);
  rec.record(40);
  rec.flushSummary();
  await flushNow();

  expect(frames()).toHaveLength(1);
  const f = frames()[0];
  expect(f.dim).toBe('paint');
  expect(f.paintMs).toBe(25);
  expect(f.gapMs).toBe(0);
  expect(f.worstMs).toBe(40);
  expect(f.over33).toBe(1);
});

it('a gap window is tagged dim: gap, with paintMs as an explicit filler', async () => {
  const rec = newGapRecorder();
  rec.record(16);
  rec.record(120);
  rec.flushSummary();
  await flushNow();

  expect(frames()).toHaveLength(1);
  const f = frames()[0];
  expect(f.dim).toBe('gap');
  expect(f.gapMs).toBe(68);
  expect(f.paintMs).toBe(0);
  expect(f.over100).toBe(1);
});

it('the two dimensions stay distinguishable in one stream', async () => {
  const paint = newPaintRecorder();
  const gap = newGapRecorder();
  paint.record(8);
  gap.record(8);
  paint.flushSummary();
  gap.flushSummary();
  await flushNow();

  expect(frames().map((f) => f.dim)).toEqual(['paint', 'gap']);
});

it('emits nothing while Dev mode is off (§11 zero-cost)', async () => {
  resetObsConfigForTests();
  const rec = newPaintRecorder();
  rec.record(12);
  rec.flushSummary();
  await flushNow();
  expect(frames()).toHaveLength(0);
});
