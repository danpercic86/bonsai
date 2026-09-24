import { beforeEach, describe, expect, it } from 'vitest';
import { countIpcCalls, countReactCommit, perfCounterSink, perfSnapshot, resetPerfCounters } from './perfCounters';

beforeEach(() => resetPerfCounters());

describe('perf counters (window.__bonsai.perf)', () => {
  it('counts IPC calls per command, preserving `this` and return values', async () => {
    const api = countIpcCalls({
      base: 2,
      double(this: { base: number }, n: number): number {
        return n * this.base;
      },
      async getStatus(): Promise<string> {
        return 'ok';
      },
    });
    expect(api.double(3)).toBe(6);
    expect(api.double(4)).toBe(8);
    await expect(api.getStatus()).resolves.toBe('ok');
    expect(api.base).toBe(2);
    expect(perfSnapshot()).toMatchObject({ ipcCalls: 3, ipcByCmd: { double: 2, getStatus: 1 } });
  });

  it('the sink counts only unprompted shifts and every focus loss; reset zeroes all', () => {
    const base = { type: 'shift', region: 'graph', value: 0.1, at: 1 } as const;
    perfCounterSink({ ...base, phase: 'interactive', hadRecentInput: false, resizing: false });
    perfCounterSink({ ...base, phase: 'interactive', hadRecentInput: true, resizing: false });
    perfCounterSink({ ...base, phase: 'boot', hadRecentInput: false, resizing: false });
    perfCounterSink({ type: 'focus-lost', fromRegion: 'details', phase: 'boot', via: 'detached', at: 1 });
    countReactCommit();
    expect(perfSnapshot()).toEqual({
      reactCommits: 1,
      ipcCalls: 0,
      ipcByCmd: {},
      layoutShiftsUnprompted: 1,
      focusLosses: 1,
    });
    resetPerfCounters();
    expect(perfSnapshot()).toEqual({
      reactCommits: 0,
      ipcCalls: 0,
      ipcByCmd: {},
      layoutShiftsUnprompted: 0,
      focusLosses: 0,
    });
  });
});
