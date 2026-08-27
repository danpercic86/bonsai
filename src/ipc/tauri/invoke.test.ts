/**
 * P91 §2.2 — the transport-level trace stamp, plus the guard that keeps it the
 * ONLY transport: a `src/ipc/tauri/*.ts` file that imports `invoke` straight
 * from `@tauri-apps/api/core` would silently dispatch untraced commands, and
 * increment 3's `ipc.recv` criterion would fail for exactly those commands.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const calls: Array<[string, unknown]> = [];

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => {
    calls.push([cmd, args]);
    return Promise.resolve(null);
  },
  Channel: class {},
}));

const { invoke } = await import('./invoke');
const { configureObs, resetObsConfigForTests } = await import('../../obs/enabled');
const { resetTraceForTests, withIpcSpan, withTrace } = await import('../../obs/trace');

const DEV_ON = {
  enabled: true,
  level: 'debug',
  captureIpc: true,
  captureReact: true,
  captureFrames: false,
  includeRawNames: false,
} as const;

beforeEach(() => {
  calls.length = 0;
  resetObsConfigForTests();
  resetTraceForTests();
});

afterEach(() => {
  resetObsConfigForTests();
  resetTraceForTests();
});

describe('invoke wrapper', () => {
  it('forwards args untouched when Dev mode is off', async () => {
    await invoke('get_status', { repoId: 'r1' });
    expect(calls[0]).toEqual(['get_status', { repoId: 'r1' }]);
  });

  it('injects __trace/__span at the PAYLOAD top level when on', async () => {
    configureObs({ ...DEV_ON });
    const payload = { repoId: 'r1' };
    await withTrace('click', 'status.refresh', () =>
      withIpcSpan('00000a', () => invoke('get_status', payload)),
    );
    expect(calls[0][1]).toEqual({ repoId: 'r1', __trace: expect.any(String), __span: '00000a' });
    // The caller's object is never mutated.
    expect(payload).toEqual({ repoId: 'r1' });
  });

  it('creates a payload when a command takes no args', async () => {
    configureObs({ ...DEV_ON });
    await withTrace('boot', 'session.info', () => invoke('head_info'));
    expect(calls[0][1]).toEqual({ __trace: expect.any(String) });
  });

  it('stamps nothing without an ambient trace (§2.5: never guess)', async () => {
    configureObs({ ...DEV_ON });
    await invoke('head_info', { repoId: 'r1' });
    expect(calls[0][1]).toEqual({ repoId: 'r1' });
  });

  it('leaves a binary payload untouched', async () => {
    configureObs({ ...DEV_ON });
    const bytes = new Uint8Array([1, 2, 3]);
    await withTrace('click', 'blob.write', () => invoke('write_blob', bytes));
    expect(calls[0][1]).toBe(bytes);
  });
});

describe('transport exhaustiveness guard', () => {
  // Read via Vite's glob (no @types/node in this tsconfig) so the assertion runs
  // over the real file text of every sibling module.
  const sources = import.meta.glob('./*.ts', {
    query: '?raw',
    import: 'default',
    eager: true,
  }) as Record<string, string>;
  const modules = Object.entries(sources).filter(
    ([path]) => !path.endsWith('/invoke.ts') && !path.endsWith('.test.ts'),
  );

  it('sees every sibling module', () => {
    expect(modules.length).toBeGreaterThan(15);
  });

  it('no src/ipc/tauri file imports invoke directly from @tauri-apps/api/core', () => {
    const offenders = modules
      .filter(([, src]) => /import[^;]*\binvoke\b[^;]*@tauri-apps\/api\/core/.test(src))
      .map(([path]) => path);
    expect(offenders).toEqual([]);
  });

  it('every module that invokes imports the wrapper', () => {
    const missing = modules
      .filter(([, src]) => /\binvoke[<(]/.test(src) && !/from '\.\/invoke'/.test(src))
      .map(([path]) => path);
    expect(missing).toEqual([]);
  });
});
