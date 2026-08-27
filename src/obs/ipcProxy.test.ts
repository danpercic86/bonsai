/**
 * P91 §12 row 2 acceptance tests for the frontend pipeline.
 *
 * Every criterion in the row is covered here except the cross-side `argsHash`
 * vectors, which live in `redact.test.ts` next to the redactor they pin.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { configureObs, resetObsConfigForTests } from './enabled';
import {
  appendCallCount,
  flushNow,
  refreshSalt,
  resetBatcherForTests,
  FLUSH_MS,
} from './batcher';
import { instrumentIpc, resetIpcProxyForTests } from './ipcProxy';
import { clearSessionSalt } from './redact';
import { resetTraceForTests, withTrace } from './trace';
import type { DevSettings } from '../ipc/types/settings';
import type { LogRecord } from './types';

const SALT = '00112233445566778899aabbccddeeff';

const DEV_ON: DevSettings = {
  enabled: true,
  level: 'debug',
  captureIpc: true,
  captureReact: true,
  captureFrames: false,
  includeRawNames: false,
};

interface FakeApi {
  openRepo(path: string): Promise<{ repoId: string }>;
  /** Real-api shape: positional, and the wrapper builds its own payload. */
  getStatus(repoId: string): Promise<number>;
  failing(): Promise<never>;
  onRepoChanged(cb: (n: number) => void): Promise<() => void>;
  logAppend(records: LogRecord[]): Promise<void>;
  logSessionInfo(): Promise<{ salt: string }>;
}

let sunk: LogRecord[] = [];
let saltReported = SALT;
let seen: unknown[][] = [];

function makeApi(): FakeApi {
  return {
    async openRepo(path: string) {
      seen.push([path]);
      return { repoId: 'r1' };
    },
    async getStatus(repoId: string) {
      seen.push([repoId]);
      return 3;
    },
    async failing(): Promise<never> {
      throw Object.assign(new Error('nope'), { kind: 'git' });
    },
    async onRepoChanged(cb: (n: number) => void) {
      cb(1);
      return () => undefined;
    },
    async logAppend(records: LogRecord[]) {
      sunk.push(...records);
    },
    async logSessionInfo() {
      return { salt: saltReported };
    },
  };
}

async function enable(dev: DevSettings = DEV_ON): Promise<void> {
  configureObs(dev);
  await refreshSalt();
}

beforeEach(() => {
  sunk = [];
  seen = [];
  saltReported = SALT;
  resetObsConfigForTests();
  resetBatcherForTests();
  resetIpcProxyForTests();
  resetTraceForTests();
  clearSessionSalt();
});

afterEach(() => {
  vi.useRealTimers();
  resetObsConfigForTests();
  resetBatcherForTests();
  clearSessionSalt();
});

describe('Dev mode OFF — zero cost (§11, §12 row 2)', () => {
  it('preserves method identity and installs no wrapper', async () => {
    const api = makeApi();
    const ipc = instrumentIpc(api);
    expect(ipc.openRepo).toBe(api.openRepo);
    expect(ipc.openRepo).toBe(ipc.openRepo);
    await ipc.openRepo('D:/repos/secret-project');
    await flushNow();
    expect(sunk).toHaveLength(0);
    // Args reach the backend untouched: no `__trace` was injected.
    expect(seen[0]).toEqual(['D:/repos/secret-project']);
  });

  it('re-checks the gate per access, so toggling needs no reload', async () => {
    const api = makeApi();
    const ipc = instrumentIpc(api);
    expect(ipc.openRepo).toBe(api.openRepo);
    await enable();
    expect(ipc.openRepo).not.toBe(api.openRepo);
    // ...and identity is STABLE while on (React dep arrays compare by reference).
    expect(ipc.openRepo).toBe(ipc.openRepo);
    configureObs(null);
    expect(ipc.openRepo).toBe(api.openRepo);
  });

  it('stays inert while enabled but saltless (never hashes unsalted)', async () => {
    const api = makeApi();
    saltReported = '';
    const ipc = instrumentIpc(api);
    await enable();
    expect(ipc.openRepo).toBe(api.openRepo);
  });
});

describe('Dev mode ON — paired records', () => {
  it('produces ipc.call/ipc.result sharing one trace and one span', async () => {
    const api = makeApi();
    const ipc = instrumentIpc(api);
    await enable();
    await withTrace('click', 'sidebar.branch.checkout', () => ipc.openRepo('D:/repos/x'));
    await flushNow();
    const call = sunk.find((r) => r.kind === 'ipc.call');
    const result = sunk.find((r) => r.kind === 'ipc.result');
    expect(call).toBeDefined();
    expect(result).toBeDefined();
    expect(call?.cmd).toBe('openRepo');
    expect(call?.trace).toBeTruthy();
    expect(result?.trace).toBe(call?.trace);
    expect(result?.span).toBe(call?.span);
    expect(result?.outcome).toBe('ok');
    expect(typeof result?.ms).toBe('number');
    expect(sunk.every((r) => r.src === 'ui')).toBe(true);
  });

  it('forwards arguments untouched; injection is the transport layer job', async () => {
    // §2.2 injection happens in `src/ipc/tauri/invoke.ts`, the only layer that
    // sees the invoke payload map; the proxy must not touch domain arguments
    // (on the mock path they are persisted verbatim).
    const api = makeApi();
    const ipc = instrumentIpc(api);
    await enable();
    await withTrace('click', 'status.refresh', () => ipc.getStatus('r1'));
    expect(seen[0]).toEqual(['r1']);
    // ...but the span IS published for the transport layer for the duration of
    // the call, which is what lets the wrapper stamp the payload.
    const call = (await (async () => {
      await flushNow();
      return sunk.find((r) => r.kind === 'ipc.call');
    })()) as Record<string, unknown> | undefined;
    expect(call?.span).toBeTruthy();
  });

  it('records an error outcome with its code and rethrows', async () => {
    const api = makeApi();
    const ipc = instrumentIpc(api);
    await enable();
    await expect(ipc.failing()).rejects.toThrow('nope');
    await flushNow();
    const result = sunk.find((r) => r.kind === 'ipc.result');
    expect(result?.outcome).toBe('err');
    expect(result?.errCode).toBe('git');
  });

  it('emits an event record when a wrapped callback is delivered', async () => {
    const api = makeApi();
    const ipc = instrumentIpc(api);
    await enable();
    await withTrace('boot', 'repo.subscribe', () => ipc.onRepoChanged(() => undefined));
    await flushNow();
    const ev = sunk.find((r) => r.kind === 'event');
    expect(ev?.name).toBe('onRepoChanged#0');
    expect(ev?.causedBy).toBeTruthy();
  });
});

describe('redaction at the boundary (§12 row 2)', () => {
  it('logs argsHash + argsShape and NO path substring', async () => {
    const api = makeApi();
    const ipc = instrumentIpc(api);
    await enable();
    const secretPath = 'D:/repos/acme-private/src/billing.ts';
    await ipc.openRepo(secretPath);
    await flushNow();
    expect(sunk.length).toBeGreaterThan(0);
    const call = sunk.find((r) => r.kind === 'ipc.call');
    expect(call?.argsHash).toMatch(/^[0-9a-f]{8}$/);
    expect(call?.argsShape).toEqual({ '0': 'str' });
    const jsonl = sunk.map((r) => JSON.stringify(r)).join('\n');
    expect(jsonl).not.toContain(secretPath);
    expect(jsonl).not.toContain('acme-private');
    expect(jsonl).not.toContain('billing.ts');
  });

  it('emits no bare ordinal from src: ui over a full fixture run', async () => {
    const api = makeApi();
    const ipc = instrumentIpc(api);
    await enable();
    await withTrace('click', 'fixture.run', async () => {
      await ipc.openRepo('D:/repos/x');
      await ipc.getStatus('r1');
      await ipc.onRepoChanged(() => undefined);
    });
    await flushNow();
    const jsonl = sunk
      .filter((r) => r.src === 'ui')
      .map((r) => JSON.stringify(r))
      .join('\n');
    expect(jsonl).not.toMatch(/(^|[^:\w])(ref|path|repo|remote|other)#\d/);
  });

  it('includes values only in raw mode', async () => {
    const api = makeApi();
    const ipc = instrumentIpc(api);
    await enable({ ...DEV_ON, includeRawNames: true });
    await ipc.openRepo('D:/repos/x');
    await flushNow();
    const call = sunk.find((r) => r.kind === 'ipc.call');
    expect(call?.args).toEqual({ '0': 'D:/repos/x' });
  });
});

describe('batching + self-amplification (§11, §2.3)', () => {
  it('makes at most one logAppend per 500 ms window', async () => {
    vi.useFakeTimers();
    const api = makeApi();
    const ipc = instrumentIpc(api);
    configureObs(DEV_ON);
    await refreshSalt();
    const before = appendCallCount();
    for (let i = 0; i < 20; i += 1) await ipc.openRepo(`D:/repos/${i}`);
    // 40 records buffered (call+result), under the 100-record early flush.
    expect(appendCallCount()).toBe(before);
    await vi.advanceTimersByTimeAsync(FLUSH_MS + 5);
    expect(appendCallCount()).toBe(before + 1);
    await vi.advanceTimersByTimeAsync(FLUSH_MS * 4);
    expect(appendCallCount()).toBe(before + 1);
    expect(sunk.length).toBe(40);
  });

  it('never instruments the sink itself', async () => {
    const api = makeApi();
    const ipc = instrumentIpc(api);
    await enable();
    // Identity is preserved for the excluded commands even while ENABLED.
    expect(ipc.logAppend).toBe(api.logAppend);
    expect(ipc.logSessionInfo).toBe(api.logSessionInfo);
    await ipc.openRepo('D:/repos/x');
    await flushNow();
    const afterFirst = sunk.length;
    expect(afterFirst).toBe(2);
    // A second flush (and the sink calls it made) adds nothing: no storm.
    await flushNow();
    await ipc.logSessionInfo();
    await flushNow();
    expect(sunk.length).toBe(afterFirst);
  });

  it('is inert during the salt-refetch window, never hashing with a stale salt', async () => {
    const api = makeApi();
    const ipc = instrumentIpc(api);
    await enable();
    saltReported = 'ffeeddccbbaa99887766554433221100';
    configureObs({ ...DEV_ON, level: 'trace' });
    // No ticks: the new salt has NOT arrived yet, so the proxy must hand back
    // the original method rather than hash against the old session's salt.
    expect(ipc.openRepo).toBe(api.openRepo);
    await ipc.openRepo('D:/repos/x');
    await flushNow();
    expect(sunk.filter((r) => r.kind === 'ipc.call')).toHaveLength(0);
  });

  it('refetches the salt when a settings change restarts the session', async () => {
    const api = makeApi();
    const ipc = instrumentIpc(api);
    await enable();
    await ipc.openRepo('D:/repos/x');
    await flushNow();
    const first = sunk.find((r) => r.kind === 'ipc.call')?.argsHash;
    sunk = [];
    // Rust restarts the session (new file, NEW salt) when level changes.
    saltReported = 'ffeeddccbbaa99887766554433221100';
    configureObs({ ...DEV_ON, level: 'trace' });
    await Promise.resolve();
    await new Promise<void>((resolve) => {
      setTimeout(resolve, 0);
    });
    await ipc.openRepo('D:/repos/x');
    await flushNow();
    const second = sunk.find((r) => r.kind === 'ipc.call')?.argsHash;
    expect(second).toBeTruthy();
    expect(second).not.toBe(first);
  });
});
