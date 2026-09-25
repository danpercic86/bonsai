/**
 * P122c / P122d — the proxy's per-emit `event` records and its `superseded`
 * stamp, pinned against the field session `s229d042a` (six mounted tabs).
 */
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { configureObs, resetObsConfigForTests } from './enabled';
import { flushNow, refreshSalt, resetBatcherForTests } from './batcher';
import { instrumentIpc, resetIpcProxyForTests } from './ipcProxy';
import { clearSessionSalt } from './redact';
import { resetTraceForTests } from './trace';
import type { DevSettings } from '../ipc/types/settings';
import type { LogRecord } from './types';

const DEV_ON: DevSettings = {
  enabled: true,
  level: 'debug',
  captureIpc: true,
  captureReact: false,
  captureFrames: false,
  includeRawNames: false,
};

interface Payload {
  repoId: string;
}

interface FakeApi {
  onRepoChanged(cb: (p: Payload) => void): Promise<() => void>;
  listBranches(repoId: string): Promise<number>;
  logAppend(records: LogRecord[]): Promise<void>;
  logSessionInfo(): Promise<{ salt: string }>;
}

let sunk: LogRecord[] = [];
/** The mock's stand-in for Tauri's emit loop: one payload, every listener, one turn. */
let listeners: ((p: Payload) => void)[] = [];
let pending: ((n: number) => void)[] = [];

function emit(p: Payload): void {
  for (const l of listeners) l(p);
}

function makeApi(): FakeApi {
  return {
    async onRepoChanged(cb) {
      listeners.push(cb);
      return () => undefined;
    },
    listBranches() {
      return new Promise<number>((resolve) => {
        pending.push(resolve);
      });
    },
    async logAppend(records) {
      sunk.push(...records);
    },
    async logSessionInfo() {
      return { salt: '00112233445566778899aabbccddeeff' };
    },
  };
}

async function setup(): Promise<FakeApi> {
  const ipc = instrumentIpc(makeApi());
  configureObs(DEV_ON);
  await refreshSalt();
  return ipc;
}

/** Lets the delivery microtask run, then drains the batcher. */
async function drain(): Promise<void> {
  await Promise.resolve();
  await flushNow();
}

const events = (): LogRecord[] => sunk.filter((r) => r.kind === 'event');
const results = (): LogRecord[] => sunk.filter((r) => r.kind === 'ipc.result');

beforeEach(() => {
  sunk = [];
  listeners = [];
  pending = [];
  resetObsConfigForTests();
  resetBatcherForTests();
  resetIpcProxyForTests();
  resetTraceForTests();
  clearSessionSalt();
});

afterEach(() => {
  resetObsConfigForTests();
  resetBatcherForTests();
  clearSessionSalt();
});

describe('P122c — one event record per backend emit, not per mounted tab', () => {
  it('a fan-out to six tabs is one record with listeners: 6', async () => {
    const ipc = await setup();
    const seen: string[] = [];
    for (const tab of ['a', 'b', 'c', 'd', 'e', 'f']) {
      // Each tab's listener early-returns on another repo's event — the real
      // subscription's filter; all six are still invoked.
      await ipc.onRepoChanged((p) => {
        if (p.repoId === tab) seen.push(tab);
      });
    }
    emit({ repoId: 'c' });
    await drain();
    expect(seen).toEqual(['c']);
    expect(events()).toHaveLength(1);
    expect(events()[0]).toMatchObject({ name: 'onRepoChanged#0', listeners: 6, delivered: true });
  });

  it('distinct emits stay distinct records, even within one turn', async () => {
    const ipc = await setup();
    await ipc.onRepoChanged(() => undefined);
    await ipc.onRepoChanged(() => undefined);
    emit({ repoId: 'a' });
    emit({ repoId: 'b' });
    await drain();
    expect(events().map((e) => e.listeners)).toEqual([2, 2]);
  });

  it('the same payload object re-delivered to the same listener is a new emit', async () => {
    const ipc = await setup();
    await ipc.onRepoChanged(() => undefined);
    const p = { repoId: 'a' };
    emit(p);
    emit(p);
    await drain();
    expect(events().map((e) => e.listeners)).toEqual([1, 1]);
  });

  it('20 backend emits to 6 tabs is 20 records, so event-storm counts emits', async () => {
    const ipc = await setup();
    for (let i = 0; i < 6; i += 1) await ipc.onRepoChanged(() => undefined);
    for (let i = 0; i < 20; i += 1) {
      emit({ repoId: String(i % 6) });
      await Promise.resolve();
    }
    await drain();
    // Before P122c this was 120 records (20 × 6); the sink rule's ≥20 threshold
    // then fired on ~4 emits.
    expect(events()).toHaveLength(20);
    expect(events().every((e) => e.listeners === 6)).toBe(true);
  });
});

describe('P122d — the superseded stamp is per repo and skips subscriptions', () => {
  it('the same repo: the older in-flight result is superseded', async () => {
    const ipc = await setup();
    const first = ipc.listBranches('repo-a');
    const second = ipc.listBranches('repo-a');
    pending[1]?.(1);
    pending[0]?.(0);
    await Promise.all([first, second]);
    await flushNow();
    expect(results().map((r) => r.outcome)).toEqual(['ok', 'superseded']);
  });

  it('another repo in flight does not supersede', async () => {
    const ipc = await setup();
    const a = ipc.listBranches('repo-a');
    const b = ipc.listBranches('repo-b');
    pending[0]?.(0);
    pending[1]?.(1);
    await Promise.all([a, b]);
    await flushNow();
    expect(results().map((r) => r.outcome)).toEqual(['ok', 'ok']);
  });

  it('a call after the previous one settled is not superseded', async () => {
    const ipc = await setup();
    const a = ipc.listBranches('repo-a');
    pending[0]?.(0);
    await a;
    const b = ipc.listBranches('repo-a');
    pending[1]?.(1);
    await b;
    await flushNow();
    expect(results().map((r) => r.outcome)).toEqual(['ok', 'ok']);
  });

  it('concurrent subscriptions are never superseded', async () => {
    const ipc = await setup();
    await Promise.all([ipc.onRepoChanged(() => undefined), ipc.onRepoChanged(() => undefined)]);
    await flushNow();
    expect(results().map((r) => r.outcome)).toEqual(['ok', 'ok']);
  });
});
